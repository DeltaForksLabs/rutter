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
    if text.is_empty() {
        return;
    }

    let font = get_cached_font(font_cache, "sans-serif", font_size);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_anti_alias(true);

    // Baseline vertical: centro da caixa + ajuste ótico do cap-height
    let y = (size.1 / 2.0) + (font_size / 3.0);

    if center {
        let text_width = font.measure_str(text, Some(&paint)).0;
        let x = (size.0 - text_width) / 2.0;
        canvas.draw_str(text, (x, y), &font, &paint);
    } else {
        canvas.draw_str(text, (0.0, y), &font, &paint);
    }
}
