use super::*;
use crate::engine::widget_state::ContextMenuState;
use crate::widgets::dropdown_menu::{DropdownMenuSurface, entries_at_level, row_rect};
use skia_safe::{Color, Surface, surfaces};
use taffy::prelude::Style;

const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#ff0000"/><rect x="8" width="8" height="16" fill="#0000ff"/></svg>"##;
struct NonClone;

struct RasterView {
    viewport: (f32, f32),
    direction: LayoutDirection,
    theme: Theme,
    scale: f32,
}

struct ContextRaster<'a> {
    widget: Widget<'a, NonClone>,
    states: HashMap<u64, WidgetState>,
    fonts: HashMap<(String, u32), Font>,
    taffy: TaffyTree<RutterContext>,
    root: NodeId,
}

impl<'a> ContextRaster<'a> {
    fn new(entries: &'a [ContextMenuEntry<'a, NonClone>]) -> Self {
        let widget = Widget::context_menu(
            Widget::Spacer {
                style: Style::default(),
            },
            entries,
            Style::default(),
        )
        .with_id(42);
        let mut state = ContextMenuState::default();
        state.open_at(80.0, 20.0);
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        Self {
            widget,
            states: HashMap::from([(42, WidgetState::ContextMenu(state))]),
            fonts: HashMap::new(),
            taffy,
            root,
        }
    }

    fn overlay(&self) -> ContextMenuOverlay<'_, NonClone> {
        let mut menus = Vec::new();
        collect_open_context_menus(&self.widget, &self.states, &mut Vec::new(), &mut menus);
        menus.pop().unwrap()
    }

    fn panels(&mut self, view: &RasterView) -> Vec<DropdownMenuSurface> {
        let font = get_cached_font(&mut self.fonts, "sans-serif", view.theme.font_body);
        context_menu_overlay::context_surfaces(
            &self.overlay(),
            view.viewport,
            view.direction,
            &font,
        )
    }

    fn paint(
        &mut self,
        surface: &mut Surface,
        view: &RasterView,
        native: bool,
        cache: &mut ImageRenderCache,
    ) {
        if native {
            draw_native_menu(
                surface.canvas(),
                &self.taffy,
                self.root,
                &self.widget,
                &self.states,
                NativeMenuKind::Context(42),
                Point::default(),
                view.viewport,
                Point::new(-1.0, -1.0),
                &mut self.fonts,
                cache,
                &view.theme,
                view.scale,
                view.direction,
            );
        } else {
            let font = get_cached_font(&mut self.fonts, "sans-serif", view.theme.font_body);
            draw_context_menu_overlays(
                surface.canvas(),
                &self.widget,
                &self.states,
                Point::new(-1.0, -1.0),
                &font,
                cache,
                &view.theme,
                view.scale,
                None,
                view.direction,
            );
        }
    }

    fn paint_full_parent(&mut self, surface: &mut Surface, view: &RasterView) {
        self.taffy
            .set_style(
                self.root,
                Style {
                    direction: match view.direction {
                        LayoutDirection::Ltr => taffy::Direction::Ltr,
                        LayoutDirection::Rtl => taffy::Direction::Rtl,
                    },
                    ..Style::default()
                },
            )
            .unwrap();
        draw_widgets_with_cache_and_custom_state(
            surface.canvas(),
            &self.taffy,
            self.root,
            &self.widget,
            &mut cosmic_text::FontSystem::new(),
            &mut cosmic_text::SwashCache::new(),
            Point::new(-1.0, -1.0),
            None,
            &HashMap::new(),
            &self.states,
            &HashMap::new(),
            &mut self.fonts,
            &mut TextBufferCache::default(),
            &mut ImageRenderCache::default(),
            true,
            &view.theme,
            view.scale,
            None,
        );
    }
}

fn surface(view: &RasterView) -> Surface {
    let mut surface = surfaces::raster_n32_premul((
        (view.viewport.0 * view.scale).ceil() as i32,
        (view.viewport.1 * view.scale).ceil() as i32,
    ))
    .unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    surface
}

fn pixel(surface: &mut Surface, point: Point, scale: f32) -> Color {
    surface
        .peek_pixels()
        .unwrap()
        .get_color(((point.x * scale) as i32, (point.y * scale) as i32))
}

fn color_distance(left: Color, right: Color) -> i32 {
    (i32::from(left.r()) - i32::from(right.r())).abs()
        + (i32::from(left.g()) - i32::from(right.g())).abs()
        + (i32::from(left.b()) - i32::from(right.b())).abs()
}

