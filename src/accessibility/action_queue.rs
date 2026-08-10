// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use accesskit::{ActionHandler, ActionRequest};

type WakeCallback = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct AccessibilityActionQueue {
    generation: u64,
    requests: VecDeque<(u64, ActionRequest)>,
    wake: Option<WakeCallback>,
}

#[derive(Clone, Default)]
pub(crate) struct AccessibilityActionInbox {
    queue: Arc<Mutex<AccessibilityActionQueue>>,
}

impl fmt::Debug for AccessibilityActionInbox {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccessibilityActionInbox")
            .finish_non_exhaustive()
    }
}

impl AccessibilityActionInbox {
    pub(crate) fn handler(&self) -> QueuedActionHandler {
        let generation = locked_queue(&self.queue).generation;
        QueuedActionHandler {
            queue: Arc::clone(&self.queue),
            generation,
        }
    }

    pub(crate) fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        locked_queue(&self.queue).wake = Some(Arc::new(wake));
    }

    pub(crate) fn drain(&self) -> Vec<ActionRequest> {
        let mut queue = locked_queue(&self.queue);
        let generation = queue.generation;
        queue
            .requests
            .drain(..)
            .filter_map(|(request_generation, request)| {
                (request_generation == generation).then_some(request)
            })
            .collect()
    }

    pub(crate) fn retire_generation(&self) {
        let mut queue = locked_queue(&self.queue);
        queue.generation = queue.generation.wrapping_add(1);
        queue.requests.clear();
    }
}

#[derive(Clone)]
pub(crate) struct QueuedActionHandler {
    queue: Arc<Mutex<AccessibilityActionQueue>>,
    generation: u64,
}

impl ActionHandler for QueuedActionHandler {
    fn do_action(&mut self, request: ActionRequest) {
        let wake = {
            let mut queue = locked_queue(&self.queue);
            if queue.generation != self.generation {
                return;
            }
            queue.requests.push_back((self.generation, request));
            queue.wake.clone()
        };
        if let Some(wake) = wake {
            wake();
        }
    }
}

fn locked_queue(
    queue: &Mutex<AccessibilityActionQueue>,
) -> MutexGuard<'_, AccessibilityActionQueue> {
    queue
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use accesskit::{Action, NodeId, TreeId};

    #[test]
    fn queued_action_handler_preserves_order_and_wakes_runtime() {
        let inbox = AccessibilityActionInbox::default();
        let wake_count = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&wake_count);
        inbox.set_waker(move || {
            observed.fetch_add(1, Ordering::Relaxed);
        });
        let mut handler = inbox.handler();
        handler.do_action(request(Action::Focus, 7));
        handler.do_action(request(Action::Click, 9));

        let requests = inbox.drain();

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].target_node, NodeId(7));
        assert_eq!(requests[1].action, Action::Click);
        assert_eq!(wake_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn retired_handler_cannot_enqueue_for_recreated_adapter() {
        let inbox = AccessibilityActionInbox::default();
        let mut stale_handler = inbox.handler();
        inbox.retire_generation();
        stale_handler.do_action(request(Action::Click, 3));

        assert!(inbox.drain().is_empty());
    }

    fn request(action: Action, target: u64) -> ActionRequest {
        ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target),
            data: None,
        }
    }
}
