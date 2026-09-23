// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Canvas, Font, Paint, Point, RRect, Rect, paint};
use unicode_segmentation::UnicodeSegmentation;

use crate::pointer::{ActiveDragBadge, DragBadgeContent, DragBadgeIcon};
use crate::render::text::{draw_single_line_text, get_cached_font, measure_single_line_text};

const BADGE_SIDE: f32 = 32.0;
const BADGE_OFFSET: f32 = 16.0;
const MAX_TEXT_BADGE_WIDTH: f32 = 120.0;
const TEXT_INSET: f32 = 10.0;
const ACCEPTANCE_SPACE: f32 = 12.0;

fn badge_origin(pointer: Point, viewport: (f32, f32), badge_width: f32) -> Option<Point> {
    let (width, height) = viewport;
    if !pointer.x.is_finite()
        || !pointer.y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return None;
    }
    let x = if pointer.x + BADGE_OFFSET + badge_width <= width {
        pointer.x + BADGE_OFFSET
    } else {
        pointer.x - BADGE_OFFSET - badge_width
    };
    let y = if pointer.y + BADGE_OFFSET + BADGE_SIDE <= height {
        pointer.y + BADGE_OFFSET
    } else {
        pointer.y - BADGE_OFFSET - BADGE_SIDE
    };
    Some(Point::new(
        x.clamp(0.0, (width - badge_width).max(0.0)),
        y.clamp(0.0, (height - BADGE_SIDE).max(0.0)),
    ))
}

/// Drawn after all widget/overlay content, outside hit testing and AccessKit.
pub(crate) fn draw_drag_badge(
    canvas: &Canvas,
    badge: &ActiveDragBadge,
    pointer: Point,
    viewport: (f32, f32),
    font_cache: &mut HashMap<(String, u32), Font>,
) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let preferred_width = match &badge.appearance.content {
        DragBadgeContent::Text(text) => {
            let font = get_cached_font(font_cache, "sans-serif", 12.0);
            let acceptance = if badge.can_drop {
                ACCEPTANCE_SPACE
            } else {
                0.0
            };
            (measure_single_line_text(&font, text, &paint) + TEXT_INSET * 2.0 + acceptance)
                .clamp(BADGE_SIDE, MAX_TEXT_BADGE_WIDTH)
        }
        _ => BADGE_SIDE,
    };
    let badge_width = if viewport.0 >= BADGE_SIDE {
        preferred_width.min(viewport.0)
    } else {
        preferred_width
    };
    let Some(origin) = badge_origin(pointer, viewport, badge_width) else {
        return;
    };
    canvas.save();
    canvas.clip_rect(
        Rect::from_xywh(0.0, 0.0, viewport.0, viewport.1),
        None,
        true,
    );
    canvas.translate((origin.x, origin.y));
    paint.set_color(badge.appearance.foreground);
    let outer = Rect::from_xywh(2.0, 2.0, badge_width - 4.0, 28.0);
    canvas.draw_round_rect(outer, 14.0, 14.0, &paint);
    paint.set_color(badge.appearance.background);
    let inner = Rect::from_xywh(4.0, 4.0, badge_width - 8.0, 24.0);
    canvas.draw_round_rect(inner, 12.0, 12.0, &paint);
    paint.set_color(badge.appearance.foreground);
    draw_badge_content(canvas, badge, badge_width, &mut paint, font_cache);
    canvas.restore();
}

fn draw_badge_content(
    canvas: &Canvas,
    badge: &ActiveDragBadge,
    badge_width: f32,
    paint: &mut Paint,
    font_cache: &mut HashMap<(String, u32), Font>,
) {
    // Only text badges grow; keep every glyph and decoded image within its face.
    canvas.save();
    let face = Rect::from_xywh(4.0, 4.0, badge_width - 8.0, 24.0);
    canvas.clip_rrect(RRect::new_rect_xy(face, 12.0, 12.0), None, true);
    match &badge.appearance.content {
        DragBadgeContent::Status => {
            if badge.can_drop {
                canvas.draw_rect(Rect::from_xywh(10.0, 14.5, 12.0, 3.0), paint);
                canvas.draw_rect(Rect::from_xywh(14.5, 10.0, 3.0, 12.0), paint);
            } else {
                canvas.draw_circle((16.0, 16.0), 2.5, paint);
            }
        }
        DragBadgeContent::Icon(icon) => draw_badge_icon(canvas, *icon, paint),
        DragBadgeContent::Text(text) => {
            let font = get_cached_font(font_cache, "sans-serif", 12.0);
            let acceptance = if badge.can_drop {
                ACCEPTANCE_SPACE
            } else {
                0.0
            };
            let max_width = badge_width - TEXT_INSET * 2.0 - acceptance;
            canvas.save();
            canvas.clip_rect(
                Rect::from_xywh(TEXT_INSET, 4.0, max_width, 24.0),
                None,
                true,
            );
            let label = truncated_badge_text(text, &font, paint, max_width);
            draw_single_line_text(canvas, &label, (TEXT_INSET, 20.0), &font, paint);
            canvas.restore();
        }
        DragBadgeContent::Image(image) => {
            canvas.save();
            canvas.clip_rrect(
                RRect::new_rect_xy(Rect::from_xywh(6.0, 6.0, 20.0, 20.0), 10.0, 10.0),
                None,
                true,
            );
            canvas.draw_image(image.as_ref(), (6.0, 6.0), Some(paint));
            canvas.restore();
        }
    }
    canvas.restore();
    if badge.can_drop && !matches!(&badge.appearance.content, DragBadgeContent::Status) {
        draw_acceptance_mark(canvas, badge, badge_width, paint);
    }
}

