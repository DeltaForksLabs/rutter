use super::*;
use crate::engine::widget_state::{ContextMenuState, WidgetState};
use crate::render::hit_test::{
    ContextMenuOverlayHit, hit_test_context_menu_overlay_with_direction,
};
use crate::widget::{ContextMenuEntry, Widget};
use crate::widgets::dropdown_menu::{DropdownMenuEntryKind, row_rect};
use std::collections::HashMap;
use taffy::prelude::Style;

fn font(size: f32) -> Font {
    crate::render::get_cached_font(&mut HashMap::new(), "sans-serif", size)
}

#[test]
#[ignore = "manual CPU profiling probe; no display required and no timing assertions"]
fn context_menu_geometry_cpu_profile() {
    use std::hint::black_box;
    use std::time::Instant;

    let entries = [ContextMenuEntry::submenu(
        "Organize",
        vec![
            ContextMenuEntry::submenu(
                "Move to",
                vec![ContextMenuEntry::item("Projects", 1).with_shortcut_label("CTRL+SHIFT+P")],
            )
            .with_shortcut_label("ALT+M"),
        ],
    )
    .with_shortcut_label("CTRL+O")];
    let mut state = crate::dropdown_menu::DropdownMenuState::default();
    state.expand_submenu(vec![0, 0], Some(0));
    let menus = [ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(20.0, 20.0),
        state,
    }];
    let viewport = (1600.0, 600.0);
    let mut fonts = HashMap::new();
    let font = crate::render::get_cached_font(&mut fonts, "sans-serif", 16.0);
    assert_eq!(
        context_surfaces(&menus[0], viewport, LayoutDirection::Ltr, &font).len(),
        3
    );
    let mut samples = Vec::with_capacity(32);
    for _ in 0..32 {
        let start = Instant::now();
        let targets = context_cursor_targets(
            black_box(&menus),
            black_box(Point::new(50.0, 40.0)),
            viewport,
            LayoutDirection::Ltr,
            black_box(&font),
        );
        assert!(black_box(targets).0.is_some());
        samples.push(start.elapsed().as_micros());
    }
    samples.sort_unstable();
    println!(
        "context_menu_geometry_cpu_profile: samples=32 panels=3 min={}us median={}us p95={}us max={}us",
        samples[0], samples[16], samples[30], samples[31]
    );
}

fn entries() -> Vec<ContextMenuEntry<'static, u8>> {
    vec![
        ContextMenuEntry::item("Run", 1)
            .with_svg_icon(b"invalid")
            .with_shortcut_label("CTRL+R"),
        ContextMenuEntry::separator(),
        ContextMenuEntry::submenu(
            "More",
            vec![
                ContextMenuEntry::disabled("Locked").with_shortcut_label("CTRL+L"),
                ContextMenuEntry::submenu(
                    "Deeper",
                    vec![ContextMenuEntry::item("Leaf", 42).with_shortcut_label("CTRL+F")],
                )
                .with_svg_icon(b"invalid"),
            ],
        )
        .with_shortcut_label("ALT+M"),
        ContextMenuEntry::disabled_submenu("Blocked", vec![ContextMenuEntry::item("Hidden", 9)])
            .with_svg_icon(b"invalid")
            .with_shortcut_label("ALT+B"),
    ]
}

