// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::font_style::{Slant, Weight, Width};
use skia_safe::textlayout::{
    FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextDecoration, TextDirection,
    TextStyle,
};
use skia_safe::{Canvas, Color, FontMgr, FontStyle};

use crate::widgets::rich_text::{
    OwnedRichTextSpec, RichTextColor, RichTextSlant, RichTextSpanStyle, RichTextStyle,
};

const UNCONSTRAINED_PARAGRAPH_WIDTH: f32 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RichTextDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RichTextWidth {
    Definite(f32),
    MinContent,
    MaxContent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct RichTextMetrics {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedRichTextStyle {
    size: f32,
    color: Color,
    weight: u16,
    slant: RichTextSlant,
    underline: bool,
}

/// Retains the font resources used to measure and paint rich text consistently.
pub struct RichTextRenderer {
    font_collection: FontCollection,
}

impl Default for RichTextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl RichTextRenderer {
    pub(crate) fn new() -> Self {
        let mut font_collection = FontCollection::new();
        font_collection.set_default_font_manager(FontMgr::new(), Some("sans-serif"));
        font_collection.paragraph_cache_mut().turn_on(false);
        font_collection.paragraph_cache_mut().reset();
        Self { font_collection }
    }

    pub(crate) fn clear(&mut self) {
        self.font_collection.paragraph_cache_mut().reset();
        self.font_collection.clear_caches();
    }

    pub(crate) fn measure(
        &self,
        content: &OwnedRichTextSpec,
        width: RichTextWidth,
        direction: RichTextDirection,
    ) -> RichTextMetrics {
        if owned_spec_is_empty(content) {
            return RichTextMetrics::default();
        }
        let mut paragraph = self.build_paragraph(content, Color::BLACK, direction);
        let layout_width = initial_layout_width(width);
        paragraph.layout(layout_width);
        finish_paragraph_measurement(paragraph, width)
    }

    pub(crate) fn draw(
        &self,
        canvas: &Canvas,
        content: &OwnedRichTextSpec,
        bounds: (f32, f32),
        fallback_color: Color,
        direction: RichTextDirection,
    ) {
        if !valid_paint_bounds(bounds) || owned_spec_is_empty(content) {
            return;
        }
        let mut paragraph = self.build_paragraph(content, fallback_color, direction);
        paragraph.layout(bounds.0);
        let vertical_offset = ((bounds.1 - paragraph.height()) / 2.0).max(0.0);
        paragraph.paint(canvas, (0.0, vertical_offset));
    }

    fn build_paragraph(
        &self,
        content: &OwnedRichTextSpec,
        fallback_color: Color,
        direction: RichTextDirection,
    ) -> Paragraph {
        let defaults = resolve_rich_text_style(content.default_style(), None, fallback_color);
        let paragraph_style = paragraph_style(defaults, direction);
        let mut builder = ParagraphBuilder::new(&paragraph_style, self.font_collection.clone());
        for span in content.spans() {
            append_rich_text_span(
                &mut builder,
                span.text(),
                defaults,
                fallback_color,
                span.style(),
            );
        }
        builder.build()
    }
}

fn append_rich_text_span(
    builder: &mut ParagraphBuilder,
    text: &str,
    defaults: ResolvedRichTextStyle,
    fallback_color: Color,
    overrides: &RichTextSpanStyle,
) {
    if text.is_empty() {
        return;
    }
    let resolved = resolve_span_style(defaults, fallback_color, overrides);
    let style = skia_text_style(resolved);
    builder.push_style(&style);
    builder.add_text(text);
    builder.pop();
}

fn paragraph_style(
    defaults: ResolvedRichTextStyle,
    direction: RichTextDirection,
) -> ParagraphStyle {
    let mut style = ParagraphStyle::new();
    style.set_text_style(&skia_text_style(defaults));
    style.set_text_direction(match direction {
        RichTextDirection::LeftToRight => TextDirection::LTR,
        RichTextDirection::RightToLeft => TextDirection::RTL,
    });
    style
}

fn skia_text_style(resolved: ResolvedRichTextStyle) -> TextStyle {
    let mut style = TextStyle::new();
    style.set_color(resolved.color);
    style.set_font_size(resolved.size);
    style.set_font_families(&["sans-serif"]);
    style.set_font_style(FontStyle::new(
        Weight::from(i32::from(resolved.weight)),
        Width::NORMAL,
        skia_slant(resolved.slant),
    ));
    if resolved.underline {
        style.set_decoration_type(TextDecoration::UNDERLINE);
        style.set_decoration_color(resolved.color);
    }
    style
}

fn resolve_rich_text_style(
    defaults: &RichTextStyle,
    overrides: Option<&RichTextSpanStyle>,
    fallback_color: Color,
) -> ResolvedRichTextStyle {
    let overrides = overrides.copied().unwrap_or_default();
    ResolvedRichTextStyle {
        size: overrides.size().unwrap_or(defaults.size()).get(),
        color: match overrides.color_override() {
            Some(Some(color)) => skia_color(color),
            Some(None) => fallback_color,
            None => defaults.color().map(skia_color).unwrap_or(fallback_color),
        },
        weight: overrides.weight().unwrap_or(defaults.weight()).get(),
        slant: overrides.slant().unwrap_or(defaults.slant()),
        underline: overrides.underline().unwrap_or(defaults.underline()),
    }
}

fn resolve_span_style(
    defaults: ResolvedRichTextStyle,
    fallback_color: Color,
    overrides: &RichTextSpanStyle,
) -> ResolvedRichTextStyle {
    ResolvedRichTextStyle {
        size: overrides
            .size()
            .map(|size| size.get())
            .unwrap_or(defaults.size),
        color: match overrides.color_override() {
            None => defaults.color,
            Some(Some(color)) => skia_color(color),
            Some(None) => fallback_color,
        },
        weight: overrides
            .weight()
            .map(|weight| weight.get())
            .unwrap_or(defaults.weight),
        slant: overrides.slant().unwrap_or(defaults.slant),
        underline: overrides.underline().unwrap_or(defaults.underline),
    }
}

fn finish_paragraph_measurement(mut paragraph: Paragraph, width: RichTextWidth) -> RichTextMetrics {
    let measured_width = match width {
        RichTextWidth::Definite(limit) => paragraph.longest_line().min(limit.max(0.0)),
        RichTextWidth::MinContent => paragraph.min_intrinsic_width(),
        RichTextWidth::MaxContent => paragraph.max_intrinsic_width(),
    }
    .max(0.0)
    .ceil();
    paragraph.layout(measured_width.max(1.0));
    RichTextMetrics {
        width: measured_width,
        height: paragraph.height().max(0.0).ceil(),
    }
}

fn initial_layout_width(width: RichTextWidth) -> f32 {
    match width {
        RichTextWidth::Definite(value) if value.is_finite() => value.max(0.0),
        RichTextWidth::Definite(_) => 0.0,
        RichTextWidth::MinContent | RichTextWidth::MaxContent => UNCONSTRAINED_PARAGRAPH_WIDTH,
    }
}

fn owned_spec_is_empty(content: &OwnedRichTextSpec) -> bool {
    content.spans().iter().all(|span| span.text().is_empty())
}

fn valid_paint_bounds(bounds: (f32, f32)) -> bool {
    bounds.0.is_finite() && bounds.1.is_finite() && bounds.0 > 0.0 && bounds.1 > 0.0
}

fn skia_color(color: RichTextColor) -> Color {
    Color::from_argb(color.alpha(), color.red(), color.green(), color.blue())
}

fn skia_slant(slant: RichTextSlant) -> Slant {
    match slant {
        RichTextSlant::Upright => Slant::Upright,
        RichTextSlant::Italic => Slant::Italic,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/rich_text_renderer_unit_tests.rs"]
mod tests;
