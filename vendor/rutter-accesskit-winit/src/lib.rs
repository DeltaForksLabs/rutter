// Copyright 2022 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file).
// Modified for rutter-winit 0.31.0-beta.3: trait windows and wake-only proxies.

//! AccessKit's native-platform adapter for the local rutter-winit fork.
//!
//! On Unix, the platform adapter internally spawns asynchronous tasks. Enable
//! `tokio` instead of the default `async-io` feature when using that runtime.

#[cfg(all(
    feature = "accesskit_unix",
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    not(feature = "async-io"),
    not(feature = "tokio")
))]
compile_error!("Either \"async-io\" (default) or \"tokio\" feature must be enabled.");

#[cfg(all(
    feature = "accesskit_unix",
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    feature = "async-io",
    feature = "tokio"
))]
compile_error!(
    "Both \"async-io\" (default) and \"tokio\" features cannot be enabled at the same time."
);

use std::sync::mpsc::Sender;

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use winit::{
    event::WindowEvent as WinitWindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

mod platform_impl;

/// An accessibility request delivered via an application-owned channel.
#[derive(Debug)]
pub struct Event {
    pub window_id: WindowId,
    pub window_event: WindowEvent,
}

/// An accessibility event associated with a window.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    InitialTreeRequested,
    ActionRequested(ActionRequest),
    AccessibilityDeactivated,
}

// Winit 0.31 proxies only wake an event loop; the channel holds the actual
// events. Applications drain its receiver in ApplicationHandler::proxy_wake_up.
#[derive(Clone)]
struct WinitEventDispatcher {
    sender: Sender<Event>,
    proxy: EventLoopProxy,
}

impl WinitEventDispatcher {
    fn dispatch(&self, window_id: WindowId, window_event: WindowEvent) {
        // A disconnected channel means the application no longer accepts
        // accessibility events; waking its event loop would have no effect.
        if self
            .sender
            .send(Event {
                window_id,
                window_event,
            })
            .is_ok()
        {
            self.proxy.wake_up();
        }
    }
}

struct WinitActivationHandler {
    window_id: WindowId,
    dispatcher: WinitEventDispatcher,
}

impl ActivationHandler for WinitActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.dispatcher
            .dispatch(self.window_id, WindowEvent::InitialTreeRequested);
        None
    }
}

struct WinitActionHandler {
    window_id: WindowId,
    dispatcher: WinitEventDispatcher,
}

impl ActionHandler for WinitActionHandler {
    fn do_action(&mut self, request: ActionRequest) {
        self.dispatcher
            .dispatch(self.window_id, WindowEvent::ActionRequested(request));
    }
}

struct WinitDeactivationHandler {
    window_id: WindowId,
    dispatcher: WinitEventDispatcher,
}

impl DeactivationHandler for WinitDeactivationHandler {
    fn deactivate_accessibility(&mut self) {
        self.dispatcher
            .dispatch(self.window_id, WindowEvent::AccessibilityDeactivated);
    }
}

/// Bridges a winit window to the native AccessKit adapter on the current platform.
pub struct Adapter {
    inner: platform_impl::Adapter,
}

impl Adapter {
    /// Creates an adapter using a channel for AccessKit events and a winit
    /// proxy to wake the application event loop. Drain the receiver in
    /// `ApplicationHandler::proxy_wake_up` and call `update_if_active` with a
    /// full tree after an `InitialTreeRequested` event.
    ///
    /// Create the adapter before showing the window; this method panics if
    /// the window is already visible. Dropping the receiver stops delivery.
    pub fn with_event_loop_proxy(
        event_loop: &dyn ActiveEventLoop,
        window: &dyn Window,
        sender: Sender<Event>,
        proxy: EventLoopProxy,
    ) -> Self {
        let window_id = window.id();
        let dispatcher = WinitEventDispatcher { sender, proxy };
        Self::with_direct_handlers(
            event_loop,
            window,
            WinitActivationHandler {
                window_id,
                dispatcher: dispatcher.clone(),
            },
            WinitActionHandler {
                window_id,
                dispatcher: dispatcher.clone(),
            },
            WinitDeactivationHandler {
                window_id,
                dispatcher,
            },
        )
    }

