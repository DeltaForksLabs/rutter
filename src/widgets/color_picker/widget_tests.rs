// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{controls::ColorControl, *};
use crate::{
    CustomAccessibility, CustomAccessibilityAction, CustomAccessibilityRole, CustomEventOutcome,
    CustomInteraction, CustomLayout, CustomPaintContext, CustomPoint, CustomPointerEvent,
    CustomSize, CustomWidgetState, CustomWidgetV1, LayoutDirection, ShortcutEvent, ShortcutKey,
    ShortcutNamedKey, Theme, Widget, WidgetIdSnapshot,
};
use skia_safe::{Color, surfaces};
use taffy::Style;

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Copy,
    Sv((f32, f32)),
    Hue(f32),
    Alpha(f32),
    Format(usize),
    Hex(String),
    Channel(String),
}

#[test]
fn copy_button_exposes_accessible_label_embedded_svg_and_native_message() {
    let state = ColorPickerState::new(PickerColor::from_rgb8(10, 20, 30));
    let picker = Widget::color_picker(
        &state,
        ColorPickerIds::new(100).unwrap(),
        callbacks().with_copy_button(Msg::Copy),
        Style::default(),
    );
    let Widget::Column { children, .. } = picker else {
        panic!("expected picker column")
    };
    let Widget::Row { children, .. } = &children[3] else {
        panic!("expected copy/editor row")
    };
    let Widget::ButtonContent {
        label,
        child,
        on_press,
        style,
        variant,
        ..
    } = &children[1]
    else {
        panic!("expected accessible copy button")
    };
    assert_eq!(*label, "Copy color as HEX");
    assert_eq!(*on_press, Msg::Copy);
    assert_eq!(*variant, crate::ButtonVariant::Ghost);
    assert_eq!(style.flex_shrink, 0.0);
    assert!(matches!(child.as_ref(), Widget::Image { data, .. } if data.starts_with(b"<svg")));
}

