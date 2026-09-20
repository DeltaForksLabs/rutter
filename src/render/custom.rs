// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::{Canvas, Contains, PictureRecorder, Point, Rect as SkiaRect};
use taffy::prelude::Style;

use crate::i18n::LayoutDirection;
use crate::theme::Theme;
use crate::widget::{
    CustomLayout, CustomPaintContext, CustomSize, CustomWidgetState, CustomWidgetV1,
};

pub(crate) struct CustomRenderFrame<'a, Message> {
    pub(crate) widget: &'a dyn CustomWidgetV1<Message>,
    pub(crate) style: &'a Style,
    pub(crate) size: (f32, f32),
    pub(crate) mouse: Point,
    pub(crate) is_focused: bool,
    pub(crate) shows_interaction_effects: bool,
    pub(crate) runtime_state: &'a CustomWidgetState,
    pub(crate) theme: &'a Theme,
    pub(crate) scale_factor: f32,
    pub(crate) direction: LayoutDirection,
}

pub(crate) fn draw_custom_widget<Message>(canvas: &Canvas, frame: CustomRenderFrame<'_, Message>) {
    let bounds = custom_bounds(frame.size);
    let mut recorder = PictureRecorder::new();
    let recording_canvas = recorder.begin_recording(bounds, false);
    frame.widget.paint(CustomPaintContext {
        canvas: recording_canvas,
        layout: CustomLayout {
            style: frame.style,
            measured_size: CustomSize {
                width: bounds.width(),
                height: bounds.height(),
            },
        },
        theme: frame.theme,
        scale_factor: frame.scale_factor,
        direction: frame.direction,
        is_focused: frame.is_focused,
        is_hovered: frame.shows_interaction_effects && bounds.contains(frame.mouse),
        runtime_state: frame.runtime_state,
    });
    replay_confined_custom_picture(canvas, &mut recorder, bounds);
}

fn custom_bounds(size: (f32, f32)) -> SkiaRect {
    SkiaRect::from_xywh(0.0, 0.0, size.0.max(0.0), size.1.max(0.0))
}

fn replay_confined_custom_picture(
    canvas: &Canvas,
    recorder: &mut PictureRecorder,
    bounds: SkiaRect,
) {
    let Some(picture) = recorder.finish_recording_as_picture(None) else {
        return;
    };
    canvas.save();
    canvas.clip_rect(bounds, None, true);
    canvas.draw_picture(&picture, None, None);
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use skia_safe::{Color, Paint, Rect as SkiaRect, surfaces};
    use taffy::prelude::Style;

    use super::{CustomRenderFrame, draw_custom_widget};
    use crate::i18n::LayoutDirection;
    use crate::widget::{CustomPaintContext, CustomWidgetState, CustomWidgetV1};

    struct OversizedPainter;

    impl CustomWidgetV1<()> for OversizedPainter {
        fn paint(&self, context: CustomPaintContext<'_>) {
            let mut paint = Paint::default();
            paint.set_color(Color::RED);
            context
                .canvas
                .draw_rect(SkiaRect::from_xywh(0.0, 0.0, 30.0, 30.0), &paint);
        }
    }

    #[test]
    fn custom_paint_is_clipped_to_its_measured_bounds() {
        let mut surface = surfaces::raster_n32_premul((20, 20)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);

        draw_custom_widget(
            surface.canvas(),
            CustomRenderFrame {
                widget: &OversizedPainter,
                style: &Style::default(),
                size: (10.0, 10.0),
                mouse: skia_safe::Point::new(1.0, 1.0),
                is_focused: false,
                shows_interaction_effects: true,
                runtime_state: &CustomWidgetState::default(),
                theme: &crate::Theme::default(),
                scale_factor: 1.0,
                direction: LayoutDirection::Ltr,
            },
        );

        let pixels = surface.peek_pixels().unwrap();
        assert_eq!(pixels.get_color((9, 9)), Color::RED);
        assert_eq!(pixels.get_color((15, 15)), Color::TRANSPARENT);
    }
}
