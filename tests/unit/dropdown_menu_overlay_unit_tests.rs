use skia_safe::{Color, surfaces};

use super::*;
use crate::render::select_overlay::collector::OverlayOwner;

struct NonClone;

fn viewport() -> (f32, f32) {
    (500.0, 400.0)
}

fn overlay<'a>(
    entries: &'a [DropdownMenuEntry<'a, NonClone>],
    state: DropdownMenuState,
) -> DropdownOverlay<'a, NonClone> {
    DropdownOverlay {
        id: 42,
        entries,
        anchor: SkiaRect::from_xywh(20.0, 20.0, 100.0, 32.0),
        visible_anchor: SkiaRect::from_xywh(20.0, 20.0, 100.0, 32.0),
        state,
        owner: OverlayOwner::default(),
    }
}

#[test]
fn root_and_submenu_are_independent_top_level_surfaces() {
    let entries = vec![DropdownMenuEntry::submenu(
        "More",
        vec![DropdownMenuEntry::item("Child", NonClone)],
    )];
    let mut state = DropdownMenuState::default();
    assert!(state.open_submenu(&entries, vec![0]));
    let overlay = overlay(&entries, state);

    let surfaces = overlay_surfaces(&overlay, viewport(), LayoutDirection::Ltr);

    assert_eq!(surfaces.len(), 2);
    assert_eq!(surfaces[0].level_path, Vec::<usize>::new());
    assert_eq!(surfaces[1].level_path, vec![0]);
    assert!(surfaces[1].rect.left >= surfaces[0].rect.right - 4.0);
}

#[test]
fn open_trigger_is_exempt_from_outside_dismissal() {
    let entries = vec![DropdownMenuEntry::item("Run", NonClone)];
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let overlay = overlay(&entries, state);

    let hit = hit_test_dropdown_menu_overlay(
        &[overlay],
        Point::new(40.0, 36.0),
        viewport(),
        LayoutDirection::Ltr,
    );

    assert_eq!(hit, Some(DropdownMenuOverlayHit::Trigger { id: 42 }));
}

#[test]
fn clipped_trigger_exempts_only_its_visible_region() {
    let entries = vec![DropdownMenuEntry::item("Run", NonClone)];
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let mut overlay = overlay(&entries, state);
    overlay.visible_anchor = SkiaRect::from_xywh(20.0, 20.0, 40.0, 32.0);

    let hit = hit_test_dropdown_menu_overlay(
        &[overlay],
        Point::new(90.0, 36.0),
        viewport(),
        LayoutDirection::Ltr,
    );

    assert_eq!(hit, Some(DropdownMenuOverlayHit::Dismiss { id: 42 }));
}

#[test]
fn disabled_entry_hit_preserves_path_and_kind() {
    let entries = vec![DropdownMenuEntry::<NonClone>::disabled_checkbox(
        "Locked", true,
    )];
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let overlay = overlay(&entries, state);
    let surfaces = overlay_surfaces(&overlay, viewport(), LayoutDirection::Ltr);
    let row = row_rect(&surfaces[0], &entries, 0).unwrap();

    let hit =
        hit_test_dropdown_menu_overlay(&[overlay], row.center(), viewport(), LayoutDirection::Ltr);

    assert_eq!(
        hit,
        Some(DropdownMenuOverlayHit::Entry {
            id: 42,
            path: vec![0],
            kind: DropdownMenuEntryKind::Checkbox,
            disabled: true,
        })
    );
}

#[test]
fn point_outside_surfaces_and_trigger_dismisses_menu() {
    let entries = vec![DropdownMenuEntry::item("Run", NonClone)];
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);

    let hit = hit_test_dropdown_menu_overlay(
        &[overlay(&entries, state)],
        Point::new(480.0, 380.0),
        viewport(),
        LayoutDirection::Ltr,
    );

    assert_eq!(hit, Some(DropdownMenuOverlayHit::Dismiss { id: 42 }));
}

#[test]
fn long_list_surface_is_a_scroll_target() {
    let entries = (0..20)
        .map(|_| DropdownMenuEntry::item("Entry", NonClone))
        .collect::<Vec<_>>();
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let overlay = overlay(&entries, state);
    let surface = &overlay_surfaces(&overlay, viewport(), LayoutDirection::Ltr)[0];

    let target = dropdown_menu_scroll_target_at(
        &[overlay],
        surface.rect.center(),
        viewport(),
        LayoutDirection::Ltr,
    );

    let target = target.unwrap();
    assert_eq!(target.id, 42);
    assert_eq!(target.level, 0);
    assert_eq!(target.current_scroll, 0.0);
    assert!(target.max_scroll > 0.0);
}

#[test]
fn raster_draw_changes_pixels_inside_menu_surface() {
    let entries = vec![DropdownMenuEntry::item("Run", NonClone)];
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let overlay = overlay(&entries, state);
    let menu_rect = overlay_surfaces(&overlay, viewport(), LayoutDirection::Ltr)[0].rect;
    let mut surface = surfaces::raster_n32_premul((500, 400)).unwrap();
    surface.canvas().clear(Color::RED);
    let mut fonts = HashMap::new();

    draw_collected_overlays(
        surface.canvas(),
        &[overlay],
        viewport(),
        Point::new(0.0, 0.0),
        &mut fonts,
        &Theme::light(),
        LayoutDirection::Ltr,
    );

    let point = (menu_rect.left as i32 + 2, menu_rect.top as i32 + 2);
    assert_ne!(surface.peek_pixels().unwrap().get_color(point), Color::RED);
}
