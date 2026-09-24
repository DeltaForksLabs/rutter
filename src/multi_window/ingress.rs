// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{self, Display};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, TryLockError};

use winit::event_loop::EventLoopProxy;

use super::SurfaceId;

/// Maximum number of queued messages or messages delivered per event-loop wakeup.
pub const MAX_MESSAGE_INGRESS_CAPACITY: usize = 4096;

/// Explicit memory and per-event budgets for worker-to-application messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageIngressConfig {
    /// Maximum number of messages waiting across all sender clones.
    pub capacity: NonZeroUsize,
    /// Maximum number of messages delivered in one native user-event callback.
    pub maximum_messages_per_wakeup: NonZeroUsize,
}

impl MessageIngressConfig {
    /// Validates both nonzero values against [`MAX_MESSAGE_INGRESS_CAPACITY`].
    pub fn try_new(
        capacity: usize,
        maximum_messages_per_wakeup: usize,
    ) -> Result<Self, MessageIngressConfigError> {
        let capacity =
            NonZeroUsize::new(capacity).ok_or(MessageIngressConfigError::ZeroCapacity)?;
        let maximum_messages_per_wakeup = NonZeroUsize::new(maximum_messages_per_wakeup)
            .ok_or(MessageIngressConfigError::ZeroDrainBudget)?;
        let config = Self {
            capacity,
            maximum_messages_per_wakeup,
        };
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn validate(self) -> Result<(), MessageIngressConfigError> {
        if self.capacity.get() > MAX_MESSAGE_INGRESS_CAPACITY {
            return Err(MessageIngressConfigError::CapacityTooLarge {
                capacity: self.capacity.get(),
            });
        }
        if self.maximum_messages_per_wakeup.get() > MAX_MESSAGE_INGRESS_CAPACITY {
            return Err(MessageIngressConfigError::DrainBudgetTooLarge {
                maximum_messages_per_wakeup: self.maximum_messages_per_wakeup.get(),
            });
        }
        Ok(())
    }
}

/// Invalid ingress queue capacity or delivery budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageIngressConfigError {
    ZeroCapacity,
    ZeroDrainBudget,
    CapacityTooLarge { capacity: usize },
    DrainBudgetTooLarge { maximum_messages_per_wakeup: usize },
}

impl Display for MessageIngressConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => formatter.write_str("message ingress capacity must be nonzero"),
            Self::ZeroDrainBudget => {
                formatter.write_str("message ingress drain budget must be nonzero")
            }
            Self::CapacityTooLarge { capacity } => write!(
                formatter,
                "message ingress capacity {capacity} exceeds maximum {MAX_MESSAGE_INGRESS_CAPACITY}"
            ),
            Self::DrainBudgetTooLarge {
                maximum_messages_per_wakeup,
            } => write!(
                formatter,
                "message ingress drain budget {maximum_messages_per_wakeup} exceeds maximum {MAX_MESSAGE_INGRESS_CAPACITY}"
            ),
        }
    }
}

impl Error for MessageIngressConfigError {}

/// A failed, nonblocking attempt to enqueue an owned application message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageIngressError {
    Full { capacity: usize },
    Busy,
    Closed,
}

impl Display for MessageIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full { capacity } => write!(
                formatter,
                "message ingress queue is full (capacity {capacity})"
            ),
            Self::Busy => {
                formatter.write_str("message ingress queue is temporarily busy; retry later")
            }
            Self::Closed => formatter.write_str("message ingress has closed"),
        }
    }
}

impl Error for MessageIngressError {}

trait IngressWaker: Send + Sync {
    fn wake(&self) -> bool;
}

impl IngressWaker for EventLoopProxy<()> {
    fn wake(&self) -> bool {
        self.send_event(()).is_ok()
    }
}

struct QueuedMessage<Message> {
    surface: SurfaceId,
    message: Message,
}

struct IngressState<Message> {
    queue: VecDeque<QueuedMessage<Message>>,
    wake_pending: bool,
    closed: bool,
}

struct SharedIngress<Message> {
    state: Mutex<IngressState<Message>>,
    waker: Box<dyn IngressWaker>,
    capacity: usize,
}

/// Cloneable, surface-targeted sender; it exposes neither UI state nor Winit handles.
///
/// Clones return [`MessageIngressError::Closed`] after the runtime exits. Senders do not keep
/// surface state alive; queued messages addressed to retired surfaces are dropped on delivery.
pub struct MultiWindowMessageSender<Message> {
    shared: Arc<SharedIngress<Message>>,
}