#[test]
fn context_menu_nested_hit_testing_matches_rendered_panels_and_outside_dismissal() {
    let entries = entries();
    let widget = Widget::context_menu(
        Widget::Spacer {
            style: Style::default(),
        },
        &entries,
        Style::default(),
    )
    .with_id(42);
    let mut state = ContextMenuState::default();
    state.open_at(100.0, 60.0);
    state.navigation.expand_submenu(vec![2, 1], Some(0));
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let menu = ContextMenuOverlay {
            id: 42,
            entries: &entries,
            anchor: Point::new(100.0, 60.0),
            state: state.navigation.clone(),
        };
        let viewport = (800.0, 500.0);
        let font = font(14.0);
        let panels = context_surfaces(&menu, viewport, direction, &font);
        assert_eq!(panels.len(), 3);
        let states = HashMap::from([(42, WidgetState::ContextMenu(state.clone()))]);
        let point = row_rect(&panels[2], entries_at_level(&entries, &[2, 1]).unwrap(), 0)
            .unwrap()
            .center();
        assert!(matches!(
            hit_test_context_menu_overlay_with_direction(
                &widget, point, viewport, &states, &font, direction
            ),
            Some(ContextMenuOverlayHit::Item { id: 42, msg: 42 })
        ));
        let parent = row_rect(&panels[0], &entries, 2).unwrap().center();
        assert!(matches!(
            hit_test_context_menu_overlay_with_direction(
                &widget, parent, viewport, &states, &font, direction
            ),
            Some(ContextMenuOverlayHit::Submenu { id: 42, path }) if path == [2]
        ));
        let (hover, scroll) = context_cursor_targets(&[menu], point, viewport, direction, &font);
        assert!(
            matches!(hover, Some(DropdownMenuOverlayHit::Entry { path, kind: DropdownMenuEntryKind::Item, disabled: false, .. }) if path == [2, 1, 0])
        );
        assert_eq!(scroll.unwrap().level, 2);
        assert!(matches!(
            hit_test_context_menu_overlay_with_direction(
                &widget,
                Point::new(799.0, 499.0),
                viewport,
                &states,
                &font,
                direction
            ),
            Some(ContextMenuOverlayHit::Dismiss)
        ));
    }
}

#[test]
fn context_menu_disabled_and_separator_rows_consume_without_actions() {
    let entries = entries();
    let widget = Widget::context_menu(
        Widget::Spacer {
            style: Style::default(),
        },
        &entries,
        Style::default(),
    )
    .with_id(42);
    let mut state = ContextMenuState::default();
    state.open_at(20.0, 20.0);
    state.navigation.expand_submenu(vec![2], None);
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(20.0, 20.0),
        state: state.navigation.clone(),
    };
    let font = font(14.0);
    let panels = context_surfaces(&menu, (800.0, 500.0), LayoutDirection::Ltr, &font);
    let states = HashMap::from([(42, WidgetState::ContextMenu(state))]);
    for (panel, level, index) in [
        (&panels[0], &entries[..], 1),
        (&panels[0], &entries[..], 3),
        (&panels[1], entries_at_level(&entries, &[2]).unwrap(), 0),
    ] {
        let point = row_rect(panel, level, index).unwrap().center();
        assert!(matches!(
            hit_test_context_menu_overlay_with_direction(
                &widget,
                point,
                (800.0, 500.0),
                &states,
                &font,
                LayoutDirection::Ltr
            ),
            Some(ContextMenuOverlayHit::Consume)
        ));
    }
}

#[test]
fn context_menu_pointer_anchor_has_no_trigger_gap_and_clamps_edges_in_rtl() {
    let entries = entries();
    let mut state = crate::dropdown_menu::DropdownMenuState::default();
    state.open_at_index(Some(0));
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(400.0, 60.0),
        state: state.clone(),
    };
    let font = font(14.0);
    let ltr = context_surfaces(&menu, (800.0, 500.0), LayoutDirection::Ltr, &font);
    let rtl = context_surfaces(&menu, (800.0, 500.0), LayoutDirection::Rtl, &font);
    assert_eq!(ltr[0].rect.left, 400.0);
    assert_eq!(rtl[0].rect.right, 400.0);
    assert_eq!(ltr[0].rect.top, 60.0);
    assert_eq!(rtl[0].rect.top, 60.0);
    state.expand_submenu(vec![2, 1], None);
    let edge = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(795.0, 495.0),
        state,
    };
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let panels = context_surfaces(&edge, (800.0, 500.0), direction, &font);
        assert_eq!(panels.len(), 3);
        for panel in panels {
            assert!(panel.rect.left >= 8.0 && panel.rect.right <= 792.0);
            assert!(panel.rect.top >= 8.0 && panel.rect.bottom <= 492.0);
        }
    }
}

