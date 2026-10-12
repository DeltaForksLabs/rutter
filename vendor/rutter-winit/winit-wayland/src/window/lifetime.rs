//! Frontend access is revoked as soon as the native layer lifetime becomes terminal.
use super::{Window, WindowState};

impl Window {
    pub(super) fn with_state<T>(
        &self,
        unavailable: T,
        request: impl FnOnce(&mut WindowState) -> T,
    ) -> T {
        if self.handles.window_requests.is_terminal() {
            return unavailable;
        }
        let mut state = self.window_state.lock().unwrap();
        if self.handles.window_requests.is_terminal() {
            return unavailable;
        }
        request(&mut state)
    }

    pub(super) fn native_available(&self) -> bool {
        !self.handles.window_requests.is_terminal()
    }
}