impl<Message> Clone for MultiWindowMessageSender<Message> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<Message: Send + 'static> MultiWindowMessageSender<Message> {
    /// Enqueues without waiting for space or for another thread to release the queue lock.
    /// Failed sends consume the supplied message; coalesce replaceable snapshots on the worker.
    pub fn try_send(
        &self,
        surface: SurfaceId,
        message: Message,
    ) -> Result<(), MessageIngressError> {
        let mut state = match self.shared.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => return Err(MessageIngressError::Busy),
            Err(TryLockError::Poisoned(_)) => return Err(MessageIngressError::Closed),
        };
        if state.closed {
            return Err(MessageIngressError::Closed);
        }
        if state.queue.len() == self.shared.capacity {
            return Err(MessageIngressError::Full {
                capacity: self.shared.capacity,
            });
        }
        state.queue.push_back(QueuedMessage { surface, message });
        if !state.wake_pending {
            state.wake_pending = true;
            if !self.shared.waker.wake() {
                state.queue.clear();
                state.closed = true;
                return Err(MessageIngressError::Closed);
            }
        }
        Ok(())
    }
}

/// The event-loop-owned side of an optional ingress boundary.
pub(crate) struct MessageIngress<Message> {
    shared: Arc<SharedIngress<Message>>,
    maximum_messages_per_wakeup: usize,
}

impl<Message> MessageIngress<Message> {
    pub(crate) fn new(
        config: MessageIngressConfig,
        proxy: EventLoopProxy<()>,
    ) -> (Self, MultiWindowMessageSender<Message>) {
        Self::with_waker(config, Box::new(proxy))
    }

    fn with_waker(
        config: MessageIngressConfig,
        waker: Box<dyn IngressWaker>,
    ) -> (Self, MultiWindowMessageSender<Message>) {
        let shared = Arc::new(SharedIngress {
            state: Mutex::new(IngressState {
                queue: VecDeque::with_capacity(config.capacity.get()),
                wake_pending: false,
                closed: false,
            }),
            waker,
            capacity: config.capacity.get(),
        });
        (
            Self {
                shared: Arc::clone(&shared),
                maximum_messages_per_wakeup: config.maximum_messages_per_wakeup.get(),
            },
            MultiWindowMessageSender { shared },
        )
    }

    /// Takes a bounded FIFO batch and schedules one more native event if messages remain.
    /// Application callbacks must run only after this method releases the queue mutex.
    pub(crate) fn drain_batch(&self) -> Vec<(SurfaceId, Message)> {
        let mut state = self.shared.state.lock().expect("ingress queue poisoned");
        if state.closed {
            return Vec::new();
        }
        let batch = (0..self.maximum_messages_per_wakeup)
            .map_while(|_| state.queue.pop_front())
            .map(|item| (item.surface, item.message))
            .collect();
        if state.queue.is_empty() {
            state.wake_pending = false;
        } else if !self.shared.waker.wake() {
            state.closed = true;
            state.queue.clear();
            return Vec::new();
        }
        batch
    }

    /// Ensures messages sent during startup or suspension get an event after resume.
    pub(crate) fn rearm(&self) {
        let mut state = self.shared.state.lock().expect("ingress queue poisoned");
        if !state.closed && !state.queue.is_empty() && !self.shared.waker.wake() {
            state.closed = true;
            state.queue.clear();
        }
    }

    pub(crate) fn close(&self) {
        let mut state = self.shared.state.lock().expect("ingress queue poisoned");
        state.closed = true;
        state.queue.clear();
        state.wake_pending = false;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;

    #[derive(Default)]
    struct WakeCounter {
        count: AtomicUsize,
        closed: AtomicBool,
    }

    impl IngressWaker for Arc<WakeCounter> {
        fn wake(&self) -> bool {
            if self.closed.load(Ordering::SeqCst) {
                return false;
            }
            self.count.fetch_add(1, Ordering::SeqCst);
            true
        }
    }

    fn fixture(
        capacity: usize,
        budget: usize,
    ) -> (
        MessageIngress<u8>,
        MultiWindowMessageSender<u8>,
        Arc<WakeCounter>,
    ) {
        let waker = Arc::new(WakeCounter::default());
        let (ingress, sender) = MessageIngress::with_waker(
            MessageIngressConfig::try_new(capacity, budget).unwrap(),
            Box::new(Arc::clone(&waker)),
        );
        (ingress, sender, waker)
    }

    #[test]
    fn configuration_rejects_zero_and_oversized_values() {
        assert_eq!(
            MessageIngressConfig::try_new(0, 1),
            Err(MessageIngressConfigError::ZeroCapacity)
        );
        assert_eq!(
            MessageIngressConfig::try_new(1, 0),
            Err(MessageIngressConfigError::ZeroDrainBudget)
        );
        assert_eq!(
            MessageIngressConfig::try_new(MAX_MESSAGE_INGRESS_CAPACITY + 1, 1),
            Err(MessageIngressConfigError::CapacityTooLarge {
                capacity: MAX_MESSAGE_INGRESS_CAPACITY + 1,
            })
        );
        assert_eq!(
            MessageIngressConfig::try_new(1, MAX_MESSAGE_INGRESS_CAPACITY + 1),
            Err(MessageIngressConfigError::DrainBudgetTooLarge {
                maximum_messages_per_wakeup: MAX_MESSAGE_INGRESS_CAPACITY + 1,
            })
        );
        let manually_constructed = MessageIngressConfig {
            capacity: NonZeroUsize::new(MAX_MESSAGE_INGRESS_CAPACITY + 1).unwrap(),
            maximum_messages_per_wakeup: NonZeroUsize::new(1).unwrap(),
        };
        assert!(manually_constructed.validate().is_err());
    }

    #[test]
    fn typed_sender_can_be_shared_between_workers() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MultiWindowMessageSender<u8>>();
    }

