//! Unconsumed document navigation belongs to the focus scope, never hover.
use super::*;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn handle_scroll_navigation_key(&mut self, key: &Key) -> bool {
        let id = if self.engine.focused_widget_id.is_some() {
            self.engine.runtime_caches.viewport_geometry.focused_scroll
        } else {
            // With no focused control, retain pointer-selected viewport behavior.
            self.engine.active_scroll_id
        };
        let Some(id) = id else { return false };
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_scroll_mut)
        else {
            return false;
        };
        let delta = match key {
            Key::Named(NamedKey::ArrowDown) => 40.0,
            Key::Named(NamedKey::ArrowUp) => -40.0,
            Key::Named(NamedKey::PageDown) => state.viewport_h * 0.9,
            Key::Named(NamedKey::PageUp) => -state.viewport_h * 0.9,
            Key::Named(NamedKey::Home) => -state.offset_y,
            Key::Named(NamedKey::End) => state.content_height - state.viewport_h - state.offset_y,
            _ => return false,
        };
        state.scroll_by(delta);
        self.engine.cancel_smooth_scroll(id);
        self.redraw();
        true
    }
}
