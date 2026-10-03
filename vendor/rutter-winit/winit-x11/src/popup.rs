//! Placement of root-level X11 popup windows using the shared xdg-style positioner.

use dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use winit_core::window::WindowPositioner;

pub(crate) fn place_popup(
    positioner: &WindowPositioner,
    scale_factor: f64,
    parent_origin: PhysicalPosition<i32>,
    monitor_origin: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    popup_size: PhysicalSize<u32>,
) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let clip_origin = LogicalPosition::new(
        (monitor_origin.x - parent_origin.x) as f64 / scale_factor,
        (monitor_origin.y - parent_origin.y) as f64 / scale_factor,
    );
    let clip_size = monitor_size.to_logical::<f64>(scale_factor);
    let popup_size = popup_size.to_logical::<f64>(scale_factor);
    let (origin, size) = winit_common::positioner::place_window(
        positioner,
        scale_factor,
        popup_size,
        (clip_origin, clip_size),
    );
    let origin = LogicalPosition::new(
        origin.x + parent_origin.x as f64 / scale_factor,
        origin.y + parent_origin.y as f64 / scale_factor,
    );
    (origin.to_physical(scale_factor), size.to_physical(scale_factor))
}

pub(crate) fn outside_popup(x: f64, y: f64, size: PhysicalSize<u32>) -> bool {
    !(0.0..f64::from(size.width)).contains(&x) || !(0.0..f64::from(size.height)).contains(&y)
}

#[cfg(test)]
mod tests {
    use dpi::{LogicalPosition, LogicalSize, Position, Size};
    use winit_core::window::{WindowAnchor, WindowConstraintAdjustment, WindowGravity};

    use super::*;

    fn menu_positioner(anchor_x: f64, adjustment: WindowConstraintAdjustment) -> WindowPositioner {
        WindowPositioner::new(
            WindowAnchor::BottomLeft,
            (
                Position::Logical(LogicalPosition::new(anchor_x, 10.)),
                Size::Logical(LogicalSize::new(20., 20.)),
            ),
            Position::Logical(LogicalPosition::new(0., 0.)),
            WindowGravity::BottomRight,
            adjustment,
        )
    }

    #[test]
    fn popup_can_extend_beyond_parent_surface() {
        let positioner = menu_positioner(10., WindowConstraintAdjustment::empty());
        let (origin, size) = place_popup(
            &positioner,
            1.,
            (100, 200).into(),
            (0, 0).into(),
            (800, 600).into(),
            (80, 50).into(),
        );
        assert_eq!(origin, PhysicalPosition::new(110, 230));
        assert_eq!(size, PhysicalSize::new(80, 50));
    }

    #[test]
    fn popup_flips_at_monitor_boundary_with_scale() {
        let positioner = menu_positioner(70., WindowConstraintAdjustment::FLIP_X);
        let (origin, size) = place_popup(
            &positioner,
            2.,
            (290, 20).into(),
            (200, 0).into(),
            (300, 400).into(),
            (80, 40).into(),
        );
        assert_eq!(origin, PhysicalPosition::new(390, 80));
        assert_eq!(size, PhysicalSize::new(80, 40));
    }

    #[test]
    fn outside_click_uses_popup_surface_bounds() {
        let size = PhysicalSize::new(80, 40);
        assert!(!outside_popup(0., 0., size));
        assert!(!outside_popup(79.9, 39.9, size));
        assert!(outside_popup(-1., 10., size));
        assert!(outside_popup(10., -1., size));
        assert!(outside_popup(80., 10., size));
        assert!(outside_popup(10., 40., size));
        assert!(outside_popup(f64::NAN, 10., size));
    }
}
