use super::*;
use crate::widgets::dropdown_menu::DropdownMenuEntry;

#[test]
fn native_unequal_width_chain_preserves_measurements_and_shrinks_without_covering_ancestors() {
    let entries = [DropdownMenuEntry::submenu(
        "Wide parent",
        vec![DropdownMenuEntry::submenu(
            "Medium child",
            vec![DropdownMenuEntry::item("Leaf", ())],
        )],
    )];
    let mut state = DropdownMenuState::default();
    state.expand_submenu(vec![0, 0], Some(0));
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let requested = build_native_menu_surfaces(
            Point::new(0.0, 0.0),
            &entries,
            &state,
            direction,
            &|level| match level[0].entry_label().unwrap() {
                "Wide parent" => 500.0,
                "Medium child" => 240.0,
                _ => 160.0,
            },
        );
        for width in [892.0, 400.0] {
            let bounds = Rect::from_xywh(-100.0, -40.0, width, 120.0);
            let resolved = resolve_native_menu_surfaces(
                &requested,
                &entries,
                &state,
                Point::new(0.0, 0.0),
                bounds,
            )
            .unwrap();
            for (index, panel) in resolved.iter().enumerate() {
                assert!(bounds.contains(panel.rect));
                assert!(panel.rect.width() >= 64.0);
                if width == 892.0 {
                    assert_eq!(panel.rect.width(), requested[index].rect.width());
                }
                if index > 0 {
                    let parent = &resolved[index - 1];
                    match direction {
                        LayoutDirection::Ltr => {
                            assert!(panel.rect.left >= parent.rect.right - SUBMENU_OVERLAP - 0.001)
                        }
                        LayoutDirection::Rtl => {
                            assert!(panel.rect.right <= parent.rect.left + SUBMENU_OVERLAP + 0.001)
                        }
                    }
                }
            }
        }
    }
}

fn scrolled_chain() -> (
    Vec<DropdownMenuEntry<'static, ()>>,
    DropdownMenuState,
    Vec<DropdownMenuSurface>,
) {
    let mut entries: Vec<_> = (0..30)
        .map(|_| DropdownMenuEntry::item("Row", ()))
        .collect();
    entries[10] = DropdownMenuEntry::submenu("More", vec![DropdownMenuEntry::item("Child", ())]);
    let mut state = DropdownMenuState::default();
    state.expand_submenu(vec![10], Some(0));
    state.scroll_level(0, 240.0, 1000.0);
    let requested = build_native_menu_surfaces(
        Point::new(0.0, 0.0),
        &entries,
        &state,
        LayoutDirection::Ltr,
        &estimate_level_width,
    );
    (entries, state, requested)
}

#[test]
fn native_children_follow_resolved_parent_scroll_instead_of_requested_offsets() {
    let (entries, mut state, requested) = scrolled_chain();
    state.scroll_level(0, 260.0, 1000.0);
    let bounds = Rect::from_xywh(-80.0, -40.0, 240.0, 200.0);
    let resolved =
        resolve_native_menu_surfaces(&requested, &entries, &state, Point::new(0.0, 0.0), bounds)
            .unwrap();
    let row = row_rect(&resolved[0], &entries, 10).unwrap();
    assert_eq!(resolved[1].rect.top, row.top - MENU_PADDING);
    assert_ne!(resolved[1].rect.top, requested[1].rect.top + bounds.top);
    assert_eq!(
        point_to_entry(&resolved[0], &entries, row.center()),
        Some(10)
    );
    let child = entries_at_level(&entries, &[10]).unwrap();
    assert_eq!(
        point_to_entry(&resolved[1], child, resolved[1].rect.center()),
        Some(0)
    );
    assert!(resolved[1].rect.left >= resolved[0].rect.right - SUBMENU_OVERLAP);
}

#[test]
fn native_constrained_chain_reveals_invoking_row_and_preserves_complete_columns() {
    let (entries, mut state, requested) = scrolled_chain();
    state.scroll_level(0, 0.0, 1000.0); // Invoking row is outside the requested root.
    let bounds = Rect::from_xywh(0.0, 0.0, 200.0, 120.0);
    let resolved =
        resolve_native_menu_surfaces(&requested, &entries, &state, Point::new(0.0, 0.0), bounds)
            .unwrap();
    let row = row_rect(&resolved[0], &entries, 10).unwrap();
    assert!(row.top >= resolved[0].rect.top + MENU_PADDING);
    assert!(row.bottom <= resolved[0].rect.bottom - MENU_PADDING);
    assert!(
        resolved
            .iter()
            .all(|panel| bounds.contains(panel.rect) && panel.rect.width() >= 64.0)
    );
    assert_eq!(
        resolved[1].rect,
        place_submenu_surface(
            resolved[0].rect,
            row,
            resolved[1].rect.width(),
            resolved[1].rect.height(),
            Rect::from_xywh(-8.0, -8.0, 216.0, 136.0),
            LayoutDirection::Ltr
        )
    );
    assert!(
        resolve_native_menu_surfaces(
            &requested,
            &entries,
            &state,
            Point::new(0.0, 0.0),
            Rect::from_xywh(0.0, 0.0, 120.0, 120.0)
        )
        .is_err()
    );
}
