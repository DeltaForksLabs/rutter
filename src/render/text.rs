// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — render/text.rs
// Utilitários de renderização de texto via Skia.
//
// NOTA ARQUITETURAL: atualmente usa canvas.draw_str() (Skia
// direto). A roadmap prevê migrar para o pipeline completo
// cosmic-text → SwashCache → blit, que suporta bidi e
// ligatures. Por ora o cosmic-text é usado apenas para
// editing/shaping nos TextInputs.
// ============================================================

use std::collections::HashMap;

use skia_safe::{Color as SkiaColor, Font, FontMgr, FontStyle, Paint, Point, canvas::Canvas};

pub use super::text_cache::{TextBufferCache, TextShapeCacheLimits, TextShapeRequest};
use crate::text_controls::{TextControlPolicy, normalize_text_controls};

// ── Cache de fontes Skia ─────────────────────────────────────

/// Retorna uma `Font` Skia cacheada, criando-a no primeiro uso.
///
/// # FIX #7 — Precisão fracionária no tamanho
/// A chave usa `f32::to_bits()` em vez de `size as u32`, evitando
/// que 11.5px e 11.9px colidam na mesma entrada do cache.
pub fn get_cached_font(cache: &mut HashMap<(String, u32), Font>, family: &str, size: f32) -> Font {
    // to_bits() preserva precisão bit-a-bit do f32
    let key = (family.to_string(), size.to_bits());

    cache
        .entry(key)
        .or_insert_with(|| {
            let tf = FontMgr::new()
                .match_family_style(family, FontStyle::normal())
                .unwrap_or_else(|| {
                    // Fallback: qualquer fonte disponível
                    FontMgr::new()
                        .match_family_style("", FontStyle::normal())
                        .expect("Nenhuma fonte disponível no sistema")
                });
            Font::new(tf, size)
        })
        .clone()
}

// ── Renderização de texto simples ────────────────────────────

/// Inputs used internally to draw one line of text inside a bounding box.
pub(crate) struct TextDrawInput<'a> {
    pub canvas: &'a Canvas,
    pub text: &'a str,
    pub size: (f32, f32),
    pub color: SkiaColor,
    pub font_size: f32,
    pub font_cache: &'a mut HashMap<(String, u32), Font>,
    pub center: bool,
    pub control_policy: TextControlPolicy,
}

/// Draws normalized text inside the supplied bounding box.
///
/// - `center = true`  → centraliza horizontalmente e verticalmente
/// - `center = false` → alinha à esquerda, centralizado verticalmente
///
/// # Examples
///
/// ```no_run
/// # use std::collections::HashMap;
/// # use rutter::skia_safe::{Color, Point, surfaces};
/// let mut surface = surfaces::raster_n32_premul((120, 48)).unwrap();
/// let mut fonts = HashMap::new();
/// rutter::render::text::draw_text(
///     surface.canvas(), "first\nsecond", Point::default(), (120.0, 48.0),
///     Color::BLACK, 14.0, &mut fonts, false,
/// );
/// ```
#[allow(
    clippy::too_many_arguments,
    reason = "The stable public renderer API retains its original positional signature."
)]
pub fn draw_text(
    canvas: &Canvas,
    text: &str,
    _position: Point,
    size: (f32, f32),
    color: SkiaColor,
    font_size: f32,
    font_cache: &mut HashMap<(String, u32), Font>,
    center: bool,
) {
    draw_text_line(TextDrawInput {
        canvas,
        text,
        size,
        color,
        font_size,
        font_cache,
        center,
        control_policy: TextControlPolicy::PreserveLineBreaks,
    });
}

pub(crate) fn draw_text_line(input: TextDrawInput<'_>) {
    let TextDrawInput {
        canvas,
        text,
        size,
        color,
        font_size,
        font_cache,
        center,
        control_policy,
    } = input;
    let normalized = normalize_text_controls(text, control_policy);
    if normalized.is_empty() {
        return;
    }

    let font = get_cached_font(font_cache, "sans-serif", font_size);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_anti_alias(true);

    draw_normalized_text(
        canvas,
        normalized.as_ref(),
        size,
        font_size,
        &font,
        &paint,
        center,
    );
}

