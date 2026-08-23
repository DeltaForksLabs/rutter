// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{
    Color as SkiaColor, Contains, Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas,
    paint,
};

use super::{
    draw_focus_outline,
    text::{TextDrawInput, draw_text_line},
};
use crate::theme::Theme;
use crate::widgets::counter::counter_action_width;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CounterSegment {
    Decrement,
    Value,
    Increment,
}

pub(super) struct CounterRenderInput<'a> {
    pub(super) canvas: &'a Canvas,
    pub(super) value: i64,
    pub(super) min: i64,
    pub(super) max: i64,
    pub(super) is_focused: bool,
    pub(super) size: (f32, f32),
    pub(super) mouse: Point,
    pub(super) font_cache: &'a mut HashMap<(String, u32), Font>,
    pub(super) theme: &'a Theme,
}

#[derive(Clone, Copy)]
struct CounterSegments {
    decrement: SkiaRect,
    value: SkiaRect,
    increment: SkiaRect,
}

pub(super) fn draw_counter(mut input: CounterRenderInput<'_>) {
    let segments = counter_segments(input.size);
    let can_decrement = input.min <= input.max && input.value > input.min;
    let can_increment = input.min <= input.max && input.value < input.max;
    draw_counter_frame(&input);
    draw_counter_actions(&input, segments, can_decrement, can_increment);
    draw_counter_separators(&input, segments);
    draw_counter_labels(&mut input, segments, can_decrement, can_increment);
}

pub(crate) fn counter_segment_at(size: (f32, f32), point: Point) -> CounterSegment {
    let segments = counter_segments(size);
    if segments.decrement.contains(point) {
        return CounterSegment::Decrement;
    }
    if segments.increment.contains(point) {
        return CounterSegment::Increment;
    }
    CounterSegment::Value
}

fn counter_segments(size: (f32, f32)) -> CounterSegments {
    let action_width = counter_button_width(size);
    CounterSegments {
        decrement: SkiaRect::from_xywh(0.0, 0.0, action_width, size.1),
        value: SkiaRect::from_xywh(
            action_width,
            0.0,
            (size.0 - action_width * 2.0).max(0.0),
            size.1,
        ),
        increment: SkiaRect::from_xywh(size.0 - action_width, 0.0, action_width, size.1),
    }
}

fn counter_button_width(size: (f32, f32)) -> f32 {
    let available_width = (size.0 / 3.0).max(0.0);
    counter_action_width(size.1).min(available_width)
}

fn draw_counter_frame(input: &CounterRenderInput<'_>) {
    let rect = SkiaRect::from_xywh(0.0, 0.0, input.size.0, input.size.1);
    let mut fill = Paint::default();
    fill.set_color(Theme::alpha(input.theme.on_surface, 10));
    fill.set_anti_alias(true);
    input.canvas.draw_rrect(
        RRect::new_rect_xy(rect, input.theme.radius_sm, input.theme.radius_sm),
        &fill,
    );
    draw_counter_border(input.canvas, rect, input.theme);
}

fn draw_counter_border(canvas: &Canvas, rect: SkiaRect, theme: &Theme) {
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(Theme::alpha(theme.on_surface, 100));
    border.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
        &border,
    );
}

fn draw_counter_actions(
    input: &CounterRenderInput<'_>,
    segments: CounterSegments,
    can_decrement: bool,
    can_increment: bool,
) {
    let bounds = SkiaRect::from_xywh(0.0, 0.0, input.size.0, input.size.1);
    input.canvas.save();
    input.canvas.clip_rrect(
        RRect::new_rect_xy(bounds, input.theme.radius_sm, input.theme.radius_sm),
        None,
        true,
    );
    draw_counter_action(input, segments.decrement, can_decrement);
    draw_counter_action(input, segments.increment, can_increment);
    input.canvas.restore();
}