fn region_pixels(surface: &mut Surface, rect: SkiaRect, scale: f32) -> Vec<Color> {
    let pixels = surface.peek_pixels().unwrap();
    let mut colors = Vec::new();
    for y in (rect.top * scale).ceil() as i32..(rect.bottom * scale).floor() as i32 {
        for x in (rect.left * scale).ceil() as i32..(rect.right * scale).floor() as i32 {
            colors.push(pixels.get_color((x, y)));
        }
    }
    colors
}

#[test]
fn context_menu_native_and_overlay_icons_keep_colors_at_all_scales_and_reuse_cache() {
    let icon = ICON.to_vec();
    let shortcut = String::from("CTRL+C");
    let entries = [
        ContextMenuEntry::item("Copy", NonClone)
            .with_svg_icon(&icon)
            .with_shortcut_label(&shortcut),
        ContextMenuEntry::disabled("Locked")
            .with_svg_icon(&icon)
            .with_shortcut_label(&shortcut),
        ContextMenuEntry::submenu(
            "More",
            vec![
                ContextMenuEntry::item("Child", NonClone)
                    .with_svg_icon(&icon)
                    .with_shortcut_label("ALT+C"),
            ],
        )
        .with_svg_icon(&icon)
        .with_shortcut_label("ALT+M"),
        ContextMenuEntry::disabled_submenu("Blocked", Vec::new()).with_svg_icon(&icon),
    ];
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                let view = RasterView {
                    viewport: (900.0, 300.0),
                    direction,
                    theme: theme.clone(),
                    scale,
                };
                for native in [false, true] {
                    let mut fixture = ContextRaster::new(&entries);
                    fixture
                        .states
                        .get_mut(&42)
                        .unwrap()
                        .as_context_menu_mut()
                        .unwrap()
                        .navigation
                        .expand_submenu(vec![2], None);
                    let panels = fixture.panels(&view);
                    assert_eq!(panels.len(), 2);
                    let mut output = surface(&view);
                    let mut cache = ImageRenderCache::default();
                    fixture.paint(&mut output, &view, native, &mut cache);
                    let rows: Vec<_> = (0..4)
                        .map(|index| row_rect(&panels[0], &entries, index).unwrap())
                        .collect();
                    let sample = |row: SkiaRect, offset: f32| {
                        Point::new(
                            if direction == LayoutDirection::Ltr {
                                row.left + 7.0 + offset
                            } else {
                                row.right - 23.0 + offset
                            },
                            row.center_y(),
                        )
                    };
                    assert_eq!(pixel(&mut output, sample(rows[0], 4.0), scale), Color::RED);
                    assert_eq!(
                        pixel(&mut output, sample(rows[0], 12.0), scale),
                        Color::BLUE
                    );
                    for index in [1, 3] {
                        let attenuated = pixel(&mut output, sample(rows[index], 4.0), scale);
                        assert_ne!(attenuated, view.theme.surface);
                        assert!(
                            color_distance(attenuated, view.theme.surface)
                                < color_distance(Color::RED, view.theme.surface)
                        );
                    }
                    assert_eq!(pixel(&mut output, sample(rows[2], 4.0), scale), Color::RED);
                    let child =
                        row_rect(&panels[1], entries_at_level(&entries, &[2]).unwrap(), 0).unwrap();
                    assert_eq!(pixel(&mut output, sample(child, 12.0), scale), Color::BLUE);
                    let key = svg_cache_key(&icon, (16.0, 16.0), scale);
                    let cached = cache.svg_image(key).unwrap();
                    assert_eq!(
                        (cached.width(), cached.height()),
                        ((16.0 * scale) as i32, (16.0 * scale) as i32)
                    );
                    fixture.paint(&mut output, &view, native, &mut cache);
                    assert_eq!(
                        cache.svg_image(key).unwrap().unique_id(),
                        cached.unique_id()
                    );
                    assert_eq!(pixel(&mut output, sample(rows[0], 4.0), scale), Color::RED);
                }
            }
        }
    }
}

