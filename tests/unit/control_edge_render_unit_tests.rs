use super::*;
use skia_safe::{Surface, surfaces};

#[derive(Clone, Copy, Debug)]
enum Control {
    Checkbox,
    Radio,
    Switch,
    Select,
}

struct EdgeCase<'a> {
    control: Control,
    active: bool,
    focused: bool,
    label: &'a str,
    width: f32,
    scale: f32,
    theme: &'a Theme,
}

fn paint_control_edge(case: &EdgeCase<'_>, clipped: bool) -> Surface {
    let mut surface = surfaces::raster_n32_premul((
        ((case.width + 32.0) * case.scale) as i32,
        (80.0 * case.scale) as i32,
    ))
    .unwrap();
    let canvas = surface.canvas();
    canvas.clear(case.theme.surface);
    canvas.scale((case.scale, case.scale));
    canvas.translate((16.0, 24.0));
    let height = if matches!(case.control, Control::Select) {
        44.0
    } else {
        28.0
    };
    if clipped {
        // External compact-control focus belongs to the demo section's 8px
        // safe area. Normal paint and Select focus belong to the allocation.
        let safe_area = if case.focused && !matches!(case.control, Control::Select) {
            crate::spacing::SM
        } else {
            0.0
        };
        canvas.clip_rect(
            outset_rect(SkiaRect::from_xywh(0.0, 0.0, case.width, height), safe_area),
            None,
            false,
        );
    }
    let mut fonts = HashMap::new();
    let input = LabeledControlRenderInput {
        canvas,
        label: case.label,
        is_focused: case.focused,
        size: (case.width, height),
        mouse: Point::new(-100.0, -100.0),
        font_cache: &mut fonts,
        theme: case.theme,
    };
    match case.control {
        Control::Checkbox => draw_checkbox(case.active, input),
        Control::Radio => draw_radio(case.active, input),
        Control::Switch => draw_switch(
            canvas,
            case.active,
            case.focused,
            input.size,
            input.mouse,
            case.theme,
        ),
        Control::Select => {
            let select = SelectTriggerRenderInput {
                canvas,
                options: &["Uma opção localizada muito longa para o campo"],
                selected_index: 0,
                is_open: case.active,
                label: "",
                placeholder: "Escolha...",
                is_focused: case.focused,
                size: input.size,
                mouse: input.mouse,
                font_cache: &mut fonts,
                theme: case.theme,
                direction: LayoutDirection::Ltr,
            };
            if clipped {
                draw_select_trigger(select);
            } else {
                // Bypass the trigger's own clip so the reference cannot hide
                // the same truncated border/focus outline as the actual image.
                draw_select_trigger_body(select);
            }
        }
    }
    surface
}

#[test]
fn control_edges_match_unclipped_reference_with_local_focus_safe_area() {
    for theme in [Theme::light(), Theme::dark()] {
        for viewport_width in [320.0_f32, 469.0, 800.0] {
            for scale in [1.0_f32, 1.5, 2.0] {
                for control in [
                    Control::Checkbox,
                    Control::Radio,
                    Control::Switch,
                    Control::Select,
                ] {
                    let width = match control {
                        Control::Checkbox => 180.0,
                        Control::Radio => 160.0,
                        Control::Switch => 50.0,
                        Control::Select => 320.0_f32.min(viewport_width - 64.0),
                    };
                    for active in [false, true] {
                        for focused in [false, true] {
                            for label in [
                                "",
                                "Uma descrição localizada muito longa que deve permanecer no controle",
                            ] {
                                let case = EdgeCase {
                                    control,
                                    active,
                                    focused,
                                    label,
                                    width,
                                    scale,
                                    theme: &theme,
                                };
                                let mut reference = paint_control_edge(&case, false);
                                let mut clipped = paint_control_edge(&case, true);
                                let expected = reference.peek_pixels().unwrap();
                                let actual = clipped.peek_pixels().unwrap();
                                let mut painted = false;
                                for y in 0..actual.height() {
                                    for x in 0..actual.width() {
                                        let pixel = actual.get_color((x, y));
                                        painted |= pixel != theme.surface;
                                        let reference_pixel = expected.get_color((x, y));
                                        // Skia changes rounded-edge AA when a clip touches
                                        // the primitive. Allow 8/255 channel coverage error,
                                        // but never allow lost paint outside the viewport.
                                        let channel_delta = [
                                            pixel.r().abs_diff(reference_pixel.r()),
                                            pixel.g().abs_diff(reference_pixel.g()),
                                            pixel.b().abs_diff(reference_pixel.b()),
                                            pixel.a().abs_diff(reference_pixel.a()),
                                        ]
                                        .into_iter()
                                        .max()
                                        .unwrap();
                                        let safe_area =
                                            if focused && !matches!(control, Control::Select) {
                                                crate::spacing::SM
                                            } else {
                                                0.0
                                            };
                                        let clip = outset_rect(
                                            SkiaRect::from_xywh(
                                                16.0,
                                                24.0,
                                                width,
                                                if matches!(control, Control::Select) {
                                                    44.0
                                                } else {
                                                    28.0
                                                },
                                            ),
                                            safe_area,
                                        );
                                        let inside = clip.contains(Point::new(
                                            (x as f32 + 0.5) / scale,
                                            (y as f32 + 0.5) / scale,
                                        ));
                                        let tolerance = if inside { 8 } else { 0 };
                                        assert!(
                                            channel_delta <= tolerance,
                                            "{control:?}, active={active}, focused={focused}, scale={scale}, viewport={viewport_width}, label={label:?}, pixel=({x}, {y}), delta={channel_delta}, actual={pixel:?}, expected={reference_pixel:?}"
                                        );
                                    }
                                }
                                assert!(painted, "comparison must include visible control paint");
                            }
                        }
                    }
                }
            }
        }
    }
}
