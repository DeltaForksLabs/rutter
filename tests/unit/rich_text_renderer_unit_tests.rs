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
#[ignore = "manual raster CPU profiling probe; no display required or timing assertions"]
fn rich_paragraph_cpu_profile() {
    use std::{hint::black_box, time::Instant};

    let renderer = RichTextRenderer::new();
    let content = owned_content();
    let mut surface = surfaces::raster_n32_premul((800, 120)).unwrap();
    // Warm font discovery without warming any of the measured paint keys.
    renderer.measure(
        &content,
        RichTextWidth::MaxContent,
        RichTextDirection::LeftToRight,
    );
    let builds_before = renderer.paragraph_builds.get();
    let mut samples = Vec::new();
    for _ in 0..32 {
        let start = Instant::now();
        for frame in 0..120 {
            let width = [288.0, 368.0, 736.0][frame % 3];
            let direction = if frame % 2 == 0 {
                RichTextDirection::LeftToRight
            } else {
                RichTextDirection::RightToLeft
            };
            let color = if frame % 4 < 2 {
                Color::BLACK
            } else {
                Color::WHITE
            };
            black_box(renderer.measure(
                black_box(&content),
                RichTextWidth::Definite(width),
                direction,
            ));
            surface.canvas().clear(Color::TRANSPARENT);
            renderer.draw(
                surface.canvas(),
                black_box(&content),
                (width, 120.0),
                color,
                direction,
            );
        }
        samples.push(start.elapsed().as_micros());
    }
    samples.sort_unstable();
    println!(
        "rich_paragraph_cpu_profile: samples=32 frames/sample=120 measure+draw=3840 builds={} min={}us median={}us p95={}us max={}us",
        renderer.paragraph_builds.get() - builds_before,
        samples[0],
        samples[16],
        samples[30],
        samples[31]
    );
}

#[test]
fn bounded_single_line_resolves_missing_primary_glyphs_and_shapes_both_scripts() {
    use crate::i18n::LayoutDirection;
    let mut renderer = RichTextRenderer::new();
    let manager = FontMgr::new();
    let primary = manager
        .family_names()
        .filter_map(|family| manager.match_family_style(&family, FontStyle::normal()))
        .find(|face| {
            face.unichar_to_glyph('A' as i32) != 0 && face.unichar_to_glyph('東' as i32) == 0
        })
        .expect("fallback regression requires an installed Latin font without CJK coverage");
    // Negative control: ink alone would also pass with .notdef rectangles.
    assert_eq!(primary.unichar_to_glyph('東' as i32), 0);
    let mut provider = skia_safe::textlayout::TypefaceFontProvider::new();
    provider.register_typeface(primary, "sans-serif");
    renderer
        .font_collection
        .set_asset_font_manager(FontMgr::from(provider));
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let mut paragraph =
            renderer.single_line_paragraph("東京 القاهرة", 16.0, Color::BLACK, direction);
        paragraph.layout(300.0);
        assert_eq!(
            paragraph.unresolved_glyphs(),
            Some(0),
            "install system CJK/Arabic fonts for this regression"
        );
        assert_eq!(paragraph.line_number(), 1);
        let arabic = paragraph.get_rects_for_range(
            3..10,
            skia_safe::textlayout::RectHeightStyle::Tight,
            skia_safe::textlayout::RectWidthStyle::Tight,
        );
        assert!(!arabic.is_empty());
        assert!(arabic.iter().all(|run| run.direct == TextDirection::RTL));
        let mut flattened =
            renderer.single_line_paragraph("東京\nالقاهرة\ttext", 16.0, Color::BLACK, direction);
        flattened.layout(80.0);
        assert_eq!(flattened.line_number(), 1);
        assert!(flattened.did_exceed_max_lines());
        let mut surface = surfaces::raster_n32_premul((120, 60)).unwrap();
        surface.canvas().clear(Color::MAGENTA);
        renderer.draw_single_line(
            surface.canvas(),
            "東京\nالقاهرة long long label",
            16.0,
            skia_safe::Rect::from_xywh(10.0, 10.0, 80.0, 30.0),
            Color::BLACK,
            direction,
        );
        let pixels = surface.peek_pixels().unwrap();
        let mut ink = 0;
        for y in 0..60 {
            for x in 0..120 {
                if pixels.get_color((x, y)) != Color::MAGENTA {
                    assert!((10..90).contains(&x) && (10..40).contains(&y));
                    ink += 1;
                }
            }
        }
        assert!(ink > 0);
    }
}