    /// Creates an adapter with caller-provided handlers. The activation
    /// handler can synchronously return the initial tree; each handler may be
    /// invoked on a platform-dependent thread.
    ///
    /// Create the adapter before showing the window; this method panics if
    /// the window is already visible.
    pub fn with_direct_handlers(
        event_loop: &dyn ActiveEventLoop,
        window: &dyn Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        deactivation_handler: impl 'static + DeactivationHandler + Send,
    ) -> Self {
        if window.is_visible() == Some(true) {
            panic!(
                "The AccessKit winit adapter must be created before the window is shown (made visible) for the first time."
            );
        }
        let inner = platform_impl::Adapter::new(
            event_loop,
            window,
            activation_handler,
            action_handler,
            deactivation_handler,
        );
        Self { inner }
    }

    /// Creates an adapter with a direct activation handler and channel-based
    /// action/deactivation handlers. Drain the receiver in
    /// `ApplicationHandler::proxy_wake_up`.
    ///
    /// Create the adapter before showing the window; this method panics if
    /// the window is already visible.
    pub fn with_mixed_handlers(
        event_loop: &dyn ActiveEventLoop,
        window: &dyn Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        sender: Sender<Event>,
        proxy: EventLoopProxy,
    ) -> Self {
        let window_id = window.id();
        let dispatcher = WinitEventDispatcher { sender, proxy };
        Self::with_direct_handlers(
            event_loop,
            window,
            activation_handler,
            WinitActionHandler {
                window_id,
                dispatcher: dispatcher.clone(),
            },
            WinitDeactivationHandler {
                window_id,
                dispatcher,
            },
        )
    }

    /// Forwards a window event to AccessKit before the application handles it.
    pub fn process_event(&mut self, window: &dyn Window, event: &WinitWindowEvent) {
        self.inner.process_event(window, event);
    }

    /// Updates the tree only if initialized/active; after deferred activation
    /// the first update must contain the full tree.
    pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
        self.inner.update_if_active(updater);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Action, NodeId, TreeId};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    use winit::event_loop::EventLoopProxyProvider;

    #[derive(Debug)]
    struct CountingWaker(Arc<AtomicUsize>);

    impl EventLoopProxyProvider for CountingWaker {
        fn wake_up(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn proxy_handlers_deliver_window_events_and_wake_the_loop() {
        let (sender, receiver) = mpsc::channel();
        let wakes = Arc::new(AtomicUsize::new(0));
        let dispatcher = WinitEventDispatcher {
            sender,
            proxy: EventLoopProxy::new(Arc::new(CountingWaker(wakes.clone()))),
        };
        let id = WindowId::from_raw(41);
        let mut activation = WinitActivationHandler {
            window_id: id,
            dispatcher: dispatcher.clone(),
        };
        let mut action = WinitActionHandler {
            window_id: id,
            dispatcher: dispatcher.clone(),
        };
        let mut deactivation = WinitDeactivationHandler {
            window_id: id,
            dispatcher,
        };

        assert!(activation.request_initial_tree().is_none());
        let request = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(7),
            data: None,
        };
        action.do_action(request.clone());
        deactivation.deactivate_accessibility();
        let events: Vec<_> = receiver.try_iter().collect();
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|event| event.window_id == id));
        assert_eq!(events[0].window_event, WindowEvent::InitialTreeRequested);
        assert_eq!(
            events[1].window_event,
            WindowEvent::ActionRequested(request)
        );
        assert_eq!(
            events[2].window_event,
            WindowEvent::AccessibilityDeactivated
        );
        assert_eq!(wakes.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn disconnected_receiver_does_not_wake_the_loop() {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let wakes = Arc::new(AtomicUsize::new(0));
        let dispatcher = WinitEventDispatcher {
            sender,
            proxy: EventLoopProxy::new(Arc::new(CountingWaker(wakes.clone()))),
        };
        dispatcher.dispatch(WindowId::from_raw(42), WindowEvent::InitialTreeRequested);
        assert_eq!(wakes.load(Ordering::SeqCst), 0);
    }
}