#[test]
fn embedded_copy_svg_parses_and_paints_visible_geometry() {
    let state = ColorPickerState::new(PickerColor::from_rgb8(10, 20, 30));
    let picker = Widget::color_picker(
        &state,
        ColorPickerIds::new(100).unwrap(),
        callbacks().with_copy_button(Msg::Copy),
        Style::default(),
    );
    let Widget::Column { children, .. } = picker else {
        panic!("expected column")
    };
    let Widget::Row { children, .. } = &children[3] else {
        panic!("expected editor row")
    };
    let Widget::ButtonContent { child, .. } = &children[1] else {
        panic!("expected copy button")
    };
    let Widget::Image { data, .. } = child.as_ref() else {
        panic!("expected SVG icon")
    };
    let dom = skia_safe::svg::Dom::from_bytes(data, skia_safe::FontMgr::empty()).unwrap();
    let mut surface = surfaces::raster_n32_premul((24, 24)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    dom.render(surface.canvas());
    let pixels = surface.peek_pixels().unwrap();
    let painted = (0..24)
        .flat_map(|y| (0..24).map(move |x| (x, y)))
        .filter(|&(x, y)| pixels.get_color((x, y)).a() > 0)
        .count();
    assert!(
        painted > 80,
        "copy outline must paint visible strokes, not just parse"
    );
}

#[test]
fn copy_button_preserves_identity_through_representations_and_external_colors() {
    let mut state = ColorPickerState::new(PickerColor::from_rgb8(10, 20, 30));
    let capture = |state: &ColorPickerState| {
        WidgetIdSnapshot::capture(&Widget::color_picker(
            state,
            ColorPickerIds::new(100).unwrap(),
            callbacks().with_copy_button(Msg::Copy),
            Style::default(),
        ))
        .unwrap()
    };
    let mut before = capture(&state);
    for format in ColorFormat::ALL {
        state
            .update(ColorPickerEvent::Format(format.index()))
            .unwrap();
        let after = capture(&state);
        before.validate_transition_to(&after).unwrap();
        before = after;
        state.set_color(PickerColor::from_rgba8(50, 150, 200, 128));
        let after = capture(&state);
        before.validate_reconstruction(&after).unwrap();
        before = after;
    }
}

fn callbacks() -> ColorPickerCallbacks<Msg> {
    ColorPickerCallbacks::new(
        Msg::Sv,
        Msg::Hue,
        Msg::Alpha,
        Msg::Format,
        Msg::Hex,
        [Msg::Channel; 5],
    )
}

fn plane() -> ColorControl<Msg> {
    ColorControl::Plane {
        hue: 0.0,
        saturation: 0.5,
        value: 0.5,
        on_change: Msg::Sv,
    }
}

fn key(key: ShortcutNamedKey) -> ShortcutEvent {
    ShortcutEvent {
        key: ShortcutKey::Named(key),
        control: false,
        alt: false,
        shift: false,
        super_key: false,
        repeat: false,
    }
}

fn message(outcome: CustomEventOutcome<Msg>) -> Msg {
    match outcome {
        CustomEventOutcome::Message(msg) | CustomEventOutcome::MessageAndCapture(msg) => msg,
        _ => panic!("expected an application message"),
    }
}

#[test]
fn plane_pointer_captures_clamps_drag_and_maps_vertical_value() {
    let control = plane();
    let mut runtime = CustomWidgetState::default();
    let bounds = CustomSize {
        width: 200.0,
        height: 100.0,
    };
    let press = control.pointer_event(
        CustomPointerEvent::Press {
            position: CustomPoint { x: 150.0, y: 25.0 },
            bounds,
        },
        &mut runtime,
    );
    assert!(matches!(
        press,
        CustomEventOutcome::MessageAndCapture(Msg::Sv((0.75, 0.75)))
    ));
    assert_eq!(
        message(control.pointer_event(
            CustomPointerEvent::Move {
                position: CustomPoint { x: -10.0, y: 150.0 },
                bounds,
            },
            &mut runtime
        )),
        Msg::Sv((0.0, 0.0))
    );
    assert_eq!(
        message(control.pointer_event(
            CustomPointerEvent::Release {
                position: CustomPoint { x: 250.0, y: -10.0 },
                bounds,
            },
            &mut runtime
        )),
        Msg::Sv((1.0, 1.0))
    );
}

#[test]
fn unchanged_pointer_press_still_captures_and_release_finishes_without_message() {
    let bounds = CustomSize {
        width: 100.0,
        height: 100.0,
    };
    let position = CustomPoint { x: 50.0, y: 50.0 };
    let mut runtime = CustomWidgetState::default();
    assert!(matches!(
        plane().pointer_event(CustomPointerEvent::Press { position, bounds }, &mut runtime),
        CustomEventOutcome::CapturePointer
    ));
    assert!(matches!(
        plane().pointer_event(
            CustomPointerEvent::Release { position, bounds },
            &mut runtime
        ),
        CustomEventOutcome::ReleasePointer
    ));
}

#[test]
fn pointer_rejects_nonfinite_positions_and_empty_bounds() {
    let mut runtime = CustomWidgetState::default();
    for bounds in [
        CustomSize {
            width: 0.0,
            height: 10.0,
        },
        CustomSize {
            width: f32::NAN,
            height: 10.0,
        },
        CustomSize {
            width: 10.0,
            height: -1.0,
        },
    ] {
        assert!(matches!(
            plane().pointer_event(
                CustomPointerEvent::Press {
                    position: CustomPoint { x: 0.0, y: 0.0 },
                    bounds,
                },
                &mut runtime
            ),
            CustomEventOutcome::Ignored
        ));
    }
    assert!(matches!(
        plane().pointer_event(
            CustomPointerEvent::Move {
                position: CustomPoint {
                    x: f32::INFINITY,
                    y: 0.0
                },
                bounds: CustomSize {
                    width: 10.0,
                    height: 10.0
                },
            },
            &mut runtime
        ),
        CustomEventOutcome::Ignored
    ));
}

#[test]
fn plane_keyboard_controls_each_axis_and_preserves_application_shortcuts() {
    let mut runtime = CustomWidgetState::default();
    assert_eq!(
        message(plane().keyboard_event(key(ShortcutNamedKey::ArrowRight), &mut runtime)),
        Msg::Sv((0.51, 0.5))
    );
    assert_eq!(
        message(plane().keyboard_event(key(ShortcutNamedKey::ArrowDown), &mut runtime)),
        Msg::Sv((0.5, 0.49))
    );
    let mut shifted = key(ShortcutNamedKey::ArrowUp);
    shifted.shift = true;
    assert_eq!(
        message(plane().keyboard_event(shifted, &mut runtime)),
        Msg::Sv((0.5, 0.6))
    );
    assert_eq!(
        message(plane().keyboard_event(key(ShortcutNamedKey::Home), &mut runtime)),
        Msg::Sv((0.0, 0.5))
    );
    assert_eq!(
        message(plane().keyboard_event(key(ShortcutNamedKey::End), &mut runtime)),
        Msg::Sv((1.0, 0.5))
    );
    let mut shortcut = key(ShortcutNamedKey::ArrowUp);
    shortcut.control = true;
    assert!(matches!(
        plane().keyboard_event(shortcut, &mut runtime),
        CustomEventOutcome::Ignored
    ));
    assert!(matches!(
        plane().keyboard_event(key(ShortcutNamedKey::Tab), &mut runtime),
        CustomEventOutcome::Ignored
    ));
}

#[test]
fn strips_support_pointer_keyboard_endpoints_and_accessibility_actions() {
    let hue = ColorControl::Hue {
        hue: 180.0,
        on_change: Msg::Hue,
    };
    let alpha = ColorControl::Alpha {
        color: PickerColor::try_rgba(1.0, 0.0, 0.0, 0.5).unwrap(),
        on_change: Msg::Alpha,
    };
    let mut runtime = CustomWidgetState::default();
    assert_eq!(
        message(hue.pointer_event(
            CustomPointerEvent::Press {
                position: CustomPoint { x: 75.0, y: 5.0 },
                bounds: CustomSize {
                    width: 100.0,
                    height: 10.0
                },
            },
            &mut runtime
        )),
        Msg::Hue(270.0)
    );
    assert_eq!(
        message(hue.keyboard_event(key(ShortcutNamedKey::Home), &mut runtime)),
        Msg::Hue(0.0)
    );
    assert_eq!(
        message(alpha.keyboard_event(key(ShortcutNamedKey::End), &mut runtime)),
        Msg::Alpha(1.0)
    );
    assert_eq!(
        message(alpha.keyboard_event(key(ShortcutNamedKey::PageDown), &mut runtime)),
        Msg::Alpha(0.4)
    );
    assert_eq!(
        message(alpha.accessibility_action(CustomAccessibilityAction::Increment, &mut runtime)),
        Msg::Alpha(0.51)
    );
    assert_eq!(
        message(plane().accessibility_action(CustomAccessibilityAction::Decrement, &mut runtime)),
        Msg::Sv((0.5, 0.49))
    );
    let endpoint = ColorControl::Alpha {
        color: PickerColor::from_rgb8(0, 0, 0),
        on_change: Msg::Alpha,
    };
    assert!(matches!(
        endpoint.keyboard_event(key(ShortcutNamedKey::End), &mut runtime),
        CustomEventOutcome::Consumed
    ));
}

#[test]
fn semantic_nodes_describe_values_and_preview_has_no_actions() {
    let CustomAccessibility::Node(node) = plane().accessibility() else {
        panic!("missing plane semantics")
    };
    assert_eq!(node.role, CustomAccessibilityRole::Slider);
    assert!(node.value.unwrap().contains("50.0%"));
    assert!(node.actions.increment && node.actions.decrement);
    let preview: ColorControl<Msg> = ColorControl::Preview {
        color: PickerColor::from_rgb8(255, 0, 0),
    };
    assert_eq!(preview.interaction(), CustomInteraction::VisualOnly);
    let CustomAccessibility::Node(node) = preview.accessibility() else {
        panic!("missing preview semantics")
    };
    assert_eq!(node.value.as_deref(), Some("#FF0000FF"));
    assert_eq!(node.role, CustomAccessibilityRole::Image);
    assert!(!node.actions.click && !node.actions.increment && !node.actions.decrement);
    assert!(matches!(
        preview.accessibility_action(
            CustomAccessibilityAction::Increment,
            &mut CustomWidgetState::default()
        ),
        CustomEventOutcome::Ignored
    ));
}

#[test]
fn picker_ids_are_stable_across_edits_and_separate_between_instances() {
    let mut state = ColorPickerState::new(PickerColor::from_rgb8(10, 20, 30));
    let ids = ColorPickerIds::new(100).unwrap();
    let before = WidgetIdSnapshot::capture(&Widget::color_picker(
        &state,
        ids,
        callbacks(),
        Style::default(),
    ))
    .unwrap();
    state
        .update(ColorPickerEvent::SetColor(PickerColor::from_rgb8(
            200, 150, 100,
        )))
        .unwrap();
    let after = WidgetIdSnapshot::capture(&Widget::color_picker(
        &state,
        ids,
        callbacks(),
        Style::default(),
    ))
    .unwrap();
    before.validate_reconstruction(&after).unwrap();
    let pair = Widget::Row {
        children: vec![
            Widget::color_picker(&state, ids, callbacks(), Style::default()),
            Widget::color_picker(
                &state,
                ColorPickerIds::new(200).unwrap(),
                callbacks(),
                Style::default(),
            ),
        ],
        style: Style::default(),
    };
    WidgetIdSnapshot::capture(&pair).unwrap();
    assert!(ColorPickerIds::new((1 << 63) - 10).is_err());
    assert!(ids.channel(5).is_none());
}

#[test]
fn composed_editors_follow_format_drafts_errors_and_model_replacements() {
    let mut state = ColorPickerState::new(PickerColor::from_rgb8(10, 20, 30));
    let ids = ColorPickerIds::new(100).unwrap();
    assert!(
        state
            .update(ColorPickerEvent::Channel {
                index: 0,
                text: "-".into()
            })
            .is_err()
    );
    {
        let picker = Widget::color_picker(&state, ids, callbacks(), Style::default());
        let Widget::Column { children, .. } = picker else {
            panic!("expected composed column")
        };
        let Widget::Row { children, .. } = &children[3] else {
            panic!("expected numeric editors")
        };
        assert_eq!(children.len(), 4);
        let Widget::TextInput {
            id,
            model_value,
            error_msg,
            ..
        } = &children[0]
        else {
            panic!("expected controlled editor")
        };
        assert_eq!(*id, ids.channel(0).unwrap().get());
        assert_eq!(*model_value, Some("-"));
        assert!(error_msg.is_some());
    }
    state
        .update(ColorPickerEvent::Format(ColorFormat::Hex.index()))
        .unwrap();
    let picker = Widget::color_picker(&state, ids, callbacks(), Style::default());
    let Widget::Column { children, .. } = picker else {
        panic!("expected column")
    };
    let Widget::TextInput {
        id, model_value, ..
    } = &children[3]
    else {
        panic!("expected HEX editor")
    };
    assert_eq!(*id, ids.hex().get());
    assert_eq!(*model_value, Some("#0A141EFF"));
}

fn painted(control: &ColorControl<Msg>) -> skia_safe::Surface {
    painted_with_theme(control, &Theme::default())
}

fn painted_with_theme(control: &ColorControl<Msg>, theme: &Theme) -> skia_safe::Surface {
    let mut surface = surfaces::raster_n32_premul((100, 100)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    control.paint(CustomPaintContext {
        canvas: surface.canvas(),
        layout: CustomLayout {
            style: &Style::default(),
            measured_size: CustomSize {
                width: 100.0,
                height: 100.0,
            },
        },
        theme,
        scale_factor: 1.0,
        direction: LayoutDirection::Ltr,
        is_focused: false,
        is_hovered: false,
        runtime_state: &CustomWidgetState::default(),
    });
    surface
}

#[test]
fn raster_plane_has_saturation_and_value_gradients() {
    let mut surface = painted(&plane());
    let pixels = surface.peek_pixels().unwrap();
    let light = pixels.get_color((10, 10));
    let saturated = pixels.get_color((90, 10));
    let dark = pixels.get_color((90, 90));
    assert!(light.g() > saturated.g() + 100);
    assert!(saturated.r() > dark.r() + 100);
}

#[test]
fn raster_preview_shows_checkerboard_at_zero_alpha_and_solid_opaque_color() {
    for theme in [Theme::dark(), Theme::light()] {
        let mut transparent = painted_with_theme(
            &ColorControl::Preview {
                color: PickerColor::from_rgba8(255, 0, 0, 0),
            },
            &theme,
        );
        let pixels = transparent.peek_pixels().unwrap();
        let first = pixels.get_color((10, 10));
        let second = pixels.get_color((18, 10));
        assert_ne!(first, second);
        assert_eq!(first.r(), first.g());
        assert_eq!(first.g(), first.b());
        assert_eq!(first.a(), 255);
        let mut opaque = painted_with_theme(
            &ColorControl::Preview {
                color: PickerColor::from_rgb8(255, 0, 0),
            },
            &theme,
        );
        let pixels = opaque.peek_pixels().unwrap();
        assert_eq!(pixels.get_color((10, 10)), Color::RED);
        assert_eq!(pixels.get_color((18, 10)), Color::RED);
    }
}

#[test]
fn raster_preview_composites_partial_alpha_over_uniform_theme_surface() {
    for theme in [Theme::dark(), Theme::light()] {
        let mut surface = painted_with_theme(
            &ColorControl::Preview {
                color: PickerColor::from_rgba8(255, 0, 0, 128),
            },
            &theme,
        );
        let pixels = surface.peek_pixels().unwrap();
        let first = pixels.get_color((10, 10));
        assert_eq!(first, pixels.get_color((18, 10)));
        assert_eq!(first.a(), 255);
        let alpha = 128.0 / 255.0;
        for (actual, foreground, background) in [
            (first.r(), 255.0, theme.surface.r()),
            (first.g(), 0.0, theme.surface.g()),
            (first.b(), 0.0, theme.surface.b()),
        ] {
            let expected = (foreground * alpha + f32::from(background) * (1.0 - alpha)).round();
            assert!((f32::from(actual) - expected).abs() <= 1.0);
        }
    }
}

#[test]
fn raster_preview_keeps_uniform_surface_for_positive_alpha_that_rounds_to_zero() {
    let color = PickerColor::try_rgba(1.0, 0.0, 0.0, 1e-6).unwrap();
    assert!(color.rgba()[3] > 0.0);
    assert_eq!(color.to_rgba8()[3], 0);
    for theme in [Theme::dark(), Theme::light()] {
        let mut surface = painted_with_theme(&ColorControl::Preview { color }, &theme);
        let pixels = surface.peek_pixels().unwrap();
        assert_eq!(pixels.get_color((10, 10)), theme.surface);
        assert_eq!(pixels.get_color((18, 10)), theme.surface);
    }
}

#[test]
fn raster_alpha_strip_retains_checkerboard_under_its_gradient() {
    for theme in [Theme::dark(), Theme::light()] {
        let mut surface = painted_with_theme(
            &ColorControl::Alpha {
                color: PickerColor::from_rgba8(255, 0, 0, 128),
                on_change: Msg::Alpha,
            },
            &theme,
        );
        let pixels = surface.peek_pixels().unwrap();
        // Same gradient position, adjacent tiles, away from the alpha marker.
        assert_ne!(pixels.get_color((10, 10)), pixels.get_color((10, 18)));
    }
}
