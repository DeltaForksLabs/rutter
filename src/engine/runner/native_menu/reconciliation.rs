// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Reconcile published parent models before any native-menu layout consumer.

use super::*;

impl<A: AppLogic + 'static> RutterRunner<A> {
    /// A model publication invalidates runtime ownership even when it does not
    /// request a redraw on this surface. Call at the shared-model write boundary.
    pub(crate) fn invalidate_published_model(&mut self) {
        self.engine.layout_dirty = true;
    }

    pub(super) fn reconcile_native_menu_model(&mut self) -> Result<(), RutterRunError> {
        let Some(size) = self
            .engine
            .window
            .as_ref()
            .map(|window| window.surface_size())
        else {
            return Ok(());
        };
        let parent_dirty = self.engine.layout_dirty;
        self.engine.try_ensure_layout_for_input(size)?;
        let layout_changed = self
            .native_menu
            .as_ref()
            .is_some_and(|popup| popup.parent_layout_revision != self.engine.layout_revision);
        if parent_dirty || layout_changed {
            // Even identical ID trees can have new entries, anchors or actions.
            // Revoke the old hit/paint layout until the current frame is resolved.
            self.engine.native_menu_layout = None;
            if let Some(popup) = self.native_menu.as_mut() {
                popup.begin_pending();
                popup.parent_layout_revision = self.engine.layout_revision;
            }
        }
        let Some(kind) = self.native_menu.as_ref().map(|popup| popup.kind) else {
            return Ok(());
        };
        if !self.native_menu_owner_is_live(kind) {
            self.retire_native_menu();
            return Ok(());
        }
        if parent_dirty || layout_changed {
            // Queue the newest placement before any paint/input consumer. The
            // transaction guard requests it before resolving the new panels.
            match self.requested_native_menu()? {
                NativeMenuRequest::Menu(placement, surfaces) if placement.kind == kind => {
                    let popup = self.native_menu.as_mut().unwrap();
                    popup.desired_placement = placement;
                    popup.requested_surfaces = surfaces;
                    popup.requested_origin = Point::new(
                        placement.position.x as f32 / self.engine.scale_factor,
                        placement.position.y as f32 / self.engine.scale_factor,
                    );
                }
                NativeMenuRequest::Unavailable(kind, reason) => {
                    self.disable_native_menus(kind, reason)?;
                }
                _ => self.retire_native_menu(),
            }
        }
        Ok(())
    }

    fn native_menu_owner_is_live(&self, kind: NativeMenuKind) -> bool {
        use crate::engine::widget_state::WidgetState;

        match kind {
            NativeMenuKind::Context(id) => self
                .engine
                .widget_states
                .get(&id)
                .and_then(WidgetState::as_context_menu)
                .is_some_and(|menu| menu.is_open),
            NativeMenuKind::Dropdown(id) => self
                .engine
                .widget_states
                .get(&id)
                .and_then(WidgetState::as_dropdown_menu)
                .is_some_and(|menu| menu.is_open()),
        }
    }

    pub(super) fn retire_native_menu(&mut self) {
        // Dropping the native surface also releases its compositor popup grab.
        // Never restore focus to an opener that reconciliation has removed.
        self.engine.native_menu_layout = None;
        if self.native_menu.take().is_some() {
            self.redraw();
        }
    }
}
