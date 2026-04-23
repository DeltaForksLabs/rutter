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

use std::collections::{HashMap, VecDeque};

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Wrap};
use skia_safe::{Color as SkiaColor, Font, FontMgr, FontStyle, Paint, Point, canvas::Canvas};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextShapeKey {
    text: String,
    font_size_bits: u32,
    line_height_bits: u32,
    width_bits: Option<u32>,
    height_bits: Option<u32>,
    wrap: u8,
}

impl TextShapeKey {
    fn from_request(request: &TextShapeRequest<'_>) -> Self {
        Self {
            text: request.text.to_string(),
            font_size_bits: request.font_size.to_bits(),
            line_height_bits: request.line_height.to_bits(),
            width_bits: request.width.map(f32::to_bits),
            height_bits: request.height.map(f32::to_bits),
            wrap: wrap_key(request.wrap),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextShapeRequest<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub line_height: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub wrap: Wrap,
}

impl<'a> TextShapeRequest<'a> {
    pub fn new(text: &'a str, font_size: f32, line_height: f32) -> Self {
        Self {
            text,
            font_size,
            line_height,
            width: None,
            height: None,
            wrap: Wrap::WordOrGlyph,
        }
    }

    pub fn with_bounds(mut self, width: Option<f32>, height: Option<f32>) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = wrap;
        self
    }
}

#[derive(Debug)]
pub struct TextBufferCache {
    entries: HashMap<TextShapeKey, Buffer>,
    order: VecDeque<TextShapeKey>,
    capacity: usize,
}

impl Default for TextBufferCache {
    fn default() -> Self {
        Self::new(128)
    }
}

impl TextBufferCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn contains(&self, request: TextShapeRequest<'_>) -> bool {
        self.entries
            .contains_key(&TextShapeKey::from_request(&request))
    }

    pub fn get_or_shape<'a>(
        &'a mut self,
        fs: &mut FontSystem,
        request: TextShapeRequest<'_>,
    ) -> &'a Buffer {
        let key = TextShapeKey::from_request(&request);
        if self.entries.contains_key(&key) {
            self.touch(&key);
        } else {
            self.insert(fs, key.clone(), request);
        }
        self.entries
            .get(&key)
            .expect("text buffer cache entry missing after insert")
    }

    fn touch(&mut self, key: &TextShapeKey) {
        self.order.retain(|existing| existing != key);
        self.order.push_back(key.clone());
    }

    fn insert(&mut self, fs: &mut FontSystem, key: TextShapeKey, request: TextShapeRequest<'_>) {
        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        let buffer = shape_text_buffer(fs, request);
        self.entries.insert(key.clone(), buffer);
        self.order.push_back(key);
    }
}

fn shape_text_buffer(fs: &mut FontSystem, request: TextShapeRequest<'_>) -> Buffer {
    let mut buffer = Buffer::new(fs, Metrics::new(request.font_size, request.line_height));
    buffer.set_wrap(fs, request.wrap);
    buffer.set_size(fs, request.width, request.height);
    buffer.set_text(fs, request.text, &Attrs::new(), Shaping::Advanced, None);
    buffer
}

const fn wrap_key(wrap: Wrap) -> u8 {
    match wrap {
        Wrap::None => 0,
        Wrap::Glyph => 1,
        Wrap::Word => 2,
        Wrap::WordOrGlyph => 3,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_buffer_cache_reuses_same_request() {
        let mut cache = TextBufferCache::new(2);
        let mut fs = FontSystem::new();
        let request = TextShapeRequest::new("hello", 14.0, 18.0)
            .with_bounds(Some(160.0), None)
            .with_wrap(Wrap::WordOrGlyph);

        let first_ptr = cache.get_or_shape(&mut fs, request) as *const Buffer;
        let second_ptr = cache.get_or_shape(&mut fs, request) as *const Buffer;

        assert_eq!(first_ptr, second_ptr);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn text_buffer_cache_evicts_oldest_entry() {
        let mut cache = TextBufferCache::new(2);
        let mut fs = FontSystem::new();

        let alpha = TextShapeRequest::new("alpha", 14.0, 18.0);
        let beta = TextShapeRequest::new("beta", 14.0, 18.0);
        let gamma = TextShapeRequest::new("gamma", 14.0, 18.0);

        let _ = cache.get_or_shape(&mut fs, alpha);
        let _ = cache.get_or_shape(&mut fs, beta);
        let _ = cache.get_or_shape(&mut fs, gamma);
        assert!(!cache.contains(alpha));

        let _ = cache.get_or_shape(&mut fs, alpha);
        assert!(cache.contains(alpha));
        assert_eq!(cache.len(), 2);
    }
}
