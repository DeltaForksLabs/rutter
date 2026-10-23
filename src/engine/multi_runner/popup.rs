// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Native popup parenting, transient arbitration, and close/recreate lifetime.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::dpi::LogicalSize;
use winit::event::ElementState;
use winit::keyboard::{Key, NamedKey};
use winit::window::{PopupGrab, WindowAttributes, WindowType};

use super::*;
use crate::engine::gpu::validate_native_popup_size;
use crate::multi_window::PopupConfig;

impl<A: MultiWindowAppLogic + 'static> MultiWindowRunner<A> {
    pub(super) fn surface_attributes(
        &self,
        request: &SurfaceRequest,
    ) -> Result<WindowAttributes, MultiWindowRunError> {
        let Some(popup) = request.window.popup().copied() else {
            return Ok(request.window.window_attributes());
        };
        let fail = |reason: String| popup_error(request.surface, popup, reason);
        request
            .window
            .validate_popup()
            .map_err(|error| fail(error.to_string()))?;
        if popup.parent() == request.surface {
            return Err(fail("a popup cannot parent itself".into()));
        }
        let parent_config = self
            .surface_configs
            .get(&popup.parent())
            .ok_or_else(|| fail("parent is not registered; open the parent first".into()))?;
        if parent_config.popup().is_some() {
            return Err(fail("nested arbitrary-content popups are not supported; only one application transient is allowed".into()));
        }
        if !parent_config.is_visible() {
            return Err(fail(
                "parent is hidden; expected a visible live parent".into(),
            ));
        }
        let parent = self
            .surface_runners
            .get(&popup.parent())
            .and_then(RutterRunner::native_window)
            .ok_or_else(|| {
                fail("parent has no live native surface (suspended or retired)".into())
            })?;
        let viewport = parent.surface_logical_size();
        if !popup.anchor().fits((viewport.width, viewport.height)) {
            return Err(fail(format!(
                "anchor {:?} is outside the parent's configured logical viewport {}x{}",
                popup.anchor().rectangle(),
                viewport.width,
                viewport.height
            )));
        }
        validate_requested_allocation(popup, parent.scale_factor()).map_err(fail)?;
        let handle = parent
            .window_handle()
            .map_err(|error| fail(format!("resolve live native parent handle: {error}")))?;
        if !matches!(
            handle.as_raw(),
            RawWindowHandle::Wayland(_) | RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
        ) {
            return Err(fail(
                "native arbitrary-content popups currently support Wayland and X11 parents only"
                    .into(),
            ));
        }
        let attributes = popup_attributes(&request.window, popup);
        // SAFETY: the runtime resolves this handle from its own live registry.
        // Every close/destroy/suspend/shutdown path drops children before parents.
        Ok(unsafe { attributes.with_parent_window(Some(handle.as_raw())) })
    }

    pub(super) fn validate_committed_popup(
        request: &SurfaceRequest,
        runner: &SurfaceRunner<A>,
    ) -> Result<(), MultiWindowRunError> {
        let Some(popup) = request.window.popup().copied() else {
            return Ok(());
        };
        let fail = |reason| popup_error(request.surface, popup, reason);
        let window = runner
            .native_window()
            .ok_or_else(|| fail("popup has no committed native window".into()))?;
        if window.window_type() != WindowType::Popup {
            return Err(fail(
                "backend did not create a native popup role; no top-level fallback is allowed"
                    .into(),
            ));
        }
        validate_native_popup_size(window.surface_size()).map_err(fail)
    }

    pub(super) fn arbitrary_popup(&self) -> Option<SurfaceId> {
        self.surface_configs
            .iter()
            .find_map(|(surface, config)| config.popup().map(|_| *surface))
    }

    pub(super) fn popup_keyboard_target(
        &self,
        source: SurfaceId,
        event: &WindowEvent,
    ) -> Option<(SurfaceId, WindowId)> {
        if !matches!(
            event,
            WindowEvent::KeyboardInput { .. }
                | WindowEvent::ModifiersChanged(_)
                | WindowEvent::Ime(_)
        ) {
            return None;
        }
        let popup = self.arbitrary_popup()?;
        if self.surface_configs.get(&popup)?.popup()?.parent() != source {
            return None;
        }
        let native = self.surface_runners.get(&popup)?.native_window()?.id();
        Some((popup, native))
    }

    pub(super) fn close_popup_children(&mut self, parent: SurfaceId) {
        let children: Vec<_> = self
            .surface_configs
            .iter()
            .filter_map(|(surface, config)| {
                config
                    .popup()
                    .filter(|popup| popup.parent() == parent)
                    .map(|_| *surface)
            })
            .collect();
        for child in children {
            // Collected from the live registry; child closure cannot remove its parent.
            self.close_surface(child)
                .expect("registered popup child must close");
        }
    }

    pub(super) fn discard_arbitrary_popups(&mut self) {
        while let Some(surface) = self.arbitrary_popup() {
            self.close_surface(surface)
                .expect("registered transient must close");
        }
    }

    pub(super) fn prepare_popup_open(&mut self) {
        self.discard_arbitrary_popups();
        for runner in self.surface_runners.values_mut() {
            runner.dismiss_transient_menus();
        }
    }

    /// A newly requested native menu wins over an older transient on another parent.
    /// Nested native menus inside an arbitrary popup are dismissed (single transient policy).
    pub(super) fn coordinate_native_transients(&mut self) -> Result<(), MultiWindowRunError> {
        let popup = self.arbitrary_popup();
        let mut requests = Vec::new();
        for (surface, runner) in &mut self.surface_runners {
            if Some(*surface) == popup {
                runner.dismiss_transient_menus();
            } else if runner
                .has_transient_menu_request()
                .map_err(|error| surface_error(*surface, error))?
            {
                requests.push((*surface, runner.native_menu_window_id().is_some()));
            }
        }
        let owner = requests
            .iter()
            .find(|(_, existing)| !existing)
            .or_else(|| requests.first())
            .map(|(surface, _)| *surface);
        if owner.is_some() {
            self.discard_arbitrary_popups();
            for (surface, runner) in &mut self.surface_runners {
                if Some(*surface) != owner {
                    runner.dismiss_transient_menus();
                }
            }
        }
        Ok(())
    }

    /// Returns true when runtime dismissal consumed this event.
    pub(super) fn popup_lifecycle_event(
        &mut self,
        surface: SurfaceId,
        event: &WindowEvent,
    ) -> Result<bool, MultiWindowRunError> {
        let Some(popup) = self.arbitrary_popup() else {
            return Ok(false);
        };
        let parent = self
            .config_for(popup)?
            .popup()
            .expect("popup config")
            .parent();
        if (surface == popup || surface == parent) && is_popup_escape(event) {
            self.close_surface(popup)?;
            return Ok(true);
        }
        if self.popup_anchor_pressed(surface, event) {
            // Let the invoking widget toggle its existing popup closed. Closing
            // first would publish "closed" before its toggle message and reopen it.
            return Ok(false);
        }
        if surface != popup
            && matches!(
                event,
                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    ..
                } | WindowEvent::SurfaceResized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
                    | WindowEvent::Moved(_)
            )
            && (surface == parent || matches!(event, WindowEvent::PointerButton { .. }))
        {
            self.close_surface(popup)?;
        }
        if surface == popup
            && let WindowEvent::SurfaceResized(size) = event
        {
            let config = *self.config_for(popup)?.popup().expect("popup config");
            validate_native_popup_size(*size)
                .map_err(|reason| popup_error(popup, config, reason))?;
        }
        Ok(false)
    }

    fn popup_anchor_pressed(&self, surface: SurfaceId, event: &WindowEvent) -> bool {
        let WindowEvent::PointerButton {
            state: ElementState::Pressed,
            position,
            ..
        } = event
        else {
            return false;
        };
        let Some(popup) = self
            .arbitrary_popup()
            .and_then(|id| self.surface_configs.get(&id))
            .and_then(WindowConfig::popup)
        else {
            return false;
        };
        if popup.parent() != surface {
            return false;
        }
        let Some(parent) = self
            .surface_runners
            .get(&surface)
            .and_then(RutterRunner::native_window)
        else {
            return false;
        };
        let point = position.to_logical::<f64>(parent.scale_factor());
        let (x, y, width, height) = popup.anchor().rectangle();
        point.x >= f64::from(x)
            && point.y >= f64::from(y)
            && point.x < f64::from(x) + f64::from(width)
            && point.y < f64::from(y) + f64::from(height)
    }
}

