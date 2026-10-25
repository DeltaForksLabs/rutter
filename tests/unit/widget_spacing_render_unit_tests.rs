use super::*;
use crate::layout::build_taffy_tree_with_direction;
use skia_safe::{Surface, surfaces};
use taffy::prelude::{Size, Style};

fn field_surface(
    multiline: bool,
    label: &str,
    error: &str,
    width: f32,
    direction: LayoutDirection,
    theme: &Theme,
) -> Surface {
    let style = Style {
        size: Size::from_lengths(width, 40.0),
        ..Style::default()
    };
    let widget: Widget<'_, ()> = if multiline {
        Widget::text_area(
            |_| (),
            None,
            style,
            label,
            InputState::Error,
            "",
            Some(error.to_owned()),
        )
    } else {
        Widget::text_input(
            |_| (),
            None,
            style,
            label,
            "",
            InputState::Error,
            Some(error.to_owned()),
            false,
        )
    }
    .with_id(17);
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree_with_direction(
        &mut taffy,
        &widget,
        fonts.clone(),
        &HashMap::new(),
        direction,
    );
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(width as u32, 200),
        fonts,
        &RichTextRenderer::default(),
    );
    let mut surface = surfaces::raster_n32_premul((240, 160)).unwrap();
    surface.canvas().clear(SkiaColor::TRANSPARENT);
    surface.canvas().translate((12.0, 12.0));
    draw_widgets(
        surface.canvas(),
        &taffy,
        root,
        &widget,
        &mut FontSystem::new(),
        &mut SwashCache::new(),
        Point::new(-100.0, -100.0),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &mut HashMap::new(),
        &mut TextBufferCache::default(),
        false,
        theme,
        1.0,
    );
    surface
}

fn assert_annotation_rows_match(
    surface: &mut Surface,
    width: f32,
    label: &str,
    error: &str,
    direction: LayoutDirection,
    theme: &Theme,
) {
    let mut expected = surfaces::raster_n32_premul((240, 160)).unwrap();
    let canvas = expected.canvas();
    canvas.clear(SkiaColor::TRANSPARENT);
    canvas.translate((12.0, 12.0));
    let mut fonts = HashMap::new();
    for (text, font_size, top, color) in [
        (
            label,
            theme.font_label,
            0.0,
            Theme::alpha(theme.on_surface, 180),
        ),
        (error, theme.font_small, 60.0, theme.error),
    ] {
        let font = get_cached_font(&mut fonts, "sans-serif", font_size);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.set_anti_alias(true);
        let text_width = measure_single_line_text(&font, text, &paint);
        let x = match direction {
            LayoutDirection::Ltr => 4.0,
            LayoutDirection::Rtl => width - 4.0 - text_width,
        };
        let (_, metrics) = font.metrics();
        let baseline = top + 10.0 - (metrics.ascent + metrics.descent) / 2.0;
        canvas.save();
        canvas.clip_rect(
            SkiaRect::from_xywh(4.0, top, width - 8.0, 20.0),
            None,
            false,
        );
        draw_single_line_text(canvas, text, (x, baseline), &font, &paint);
        canvas.restore();
    }
    let actual_pixels = surface.peek_pixels().unwrap();
    let expected_pixels = expected.peek_pixels().unwrap();
    for rows in [12..30, 74..92] {
        let mut ink = 0;
        for y in rows {
            for x in 0..240 {
                let actual = actual_pixels.get_color((x, y));
                assert_eq!(
                    actual,
                    expected_pixels.get_color((x, y)),
                    "annotation at ({x}, {y}), {direction:?}"
                );
                ink += usize::from(actual != SkiaColor::TRANSPARENT);
            }
        }
        assert!(ink > 0, "annotation disappeared");
    }
}

#[test]
fn short_rtl_input_labels_and_errors_follow_the_resolved_node_direction() {
    for theme in [Theme::light(), Theme::dark()] {
        for multiline in [false, true] {
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                let mut surface = field_surface(multiline, "שם", "נא", 120.0, direction, &theme);
                assert_annotation_rows_match(&mut surface, 120.0, "שם", "נא", direction, &theme);
            }
        }
    }
}

#[test]
fn ellipsized_rtl_input_annotations_remain_right_aligned_and_contained() {
    for theme in [Theme::light(), Theme::dark()] {
        let font = get_cached_font(&mut HashMap::new(), "sans-serif", theme.font_label);
        // Enough room for exactly these two characters and the ellipsis, even
        // after Taffy's integer rounding; another character cannot fit.
        let width = (measure_single_line_text(&font, "שם…", &Paint::default()) + 8.0).ceil();
        for multiline in [false, true] {
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                let mut surface = field_surface(
                    multiline,
                    "שםשםשםשםשםשםשםשםשםשם",
                    "נא",
                    width,
                    direction,
                    &theme,
                );
                assert_annotation_rows_match(&mut surface, width, "שם…", "נא", direction, &theme);
            }
        }
    }
}

