// Copyright 2022 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file).
// Modified for rutter-winit 0.31.0-beta.3: surface coordinates and trait windows.

use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, Rect, TreeUpdate};
use accesskit_unix::Adapter as UnixAdapter;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::Window,
};

pub struct Adapter {
    adapter: UnixAdapter,
}

// `surface_position` is relative to the outer window, unlike the old
// `inner_position` (which was reported in desktop coordinates).
fn root_window_bounds(
    outer_position: PhysicalPosition<i32>,
    outer_size: PhysicalSize<u32>,
    surface_position: PhysicalPosition<i32>,
    surface_size: PhysicalSize<u32>,
) -> (Rect, Rect) {
    let outer_origin: (f64, f64) = outer_position.cast::<f64>().into();
    let outer_dimensions: (f64, f64) = outer_size.cast::<f64>().into();
    let surface_offset: (f64, f64) = surface_position.cast::<f64>().into();
    let surface_dimensions: (f64, f64) = surface_size.cast::<f64>().into();
    (
        Rect::from_origin_size(outer_origin, outer_dimensions),
        Rect::from_origin_size(
            (
                outer_origin.0 + surface_offset.0,
                outer_origin.1 + surface_offset.1,
            ),
            surface_dimensions,
        ),
    )
}

impl Adapter {
    pub fn new(
        _event_loop: &dyn ActiveEventLoop,
        _window: &dyn Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        deactivation_handler: impl 'static + DeactivationHandler + Send,
    ) -> Self {
        let adapter = UnixAdapter::new(activation_handler, action_handler, deactivation_handler);
        Self { adapter }
    }

    pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
        self.adapter.update_if_active(updater);
    }

    pub fn process_event(&mut self, window: &dyn Window, event: &WindowEvent) {
        match event {
            WindowEvent::Moved(outer_position) => {
                self.update_window_bounds(window, *outer_position, window.surface_size());
            }
            WindowEvent::SurfaceResized(size) => {
                self.update_window_bounds(
                    window,
                    window.outer_position().unwrap_or_default(),
                    *size,
                );
            }
            WindowEvent::Focused(is_focused) => {
                self.adapter.update_window_focus_state(*is_focused);
            }
            _ => {}
        }
    }

    fn update_window_bounds(
        &mut self,
        window: &dyn Window,
        outer_position: PhysicalPosition<i32>,
        surface_size: PhysicalSize<u32>,
    ) {
        let (outer, surface) = root_window_bounds(
            outer_position,
            window.outer_size(),
            window.surface_position(),
            surface_size,
        );
        self.adapter.set_root_window_bounds(outer, surface);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_bounds_include_outer_origin_and_decoration_offset() {
        let (outer, surface) = root_window_bounds(
            (-100, 200).into(),
            (400, 300).into(),
            (8, 32).into(),
            (384, 268).into(),
        );
        assert_eq!(outer, Rect::new(-100.0, 200.0, 300.0, 500.0));
        assert_eq!(surface, Rect::new(-92.0, 232.0, 292.0, 500.0));
    }

    #[test]
    fn surface_bounds_can_extend_outside_outer_window() {
        let (_, surface) = root_window_bounds(
            (100, 200).into(),
            (200, 100).into(),
            (-10, -20).into(),
            (50, 40).into(),
        );
        assert_eq!(surface, Rect::new(90.0, 180.0, 140.0, 220.0));
    }
}