#[test]
fn bounded_single_line_flattens_controls_and_aligns_to_explicit_inline_start() {
    use crate::i18n::LayoutDirection;
    let renderer = RichTextRenderer::new();
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let mut flattened =
            renderer.single_line_paragraph("東京\nالقاهرة\ttext", 16.0, Color::BLACK, direction);
        let mut plain =
            renderer.single_line_paragraph("東京 القاهرة text", 16.0, Color::BLACK, direction);
        flattened.layout(400.0);
        plain.layout(400.0);
        assert!((flattened.longest_line() - plain.longest_line()).abs() < 0.01);
        assert_eq!(flattened.line_number(), 1);
        assert!(!flattened.did_exceed_max_lines());
        let mut surface = surfaces::raster_n32_premul((200, 40)).unwrap();
        surface.canvas().clear(Color::MAGENTA);
        renderer.draw_single_line(
            surface.canvas(),
            "القاهرة",
            16.0,
            skia_safe::Rect::from_xywh(10.0, 5.0, 180.0, 30.0),
            Color::BLACK,
            direction,
        );
        let pixels = surface.peek_pixels().unwrap();
        let ink: Vec<_> = (5..35)
            .flat_map(|y| (10..190).map(move |x| (x, y)))
            .filter(|point| pixels.get_color(*point) != Color::MAGENTA)
            .collect();
        assert!(!ink.is_empty());
        assert!(
            ink.iter()
                .all(|(x, _)| if direction == LayoutDirection::Rtl {
                    *x > 100
                } else {
                    *x < 100
                }),
            "short shaped Arabic follows explicit inline-start alignment"
        );
    }
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

#[test]
fn cached_paint_matches_fresh_pixels_and_does_not_change_measurement() {
    let renderer = RichTextRenderer::new();
    let content = RichText::from_spans([
        RichTextSpan::new("東京 القاهرة alpha beta "),
        RichTextSpan::new("bold wrapped text").bold().underline(),
    ])
    .to_owned_spec();
    for direction in [
        RichTextDirection::LeftToRight,
        RichTextDirection::RightToLeft,
    ] {
        for width in [80.0, 288.0, 736.0] {
            let metrics = [
                RichTextWidth::Definite(width),
                RichTextWidth::MinContent,
                RichTextWidth::MaxContent,
            ]
            .map(|constraint| renderer.measure(&content, constraint, direction));
            for color in [Color::BLACK, Color::WHITE] {
                for scale in [1.0, 1.5, 2.0] {
                    for height in [60.0, 100.0] {
                        let mut cached = surfaces::raster_n32_premul((1600, 240)).unwrap();
                        let mut fresh = surfaces::raster_n32_premul((1600, 240)).unwrap();
                        for canvas in [cached.canvas(), fresh.canvas()] {
                            canvas.clear(Color::TRANSPARENT);
                            canvas.scale((scale, scale));
                            canvas.translate((3.0, 2.0));
                            canvas.clip_rect(
                                skia_safe::Rect::from_xywh(0.0, 0.0, width, height),
                                None,
                                true,
                            );
                        }
                        renderer.draw(cached.canvas(), &content, (width, height), color, direction);
                        let mut paragraph = renderer.build_paragraph(&content, color, direction);
                        paragraph.layout(width);
                        paragraph.paint(
                            fresh.canvas(),
                            (0.0, ((height - paragraph.height()) / 2.0).max(0.0)),
                        );
                        assert_eq!(
                            cached.peek_pixels().unwrap().bytes(),
                            fresh.peek_pixels().unwrap().bytes()
                        );
                    }
                }
            }
            for (constraint, expected) in [
                RichTextWidth::Definite(width),
                RichTextWidth::MinContent,
                RichTextWidth::MaxContent,
            ]
            .into_iter()
            .zip(metrics)
            {
                assert_eq!(renderer.measure(&content, constraint, direction), expected);
            }
        }
    }
}

fn paint_for_cache(
    renderer: &RichTextRenderer,
    content: &OwnedRichTextSpec,
    width: f32,
    color: Color,
    direction: RichTextDirection,
) {
    let mut surface = surfaces::raster_n32_premul((400, 100)).unwrap();
    renderer.draw(surface.canvas(), content, (width, 100.0), color, direction);
}

