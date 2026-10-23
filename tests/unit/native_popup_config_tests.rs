use super::*;
use crate::{CloseBehavior, LayerAnchors, LayerShellConfig, WindowConfig};

fn request(direction: PopupDirection) -> PopupConfig {
    PopupConfig::new(
        SurfaceId::PRIMARY,
        PopupAnchor::new(8, 0, 24, 32).unwrap(),
        320,
        360,
        direction,
    )
    .unwrap()
}

#[test]
fn public_popup_config_requires_grab_by_default_and_explicitly_opts_out() {
    let popup: crate::PopupConfig = request(crate::PopupDirection::Below);
    assert!(popup.requires_input_grab());
    let non_grabbing = popup.with_grab(false);
    assert!(!non_grabbing.requires_input_grab());
    assert_eq!(non_grabbing.with_grab(true), popup);
    assert_eq!(non_grabbing.parent(), popup.parent());
    assert_eq!(non_grabbing.anchor(), popup.anchor());
    assert_eq!(non_grabbing.logical_size(), popup.logical_size());
    assert_eq!(non_grabbing.constraints(), popup.constraints());
}

#[test]
fn popup_geometry_rejects_protocol_overflow_zero_and_excessive_allocations() {
    for (x, y, width, height) in [
        (-1, 0, 1, 1),
        (0, -1, 1, 1),
        (0, 0, 0, 1),
        (i32::MAX, 0, u32::MAX, 1),
        (999_999, 0, 2, 1),
        (0, i32::MAX, 1, u32::MAX),
    ] {
        assert!(PopupAnchor::new(x, y, width, height).is_err());
    }
    for (width, height) in [
        (0, 1),
        (1, 0),
        (4097, 1),
        (4096, 1025),
        (u32::MAX, u32::MAX),
    ] {
        assert!(
            PopupConfig::new(
                SurfaceId::PRIMARY,
                PopupAnchor::new(0, 0, 1, 1).unwrap(),
                width,
                height,
                PopupDirection::Below
            )
            .is_err()
        );
    }
    assert!(
        request(PopupDirection::Left)
            .with_offset(i32::MIN, 0)
            .is_err()
    );
    assert!(
        request(PopupDirection::Below)
            .with_constraints(PopupConstraints::from_bits_retain(1 << 31))
            .is_err()
    );
    assert_eq!(
        PopupAnchor::new(999_999, 999_999, 1, 1)
            .unwrap()
            .rectangle(),
        (999_999, 999_999, 1, 1)
    );
    assert!(
        request(PopupDirection::Left)
            .with_offset(-1_000_000, 1_000_000)
            .is_ok()
    );
    assert!(
        request(PopupDirection::Right)
            .with_offset(0, 1_000_001)
            .is_err()
    );
    assert!(
        PopupConfig::new(
            SurfaceId::PRIMARY,
            PopupAnchor::new(0, 0, 1, 1).unwrap(),
            4096,
            1024,
            PopupDirection::Below,
        )
        .is_ok()
    );
}

#[test]
fn four_edges_preserve_logical_anchor_offset_and_exact_constraint_policy() {
    for (direction, expected_anchor, expected_gravity) in [
        (PopupDirection::Above, WindowAnchor::Top, WindowGravity::Top),
        (
            PopupDirection::Below,
            WindowAnchor::Bottom,
            WindowGravity::Bottom,
        ),
        (
            PopupDirection::Left,
            WindowAnchor::Left,
            WindowGravity::Left,
        ),
        (
            PopupDirection::Right,
            WindowAnchor::Right,
            WindowGravity::Right,
        ),
    ] {
        let config = request(direction)
            .with_offset(-4, 6)
            .unwrap()
            .with_constraints(PopupConstraints::SLIDE_X | PopupConstraints::SLIDE_Y)
            .unwrap();
        let positioner = config.positioner();
        assert_eq!(positioner.anchor, expected_anchor);
        assert_eq!(positioner.gravity, expected_gravity);
        assert_eq!(
            positioner.constraint_adjustment,
            PopupConstraints::SLIDE_X | PopupConstraints::SLIDE_Y
        );
        for scale in [1.0, 1.25, 2.0] {
            assert_eq!(
                positioner.anchor_rect.0.to_logical::<f64>(scale),
                LogicalPosition::new(8.0, 0.0)
            );
            assert_eq!(
                positioner.anchor_rect.1.to_logical::<f64>(scale),
                LogicalSize::new(24.0, 32.0)
            );
            assert_eq!(
                positioner.offset.to_logical::<f64>(scale),
                LogicalPosition::new(-4.0, 6.0)
            );
        }
    }
}

#[test]
fn popup_anchor_checks_parent_content_instead_of_constraining_popup_extent() {
    let config = request(PopupDirection::Above);
    assert!(config.anchor().fits((32.0, 32.0)));
    assert!(!config.anchor().fits((31.99, 32.0)));
    assert!(!config.anchor().fits((32.0, 31.99)));
    assert!(!config.anchor().fits((f64::NAN, 32.0)));
    assert!(!config.anchor().fits((32.0, f64::INFINITY)));
    assert!(config.logical_size().1 > 32);
}

#[test]
fn conflicting_native_roles_and_geometry_are_rejected_in_either_builder_order() {
    let popup = request(PopupDirection::Below);
    let layer = LayerShellConfig::new("test", LayerAnchors::TOP, 640, 32).unwrap();
    for config in [
        WindowConfig::default()
            .with_popup(popup)
            .with_layer_shell(layer.clone()),
        WindowConfig::default()
            .with_layer_shell(layer)
            .with_popup(popup),
        WindowConfig::default()
            .with_popup(popup)
            .with_panel(crate::PanelConfig::new("test", crate::PanelEdge::Top, 0, 32).unwrap()),
        WindowConfig::default()
            .with_position(0, 0)
            .with_popup(popup),
        WindowConfig::default()
            .with_popup(popup)
            .with_inner_size(100, 100)
            .unwrap(),
        WindowConfig::default()
            .with_min_inner_size(1, 1)
            .unwrap()
            .with_popup(popup),
        WindowConfig::default()
            .with_popup(popup)
            .with_max_inner_size(100, 100)
            .unwrap(),
        WindowConfig::default()
            .with_popup(popup)
            .with_visible(false),
        WindowConfig::default()
            .with_popup(popup)
            .with_close_behavior(CloseBehavior::ExitApplication),
    ] {
        assert_eq!(
            config.validate_popup(),
            Err(PopupConfigError::ConflictingWindowHints)
        );
    }
    assert!(
        WindowConfig::default()
            .with_popup(popup)
            .validate_popup()
            .is_ok()
    );
    assert!(
        WindowConfig::default()
            .with_position(-40, 10)
            .with_inner_size(800, 600)
            .unwrap()
            .validate_popup()
            .is_ok()
    );
}
