use skia_safe::{Color, surfaces};

use super::{initial_window_attributes, prepare_top_level_canvas};
use crate::{SurfaceConfig, Theme};

#[test]
fn surface_config_controls_native_window_transparency() {
    assert!(!initial_window_attributes(SurfaceConfig::default()).transparent());
    assert!(initial_window_attributes(SurfaceConfig::transparent()).transparent());
}

#[test]
fn opaque_canvas_clear_tracks_light_and_dark_theme_surfaces() {
    let light = Theme::light();
    let dark = Theme::dark();

    assert_eq!(
        prepared_surface_pixel(SurfaceConfig::default(), &light),
        light.surface
    );
    assert_eq!(
        prepared_surface_pixel(SurfaceConfig::default(), &dark),
        dark.surface
    );
}

#[test]
fn transparent_canvas_clear_ignores_resolved_theme_surface() {
    assert_eq!(
        prepared_surface_pixel(SurfaceConfig::transparent(), &Theme::dark()),
        Color::TRANSPARENT
    );
}

fn prepared_surface_pixel(surface_config: SurfaceConfig, theme: &Theme) -> Color {
    let mut surface = surfaces::raster_n32_premul((2, 2)).unwrap();
    surface.canvas().clear(Color::RED);
    prepare_top_level_canvas(surface.canvas(), surface_config, theme, 1.0);
    surface.peek_pixels().unwrap().get_color((0, 0))
}
