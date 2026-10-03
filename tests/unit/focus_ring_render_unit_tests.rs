use super::{SkiaRect, Theme, draw_focus_outline};
use skia_safe::surfaces;

#[test]
fn focus_ring_is_compact_and_visible_in_both_themes_at_supported_scales() {
    for theme in [Theme::light(), Theme::dark()] {
        for scale in [1.0_f32, 1.5, 2.0] {
            let extent = (64.0 * scale) as i32;
            let mut surface = surfaces::raster_n32_premul((extent, extent)).unwrap();
            let canvas = surface.canvas();
            canvas.clear(theme.surface);
            canvas.scale((scale, scale));
            draw_focus_outline(
                canvas,
                SkiaRect::from_xywh(16.0, 16.0, 32.0, 32.0),
                6.0,
                &theme,
            );

            let pixels = surface.peek_pixels().unwrap();
            let pixel = |x: f32, y: f32| pixels.get_color(((x * scale) as i32, (y * scale) as i32));
            // The previous outer stroke extended 3.5 logical pixels beyond the
            // component. Keep surrounding layout clear while retaining focus.
            assert_eq!(pixel(13.0, 32.0), theme.surface);
            assert_eq!(pixel(32.0, 13.0), theme.surface);
            assert_eq!(pixel(19.0, 32.0), theme.surface);
            assert_ne!(pixel(15.0, 32.0), theme.surface);
            assert_ne!(pixel(32.0, 15.0), theme.surface);
        }
    }
}
