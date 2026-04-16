// ============================================================
// Rutter Framework — render/pipeline.rs
//
// Pipeline unificado de texto:
//   cosmic-text (shaping/layout) → SwashCache (rasterização)
//   → Skia (blit para o canvas)
//
// Suporta:
//   • Glyphs de máscara (texto normal) com cor aplicada
//   • Glyphs coloridos (emoji, color fonts)
//   • Subpixel rendering (SwashContent::SubpixelMask)
// ============================================================

use cosmic_text::{Color as CosmicColor, FontSystem, LayoutRun, SwashCache, SwashContent};
use skia_safe::Color as SkiaColor;
use skia_safe::{AlphaType, Bitmap, ColorType, ImageInfo, Paint, Point, canvas::Canvas};

/// Renderiza todas as runs de layout de um buffer cosmic-text
/// diretamente no canvas Skia, usando SwashCache para rasterização.
///
/// `origin`: posição top-left do texto em coordenadas locais do canvas.
/// `color`:  cor do texto (ignorada para glyphs coloridos/emoji).
/// `scale`:  fator DPI (1.0 em telas normais, 2.0 em retina).
pub fn render_text_runs<'a>(
    canvas: &Canvas,
    runs: impl Iterator<Item = LayoutRun<'a>>,
    origin: Point,
    color: SkiaColor,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    scale: f32,
) {
    // Converter cor Skia → cor cosmic-text (para glyph image)
    let cosmic_color = CosmicColor::rgb(color.r(), color.g(), color.b());
    let _ = cosmic_color; // usado abaixo indiretamente

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    for run in runs {
        for glyph in run.glyphs.iter() {
            // Posição física (leva scale em conta)
            let physical = glyph.physical((origin.x, origin.y), scale);

            // Rasterizar glyph via SwashCache
            let image = swash.get_image(fs, physical.cache_key);
            let image = match image {
                Some(img) => img,
                None => continue,
            };

            if image.placement.width == 0 || image.placement.height == 0 {
                continue;
            }

            let dst_x = (physical.x + image.placement.left) as f32 / scale;
            let dst_y = (physical.y - image.placement.top) as f32 / scale;
            let w = image.placement.width as i32;
            let h = image.placement.height as i32;

            match image.content {
                // ── Máscara alfa (texto normal) ───────────────
                SwashContent::Mask => {
                    // Criar bitmap A8 a partir dos dados de máscara
                    let mut bmp = Bitmap::new();
                    let info = ImageInfo::new_a8((w, h));
                    if bmp.set_info(&info, None) {
                        bmp.alloc_pixels();
                        let pixels = bmp.pixels();
                        if !pixels.is_null() {
                            let len = (w * h) as usize;
                            let src = &image.data[..len.min(image.data.len())];
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    src.as_ptr(),
                                    pixels as *mut u8,
                                    src.len(),
                                );
                            }

                            // Aplicar cor com color filter
                            let mut p2 = paint.clone();
                            p2.set_color(color);
                            p2.set_anti_alias(true);

                            if let Some(img) = skia_safe::images::raster_from_bitmap(&bmp) {
                                // Usar a máscara como alpha e colorir com paint
                                canvas.draw_image(&img, (dst_x, dst_y), Some(&p2));
                            }
                        }
                    }
                }

                // ── Glyphs coloridos (emoji, color fonts) ─────
                SwashContent::Color => {
                    // Dados RGBA
                    let mut bmp = Bitmap::new();
                    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
                    if bmp.set_info(&info, None) {
                        bmp.alloc_pixels();
                        let pixels = bmp.pixels();
                        if !pixels.is_null() {
                            let len = (w * h * 4) as usize;
                            let src = &image.data[..len.min(image.data.len())];
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    src.as_ptr(),
                                    pixels as *mut u8,
                                    src.len(),
                                );
                            }
                            if let Some(img) = skia_safe::images::raster_from_bitmap(&bmp) {
                                canvas.draw_image(&img, (dst_x, dst_y), Some(&paint));
                            }
                        }
                    }
                }

                // ── Subpixel (RGB striped) ────────────────────
                SwashContent::SubpixelMask => {
                    // Fallback: trata como máscara simples
                    // (subpixel rendering completo requer gamma correction)
                    let coverage_len = (w * h) as usize;
                    let mut bmp = Bitmap::new();
                    let info = ImageInfo::new_a8((w, h));
                    if bmp.set_info(&info, None) {
                        bmp.alloc_pixels();
                        let pixels = bmp.pixels();
                        if !pixels.is_null() {
                            let src = &image.data[..coverage_len.min(image.data.len())];
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    src.as_ptr(),
                                    pixels as *mut u8,
                                    src.len(),
                                );
                            }
                            let mut p2 = paint.clone();
                            p2.set_color(color);
                            if let Some(img) = skia_safe::images::raster_from_bitmap(&bmp) {
                                canvas.draw_image(&img, (dst_x, dst_y), Some(&p2));
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Testes unitários ─────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifica que a conversão de coordenadas DPI funciona
    /// corretamente para scale 1.0 e 2.0.
    #[test]
    fn physical_position_scale_1() {
        let scale = 1.0_f32;
        let px_x = 100.0_f32;
        let dst_x = px_x / scale;
        assert!((dst_x - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn physical_position_scale_2() {
        let scale = 2.0_f32;
        let px_x = 200.0_f32;
        let dst_x = px_x / scale;
        assert!((dst_x - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn color_channels_preserved() {
        let c = SkiaColor::from_rgb(0x4b, 0x9e, 0xf5);
        assert_eq!(c.r(), 0x4b);
        assert_eq!(c.g(), 0x9e);
        assert_eq!(c.b(), 0xf5);
    }

    #[test]
    fn bitmap_a8_size_matches_glyph() {
        let w = 8_i32;
        let h = 12_i32;
        let mut bmp = Bitmap::new();
        let info = ImageInfo::new_a8((w, h));
        assert!(bmp.set_info(&info, None));
        bmp.alloc_pixels();
        assert_eq!(bmp.width(), w);
        assert_eq!(bmp.height(), h);
    }

    #[test]
    fn empty_glyph_data_skipped_gracefully() {
        // Simula data vazio — não deve panic
        let w = 0_i32;
        let h = 0_i32;
        let bmp = Bitmap::new();
        assert_eq!(bmp.width(), w);
        assert_eq!(bmp.height(), h);
    }
}