fn truncated_badge_text(text: &str, font: &Font, paint: &Paint, max_width: f32) -> String {
    if measure_single_line_text(font, text, paint) <= max_width {
        return text.into();
    }
    if measure_single_line_text(font, "…", paint) > max_width {
        return String::new();
    }
    let mut label = String::new();
    for grapheme in text.graphemes(true) {
        let candidate = format!("{label}{grapheme}…");
        if measure_single_line_text(font, &candidate, paint) > max_width {
            break;
        }
        label.push_str(grapheme);
    }
    label.push('…');
    label
}

fn draw_badge_icon(canvas: &Canvas, icon: DragBadgeIcon, paint: &mut Paint) {
    match icon {
        DragBadgeIcon::Copy => {
            paint.set_style(paint::Style::Stroke);
            paint.set_stroke_width(2.0);
            canvas.draw_rect(Rect::from_xywh(10.0, 9.0, 10.0, 11.0), paint);
            canvas.draw_rect(Rect::from_xywh(13.0, 12.0, 10.0, 11.0), paint);
            paint.set_style(paint::Style::Fill);
        }
        DragBadgeIcon::Move => {
            canvas.draw_rect(Rect::from_xywh(9.0, 14.5, 14.0, 3.0), paint);
            canvas.draw_rect(Rect::from_xywh(14.5, 9.0, 3.0, 14.0), paint);
        }
    }
}

fn draw_acceptance_mark(
    canvas: &Canvas,
    badge: &ActiveDragBadge,
    badge_width: f32,
    paint: &mut Paint,
) {
    let x = badge_width - 7.0;
    paint.set_color(badge.appearance.foreground);
    canvas.draw_circle((x, 25.0), 5.0, paint);
    paint.set_color(badge.appearance.background);
    canvas.draw_circle((x, 25.0), 3.5, paint);
    paint.set_color(badge.appearance.foreground);
    canvas.draw_rect(Rect::from_xywh(x - 2.5, 24.5, 5.0, 1.0), paint);
    canvas.draw_rect(Rect::from_xywh(x - 0.5, 22.5, 1.0, 5.0), paint);
}

#[cfg(test)]
mod tests {
    use skia_safe::{Color, surfaces};

    use super::*;
    use crate::pointer::{DragBadge, DragBadgeIcon};

    #[test]
    fn badge_prefers_below_right_and_flips_at_viewport_edges() {
        assert_eq!(
            badge_origin(Point::new(20.0, 30.0), (200.0, 200.0), BADGE_SIDE),
            Some(Point::new(36.0, 46.0))
        );
        assert_eq!(
            badge_origin(Point::new(180.0, 190.0), (200.0, 200.0), BADGE_SIDE),
            Some(Point::new(132.0, 142.0))
        );
        assert_eq!(
            badge_origin(Point::new(180.0, 190.0), (200.0, 200.0), 120.0),
            Some(Point::new(44.0, 142.0))
        );
        assert!(badge_origin(Point::new(f32::NAN, 0.0), (200.0, 200.0), BADGE_SIDE).is_none());
        assert!(badge_origin(Point::new(10.0, 10.0), (0.0, 200.0), BADGE_SIDE).is_none());
    }

    #[test]
    fn badge_paints_dot_or_plus_using_the_declared_colors() {
        let appearance = DragBadge::new(Color::from_rgb(255, 191, 0), Color::BLACK);
        for can_drop in [false, true] {
            let mut surface = surfaces::raster_n32_premul((80, 80)).unwrap();
            surface.canvas().clear(Color::TRANSPARENT);
            draw_drag_badge(
                surface.canvas(),
                &ActiveDragBadge {
                    appearance: appearance.clone(),
                    can_drop,
                },
                Point::new(0.0, 0.0),
                (80.0, 80.0),
                &mut HashMap::new(),
            );
            let pixels = surface.peek_pixels().unwrap();
            assert_eq!(pixels.get_color((32, 32)), appearance.foreground);
            assert_eq!(pixels.get_color((32, 24)), appearance.background);
            assert_eq!(pixels.get_color((0, 0)), Color::TRANSPARENT);
            assert_eq!(
                pixels.get_color((32, 28)),
                if can_drop {
                    appearance.foreground
                } else {
                    appearance.background
                }
            );
        }
    }

