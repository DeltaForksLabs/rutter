use super::*;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntry, DropdownMenuState, point_to_entry, row_rect,
};
use winit::dpi::LogicalPosition;
use winit::window::WindowConstraintAdjustment;

fn context_hover_update(
    runtime: &crate::engine::DropdownMenuRuntime<u8>,
    state: &mut DropdownMenuState,
    previous: usize,
    next: usize,
) -> DropdownHoverUpdate {
    let hit = |index| {
        Some(
            crate::render::dropdown_menu_overlay::DropdownMenuOverlayHit::Entry {
                id: 42,
                path: vec![index],
                kind: runtime.entry_kind(&[index]).unwrap(),
                disabled: runtime.is_disabled(&[index]),
            },
        )
    };
    DropdownHoverUpdate {
        previous_hover: hit(previous),
        hover: hit(next),
        scroll_target: None,
        outcome: runtime.hover_entry(state, vec![next]),
    }
}

#[test]
fn native_menu_settled_context_hover_skips_frames_and_local_highlight_is_popup_only() {
    let runtime = crate::engine::DropdownMenuRuntime::from_context_entries(&[
        crate::ContextMenuEntry::item("First", 1),
        crate::ContextMenuEntry::item("Second", 2),
    ]);
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    let kind = NativeMenuKind::Context(42);
    let same = context_hover_update(&runtime, &mut state, 0, 0);
    assert_eq!(
        native_hover_redraw(kind, true, false, &same),
        NativeHoverRedraw::None
    );
    let changed = context_hover_update(&runtime, &mut state, 0, 1);
    assert_eq!(
        native_hover_redraw(kind, true, false, &changed),
        NativeHoverRedraw::Popup
    );
    let settled = context_hover_update(&runtime, &mut state, 1, 1);
    assert_eq!(
        native_hover_redraw(kind, true, false, &settled),
        NativeHoverRedraw::None
    );
    assert_eq!(
        native_hover_redraw(kind, false, false, &settled),
        NativeHoverRedraw::Both
    );
    assert_eq!(
        native_hover_redraw(kind, true, true, &settled),
        NativeHoverRedraw::Both
    );
    assert_eq!(
        native_hover_redraw(NativeMenuKind::Dropdown(42), true, false, &settled),
        NativeHoverRedraw::Both
    );
}

#[test]
fn native_menu_geometry_reveal_branch_and_lower_hover_transitions_redraw_both() {
    let runtime = crate::engine::DropdownMenuRuntime::from_context_entries(&[
        crate::ContextMenuEntry::item("First", 1),
        crate::ContextMenuEntry::submenu("More", vec![crate::ContextMenuEntry::item("Child", 2)]),
    ]);
    let kind = NativeMenuKind::Context(42);
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    state.scroll_level(0, 10.0, 100.0);
    for (previous, next) in [(0, 0), (0, 1), (1, 0)] {
        let update = context_hover_update(&runtime, &mut state, previous, next);
        assert_eq!(
            native_hover_redraw(kind, true, false, &update),
            NativeHoverRedraw::Both
        );
    }
    let mut update = context_hover_update(&runtime, &mut state, 0, 0);
    update.outcome = MenuHoverOutcome::Changed {
        geometry_changed: false,
    };
    update.scroll_target = Some(
        crate::render::dropdown_menu_overlay::DropdownMenuScrollTarget {
            id: 42,
            level: 0,
            current_scroll: 10.0,
            max_scroll: 100.0,
        },
    );
    assert_eq!(
        native_hover_redraw(kind, true, false, &update),
        NativeHoverRedraw::Both
    );
    update.previous_hover = None;
    assert_eq!(
        native_hover_redraw(kind, true, false, &update),
        NativeHoverRedraw::Both
    );
    update.hover = None;
    assert_eq!(
        native_hover_redraw(kind, true, false, &update),
        NativeHoverRedraw::Both
    );
}

#[test]
fn native_popup_requires_alpha_presentation_for_gaps_between_panels() {
    let attributes = native_menu_attributes(
        MenuPlacement {
            kind: NativeMenuKind::Dropdown(7),
            position: PhysicalPosition::new(8, 44),
            size: PhysicalSize::new(316, 72),
        },
        PhysicalSize::new(800, 30),
        1.0,
    );
    assert!(attributes.transparent());
}

fn snapshot(scale: f64) -> PopupSurfaceGeometry {
    PopupSurfaceGeometry {
        parent_position: LogicalPosition::new(-100.0, 30.0),
        surface_size: winit::dpi::LogicalSize::new(160, 72).to_physical(scale),
        scale_factor: scale,
        configure_serial: Some(42),
    }
}