#[test]
fn context_menu_command_and_shortcut_ink_stays_in_separate_mirrored_columns() {
    let entries = [
        ContextMenuEntry::item("MMMMMMMMMMMMMMMMMMMMMMMMMMMM", NonClone)
            .with_svg_icon(ICON)
            .with_shortcut_label("CTRL+C"),
        ContextMenuEntry::item("Copy", NonClone)
            .with_svg_icon(ICON)
            .with_shortcut_label("SHIFT+ALT+CTRL+SUPER+LONG+LABEL"),
        ContextMenuEntry::item("Paste", NonClone)
            .with_svg_icon(ICON)
            .with_shortcut_label("CTRL+C"),
    ];
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                for viewport in [(1400.0, 220.0), (160.0, 220.0), (50.0, 220.0)] {
                    let view = RasterView {
                        viewport,
                        direction,
                        theme: theme.clone(),
                        scale,
                    };
                    for native in [false, true] {
                        let mut fixture = ContextRaster::new(&entries);
                        let panels = fixture.panels(&view);
                        let panel = &panels[0];
                        let font =
                            get_cached_font(&mut fixture.fonts, "sans-serif", theme.font_body);
                        let paint = Paint::default();
                        let max_command =
                            measure_single_line_text(&font, entries[0].label().unwrap(), &paint);
                        let max_shortcut = measure_single_line_text(
                            &font,
                            entries[1].shortcut_label().unwrap(),
                            &paint,
                        );
                        if viewport.0 > 1000.0 {
                            assert!(
                                (panel.rect.width()
                                    - (30.0 + max_command + 16.0 + max_shortcut + 10.0 + 8.0)
                                        .max(168.0))
                                .abs()
                                    < 0.001
                            );
                        }
                        let available = (panel.rect.width() - 40.0).max(0.0);
                        let shortcut_width = if available >= max_command + 16.0 + max_shortcut {
                            max_shortcut
                        } else if available >= 48.0 {
                            max_shortcut.min((available - 16.0) * 0.5)
                        } else {
                            0.0
                        };
                        let mut output = surface(&view);
                        fixture.paint(&mut output, &view, native, &mut ImageRenderCache::default());
                        if shortcut_width == 0.0 {
                            // On the tiny panel, text and icons are suppressed;
                            // nothing leaks across its bounds or trailing gutter.
                            let row = row_rect(panel, &entries, 0).unwrap();
                            let interior = SkiaRect::from_xywh(
                                row.left + 2.0,
                                row.top + 4.0,
                                row.width() - 4.0,
                                row.height() - 8.0,
                            );
                            assert!(
                                region_pixels(&mut output, interior, scale)
                                    .iter()
                                    .all(|color| *color == theme.surface),
                                "a panel narrower than its gutters cannot paint row content"
                            );
                            for outside in [
                                SkiaRect::from_ltrb(0.0, row.top, row.left - 2.0, row.bottom),
                                SkiaRect::from_ltrb(
                                    row.right + 2.0,
                                    row.top,
                                    viewport.0,
                                    row.bottom,
                                ),
                            ] {
                                assert!(
                                    region_pixels(&mut output, outside, scale)
                                        .iter()
                                        .all(|color| *color == Color::TRANSPARENT)
                                );
                            }
                            continue;
                        }
                        let first = row_rect(panel, &entries, 0).unwrap();
                        let third = row_rect(panel, &entries, 2).unwrap();
                        let shortcut_left = if direction == LayoutDirection::Ltr {
                            panel.rect.right - 10.0 - shortcut_width
                        } else {
                            panel.rect.left + 10.0
                        };
                        let shortcut_rect = |row: SkiaRect| {
                            SkiaRect::from_xywh(
                                shortcut_left,
                                row.top + 4.0,
                                shortcut_width,
                                row.height() - 8.0,
                            )
                        };
                        let first_ink = region_pixels(&mut output, shortcut_rect(first), scale);
                        let third_ink = region_pixels(&mut output, shortcut_rect(third), scale);
                        assert!(first_ink.iter().any(|color| *color != theme.surface));
                        assert_eq!(first_ink, third_ink, "shortcuts share one panel column");
                        let gap_left = if direction == LayoutDirection::Ltr {
                            shortcut_left - 16.0
                        } else {
                            shortcut_left + shortcut_width
                        };
                        for row in [first, third] {
                            let gap = SkiaRect::from_xywh(
                                gap_left + 2.0,
                                row.top + 4.0,
                                12.0,
                                row.height() - 8.0,
                            );
                            assert!(
                                region_pixels(&mut output, gap, scale)
                                    .iter()
                                    .all(|color| *color == theme.surface),
                                "command and shortcut ink must not fill the gap"
                            );
                        }
                        let command_left = if direction == LayoutDirection::Ltr {
                            panel.rect.left + 30.0
                        } else {
                            shortcut_left + shortcut_width + 16.0
                        };
                        let command_width = available - shortcut_width - 16.0;
                        let command = SkiaRect::from_xywh(
                            command_left,
                            first.top + 4.0,
                            command_width,
                            first.height() - 8.0,
                        );
                        assert!(
                            region_pixels(&mut output, command, scale)
                                .iter()
                                .any(|color| *color != theme.surface)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn context_menu_short_shortcuts_align_to_the_panel_inline_end() {
    let entries = [
        ContextMenuEntry::item("Copy", NonClone).with_shortcut_label("C"),
        ContextMenuEntry::item("Paste", NonClone).with_shortcut_label("CTRL+SHIFT+V"),
    ];
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                let view = RasterView {
                    viewport: (900.0, 300.0),
                    direction,
                    theme: theme.clone(),
                    scale,
                };
                for native in [false, true] {
                    let mut fixture = ContextRaster::new(&entries);
                    let panels = fixture.panels(&view);
                    let row = row_rect(&panels[0], &entries, 0).unwrap();
                    let mut output = surface(&view);
                    fixture.paint(&mut output, &view, native, &mut ImageRenderCache::default());
                    let x = match direction {
                        LayoutDirection::Ltr => row.right - 26.0,
                        LayoutDirection::Rtl => row.left + 10.0,
                    };
                    let end = SkiaRect::from_xywh(x, row.top + 4.0, 16.0, row.height() - 8.0);
                    assert!(
                        region_pixels(&mut output, end, scale)
                            .iter()
                            .any(|color| *color != theme.surface),
                        "the short shortcut must occupy the trailing end of its column"
                    );
                }
            }
        }
    }
}

#[test]
fn context_menu_columns_leave_clearance_for_mirrored_submenu_arrows_and_scrollbars() {
    let entries: Vec<_> = (0..30)
        .map(|_| {
            ContextMenuEntry::<NonClone>::submenu("A long submenu command", Vec::new())
                .with_svg_icon(ICON)
                .with_shortcut_label("CTRL+SHIFT+ALT+MORE")
        })
        .collect();
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                let view = RasterView {
                    viewport: (180.0, 180.0),
                    direction,
                    theme: theme.clone(),
                    scale,
                };
                for native in [false, true] {
                    let mut fixture = ContextRaster::new(&entries);
                    let panels = fixture.panels(&view);
                    assert!(panels[0].max_scroll > 0.0);
                    let row = row_rect(&panels[0], &entries, 0).unwrap();
                    let mut output = surface(&view);
                    fixture.paint(&mut output, &view, native, &mut ImageRenderCache::default());
                    let (gap, arrow, scrollbar) = match direction {
                        LayoutDirection::Ltr => {
                            (row.right - 22.0, row.right - 14.0, row.right - 5.0)
                        }
                        LayoutDirection::Rtl => (row.left + 22.0, row.left + 14.0, row.left + 5.0),
                    };
                    assert_eq!(
                        pixel(&mut output, Point::new(gap, row.center_y()), scale),
                        theme.surface
                    );
                    let arrow_region =
                        SkiaRect::from_xywh(arrow - 5.0, row.center_y() - 5.0, 10.0, 10.0);
                    assert!(
                        region_pixels(&mut output, arrow_region, scale)
                            .iter()
                            .any(|color| *color != theme.surface)
                    );
                    assert_ne!(
                        pixel(&mut output, Point::new(scrollbar, row.center_y()), scale),
                        theme.surface
                    );
                }
            }
        }
    }
}