fn is_popup_escape(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::KeyboardInput { event, .. }
        if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::Escape))
}

fn popup_attributes(window: &WindowConfig, popup: PopupConfig) -> WindowAttributes {
    let (width, height) = popup.logical_size();
    window
        .window_attributes()
        .with_window_type(WindowType::Popup)
        .with_surface_size(LogicalSize::new(f64::from(width), f64::from(height)))
        .with_positioner(popup.positioner())
        .with_decorations(false)
        .with_resizable(false)
        // Required never falls back to a cached serial or a non-grabbing role.
        .with_popup_grab(if popup.requires_input_grab() {
            PopupGrab::Required
        } else {
            PopupGrab::None
        })
        .with_active(popup.requires_input_grab())
}

fn validate_requested_allocation(popup: PopupConfig, scale: f64) -> Result<(), String> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(format!(
            "invalid live-parent scale {scale}; expected finite positive scale"
        ));
    }
    let (width, height) = popup.logical_size();
    let width = (f64::from(width) * scale).ceil();
    let height = (f64::from(height) * scale).ceil();
    if width > f64::from(u32::MAX) || height > f64::from(u32::MAX) {
        return Err(
            "requested popup exceeds physical coordinate limits at the live parent scale".into(),
        );
    }
    validate_native_popup_size(winit::dpi::PhysicalSize::new(width as u32, height as u32))
}

fn popup_error(surface: SurfaceId, popup: PopupConfig, reason: String) -> MultiWindowRunError {
    MultiWindowRunError::Popup {
        surface,
        parent: popup.parent(),
        reason,
    }
}

#[cfg(test)]
mod tests;