#[test]
fn native_menu_rounds_outwards_and_translates_pointer_with_paint_origin() {
    let placement = physical_menu_rect(
        NativeMenuKind::Dropdown(4),
        Rect::from_xywh(10.25, -2.5, 20.5, 10.5),
        1.5,
    )
    .unwrap();
    assert_eq!(placement.position, PhysicalPosition::new(15, -4));
    assert_eq!(placement.size, PhysicalSize::new(32, 16));
    for popup_scale in [1.0, 1.5, 2.0] {
        let geometry = snapshot(popup_scale);
        assert_eq!(
            configured_rect(geometry).unwrap(),
            Rect::from_xywh(-100.0, 30.0, 160.0, 72.0)
        );
        let point = popup_logical_pointer(
            PhysicalPosition::new(20.0 * popup_scale, 12.0 * popup_scale),
            geometry,
        );
        assert_eq!(point, Point::new(-80.0, 42.0));
        for parent_scale in [1.0, 2.0] {
            let physical = PhysicalPosition::new(
                f64::from(point.x * parent_scale),
                f64::from(point.y * parent_scale),
            );
            assert_eq!(
                super::super::logical_cursor_position(physical, parent_scale),
                point
            );
        }
    }
    assert!(
        physical_menu_rect(
            NativeMenuKind::Context(1),
            Rect::from_xywh(0.0, 0.0, f32::NAN, 2.0),
            1.0
        )
        .is_err()
    );
}

#[test]
fn native_pixel_budget_rejects_requested_scale_and_configured_growth_before_allocation() {
    for scale in [64.0, 1_000_000.0, f32::INFINITY] {
        assert!(
            physical_menu_rect(
                NativeMenuKind::Context(42),
                Rect::from_xywh(0.0, 0.0, 1024.0, 768.0),
                scale
            )
            .is_err()
        );
    }
    let mut configured = snapshot(1.5);
    configured.surface_size = PhysicalSize::new(4096, 1025);
    assert!(
        configured_rect(configured)
            .unwrap_err()
            .contains("physical pixels per buffer")
    );
    let request = menu_request(
        NativeMenuKind::Context(42),
        vec![DropdownMenuSurface {
            level_path: Vec::new(),
            rect: Rect::from_xywh(0.0, 0.0, 1024.0, 320.0),
            content_height: 320.0,
            scroll_y: 0.0,
            max_scroll: 0.0,
        }],
        64.0,
    );
    assert!(matches!(
        request,
        NativeMenuRequest::Unavailable(NativeMenuKind::Context(42), _)
    ));
}

#[test]
fn native_configure_snapshot_acknowledges_position_only_updates_before_input() {
    let old = snapshot(1.0);
    let mut actual = old;
    assert!(!presentation::configure_acknowledged(
        true,
        Some(42),
        Some(old),
        actual
    ));
    actual.configure_serial = Some(43); // Ack can precede queued SurfaceResized.
    actual.parent_position = LogicalPosition::new(-80.0, -40.0);
    assert!(presentation::configure_acknowledged(
        true,
        Some(42),
        Some(old),
        actual
    ));
    assert!(!same_drawable(old, actual));
    assert_eq!(
        popup_logical_pointer(PhysicalPosition::new(20.0, 22.0), actual),
        Point::new(-60.0, -18.0)
    );
    actual.configure_serial = None;
    assert!(presentation::configure_acknowledged(
        true,
        None,
        Some(old),
        actual
    ));
}

#[test]
fn native_layout_overflows_thin_parents_with_valid_anchor_and_constraints() {
    let entries = [DropdownMenuEntry::item("Open", ())];
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    for parent in [PhysicalSize::new(800, 30), PhysicalSize::new(30, 600)] {
        for scale in [1.0, 1.5, 2.0] {
            let surfaces = build_native_menu_surfaces(
                Point::new(28.0, 28.0),
                &entries,
                &state,
                crate::i18n::LayoutDirection::Rtl,
                &crate::widgets::dropdown_menu::estimate_level_width,
            );
            assert_eq!(surfaces[0].rect.height(), 40.0);
            assert_eq!(surfaces[0].rect.width(), 160.0);
            assert!(surfaces[0].rect.left < 0.0);
            let placement =
                physical_menu_rect(NativeMenuKind::Dropdown(7), surfaces[0].rect, scale).unwrap();
            let positioner = menu_positioner(placement.position, parent, scale);
            let anchor = positioner.anchor_rect.0.to_logical::<f32>(f64::from(scale));
            assert!(anchor.x >= 0.0 && anchor.x + 1.0 <= parent.width as f32 / scale);
            assert!(anchor.y >= 0.0 && anchor.y + 1.0 <= parent.height as f32 / scale);
            assert_eq!(
                positioner.constraint_adjustment,
                WindowConstraintAdjustment::all()
            );
            let offset = positioner.offset.to_logical::<f32>(f64::from(scale));
            assert!((anchor.x + offset.x - surfaces[0].rect.left).abs() < 0.01);
        }
    }
}

