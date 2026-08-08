use skia_safe::{Color, surfaces};

use super::*;
use crate::widgets::rich_text::{
    RichText, RichTextColor, RichTextSize, RichTextSpan, RichTextStyle, RichTextWeight,
};

fn owned_content() -> OwnedRichTextSpec {
    RichText::from_spans([
        RichTextSpan::new("regular "),
        RichTextSpan::new("bold").bold(),
        RichTextSpan::new(" color")
            .with_color(RichTextColor::rgb(200, 20, 40))
            .italic()
            .underline(),
    ])
    .to_owned_spec()
}

#[test]
fn renderer_measures_rich_content_and_wraps_at_smaller_width() {
    let renderer = RichTextRenderer::new();
    let content = owned_content();
    let wide = renderer.measure(
        &content,
        RichTextWidth::Definite(500.0),
        RichTextDirection::LeftToRight,
    );
    let narrow = renderer.measure(
        &content,
        RichTextWidth::Definite(60.0),
        RichTextDirection::LeftToRight,
    );

    assert!(wide.width > 0.0);
    assert!(wide.height > 0.0);
    assert!(narrow.width <= 60.0);
    assert!(narrow.height > wide.height);
}

#[test]
fn min_and_max_content_widths_are_ordered() {
    let renderer = RichTextRenderer::new();
    let content = RichText::plain("alpha beta gamma").to_owned_spec();
    let minimum = renderer.measure(
        &content,
        RichTextWidth::MinContent,
        RichTextDirection::LeftToRight,
    );
    let maximum = renderer.measure(
        &content,
        RichTextWidth::MaxContent,
        RichTextDirection::LeftToRight,
    );

    assert!(minimum.width > 0.0);
    assert!(maximum.width >= minimum.width);
}

#[test]
fn empty_rich_text_has_zero_metrics_and_no_pixels() {
    let renderer = RichTextRenderer::new();
    let content = RichText::plain("").to_owned_spec();
    let metrics = renderer.measure(
        &content,
        RichTextWidth::Definite(100.0),
        RichTextDirection::LeftToRight,
    );
    let mut surface = surfaces::raster_n32_premul((100, 30)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    renderer.draw(
        surface.canvas(),
        &content,
        (100.0, 30.0),
        Color::BLACK,
        RichTextDirection::LeftToRight,
    );

    assert_eq!(metrics, RichTextMetrics::default());
    let pixels = surface.peek_pixels().unwrap();
    let bytes = pixels.bytes().unwrap();
    assert!(bytes.chunks_exact(4).all(|pixel| pixel[3] == 0));
}

#[test]
fn invalid_bounds_do_not_reach_paragraph_layout_or_paint() {
    let renderer = RichTextRenderer::new();
    let content = owned_content();
    let mut surface = surfaces::raster_n32_premul((100, 30)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    renderer.draw(
        surface.canvas(),
        &content,
        (f32::NAN, 30.0),
        Color::BLACK,
        RichTextDirection::LeftToRight,
    );

    let pixels = surface.peek_pixels().unwrap();
    assert!(
        pixels
            .bytes()
            .unwrap()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 0)
    );
}

#[test]
fn clearing_renderer_caches_preserves_future_measurement() {
    let mut renderer = RichTextRenderer::new();
    let content = owned_content();
    renderer.clear();

    let measured = renderer.measure(
        &content,
        RichTextWidth::Definite(200.0),
        RichTextDirection::LeftToRight,
    );
    assert!(measured.width > 0.0);
    assert!(measured.height > 0.0);
}

#[test]
fn resolved_span_style_overrides_defaults() {
    let defaults = RichTextStyle::default()
        .with_size(RichTextSize::new(18.0).unwrap())
        .with_weight(RichTextWeight::MEDIUM)
        .with_color(RichTextColor::rgb(1, 2, 3));
    let overrides = RichTextSpan::new("styled")
        .bold()
        .with_color(RichTextColor::rgb(9, 8, 7))
        .style()
        .to_owned();
    let resolved = resolve_rich_text_style(&defaults, Some(&overrides), Color::WHITE);

    assert_eq!(resolved.size, 18.0);
    assert_eq!(resolved.weight, RichTextWeight::BOLD.get());
    assert_eq!(resolved.color, Color::from_rgb(9, 8, 7));
}

#[test]
fn theme_color_reset_ignores_an_explicit_inherited_color() {
    let defaults = RichTextStyle::default().with_color(RichTextColor::rgb(1, 2, 3));
    let overrides = RichTextSpan::new("theme").with_theme_color();
    let resolved = resolve_rich_text_style(&defaults, Some(overrides.style()), Color::WHITE);

    assert_eq!(resolved.color, Color::WHITE);
}

#[test]
fn rich_text_draw_changes_raster_pixels() {
    let renderer = RichTextRenderer::new();
    let content = owned_content();
    let mut surface = surfaces::raster_n32_premul((220, 60)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    renderer.draw(
        surface.canvas(),
        &content,
        (220.0, 60.0),
        Color::BLACK,
        RichTextDirection::RightToLeft,
    );

    let pixels = surface.peek_pixels().unwrap();
    let bytes = pixels.bytes().unwrap();
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] != 0));
}