#[test]
fn context_menu_deeper_panel_scroll_reveals_and_hits_last_leaf() {
    let entries = [ContextMenuEntry::submenu(
        "More",
        vec![ContextMenuEntry::submenu(
            "Deeper",
            (0..30)
                .map(|i| ContextMenuEntry::item("Entry", i))
                .collect(),
        )],
    )];
    let mut state = crate::dropdown_menu::DropdownMenuState::default();
    state.expand_submenu(vec![0, 0], Some(29));
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(40.0, 40.0),
        state,
    };
    let panels = context_surfaces(&menu, (800.0, 400.0), LayoutDirection::Ltr, &font(14.0));
    assert_eq!(panels.len(), 3);
    assert!(panels[2].scroll_y > 0.0);
    let level = entries_at_level(&entries, &[0, 0]).unwrap();
    let point = row_rect(&panels[2], level, 29).unwrap().center();
    assert!(panels[2].rect.contains(point));
    assert!(
        matches!(context_entry_hit(&menu, &panels[2], point), Some(DropdownMenuOverlayHit::Entry { path, .. }) if path == [0, 0, 29])
    );
}

#[test]
fn context_menu_painter_and_geometry_accept_non_clone_messages_and_borrowed_labels() {
    struct NonClone;
    let label = String::from("Borrowed");
    let shortcut = String::from("CTRL+C");
    let svg = Vec::from(br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#ff0000"/></svg>"##.as_slice());
    let entries = [ContextMenuEntry::submenu(
        &label,
        vec![
            ContextMenuEntry::item(&label, NonClone)
                .with_shortcut_label(&shortcut)
                .with_svg_icon(&svg),
        ],
    )
    .with_svg_icon(&svg)
    .with_shortcut_label(&shortcut)];
    let mut state = crate::dropdown_menu::DropdownMenuState::default();
    state.expand_submenu(vec![0], None);
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(20.0, 20.0),
        state,
    };
    let mut surface = skia_safe::surfaces::raster_n32_premul((500, 400)).unwrap();
    crate::render::dropdown_menu_overlay::draw_context_menu(
        surface.canvas(),
        &menu,
        (500.0, 400.0),
        Point::new(-1.0, -1.0),
        &font(crate::Theme::dark().font_body),
        &mut crate::render::ImageRenderCache::default(),
        &crate::Theme::dark(),
        1.0,
        LayoutDirection::Ltr,
    );
    assert_eq!(
        context_surfaces(&menu, (500.0, 400.0), LayoutDirection::Ltr, &font(14.0)).len(),
        2
    );
    assert_ne!(
        surface.peek_pixels().unwrap().get_color((30, 30)),
        skia_safe::Color::TRANSPARENT
    );
}

#[test]
fn context_menu_child_widths_follow_theme_font_size_at_every_level() {
    let entries = [ContextMenuEntry::submenu(
        "Root",
        vec![
            ContextMenuEntry::submenu(
                "A child submenu with a long descriptive label",
                vec![
                    ContextMenuEntry::item("A grandchild action with a long descriptive label", 42)
                        .with_shortcut_label("CTRL+SHIFT+ALT+GRANDCHILD"),
                ],
            )
            .with_shortcut_label("CTRL+SHIFT+CHILD"),
        ],
    )
    .with_shortcut_label("CTRL+ROOT")];
    let mut state = crate::dropdown_menu::DropdownMenuState::default();
    state.expand_submenu(vec![0, 0], Some(0));
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(2500.0, 60.0),
        state,
    };
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let small = context_surfaces(&menu, (5000.0, 500.0), direction, &font(16.0));
        let large = context_surfaces(&menu, (5000.0, 500.0), direction, &font(24.0));
        for (font_size, panels) in [(16.0, &small), (24.0, &large)] {
            assert_eq!(panels.len(), 3);
            for panel in panels {
                let level = entries_at_level(&entries, &panel.level_path).unwrap();
                let expected = crate::widget::estimate_context_menu_width(level, &font(font_size));
                assert!((panel.rect.width() - expected).abs() < 0.001);
                assert!(panel.rect.left >= 8.0 && panel.rect.right <= 4992.0);
            }
        }
        assert!(large[1].rect.width() > small[1].rect.width());
        assert!(large[2].rect.width() > small[2].rect.width());
    }
}

