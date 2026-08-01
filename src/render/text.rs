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

/// Desenha uma linha de texto no canvas dentro da bounding box `size`.
///
/// - `center = true`  → centraliza horizontalmente e verticalmente
/// - `center = false` → alinha à esquerda, centralizado verticalmente
pub fn draw_text(
    canvas: &Canvas,
    text: &str,
    _pos: Point,
    size: (f32, f32),
    color: SkiaColor,
    font_size: f32,
    font_cache: &mut HashMap<(String, u32), Font>,
    center: bool,
) {
    draw_weighted_text(
        canvas, text, size, color, font_size, font_cache, center, false,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_strong_text(
    canvas: &Canvas,
    text: &str,
    size: (f32, f32),
    color: SkiaColor,
    font_size: f32,
    font_cache: &mut HashMap<(String, u32), Font>,
    center: bool,
) {
    draw_weighted_text(
        canvas, text, size, color, font_size, font_cache, center, true,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_weighted_text(
    canvas: &Canvas,
    text: &str,
    size: (f32, f32),
    color: SkiaColor,
    font_size: f32,
    font_cache: &mut HashMap<(String, u32), Font>,
    center: bool,
    embolden: bool,
) {
    if text.is_empty() {
        return;
    }

    let mut font = get_cached_font(font_cache, "sans-serif", font_size);
    font.set_embolden(embolden);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_anti_alias(true);

    // Baseline vertical: centro da caixa + ajuste ótico do cap-height
    let y = (size.1 / 2.0) + (font_size / 3.0);

    let x = if center {
        (size.0 - font.measure_str(text, Some(&paint)).0) / 2.0
    } else {
        0.0
    };
    canvas.draw_str(text, (x, y), &font, &paint);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use skia_safe::{Color, surfaces};

    use super::{draw_strong_text, get_cached_font};

    #[test]
    fn strong_text_emboldens_only_the_draw_font_clone() {
        let mut cache = HashMap::new();
        let mut surface = surfaces::raster_n32_premul((80, 30)).unwrap();
        draw_strong_text(
            surface.canvas(),
            "2026",
            (80.0, 30.0),
            Color::BLACK,
            16.0,
            &mut cache,
            true,
        );

        assert!(!get_cached_font(&mut cache, "sans-serif", 16.0).is_embolden());
    }
}