fn draw_counter_action(input: &CounterRenderInput<'_>, rect: SkiaRect, enabled: bool) {
    let hovered = enabled && rect.contains(input.mouse);
    let color = counter_action_color(enabled, hovered, input.theme);
    if color == SkiaColor::TRANSPARENT {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_anti_alias(true);
    input.canvas.draw_rect(rect, &paint);
}

fn counter_action_color(enabled: bool, hovered: bool, theme: &Theme) -> SkiaColor {
    if !enabled {
        return Theme::alpha(theme.on_surface, 5);
    }
    if hovered {
        return Theme::alpha(theme.primary, 30);
    }
    SkiaColor::TRANSPARENT
}

fn draw_counter_separators(input: &CounterRenderInput<'_>, segments: CounterSegments) {
    let mut separator = Paint::default();
    separator.set_color(Theme::alpha(input.theme.on_surface, 70));
    separator.set_stroke_width(1.0);
    separator.set_anti_alias(true);
    input.canvas.draw_line(
        (segments.decrement.right, 0.0),
        (segments.decrement.right, input.size.1),
        &separator,
    );
    input.canvas.draw_line(
        (segments.increment.left, 0.0),
        (segments.increment.left, input.size.1),
        &separator,
    );
}

fn draw_counter_labels(
    input: &mut CounterRenderInput<'_>,
    segments: CounterSegments,
    can_decrement: bool,
    can_increment: bool,
) {
    draw_counter_label(input, segments.decrement, "-", can_decrement);
    let value = input.value.to_string();
    draw_counter_label(input, segments.value, &value, true);
    draw_counter_label(input, segments.increment, "+", can_increment);
    if input.is_focused {
        draw_counter_focus(input.canvas, input.size, input.theme);
    }
}

fn draw_counter_label(
    input: &mut CounterRenderInput<'_>,
    rect: SkiaRect,
    label: &str,
    enabled: bool,
) {
    input.canvas.save();
    input.canvas.translate((rect.left, rect.top));
    draw_text_line(TextDrawInput {
        canvas: input.canvas,
        text: label,
        size: (rect.width(), rect.height()),
        color: counter_label_color(enabled, input.theme),
        font_size: input.theme.font_body,
        font_cache: input.font_cache,
        center: true,
    });
    input.canvas.restore();
}

fn counter_label_color(enabled: bool, theme: &Theme) -> SkiaColor {
    if enabled {
        return theme.on_surface;
    }
    Theme::alpha(theme.on_surface, 80)
}

fn draw_counter_focus(canvas: &Canvas, size: (f32, f32), theme: &Theme) {
    let rect = SkiaRect::from_xywh(1.0, 1.0, (size.0 - 2.0).max(0.0), (size.1 - 2.0).max(0.0));
    draw_focus_outline(canvas, rect, theme.radius_sm, theme);
}

#[cfg(test)]
mod tests {
    use skia_safe::{Color, Point, surfaces};

    use super::*;

    #[test]
    fn counter_segments_keep_actions_at_the_outer_edges() {
        let size = (120.0, 40.0);
        assert_eq!(
            counter_segment_at(size, Point::new(12.0, 20.0)),
            CounterSegment::Decrement
        );
        assert_eq!(
            counter_segment_at(size, Point::new(60.0, 20.0)),
            CounterSegment::Value
        );
        assert_eq!(
            counter_segment_at(size, Point::new(108.0, 20.0)),
            CounterSegment::Increment
        );
    }

    #[test]
    fn counter_draws_its_control_surface() {
        let mut surface = surfaces::raster_n32_premul((120, 40)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        let mut font_cache = HashMap::new();
        draw_counter(CounterRenderInput {
            canvas: surface.canvas(),
            value: 1,
            min: 0,
            max: 2,
            is_focused: true,
            size: (120.0, 40.0),
            mouse: Point::new(-1.0, -1.0),
            font_cache: &mut font_cache,
            theme: &Theme::default(),
        });

        assert_ne!(
            surface.peek_pixels().unwrap().get_color((60, 20)),
            Color::TRANSPARENT
        );
    }
}