#[test]
fn context_menu_direct_flat_open_flag_keeps_drawing_and_hit_testing() {
    let entries = [ContextMenuEntry::item("Run", 42)];
    let widget = Widget::context_menu(
        Widget::Spacer {
            style: Style::default(),
        },
        &entries,
        Style::default(),
    )
    .with_id(42);
    let states = HashMap::from([(
        42,
        WidgetState::ContextMenu(ContextMenuState {
            is_open: true,
            anchor_x: 20.0,
            anchor_y: 20.0,
            ..ContextMenuState::default()
        }),
    )]);
    assert!(matches!(
        hit_test_context_menu_overlay_with_direction(
            &widget,
            Point::new(40.0, 40.0),
            (500.0, 400.0),
            &states,
            &font(16.0),
            LayoutDirection::Ltr
        ),
        Some(ContextMenuOverlayHit::Item { id: 42, msg: 42 })
    ));
}

#[test]
fn context_menu_each_panel_allocates_independent_command_and_shortcut_maxima() {
    let entries = [
        ContextMenuEntry::item("A very long root command label", 1),
        ContextMenuEntry::submenu(
            "More",
            vec![
                ContextMenuEntry::item("A very long child command label", 2),
                ContextMenuEntry::submenu(
                    "Deeper",
                    vec![
                        ContextMenuEntry::item("A very long leaf command label", 3),
                        ContextMenuEntry::item("Short", 4)
                            .with_shortcut_label("CTRL+SHIFT+ALT+LEAF"),
                    ],
                )
                .with_shortcut_label("CTRL+SHIFT+ALT+CHILD"),
            ],
        )
        .with_shortcut_label("CTRL+SHIFT+ALT+ROOT"),
    ];
    let mut state = crate::dropdown_menu::DropdownMenuState::default();
    state.expand_submenu(vec![1, 1], None);
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(2000.0, 20.0),
        state,
    };
    let font = crate::render::text::get_cached_font(&mut HashMap::new(), "sans-serif", 18.0);
    let paint = skia_safe::Paint::default();
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let panels = context_surfaces(&menu, (4000.0, 400.0), direction, &font);
        assert_eq!(panels.len(), 3);
        for (depth, panel) in panels.iter().enumerate() {
            let level = entries_at_level(&entries, &panel.level_path).unwrap();
            let command = crate::render::text::measure_single_line_text(
                &font,
                level[0].label().unwrap(),
                &paint,
            );
            let shortcut = crate::render::text::measure_single_line_text(
                &font,
                level[1].shortcut_label().unwrap(),
                &paint,
            );
            let end_padding = if depth < 2 { 24.0 } else { 10.0 };
            let expected = (30.0 + command + 16.0 + shortcut + end_padding + 8.0).max(168.0);
            assert!((panel.rect.width() - expected).abs() < 0.001);
        }
    }
}