    #[test]
    fn text_is_truncated_without_splitting_graphemes_or_leaving_the_badge() {
        let mut font_cache = HashMap::new();
        let font = get_cached_font(&mut font_cache, "sans-serif", 12.0);
        let paint = Paint::default();
        let truncated =
            truncated_badge_text("A long card name with more words", &font, &paint, 100.0);
        assert!(truncated.ends_with('…'));
        assert!(
            truncated.chars().count() > 5,
            "label was shortened to {truncated:?}"
        );
        assert!(measure_single_line_text(&font, &truncated, &paint) <= 100.0);
        assert_eq!(
            truncated_badge_text("EmeraldLeaf", &font, &paint, 100.0),
            "EmeraldLeaf"
        );

        let emoji = truncated_badge_text("👨‍👩‍👧‍👦 with more", &font, &paint, 20.0);
        assert!(!emoji.contains('👩') || emoji.starts_with("👨‍👩‍👧‍👦"));
        assert_eq!(truncated_badge_text("long", &font, &paint, 1.0), "");

        let badge = DragBadge::new(Color::RED, Color::WHITE)
            .with_text("A long card name")
            .unwrap();
        let mut surface = surfaces::raster_n32_premul((160, 80)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        draw_drag_badge(
            surface.canvas(),
            &ActiveDragBadge {
                appearance: badge,
                can_drop: false,
            },
            Point::new(0.0, 0.0),
            (160.0, 80.0),
            &mut font_cache,
        );
        assert_eq!(
            surface.peek_pixels().unwrap().get_color((0, 0)),
            Color::TRANSPARENT
        );
        assert_eq!(
            surface.peek_pixels().unwrap().get_color((19, 32)),
            Color::WHITE
        );
        assert_eq!(
            surface.peek_pixels().unwrap().get_color((150, 32)),
            Color::TRANSPARENT
        );
    }

    #[test]
    fn text_badge_keeps_its_acceptance_mark_and_paint_inside_the_viewport() {
        let badge = DragBadge::new(Color::RED, Color::WHITE)
            .with_text("EmeraldLeaf")
            .unwrap();
        let mut surface = surfaces::raster_n32_premul((160, 45)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        draw_drag_badge(
            surface.canvas(),
            &ActiveDragBadge {
                appearance: badge,
                can_drop: true,
            },
            Point::new(138.0, 32.0),
            (140.0, 35.0),
            &mut HashMap::new(),
        );
        let pixels = surface.peek_pixels().unwrap();
        assert_ne!(pixels.get_color((115, 25)), Color::RED);
        assert_ne!(pixels.get_color((115, 25)), Color::TRANSPARENT);
        assert_eq!(pixels.get_color((141, 25)), Color::TRANSPARENT);
        assert_eq!(pixels.get_color((115, 36)), Color::TRANSPARENT);
    }

    #[test]
    fn text_badge_shrinks_to_a_narrow_viewport_before_truncating() {
        let badge = DragBadge::new(Color::RED, Color::WHITE)
            .with_text("EmeraldLeaf")
            .unwrap();
        let mut surface = surfaces::raster_n32_premul((80, 50)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        draw_drag_badge(
            surface.canvas(),
            &ActiveDragBadge {
                appearance: badge,
                can_drop: true,
            },
            Point::new(60.0, 10.0),
            (64.0, 40.0),
            &mut HashMap::new(),
        );
        let pixels = surface.peek_pixels().unwrap();
        assert_ne!(pixels.get_color((57, 25)), Color::RED);
        assert_eq!(pixels.get_color((65, 25)), Color::TRANSPARENT);
    }

    #[test]
    fn geometric_icons_keep_acceptance_indicator_inside_the_badge() {
        for icon in [DragBadgeIcon::Copy, DragBadgeIcon::Move] {
            let badge = DragBadge::new(Color::RED, Color::WHITE).with_icon(icon);
            let mut surface = surfaces::raster_n32_premul((80, 80)).unwrap();
            surface.canvas().clear(Color::TRANSPARENT);
            draw_drag_badge(
                surface.canvas(),
                &ActiveDragBadge {
                    appearance: badge,
                    can_drop: true,
                },
                Point::new(0.0, 0.0),
                (80.0, 80.0),
                &mut HashMap::new(),
            );
            let pixels = surface.peek_pixels().unwrap();
            assert_ne!(pixels.get_color((41, 41)), Color::RED);
            assert_eq!(pixels.get_color((0, 0)), Color::TRANSPARENT);
        }
    }

    #[test]
    fn raster_image_stays_inside_the_badge_face() {
        let mut source = surfaces::raster_n32_premul((20, 20)).unwrap();
        source.canvas().clear(Color::GREEN);
        let mut appearance = DragBadge::new(Color::RED, Color::WHITE);
        appearance.content = DragBadgeContent::Image(std::sync::Arc::new(source.image_snapshot()));
        let mut surface = surfaces::raster_n32_premul((80, 80)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        draw_drag_badge(
            surface.canvas(),
            &ActiveDragBadge {
                appearance,
                can_drop: false,
            },
            Point::new(0.0, 0.0),
            (80.0, 80.0),
            &mut HashMap::new(),
        );

        let pixels = surface.peek_pixels().unwrap();
        assert_eq!(pixels.get_color((32, 32)), Color::GREEN);
        assert_eq!(pixels.get_color((17, 17)), Color::TRANSPARENT);
    }
}
