// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Canvas, Color, Font, Paint, Rect};

use super::text::{BoundedTextAlignment, draw_bounded_single_line_text, get_cached_font};
use crate::i18n::LayoutDirection;
use crate::layout::field::FieldInsets;
use crate::theme::Theme;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_annotations(
    canvas: &Canvas,
    size: (f32, f32),
    label: &str,
    error: Option<&str>,
    active: bool,
    direction: LayoutDirection,
    fonts: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let insets = FieldInsets::new(label, error);
    if insets.top > 0.0 {
        draw_annotation(
            canvas,
            label,
            Rect::from_xywh(4.0, 0.0, (size.0 - 8.0).max(0.0), insets.top.min(size.1)),
            theme.font_label,
            if active {
                theme.primary
            } else {
                Theme::alpha(theme.on_surface, 180)
            },
            direction,
            fonts,
        );
    }
    if let Some(error) = error.filter(|text| !text.is_empty()) {
        let top = insets.body(size).bottom;
        draw_annotation(
            canvas,
            error,
            Rect::from_xywh(4.0, top, (size.0 - 8.0).max(0.0), (size.1 - top).max(0.0)),
            theme.font_small,
            theme.error,
            direction,
            fonts,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_annotation(
    canvas: &Canvas,
    text: &str,
    bounds: Rect,
    font_size: f32,
    color: Color,
    direction: LayoutDirection,
    fonts: &mut HashMap<(String, u32), Font>,
) {
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let font = get_cached_font(fonts, "sans-serif", font_size);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_anti_alias(true);
    draw_bounded_single_line_text(
        canvas,
        text,
        bounds,
        &font,
        &paint,
        BoundedTextAlignment::InlineStart(direction),
    );
}
