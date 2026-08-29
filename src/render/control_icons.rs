// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::{Color, Paint, Point, Rect, canvas::Canvas, paint};

const CONTROL_ICON_STROKE_WIDTH: f32 = 1.7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlChevronDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Draws a font-independent magnifier for search controls.
///
/// # Example
///
/// ```text
/// draw_search_magnifier(canvas, Point::new(8.0, 18.0), 3.5, color);
/// ```
pub(super) fn draw_search_magnifier(
    canvas: &Canvas,
    center: Point,
    radius: f32,
    color: Color,
) -> Rect {
    let icon_paint = control_icon_stroke(color);
    let diagonal = radius * std::f32::consts::FRAC_1_SQRT_2;
    let handle_end = radius * 1.65;
    canvas.draw_circle(center, radius, &icon_paint);
    canvas.draw_line(
        (center.x + diagonal, center.y + diagonal),
        (center.x + handle_end, center.y + handle_end),
        &icon_paint,
    );
    icon_bounds(center, radius, handle_end)
}

/// Draws a font-independent directional chevron for disclosure controls.
///
/// # Example
///
/// ```text
/// draw_control_chevron(canvas, Point::new(12.0, 12.0), 4.0, Down, color);
/// ```
pub(super) fn draw_control_chevron(
    canvas: &Canvas,
    center: Point,
    arm_extent: f32,
    direction: ControlChevronDirection,
    color: Color,
) -> Rect {
    let (first, tip, last) = chevron_segments(center, arm_extent, direction);
    let icon_paint = control_icon_stroke(color);
    canvas.draw_line(first, tip, &icon_paint);
    canvas.draw_line(tip, last, &icon_paint);
    segment_bounds([first, tip, last])
}

fn icon_bounds(center: Point, radius: f32, handle_end: f32) -> Rect {
    let half_stroke = CONTROL_ICON_STROKE_WIDTH / 2.0;
    Rect::from_ltrb(
        center.x - radius - half_stroke,
        center.y - radius - half_stroke,
        center.x + handle_end + half_stroke,
        center.y + handle_end + half_stroke,
    )
}

fn segment_bounds(points: [(f32, f32); 3]) -> Rect {
    let half_stroke = CONTROL_ICON_STROKE_WIDTH / 2.0;
    let min_x = points.iter().map(|point| point.0).fold(f32::MAX, f32::min);
    let min_y = points.iter().map(|point| point.1).fold(f32::MAX, f32::min);
    let max_x = points.iter().map(|point| point.0).fold(f32::MIN, f32::max);
    let max_y = points.iter().map(|point| point.1).fold(f32::MIN, f32::max);
    Rect::from_ltrb(
        min_x - half_stroke,
        min_y - half_stroke,
        max_x + half_stroke,
        max_y + half_stroke,
    )
}

fn chevron_segments(
    center: Point,
    arm_extent: f32,
    direction: ControlChevronDirection,
) -> ((f32, f32), (f32, f32), (f32, f32)) {
    let depth = arm_extent * 0.65;
    let offsets = match direction {
        ControlChevronDirection::Up => [(-arm_extent, depth), (0.0, -depth), (arm_extent, depth)],
        ControlChevronDirection::Down => {
            [(-arm_extent, -depth), (0.0, depth), (arm_extent, -depth)]
        }
        ControlChevronDirection::Left => [(depth, -arm_extent), (-depth, 0.0), (depth, arm_extent)],
        ControlChevronDirection::Right => {
            [(-depth, -arm_extent), (depth, 0.0), (-depth, arm_extent)]
        }
    };
    let [first, tip, last] = offsets.map(|(x, y)| (center.x + x, center.y + y));
    (first, tip, last)
}

fn control_icon_stroke(color: Color) -> Paint {
    let mut icon_paint = Paint::default();
    icon_paint.set_color(color);
    icon_paint.set_anti_alias(true);
    icon_paint.set_style(paint::Style::Stroke);
    icon_paint.set_stroke_cap(paint::Cap::Round);
    icon_paint.set_stroke_width(CONTROL_ICON_STROKE_WIDTH);
    icon_paint
}

#[cfg(test)]
#[path = "../../tests/unit/control_icons_unit_tests.rs"]
mod tests;