#[test]
fn configured_native_resize_scroll_reveal_and_pointer_match_displayed_rows() {
    let entries: Vec<_> = (0..30)
        .map(|index| DropdownMenuEntry::item("Row", index))
        .collect();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(29));
    let requested = build_native_menu_surfaces(
        Point::new(790.0, 28.0),
        &entries,
        &state,
        crate::i18n::LayoutDirection::Ltr,
        &crate::widgets::dropdown_menu::estimate_level_width,
    );
    for scale in [1.0, 1.5, 2.0] {
        let configured = Rect::from_xywh(-90.0, -100.0, 120.0, 72.0);
        let resolved = resolve_native_menu_surfaces(
            &requested,
            &entries,
            &state,
            Point::new(790.0, 28.0),
            configured,
        )
        .unwrap();
        assert_eq!(resolved[0].rect, configured);
        assert!(resolved[0].max_scroll > 0.0);
        let row = row_rect(&resolved[0], &entries, 29).unwrap();
        let geometry = PopupSurfaceGeometry {
            parent_position: LogicalPosition::new(-90.0, -100.0),
            surface_size: winit::dpi::LogicalSize::new(120, 72).to_physical(f64::from(scale)),
            scale_factor: f64::from(scale),
            configure_serial: Some(1),
        };
        let local = PhysicalPosition::new(
            f64::from((row.center_x() - configured.left) * scale),
            f64::from((row.center_y() - configured.top) * scale),
        );
        assert_eq!(
            point_to_entry(
                &resolved[0],
                &entries,
                popup_logical_pointer(local, geometry)
            ),
            Some(29)
        );
        state.scroll_level(0, 0.0, resolved[0].max_scroll);
        let scrolled = resolve_native_menu_surfaces(
            &requested,
            &entries,
            &state,
            Point::new(790.0, 28.0),
            configured,
        )
        .unwrap();
        assert_eq!(
            point_to_entry(&scrolled[0], &entries, Point::new(-50.0, -80.0)),
            Some(0)
        );
        state.activate_path(vec![29]);
    }
}

#[test]
fn native_context_menu_union_uses_all_panels_and_same_pointer_origin() {
    let entries = [crate::ContextMenuEntry::submenu(
        "More",
        vec![crate::ContextMenuEntry::submenu(
            "Deeper",
            vec![crate::ContextMenuEntry::item("Leaf", ())],
        )],
    )];
    let mut state = DropdownMenuState::default();
    state.expand_submenu(vec![0, 0], None);
    let font = crate::render::text::get_cached_font(
        &mut std::collections::HashMap::new(),
        "sans-serif",
        14.0,
    );
    for direction in [
        crate::i18n::LayoutDirection::Ltr,
        crate::i18n::LayoutDirection::Rtl,
    ] {
        let surfaces = build_native_menu_surfaces(
            Point::new(400.0, 100.0),
            &entries,
            &state,
            direction,
            &|entries| crate::widget::estimate_context_menu_width(entries, &font),
        );
        assert_eq!(surfaces.len(), 3);
        let union = menu_surface_union(&surfaces).unwrap();
        assert!(union.width() > surfaces[0].rect.width());
        let placement = physical_menu_rect(NativeMenuKind::Context(42), union, 1.5).unwrap();
        let geometry = PopupSurfaceGeometry {
            parent_position: placement.position.to_logical(1.5),
            surface_size: placement.size,
            scale_factor: 1.5,
            configure_serial: None,
        };
        for surface in surfaces {
            let center = surface.rect.center();
            let local = PhysicalPosition::new(
                f64::from(center.x) * 1.5 - f64::from(placement.position.x),
                f64::from(center.y) * 1.5 - f64::from(placement.position.y),
            );
            let point = popup_logical_pointer(local, geometry);
            assert!((point.x - center.x).abs() < 0.001 && (point.y - center.y).abs() < 0.001);
        }
    }
}