#[test]
fn context_menu_rejected_svg_sources_and_raster_sizes_leave_usable_rows() {
    let oversized = vec![b' '; svg::MAX_SVG_BYTES + 1];
    let deep = format!(
        "<svg>{}{}</svg>",
        "<g>".repeat(svg::MAX_SVG_DEPTH + 1),
        "</g>".repeat(svg::MAX_SVG_DEPTH + 1)
    );
    let excessive_elements = format!("<svg>{}</svg>", "<g/>".repeat(svg::MAX_SVG_ELEMENTS));
    for source in [
        b"invalid svg".as_slice(),
        b"<svg><".as_slice(),
        &oversized,
        deep.as_bytes(),
        excessive_elements.as_bytes(),
    ] {
        let entries = [ContextMenuEntry::item("Copy", NonClone)
            .with_svg_icon(source)
            .with_shortcut_label("CTRL+C")];
        let view = RasterView {
            viewport: (600.0, 200.0),
            direction: LayoutDirection::Ltr,
            theme: Theme::dark(),
            scale: 1.0,
        };
        let mut fixture = ContextRaster::new(&entries);
        let panels = fixture.panels(&view);
        let row = row_rect(&panels[0], &entries, 0).unwrap();
        let mut cache = ImageRenderCache::default();
        let mut output = surface(&view);
        for native in [false, true] {
            fixture.paint(&mut output, &view, native, &mut cache);
            assert_eq!(
                pixel(
                    &mut output,
                    Point::new(row.left + 15.0, row.center_y()),
                    1.0
                ),
                view.theme.surface
            );
            assert!(
                cache
                    .svg_image(svg_cache_key(source, (16.0, 16.0), 1.0))
                    .is_none()
            );
            assert!(entries[0].action_message().is_some());
            let command =
                SkiaRect::from_xywh(row.left + 30.0, row.top + 4.0, 30.0, row.height() - 8.0);
            assert!(
                region_pixels(&mut output, command, 1.0)
                    .iter()
                    .any(|color| *color != view.theme.surface)
            );
        }
    }
    let entries = [ContextMenuEntry::item("Copy", NonClone).with_svg_icon(ICON)];
    let fixture = ContextRaster::new(&entries);
    let mut output = surfaces::raster_n32_premul((600, 200)).unwrap();
    let mut cache = ImageRenderCache::default();
    dropdown_menu_overlay::draw_context_menu(
        output.canvas(),
        &fixture.overlay(),
        (600.0, 200.0),
        Point::new(-1.0, -1.0),
        &get_cached_font(&mut HashMap::new(), "sans-serif", Theme::dark().font_body),
        &mut cache,
        &Theme::dark(),
        512.0,
        LayoutDirection::Ltr,
    );
    assert!(
        cache
            .svg_image(svg_cache_key(ICON, (16.0, 16.0), 512.0))
            .is_none()
    );
    assert_eq!(
        pixel(&mut output, Point::new(95.0, 42.0), 1.0),
        Theme::dark().surface
    );
}