#[test]
fn ellipsized_rtl_errors_use_the_resolved_direction_independently_of_the_label() {
    for theme in [Theme::light(), Theme::dark()] {
        let font = get_cached_font(&mut HashMap::new(), "sans-serif", theme.font_small);
        let width = (measure_single_line_text(&font, "נא…", &Paint::default()) + 8.0).ceil();
        for multiline in [false, true] {
            for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
                let mut surface = field_surface(
                    multiline,
                    "שם",
                    "נאנאנאנאנאנאנאנאנאנא",
                    width,
                    direction,
                    &theme,
                );
                assert_annotation_rows_match(&mut surface, width, "שם", "נא…", direction, &theme);
            }
        }
    }
}

fn button_surface(
    theme: &Theme,
    variant: ButtonVariant,
    text: Option<&str>,
    scale: f32,
) -> Surface {
    let mut surface = surfaces::raster_n32_premul((400, 200)).unwrap();
    let canvas = surface.canvas();
    canvas.clear(SkiaColor::TRANSPARENT);
    canvas.scale((scale, scale));
    canvas.translate((32.0, 24.0));
    let mut fonts = HashMap::new();
    if let Some(text) = text {
        draw_text_button(TextButtonRenderInput {
            canvas,
            text,
            color: None,
            variant,
            is_focused: true,
            size: (72.0, 40.0),
            mouse: Point::new(-100.0, -100.0),
            font_cache: &mut fonts,
            theme,
        });
    } else {
        draw_button_frame(
            canvas,
            None,
            variant,
            true,
            (72.0, 40.0),
            Point::new(-100.0, -100.0),
            theme,
        );
    }
    surface
}

#[test]
fn narrow_localized_button_labels_stay_padded_and_leave_frames_focus_and_neighbors_clear() {
    for theme in [Theme::light(), Theme::dark()] {
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Ghost,
            ButtonVariant::Text,
        ] {
            for scale in [1.0, 1.5, 2.0] {
                for text in [
                    "Änderungen übernehmen und speichern",
                    "تأكيد إعدادات الحساب",
                    "保存所有更改并继续",
                    "Save\r\nall\tchanges",
                ] {
                    let mut actual = button_surface(&theme, variant, Some(text), scale);
                    let mut frame = button_surface(&theme, variant, None, scale);
                    let actual_pixels = actual.peek_pixels().unwrap();
                    let frame_pixels = frame.peek_pixels().unwrap();
                    let content = SkiaRect::from_xywh(
                        (32.0 + 16.0) * scale,
                        (24.0 + 8.0) * scale,
                        40.0 * scale,
                        24.0 * scale,
                    );
                    let mut text_pixels = 0;
                    for y in 0..200 {
                        for x in 0..400 {
                            let actual = actual_pixels.get_color((x, y));
                            let frame = frame_pixels.get_color((x, y));
                            if actual != frame {
                                assert!(
                                    content.contains(Point::new(x as f32, y as f32)),
                                    "label escaped padding at ({x}, {y}), scale={scale}"
                                );
                                text_pixels += 1;
                            }
                            if x >= ((32.0 + 72.0 + 8.0) * scale) as i32 {
                                assert_eq!(
                                    actual,
                                    SkiaColor::TRANSPARENT,
                                    "label painted into its neighbor"
                                );
                            }
                        }
                    }
                    assert!(
                        text_pixels > 0,
                        "narrow button lost its visible label/ellipsis"
                    );
                }
            }
        }
    }
}

#[test]
fn short_button_labels_remain_centered_without_clipping_the_focus_frame() {
    for theme in [Theme::light(), Theme::dark()] {
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Ghost,
            ButtonVariant::Text,
        ] {
            for scale in [1.0, 1.5, 2.0] {
                let mut actual = button_surface(&theme, variant, Some("OK"), scale);
                let mut expected = button_surface(&theme, variant, None, scale);
                let canvas = expected.canvas();
                let font = get_cached_font(&mut HashMap::new(), "sans-serif", theme.font_body);
                let mut paint = Paint::default();
                paint.set_anti_alias(true);
                paint.set_color(button_text_color(None, variant, false, &theme));
                let width = measure_single_line_text(&font, "OK", &paint);
                let (_, metrics) = font.metrics();
                draw_single_line_text(
                    canvas,
                    "OK",
                    (
                        (72.0 - width) / 2.0,
                        20.0 - (metrics.ascent + metrics.descent) / 2.0,
                    ),
                    &font,
                    &paint,
                );
                assert_eq!(
                    actual.peek_pixels().unwrap().bytes().unwrap(),
                    expected.peek_pixels().unwrap().bytes().unwrap()
                );
            }
        }
    }
}
