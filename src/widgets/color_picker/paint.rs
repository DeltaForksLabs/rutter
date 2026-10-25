// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::{
    Canvas, Color, Color4f, Paint, Point, Rect, TileMode, gradient_shader, paint::Style,
};

use super::{PickerColor, controls::ColorControl};
use crate::CustomPaintContext;

pub(super) fn paint_control<Msg>(control: &ColorControl<Msg>, context: CustomPaintContext<'_>) {
    let size = context.layout.measured_size;
    if !size.width.is_finite()
        || !size.height.is_finite()
        || size.width <= 0.0
        || size.height <= 0.0
    {
        return;
    }
    let bounds = Rect::from_wh(size.width, size.height);
    match *control {
        ColorControl::Plane {
            hue,
            saturation,
            value,
            ..
        } => {
            let pure_hue = PickerColor::try_hsv(hue, 1.0, 1.0, 1.0).expect("validated picker hue");
            gradient(
                context.canvas,
                bounds,
                &[Color::WHITE, skia_color(pure_hue)],
                false,
            );
            gradient(
                context.canvas,
                bounds,
                &[Color::TRANSPARENT, Color::BLACK],
                true,
            );
            marker(
                context.canvas,
                Point::new(saturation * size.width, (1.0 - value) * size.height),
            );
        }
        ColorControl::Hue { hue, .. } => {
            gradient(
                context.canvas,
                bounds,
                &[
                    Color::RED,
                    Color::YELLOW,
                    Color::GREEN,
                    Color::CYAN,
                    Color::BLUE,
                    Color::MAGENTA,
                    Color::RED,
                ],
                false,
            );
            strip_marker(context.canvas, bounds, hue / 360.0);
        }
        ColorControl::Alpha { color, .. } => {
            checkerboard(context.canvas, bounds);
            let [r, g, b, _] = color.to_rgba8();
            gradient(
                context.canvas,
                bounds,
                &[Color::from_argb(0, r, g, b), Color::from_rgb(r, g, b)],
                false,
            );
            strip_marker(context.canvas, bounds, color.rgba()[3]);
        }
        ColorControl::Preview { color } => {
            // Check the model alpha, not its rounded byte, so faint colors still
            // use the uniform surface. Reserve the pattern for full transparency.
            if color.rgba()[3] == 0.0 {
                checkerboard(context.canvas, bounds);
            } else {
                context.canvas.draw_rect(
                    bounds,
                    &Paint::new(Color4f::from(context.theme.surface), None),
                );
            }
            context
                .canvas
                .draw_rect(bounds, &Paint::new(Color4f::from(skia_color(color)), None));
        }
    }
    let mut border = Paint::new(
        Color4f::from(if context.is_focused {
            context.theme.primary
        } else {
            crate::Theme::alpha(context.theme.on_surface, 100)
        }),
        None,
    );
    border
        .set_style(Style::Stroke)
        .set_stroke_width(if context.is_focused { 3.0 } else { 1.0 });
    context
        .canvas
        .draw_rect(bounds.with_inset((1.0, 1.0)), &border);
}

fn gradient(canvas: &Canvas, bounds: Rect, colors: &[Color], vertical: bool) {
    let end = if vertical {
        Point::new(0.0, bounds.height())
    } else {
        Point::new(bounds.width(), 0.0)
    };
    let shader = gradient_shader::linear(
        (Point::new(0.0, 0.0), end),
        colors,
        None,
        TileMode::Clamp,
        None,
        None,
    )
    .expect("finite nonzero gradient with at least two colors");
    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_rect(bounds, &paint);
}

fn checkerboard(canvas: &Canvas, bounds: Rect) {
    // Keep painting bounded even when a host supplies unusually large constraints.
    let columns = (bounds.width() / 8.0).ceil().clamp(1.0, 128.0) as u32;
    let rows = (bounds.height() / 8.0).ceil().clamp(1.0, 128.0) as u32;
    let (width, height) = (
        bounds.width() / columns as f32,
        bounds.height() / rows as f32,
    );
    for row in 0..rows {
        for column in 0..columns {
            let shade = if (row + column) % 2 == 0 { 230 } else { 170 };
            canvas.draw_rect(
                Rect::from_xywh(column as f32 * width, row as f32 * height, width, height),
                &Paint::new(Color4f::from(Color::from_rgb(shade, shade, shade)), None),
            );
        }
    }
}

fn marker(canvas: &Canvas, position: Point) {
    let mut paint = Paint::new(Color4f::from(Color::BLACK), None);
    paint
        .set_anti_alias(true)
        .set_style(Style::Stroke)
        .set_stroke_width(3.0);
    canvas.draw_circle(position, 6.0, &paint);
    paint.set_color(Color::WHITE).set_stroke_width(1.5);
    canvas.draw_circle(position, 6.0, &paint);
}

fn strip_marker(canvas: &Canvas, bounds: Rect, value: f32) {
    let x = (value * bounds.width()).clamp(
        2.0_f32.min(bounds.width() / 2.0),
        (bounds.width() - 2.0).max(bounds.width() / 2.0),
    );
    let mut paint = Paint::new(Color4f::from(Color::BLACK), None);
    paint.set_anti_alias(true).set_stroke_width(5.0);
    canvas.draw_line((x, 2.0), (x, bounds.height() - 2.0), &paint);
    paint.set_color(Color::WHITE).set_stroke_width(2.5);
    canvas.draw_line((x, 2.0), (x, bounds.height() - 2.0), &paint);
}

fn skia_color(color: PickerColor) -> Color {
    let [r, g, b, a] = color.to_rgba8();
    Color::from_argb(a, r, g, b)
}