#[test]
fn paint_cache_keys_full_spec_exact_width_direction_and_fallback_color() {
    let renderer = RichTextRenderer::new();
    let content = owned_content();
    let draw = |content: &OwnedRichTextSpec, width, color, direction| {
        paint_for_cache(&renderer, content, width, color, direction)
    };
    let ltr = RichTextDirection::LeftToRight;
    draw(&content, 288.0, Color::BLACK, ltr);
    draw(&content.clone(), 288.0, Color::BLACK, ltr);
    assert_eq!(renderer.paragraph_builds.get(), 1);
    draw(
        &content,
        f32::from_bits(288.0_f32.to_bits() + 1),
        Color::BLACK,
        ltr,
    );
    draw(&content, 288.0, Color::WHITE, ltr);
    draw(
        &content,
        288.0,
        Color::BLACK,
        RichTextDirection::RightToLeft,
    );
    let changed = RichText::from_spans(
        content
            .spans()
            .iter()
            .map(|span| RichTextSpan::new(span.text()).with_style(*span.style())),
    )
    .with_default_style(
        content
            .default_style()
            .with_size(RichTextSize::new(20.0).unwrap()),
    )
    .to_owned_spec();
    draw(&changed, 288.0, Color::BLACK, ltr);
    let changed = RichText::from_spans(content.spans().iter().map(|span| {
        RichTextSpan::new(span.text())
            .with_style(span.style().with_size(RichTextSize::new(19.0).unwrap()))
    }))
    .to_owned_spec();
    draw(&changed, 288.0, Color::BLACK, ltr);
    draw(
        &RichText::plain("different text").to_owned_spec(),
        288.0,
        Color::BLACK,
        ltr,
    );
    assert_eq!(renderer.paragraph_builds.get(), 7);
    for width in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
        draw(&content, width, Color::BLACK, ltr);
    }
    assert_eq!(renderer.paragraph_builds.get(), 7);
}

#[test]
fn paint_cache_ignores_visual_offsets_and_accessibility_metadata() {
    let renderer = RichTextRenderer::new();
    let text = RichText::plain("Moving summary");
    let content = text.to_owned_spec();
    let moved = text
        .with_visual_offset(10.0, -5.0)
        .with_bounds_relative_visual_offset(0.0, -1.0)
        .with_accessibility_label("Current semantic summary")
        .to_owned_spec();
    for spec in [&content, &moved] {
        paint_for_cache(
            &renderer,
            spec,
            288.0,
            Color::BLACK,
            RichTextDirection::LeftToRight,
        );
    }
    assert_eq!(renderer.paragraph_builds.get(), 1);
}

#[test]
fn paint_cache_evicts_lru_bounds_bytes_bypasses_oversized_and_clears() {
    let mut renderer = RichTextRenderer::new();
    let content = owned_content();
    let direction = RichTextDirection::LeftToRight;
    for width in 1..=MAX_PAINT_CACHE_ENTRIES {
        paint_for_cache(&renderer, &content, width as f32, Color::BLACK, direction);
    }
    // Touch the oldest key so the next-oldest, rather than this key, is evicted.
    paint_for_cache(&renderer, &content, 1.0, Color::BLACK, direction);
    paint_for_cache(&renderer, &content, 100.0, Color::BLACK, direction);
    let builds = renderer.paragraph_builds.get();
    paint_for_cache(&renderer, &content, 1.0, Color::BLACK, direction);
    assert_eq!(renderer.paragraph_builds.get(), builds);
    paint_for_cache(&renderer, &content, 2.0, Color::BLACK, direction);
    assert_eq!(renderer.paragraph_builds.get(), builds + 1);
    renderer.clear();
    assert!(renderer.paint_cache.borrow().entries.is_empty());
    assert_eq!(renderer.paint_cache.borrow().bytes, 0);
    for index in 0..10 {
        let large = RichText::plain(format!("{index} {}", "a ".repeat(4000))).to_owned_spec();
        paint_for_cache(&renderer, &large, 300.0, Color::BLACK, direction);
        let cache = renderer.paint_cache.borrow();
        assert!(cache.bytes <= MAX_PAINT_CACHE_BYTES);
        assert!(cache.entries.len() < MAX_PAINT_CACHE_ENTRIES);
    }
    let retained = renderer.paint_cache.borrow().bytes;
    let oversized = RichText::plain("a".repeat(MAX_PAINT_ENTRY_BYTES)).to_owned_spec();
    let builds = renderer.paragraph_builds.get();
    for _ in 0..2 {
        paint_for_cache(&renderer, &oversized, 300.0, Color::BLACK, direction);
    }
    assert_eq!(renderer.paragraph_builds.get(), builds + 2);
    assert_eq!(renderer.paint_cache.borrow().bytes, retained);
    renderer.clear();
    let builds = renderer.paragraph_builds.get();
    paint_for_cache(&renderer, &content, 1.0, Color::BLACK, direction);
    assert_eq!(renderer.paragraph_builds.get(), builds + 1);
}
