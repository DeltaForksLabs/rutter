//! Non-vetoable layer retirement, with popup roles destroyed before their parent.
use winit_core::window::WindowId;

use super::state::WindowType;
use crate::state::WinitState;

impl WinitState {
    pub(crate) fn queue_terminal_layer(&mut self, root: WindowId) {
        let is_layer = self.windows.borrow().get(&root).is_some_and(|window| {
            matches!(window.lock().unwrap().window, WindowType::Layer { .. })
        });
        if !is_layer || self.native_terminal(root) {
            return;
        }
        for id in self.destruction_order(root) {
            if let Some(requests) = self.window_requests.borrow().get(&id) {
                requests.mark_terminal();
            }
        }
        self.terminal_layers.push(root);
        self.dispatched_events = true;
    }

    pub(crate) fn native_terminal(&self, id: WindowId) -> bool {
        self.window_requests.borrow().get(&id).is_none_or(|requests| requests.is_terminal())
    }

    fn destruction_order(&self, root: WindowId) -> Vec<WindowId> {
        let mut pending = vec![(root, false)];
        let mut ordered = Vec::new();
        while let Some((id, visited)) = pending.pop() {
            if visited {
                ordered.push(id);
                continue;
            }
            let children = {
                let windows = self.windows.borrow();
                let Some(window) = windows.get(&id) else {
                    continue;
                };
                window.lock().unwrap().children().clone()
            };
            pending.push((id, true));
            pending.extend(children.into_iter().map(|child| (child, false)));
        }
        ordered
    }

    /// Run only after native dispatch/constructors have released temporary SCTK clones.
    pub(crate) fn retire_terminal_layers(&mut self) -> Vec<WindowId> {
        let mut retired = Vec::new();
        for root in std::mem::take(&mut self.terminal_layers) {
            for id in self.destruction_order(root) {
                let Some(window) = self.windows.get_mut().remove(&id) else {
                    continue;
                };
                let parent = window.lock().unwrap().parent();
                if let Some(parent) = parent.and_then(|parent| self.windows.get_mut().get(&parent))
                {
                    parent.lock().unwrap().remove_child(&id);
                }
                if let Some(requests) = self.window_requests.get_mut().remove(&id) {
                    requests.mark_terminal();
                }
                // Frontends (or metadata observers) may retain this Arc. Destroy
                // native roles explicitly, independently of the Arc lifetime.
                window.lock().unwrap().retire_native();
                drop(window);
                retired.push(id);
            }
        }
        self.window_compositor_updates.retain(|update| !retired.contains(&update.window_id));
        self.events_sink.retire_windows(&retired);
        self.window_events_sink.lock().unwrap().retire_windows(&retired);
        retired
    }
}