#[test]
fn context_menu_parent_and_native_paint_use_the_retained_fractional_font() {
    let entries = [ContextMenuEntry::submenu(
        "A wide root label",
        vec![
            ContextMenuEntry::item("A wide child label", NonClone)
                .with_shortcut_label("CTRL+SHIFT+C"),
        ],
    )];
    for theme in [Theme::dark(), Theme::light()] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let mut theme = theme.clone();
            theme.font_body = 17.25;
            let view = RasterView {
                viewport: (1400.0, 300.0),
                direction,
                theme,
                scale: 1.0,
            };
            let mut fixture = ContextRaster::new(&entries);
            fixture
                .states
                .get_mut(&42)
                .unwrap()
                .as_context_menu_mut()
                .unwrap()
                .navigation
                .expand_submenu(vec![0], None);
            let mut retained = get_cached_font(&mut fixture.fonts, "serif", view.theme.font_body);
            retained.set_scale_x(1.75);
            fixture.fonts.insert(
                ("sans-serif".into(), view.theme.font_body.to_bits()),
                retained.clone(),
            );
            let panels = fixture.panels(&view);
            let cache_len = fixture.fonts.len();
            let mut expected = surface(&view);
            dropdown_menu_overlay::draw_context_menu(
                expected.canvas(),
                &fixture.overlay(),
                view.viewport,
                Point::new(-1.0, -1.0),
                &retained,
                &mut ImageRenderCache::default(),
                &view.theme,
                view.scale,
                direction,
            );
            for native in [false, true] {
                let mut output = surface(&view);
                if native {
                    fixture.paint(&mut output, &view, true, &mut ImageRenderCache::default());
                } else {
                    fixture.paint_full_parent(&mut output, &view);
                }
                for panel in &panels {
                    assert_eq!(
                        region_pixels(&mut output, panel.rect, 1.0),
                        region_pixels(&mut expected, panel.rect, 1.0)
                    );
                }
                assert_eq!(fixture.fonts.len(), cache_len);
            }
        }
    }
}

#[test]
fn context_menu_closed_parent_scene_does_not_resolve_menu_fonts() {
    let entries = [ContextMenuEntry::item("Copy", NonClone).with_shortcut_label("CTRL+C")];
    let mut fixture = ContextRaster::new(&entries);
    fixture
        .states
        .get_mut(&42)
        .unwrap()
        .as_context_menu_mut()
        .unwrap()
        .close();
    let view = RasterView {
        viewport: (600.0, 300.0),
        direction: LayoutDirection::Ltr,
        theme: Theme::dark(),
        scale: 1.0,
    };
    fixture.paint_full_parent(&mut surface(&view), &view);
    assert!(
        fixture.fonts.is_empty(),
        "a closed menu must not initialize its font"
    );
}