#[test]
fn context_menu_retained_fractional_font_drives_all_levels_and_live_metadata() {
    let mut fonts = HashMap::new();
    for size in [17.25_f32, 17.75] {
        let mut retained = crate::render::get_cached_font(&mut fonts, "serif", size);
        retained.set_scale_x(1.75);
        fonts.insert(("sans-serif".to_owned(), size.to_bits()), retained);
    }
    let mut entries = [ContextMenuEntry::submenu(
        "A root command with a descriptive label",
        vec![
            ContextMenuEntry::submenu(
                "A child command with a descriptive label",
                vec![
                    ContextMenuEntry::item("A leaf command with a descriptive label", 1)
                        .with_shortcut_label("CTRL+SHIFT+L"),
                ],
            )
            .with_shortcut_label("CTRL+SHIFT+C"),
        ],
    )
    .with_shortcut_label("CTRL+SHIFT+R")];
    let mut navigation = crate::dropdown_menu::DropdownMenuState::default();
    navigation.expand_submenu(vec![0, 0], Some(0));
    let viewport = (8000.0, 500.0);
    let cache_len = fonts.len();
    let mut widths = Vec::new();
    for size in [17.25_f32, 17.75] {
        let font = crate::render::get_cached_font(&mut fonts, "sans-serif", size);
        assert_eq!(font.size(), size);
        let menu = ContextMenuOverlay {
            id: 42,
            entries: &entries,
            anchor: Point::new(3000.0, 20.0),
            state: navigation.clone(),
        };
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let panels = context_surfaces(&menu, viewport, direction, &font);
            for _ in 0..3 {
                assert_eq!(context_surfaces(&menu, viewport, direction, &font), panels);
            }
            assert_eq!(panels.len(), 3);
            for (depth, panel) in panels.iter().enumerate() {
                let level = entries_at_level(&entries, &panel.level_path).unwrap();
                let paint = skia_safe::Paint::default();
                let command = crate::render::measure_single_line_text(
                    &font,
                    level[0].label().unwrap(),
                    &paint,
                );
                let shortcut = crate::render::measure_single_line_text(
                    &font,
                    level[0].shortcut_label().unwrap(),
                    &paint,
                );
                let end = if depth < 2 { 24.0 } else { 10.0 };
                let expected = (30.0 + command + 16.0 + shortcut + end + 8.0).max(168.0);
                assert!((panel.rect.width() - expected).abs() < 0.001);
            }
            widths.push(panels[2].rect.width());
        }
    }
    assert_eq!(fonts.len(), cache_len);
    assert!(widths[2] > widths[0]);

    let font = crate::render::get_cached_font(&mut fonts, "sans-serif", 17.25);
    let old_width = widths[0];
    let ContextMenuEntry::Decorated { entry, .. } = &mut entries[0] else {
        panic!("decorated root")
    };
    let ContextMenuEntry::Submenu {
        entries: children, ..
    } = entry.as_mut()
    else {
        panic!("root submenu")
    };
    let ContextMenuEntry::Decorated { entry, .. } = &mut children[0] else {
        panic!("decorated child")
    };
    let ContextMenuEntry::Submenu {
        entries: leaves, ..
    } = entry.as_mut()
    else {
        panic!("child submenu")
    };
    leaves[0] = ContextMenuEntry::item("A changed leaf", 9)
        .with_shortcut_label("CTRL+SHIFT+ALT+AN_UPDATED_LONG_SHORTCUT");
    let menu = ContextMenuOverlay {
        id: 42,
        entries: &entries,
        anchor: Point::new(3000.0, 20.0),
        state: navigation.clone(),
    };
    let panels = context_surfaces(&menu, viewport, LayoutDirection::Ltr, &font);
    assert_ne!(panels[2].rect.width(), old_width);
    let leaf_point = panels[2].rect.center();
    let widget = Widget::context_menu(
        Widget::Spacer {
            style: Style::default(),
        },
        &entries,
        Style::default(),
    )
    .with_id(42);
    let states = HashMap::from([(
        42,
        WidgetState::ContextMenu(ContextMenuState {
            is_open: true,
            anchor_x: 3000.0,
            anchor_y: 20.0,
            navigation,
        }),
    )]);
    assert!(matches!(
        hit_test_context_menu_overlay_with_direction(
            &widget,
            leaf_point,
            viewport,
            &states,
            &font,
            LayoutDirection::Ltr
        ),
        Some(ContextMenuOverlayHit::Item { id: 42, msg: 9 })
    ));
}
