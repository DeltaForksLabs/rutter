// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Font, canvas::Canvas};

use super::{TextDrawInput, draw_text_line};
use crate::text_controls::TextControlPolicy;
use crate::theme::Theme;
use crate::widgets::time::{ClockConfig, TimeZone, current_clock_text};

pub(crate) struct ClockRenderInput<'a> {
    pub canvas: &'a Canvas,
    pub time_zone: TimeZone,
    pub config: ClockConfig,
    pub size: (f32, f32),
    pub font_cache: &'a mut HashMap<(String, u32), Font>,
    pub theme: &'a Theme,
}

pub(crate) fn draw_clock(input: ClockRenderInput<'_>) {
    let text = current_clock_text(input.time_zone, input.config.format());
    let color = input.config.color().unwrap_or(input.theme.on_surface);
    draw_text_line(TextDrawInput {
        canvas: input.canvas,
        text: &text,
        size: input.size,
        color,
        font_size: input.config.font_size(),
        font_cache: input.font_cache,
        center: false,
        control_policy: TextControlPolicy::FlattenLineBreaks,
    });
}