    #[test]
    fn sender_preserves_fifo_across_surfaces_and_rearms_once_per_batch() {
        let (ingress, sender, waker) = fixture(4, 2);
        let first = SurfaceId::new(1);
        let second = SurfaceId::new(2);
        sender.try_send(first, 10).unwrap();
        sender.clone().try_send(second, 20).unwrap();
        sender.try_send(first, 30).unwrap();
        assert_eq!(waker.count.load(Ordering::SeqCst), 1);
        assert_eq!(ingress.drain_batch(), vec![(first, 10), (second, 20)]);
        assert_eq!(waker.count.load(Ordering::SeqCst), 2);
        sender.try_send(second, 40).unwrap();
        assert_eq!(waker.count.load(Ordering::SeqCst), 2);
        assert_eq!(ingress.drain_batch(), vec![(first, 30), (second, 40)]);
        assert_eq!(ingress.drain_batch(), Vec::new());
        sender.try_send(first, 50).unwrap();
        assert_eq!(waker.count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn full_and_contended_queues_fail_without_waiting() {
        let (ingress, sender, _) = fixture(1, 1);
        let surface = SurfaceId::PRIMARY;
        sender.try_send(surface, 1).unwrap();
        assert_eq!(
            sender.try_send(surface, 2),
            Err(MessageIngressError::Full { capacity: 1 })
        );
        assert_eq!(ingress.drain_batch(), vec![(surface, 1)]);
        let guard = ingress.shared.state.lock().unwrap();
        assert_eq!(sender.try_send(surface, 3), Err(MessageIngressError::Busy));
        drop(guard);
        assert!(ingress.drain_batch().is_empty());
    }

    #[test]
    fn closing_discards_pending_messages_and_rejects_all_sender_clones() {
        let (ingress, sender, waker) = fixture(2, 1);
        let clone = sender.clone();
        sender.try_send(SurfaceId::PRIMARY, 1).unwrap();
        ingress.close();
        assert_eq!(ingress.drain_batch(), Vec::new());
        assert_eq!(
            sender.try_send(SurfaceId::PRIMARY, 2),
            Err(MessageIngressError::Closed)
        );
        assert_eq!(
            clone.try_send(SurfaceId::PRIMARY, 3),
            Err(MessageIngressError::Closed)
        );
        assert_eq!(waker.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn closed_native_event_loop_closes_and_discards_the_queue() {
        let (ingress, sender, waker) = fixture(2, 1);
        waker.closed.store(true, Ordering::SeqCst);
        assert_eq!(
            sender.try_send(SurfaceId::PRIMARY, 1),
            Err(MessageIngressError::Closed)
        );
        assert!(ingress.drain_batch().is_empty());
        assert_eq!(
            sender.try_send(SurfaceId::PRIMARY, 2),
            Err(MessageIngressError::Closed)
        );
    }

    #[test]
    fn resume_rearms_an_outstanding_queue_without_delivering_it() {
        let (ingress, sender, waker) = fixture(2, 1);
        sender.try_send(SurfaceId::PRIMARY, 1).unwrap();
        ingress.rearm();
        assert_eq!(waker.count.load(Ordering::SeqCst), 2);
        assert_eq!(ingress.drain_batch(), vec![(SurfaceId::PRIMARY, 1)]);
    }

    #[test]
    fn drained_messages_can_be_processed_without_holding_the_queue_lock() {
        let (ingress, sender, _) = fixture(2, 1);
        sender.try_send(SurfaceId::PRIMARY, 1).unwrap();
        let batch = ingress.drain_batch();
        assert_eq!(batch, vec![(SurfaceId::PRIMARY, 1)]);
        // A callback may itself trigger a send on another clone without blocking.
        sender.clone().try_send(SurfaceId::PRIMARY, 2).unwrap();
        assert_eq!(ingress.drain_batch(), vec![(SurfaceId::PRIMARY, 2)]);
    }
}
