// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Configure reconciliation, guarded resizing and presentation readiness.

use super::*;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn resolve_native_menu(&mut self) -> Result<bool, RutterRunError> {
        let Some(popup) = self.native_menu.as_ref() else {
            return Ok(false);
        };
        let kind = popup.kind;
        let snapshot = match popup
            .backend
            .window()
            .popup_surface_geometry(self.engine.window.as_ref().unwrap().scale_factor())
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                if Instant::now() >= popup.configure_deadline {
                    self.disable_native_menus(
                        kind,
                        format!("read configured popup geometry: {error}"),
                    );
                }
                return Ok(false);
            }
        };
        let configured = match configured_rect(snapshot) {
            Ok(rect)
                if usable_menu_area(snapshot.surface_size, snapshot.scale_factor as f32, kind) =>
            {
                rect
            }
            Ok(_) => {
                self.disable_native_menus(
                    kind,
                    "configured popup cannot contain a usable menu row".into(),
                );
                return Ok(false);
            }
            Err(reason) => {
                self.disable_native_menus(kind, reason);
                return Ok(false);
            }
        };
        if !popup.ready && Instant::now() >= popup.configure_deadline {
            self.disable_native_menus(kind, "popup configure/presentation timed out".into());
            return Ok(false);
        }
        if !configure_acknowledged(
            popup.awaiting_configure,
            popup.configure_serial_before_request,
            popup.configured,
            snapshot,
        ) {
            if Instant::now() >= popup.configure_deadline {
                self.disable_native_menus(kind, "popup configure timed out".into());
            }
            return Ok(false);
        }
        let requested_origin = popup.requested_origin;
        let viewport = self
            .engine
            .window
            .as_ref()
            .unwrap()
            .surface_size()
            .to_logical::<f32>(f64::from(self.engine.scale_factor));
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        let resolved = match kind {
            NativeMenuKind::Context(id) => {
                let mut menus = Vec::new();
                collect_open_context_menus(
                    &widget,
                    &self.engine.widget_states,
                    &mut Vec::new(),
                    &mut menus,
                );
                menus.iter().find(|menu| menu.id == id).map(|menu| {
                    resolve_native_menu_surfaces(
                        &popup.requested_surfaces,
                        menu.entries,
                        &menu.state,
                        requested_origin,
                        configured,
                    )
                })
            }
            NativeMenuKind::Dropdown(id) => {
                let menus = collect_open_dropdown_overlays(
                    &widget,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    &self.engine.widget_states,
                    (viewport.width, viewport.height),
                );
                menus.iter().find(|menu| menu.id == id).map(|menu| {
                    resolve_native_menu_surfaces(
                        &popup.requested_surfaces,
                        menu.entries,
                        &menu.state,
                        requested_origin,
                        configured,
                    )
                })
            }
        };
        drop(widget);
        let resolved = match resolved {
            Some(Ok(surfaces)) => surfaces,
            Some(Err(reason)) => {
                self.disable_native_menus(kind, reason);
                return Ok(false);
            }
            None => return Ok(false),
        };
        let layout_changed = self
            .engine
            .native_menu_layout
            .as_ref()
            .is_none_or(|(previous_kind, panels)| *previous_kind != kind || *panels != resolved);
        let popup = self.native_menu.as_mut().unwrap();
        let drawable_changed = popup
            .configured
            .is_none_or(|previous| !same_drawable(previous, snapshot));
        // The budget was checked before every call, including configures that
        // arrive before SurfaceResized/RedrawRequested is delivered to the runner.
        if popup.allocated_size != snapshot.surface_size {
            if let Err(error) = popup.backend.resize(snapshot.surface_size) {
                self.disable_native_menus(kind, format!("resize configured native popup: {error}"));
                return Ok(false);
            }
            popup.allocated_size = snapshot.surface_size;
        }
        popup.awaiting_configure = false;
        popup.configured = Some(snapshot);
        self.engine.native_menu_layout = Some((kind, resolved));
        if drawable_changed || layout_changed {
            popup.ready = false;
            popup.configure_deadline = Instant::now() + POPUP_CONFIGURE_TIMEOUT;
            self.redraw();
        }
        Ok(true)
    }

    pub(super) fn present_native_menu_frame(&mut self) -> Result<bool, RutterRunError> {
        let Some(popup) = self.native_menu.as_mut() else {
            return Ok(false);
        };
        let Some(snapshot) = popup.configured else {
            return Ok(false);
        };
        if popup.awaiting_configure {
            return Ok(false);
        }
        let was_ready = popup.ready;
        popup.ready = false;
        let kind = popup.kind;
        let origin = Point::new(
            snapshot.parent_position.x as f32,
            snapshot.parent_position.y as f32,
        );
        if let Err(error) = self
            .engine
            .try_redraw_native_menu(popup.backend.as_mut(), kind, origin)
        {
            match error {
                RutterRunError::Graphics(error) => {
                    self.disable_native_menus(kind, format!("present native popup frame: {error}"));
                    return Ok(false);
                }
                error => return Err(error),
            }
        }
        // Successful end_frame is the earliest point at which the parent may
        // suppress its overlay. A configure alone is not presentation success.
        let popup = self.native_menu.as_mut().unwrap();
        let actual = match popup
            .backend
            .window()
            .popup_surface_geometry(self.engine.window.as_ref().unwrap().scale_factor())
        {
            Ok(actual) => actual,
            Err(error) => {
                self.disable_native_menus(
                    kind,
                    format!("read popup geometry after presentation: {error}"),
                );
                return Ok(false);
            }
        };
        if !same_drawable(snapshot, actual) {
            self.resolve_native_menu()?;
            return Ok(false);
        }
        popup.ready = true;
        if !was_ready {
            self.redraw_parent_after_native_presentation();
        }
        Ok(true)
    }

    pub(super) fn reconcile_native_menu_before_input(&mut self) -> Result<bool, RutterRunError> {
        if !self.resolve_native_menu()? {
            return Ok(false);
        }
        if self.native_menu.as_ref().is_some_and(|popup| popup.ready) {
            return Ok(true);
        }
        // Input can precede the queued redraw after a configure. Present the
        // resolved layout first so selection never uses an unseen/stale frame.
        self.present_native_menu_frame()
    }

    fn redraw_parent_after_native_presentation(&self) {
        if let Some(window) = &self.engine.window {
            window.request_redraw();
        }
    }
}

pub(super) fn configure_acknowledged(
    awaiting: bool,
    before: Option<u32>,
    previous: Option<PopupSurfaceGeometry>,
    actual: PopupSurfaceGeometry,
) -> bool {
    !awaiting
        || actual.configure_serial != before
        || actual.configure_serial.is_none()
            && previous.is_some_and(|previous| !same_drawable(previous, actual))
}
