// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Serialize placement requests before authorizing frames or input.

use super::*;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn reconcile_native_menu_placement(&mut self) -> Result<bool, RutterRunError> {
        let Some(popup) = self.native_menu.as_mut() else {
            return Ok(false);
        };
        let kind = popup.kind;
        if popup.expired() {
            let phase = if popup.awaiting_configure {
                "configure"
            } else {
                "presentation"
            };
            self.disable_native_menus(kind, format!("popup {phase} timed out"))?;
            return Ok(false);
        }
        if !popup.awaiting_configure && popup.placement == popup.desired_placement {
            return Ok(true);
        }
        let parent = self.engine.window.as_ref().unwrap();
        let before = match popup
            .backend
            .window()
            .popup_surface_geometry(parent.scale_factor())
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.disable_native_menus(
                    kind,
                    format!("read popup configure before reposition: {error}"),
                )?;
                return Ok(false);
            }
        };
        if let Err(reason) = configured_rect(before) {
            self.disable_native_menus(kind, reason)?;
            return Ok(false);
        }
        if !presentation::configure_acknowledged(
            popup.awaiting_configure,
            popup.configure_serial_before_request,
            popup.configured,
            before,
        ) {
            return Ok(false);
        }
        popup.awaiting_configure = false;
        popup.configure_serial_before_request = None;
        if popup.placement == popup.desired_placement {
            return Ok(true);
        }
        // Only one request is in flight. While waiting, newer desired placements
        // coalesce. Only issuing a real request starts a fresh configure budget;
        // polling or changing the queued desire cannot keep a stalled one alive.
        let placement = popup.desired_placement;
        popup.begin_pending();
        popup.pending_deadline = Some(Instant::now() + POPUP_CONFIGURE_TIMEOUT);
        popup.configure_serial_before_request = before.configure_serial;
        let actual_size = popup.backend.window().request_popup_layout(
            menu_positioner(
                placement.position,
                parent.surface_size(),
                self.engine.scale_factor,
            ),
            placement
                .size
                .to_logical::<f64>(f64::from(self.engine.scale_factor))
                .into(),
        );
        if let Some(size) = actual_size
            && let Err(reason) = validate_native_popup_size(size)
        {
            self.disable_native_menus(kind, reason)?;
            return Ok(false);
        }
        popup.awaiting_configure = actual_size.is_none();
        popup.placement = placement;
        self.engine.native_menu_layout = None;
        popup.backend.window().request_redraw();
        self.redraw();
        // Synchronous backends are reread by resolve_native_menu; the requested
        // rectangle never substitutes for authoritative configured geometry.
        Ok(actual_size.is_some())
    }
}
