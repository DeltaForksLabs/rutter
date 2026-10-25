use super::{SkiaRect, TabbarRenderInput, Theme, draw_focus_outline, draw_tabbar};
use skia_safe::surfaces;
use std::collections::HashMap;

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

#[test]
fn selected_tab_uses_only_its_underline_even_when_focused() {
    for theme in [Theme::light(), Theme::dark()] {
        for width in [320.0_f32, 800.0] {
            for scale in [1.0_f32, 1.5, 2.0] {
                for active in [0, 1] {
                    let render = |focused_tab| {
                        let mut surface = surfaces::raster_n32_premul((
                            (width * scale) as i32,
                            (48.0 * scale) as i32,
                        ))
                        .unwrap();
                        let canvas = surface.canvas();
                        canvas.scale((scale, scale));
                        draw_tabbar(TabbarRenderInput {
                            canvas,
                            tabs: &["Overview", "Details"],
                            active,
                            anim_x: active as f32 * width / 2.0,
                            focused_tab,
                            size: (width, 48.0),
                            mouse: (-10.0, -10.0).into(),
                            font_cache: &mut HashMap::new(),
                            theme: &theme,
                        });
                        surface
                    };
                    let mut unfocused = render(None);
                    let mut selected = render(Some(active));
                    let mut other = render(Some(1 - active));
                    let expected = unfocused.peek_pixels().unwrap();
                    let actual = selected.peek_pixels().unwrap();
                    let other_pixels = other.peek_pixels().unwrap();
                    let mut other_focus_visible = false;
                    for y in 0..actual.height() {
                        for x in 0..actual.width() {
                            assert_eq!(
                                actual.get_color((x, y)),
                                expected.get_color((x, y)),
                                "selected-tab outline at ({x}, {y}), width={width}, scale={scale}"
                            );
                            other_focus_visible |=
                                other_pixels.get_color((x, y)) != expected.get_color((x, y));
                        }
                    }
                    assert!(
                        other_focus_visible,
                        "unselected keyboard focus must remain visible"
                    );
                    let underline_x = (active as f32 + 0.5) * width / 2.0;
                    assert_eq!(
                        actual.get_color(((underline_x * scale) as i32, (47.0 * scale) as i32)),
                        theme.primary,
                        "the selected-tab underline must remain intact"
                    );
                }
            }
        }
    }
}
