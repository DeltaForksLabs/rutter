use skia_safe::{Color, surfaces};

use super::*;
use crate::DropdownMenuEntry;
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
fn submenu_label_clip_reserves_inline_icon_spacing_in_both_directions() {
    let row = SkiaRect::from_xywh(0.0, 0.0, 100.0, 32.0);

    assert_eq!(
        dropdown_entry_label_clip(row, LayoutDirection::Ltr, true),
        SkiaRect::from_ltrb(30.0, 0.0, 76.0, 32.0)
    );
    assert_eq!(
        dropdown_entry_label_clip(row, LayoutDirection::Rtl, true),
        SkiaRect::from_ltrb(24.0, 0.0, 70.0, 32.0)
    );
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
        true,
        &mut fonts,
        &Theme::light(),
        LayoutDirection::Ltr,
    );

    let point = (menu_rect.left as i32 + 2, menu_rect.top as i32 + 2);
    assert_ne!(surface.peek_pixels().unwrap().get_color(point), Color::RED);
}

#[test]
fn checkbox_and_radio_marks_survive_shared_menu_row_rendering() {
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for theme in [Theme::light(), Theme::dark()] {
            let entries = [
                DropdownMenuEntry::checkbox("Checked", true, NonClone),
                DropdownMenuEntry::checkbox("Unchecked", false, NonClone),
                DropdownMenuEntry::radio("Selected", true, NonClone),
                DropdownMenuEntry::radio("Unselected", false, NonClone),
            ];
            let mut state = DropdownMenuState::default();
            state.open_at_index(None);
            let menu = overlay(&entries, state);
            let panel = &overlay_surfaces(&menu, viewport(), direction)[0];
            let mut output = surfaces::raster_n32_premul((500, 400)).unwrap();
            draw_collected_overlays(
                output.canvas(),
                &[menu],
                viewport(),
                Point::new(-1.0, -1.0),
                true,
                &mut HashMap::new(),
                &theme,
                direction,
            );
            let pixels = output.peek_pixels().unwrap();
            let mut ink_counts = Vec::new();
            for index in 0..entries.len() {
                let row = row_rect(panel, &entries, index).unwrap();
                let x = match direction {
                    LayoutDirection::Ltr => row.left + 15.0,
                    LayoutDirection::Rtl => row.right - 15.0,
                };
                let mut count = 0;
                for y in (row.center_y() - 7.0) as i32..(row.center_y() + 7.0) as i32 {
                    for x in (x - 7.0) as i32..(x + 7.0) as i32 {
                        count += usize::from(pixels.get_color((x, y)) != theme.surface);
                    }
                }
                ink_counts.push(count);
            }
            assert!(ink_counts[0] > 0, "checked item must paint its tick");
            assert_eq!(
                ink_counts[1], 0,
                "unchecked item must leave its mark gutter empty"
            );
            assert!(ink_counts[3] > 0, "unselected radio must paint its outline");
            assert!(
                ink_counts[2] > ink_counts[3],
                "selected radio must also paint its center"
            );
        }
    }
}

#[test]
fn covered_menu_hides_retained_active_entry() {
    let idle = dropdown_entry_pixel(false, true);
    let highlighted = dropdown_entry_pixel(true, true);
    let masked = dropdown_entry_pixel(true, false);

    assert_ne!(highlighted, idle);
    assert_eq!(masked, idle);
}

fn dropdown_entry_pixel(active: bool, shows_interaction_effects: bool) -> Color {
    let entries = vec![DropdownMenuEntry::item("Run", NonClone)];
    let mut state = DropdownMenuState::default();
    state.open_at_index(active.then_some(0));
    let overlay = overlay(&entries, state);
    let row = row_rect(
        &overlay_surfaces(&overlay, viewport(), LayoutDirection::Ltr)[0],
        &entries,
        0,
    )
    .unwrap();
    let mut surface = surfaces::raster_n32_premul((500, 400)).unwrap();
    draw_collected_overlays(
        surface.canvas(),
        &[overlay],
        viewport(),
        Point::new(-10.0, -10.0),
        shows_interaction_effects,
        &mut HashMap::new(),
        &Theme::light(),
        LayoutDirection::Ltr,
    );
    surface
        .peek_pixels()
        .unwrap()
        .get_color((row.left as i32 + 5, row.center_y() as i32))
}
