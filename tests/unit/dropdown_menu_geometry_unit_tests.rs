use super::*;
use crate::widgets::dropdown_menu::{DropdownMenuEntry, OwnedDropdownMenuEntry, to_owned_entries};

fn viewport() -> Rect {
    Rect::from_xywh(0.0, 0.0, 500.0, 400.0)
}

fn long_entries(count: usize) -> Vec<OwnedDropdownMenuEntry<u8>> {
    let borrowed = (0..count)
        .map(|index| DropdownMenuEntry::item("Entry", index as u8))
        .collect::<Vec<_>>();
    to_owned_entries(&borrowed)
}

#[test]
fn root_flips_above_and_start_aligns_in_both_directions() {
    let anchor = Rect::from_xywh(300.0, 360.0, 80.0, 24.0);
    let ltr = place_root_surface(anchor, 160.0, 200.0, viewport(), LayoutDirection::Ltr);
    let rtl = place_root_surface(anchor, 160.0, 200.0, viewport(), LayoutDirection::Rtl);
    assert!(ltr.bottom <= anchor.top - ROOT_GAP + f32::EPSILON);
    assert_eq!(ltr.left, 300.0);
    assert_eq!(rtl.right, 380.0);
}

#[test]
fn submenu_uses_inline_end_then_horizontal_fallback() {
    let parent = Rect::from_xywh(330.0, 40.0, 160.0, 150.0);
    let row = Rect::from_xywh(330.0, 60.0, 160.0, ITEM_ROW_HEIGHT);
    let ltr = place_submenu_surface(parent, row, 160.0, 100.0, viewport(), LayoutDirection::Ltr);
    let rtl = place_submenu_surface(parent, row, 160.0, 100.0, viewport(), LayoutDirection::Rtl);
    assert!(ltr.right <= parent.left + SUBMENU_OVERLAP + f32::EPSILON);
    assert!(rtl.right <= parent.left + SUBMENU_OVERLAP + f32::EPSILON);
}

#[test]
fn long_list_uses_max_height_and_reveals_active_row() {
    let entries = long_entries(20);
    let mut state = DropdownMenuState::default();
    state.activate_path(vec![19]);
    let surfaces = build_open_menu_surfaces(
        Rect::from_xywh(20.0, 20.0, 80.0, 24.0),
        &entries,
        &state,
        viewport(),
        LayoutDirection::Ltr,
    );
    assert_eq!(surfaces[0].rect.height(), MAX_HEIGHT);
    assert!(surfaces[0].max_scroll > 0.0);
    let active = row_rect(&surfaces[0], &entries, 19).unwrap();
    assert!(active.bottom <= surfaces[0].rect.bottom - MENU_PADDING + f32::EPSILON);
}

#[test]
fn point_to_entry_skips_separator_rows() {
    let borrowed = vec![
        DropdownMenuEntry::item("First", 1_u8),
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::disabled_item("Last"),
    ];
    let entries = to_owned_entries(&borrowed);
    let mut state = DropdownMenuState::default();
    state.open_at_first(&borrowed);
    let surface = &build_open_menu_surfaces(
        Rect::from_xywh(10.0, 10.0, 80.0, 20.0),
        &entries,
        &state,
        viewport(),
        LayoutDirection::Ltr,
    )[0];
    let separator = row_rect(surface, &entries, 1).unwrap();
    let disabled = row_rect(surface, &entries, 2).unwrap();
    assert_eq!(point_to_entry(surface, &entries, separator.center()), None);
    assert_eq!(
        point_to_entry(surface, &entries, disabled.center()),
        Some(2)
    );
}

#[test]
fn scrolling_helpers_clamp_and_reveal_rows() {
    assert_eq!(maximum_scroll(500.0, 320.0), 180.0);
    assert_eq!(clamp_scroll(250.0, 180.0), 180.0);
    assert_eq!(scroll_to_reveal(0.0, 350.0, 32.0, 320.0), 70.0);
}

#[test]
fn surface_builder_follows_enabled_nested_submenus() {
    let borrowed = vec![DropdownMenuEntry::submenu(
        "More",
        vec![DropdownMenuEntry::item("Child", 1_u8)],
    )];
    let entries = to_owned_entries(&borrowed);
    let mut state = DropdownMenuState::default();
    assert!(state.open_submenu(&borrowed, vec![0]));
    let surfaces = build_open_menu_surfaces(
        Rect::from_xywh(20.0, 20.0, 80.0, 24.0),
        &entries,
        &state,
        viewport(),
        LayoutDirection::Ltr,
    );
    assert_eq!(surfaces.len(), 2);
    assert_eq!(surfaces[1].level_path, [0]);
    assert_eq!(
        surfaces[1].content_height,
        ITEM_ROW_HEIGHT + MENU_PADDING * 2.0
    );
}

#[test]
fn borrowed_entries_do_not_require_cloneable_messages() {
    struct NonClone;

    let entries = vec![DropdownMenuEntry::item("Borrowed", NonClone)];
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let surfaces = build_open_menu_surfaces(
        Rect::from_xywh(10.0, 10.0, 80.0, 24.0),
        &entries,
        &state,
        viewport(),
        LayoutDirection::Ltr,
    );
    assert_eq!(surfaces.len(), 1);
}

#[test]
fn third_level_continues_flipped_side_without_covering_root() {
    let entries = vec![DropdownMenuEntry::submenu(
        "First",
        vec![DropdownMenuEntry::submenu(
            "Second",
            vec![DropdownMenuEntry::item("Leaf", 1_u8)],
        )],
    )];
    let mut state = DropdownMenuState::default();
    state.expand_submenu(vec![0], Some(0));
    state.expand_submenu(vec![0, 0], Some(0));
    let surfaces = build_open_menu_surfaces(
        Rect::from_xywh(330.0, 20.0, 120.0, 32.0),
        &entries,
        &state,
        viewport(),
        LayoutDirection::Ltr,
    );

    assert_eq!(surfaces.len(), 3);
    assert!(surfaces[2].rect.right <= surfaces[0].rect.left + SUBMENU_OVERLAP);
}