pub(crate) fn draw_single_line_text(
    canvas: &Canvas,
    text: &str,
    origin: (f32, f32),
    font: &Font,
    paint: &Paint,
) {
    let normalized = normalize_text_controls(text, TextControlPolicy::FlattenLineBreaks);
    if normalized.is_empty() {
        return;
    }
    canvas.draw_str(normalized.as_ref(), origin, font, paint);
}

pub(crate) fn measure_single_line_text(font: &Font, text: &str, paint: &Paint) -> f32 {
    let normalized = normalize_text_controls(text, TextControlPolicy::FlattenLineBreaks);
    font.measure_str(normalized.as_ref(), Some(paint)).0
}

fn draw_normalized_text(
    canvas: &Canvas,
    text: &str,
    size: (f32, f32),
    font_size: f32,
    font: &Font,
    paint: &Paint,
    center: bool,
) {
    let line_count = text.split('\n').count();
    let baseline = first_text_baseline(size.1, font_size, line_count);
    for (line_index, line) in text.split('\n').enumerate() {
        let x = horizontal_text_position(size.0, line, font, paint, center);
        let y = baseline + line_index as f32 * font_size * 1.2;
        canvas.draw_str(line, (x, y), font, paint);
    }
}

fn first_text_baseline(box_height: f32, font_size: f32, line_count: usize) -> f32 {
    let additional_lines = line_count.saturating_sub(1) as f32;
    box_height / 2.0 + font_size / 3.0 - additional_lines * font_size * 0.6
}

fn horizontal_text_position(
    box_width: f32,
    text: &str,
    font: &Font,
    paint: &Paint,
    center: bool,
) -> f32 {
    if center {
        return (box_width - font.measure_str(text, Some(paint)).0) / 2.0;
    }
    0.0
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use skia_safe::{Color, Paint, Point, Surface, surfaces};

    use super::{draw_single_line_text, draw_text, get_cached_font, measure_single_line_text};

    #[test]
    fn public_text_drawing_places_control_normalized_lines_on_separate_rows() {
        let mut surface = surfaces::raster_n32_premul((100, 60)).unwrap();
        let mut font_cache = HashMap::new();
        surface.canvas().clear(Color::TRANSPARENT);

        draw_text(
            surface.canvas(),
            "first\r\nsecond",
            Point::default(),
            (100.0, 60.0),
            Color::BLACK,
            16.0,
            &mut font_cache,
            false,
        );

        assert!(has_ink_in_rows(&mut surface, 8, 28));
        assert!(has_ink_in_rows(&mut surface, 30, 52));
    }

    #[test]
    fn single_line_measurement_matches_flattened_control_text() {
        let mut font_cache = HashMap::new();
        let font = get_cached_font(&mut font_cache, "sans-serif", 14.0);
        let paint = Paint::default();

        assert_eq!(
            measure_single_line_text(&font, "alpha\r\nbeta\tgamma\u{0000}", &paint),
            font.measure_str("alpha beta gamma", Some(&paint)).0
        );
    }

    #[test]
    fn single_line_drawing_matches_flattened_control_text() {
        let mut raw_surface = surfaces::raster_n32_premul((100, 40)).unwrap();
        let mut expected_surface = surfaces::raster_n32_premul((100, 40)).unwrap();
        let mut font_cache = HashMap::new();
        raw_surface.canvas().clear(Color::TRANSPARENT);
        expected_surface.canvas().clear(Color::TRANSPARENT);
        let font = get_cached_font(&mut font_cache, "sans-serif", 16.0);
        let paint = Paint::default();

        draw_single_line_text(
            raw_surface.canvas(),
            "first\nsecond",
            (0.0, 24.0),
            &font,
            &paint,
        );
        draw_single_line_text(
            expected_surface.canvas(),
            "first second",
            (0.0, 24.0),
            &font,
            &paint,
        );

        assert_eq!(
            surface_bytes(&mut raw_surface),
            surface_bytes(&mut expected_surface)
        );
    }

    fn has_ink_in_rows(surface: &mut Surface, first_row: i32, last_row: i32) -> bool {
        (first_row..last_row).any(|y| {
            (0..100).any(|x| surface.peek_pixels().unwrap().get_color((x, y)) != Color::TRANSPARENT)
        })
    }

    fn surface_bytes(surface: &mut Surface) -> Vec<u8> {
        surface.peek_pixels().unwrap().bytes().unwrap().to_vec()
    }
}
