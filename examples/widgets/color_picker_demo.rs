// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Run `cargo run -- color_picker` for inline and anchored controlled pickers.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{
    AppLogic, ButtonVariant, ColorFormat, ColorPickerCallbacks, ColorPickerEvent, ColorPickerIds,
    ColorPickerState, PickerColor, RutterRunner, Theme, Widget, WidgetId,
};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const INITIAL_COLOR: PickerColor = PickerColor::from_rgb8(103, 80, 164);
// Reserve complete ranges, including editors hidden by the current format.
const INLINE_FIRST_ID: u64 = 100;
const POPUP_FIRST_ID: u64 = 200;
const POPOVER_ID: u64 = 300;
const SUMMARY_SWATCH_ID: u64 = 301;
const SCROLL_ID: u64 = 302;

pub struct ColorPickerDemoState {
    pub theme: ExampleTheme,
    pub picker: ColorPickerState,
    pub popup_open: bool,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    Picker(ColorPickerEvent),
    Preset(PickerColor),
    Reset,
    CopyColor,
    TogglePopup,
    DismissPopup,
}

pub struct ColorPickerDemo;

fn initial_color_picker_demo_state() -> ColorPickerDemoState {
    ColorPickerDemoState {
        theme: ExampleTheme::Dark,
        picker: ColorPickerState::new(INITIAL_COLOR),
        popup_open: false,
        status: "Drag the plane or strips, edit channels, or choose a preset.".into(),
    }
}

/// Applies application messages without clipboard, window, or renderer side effects.
fn reduce_color_picker_demo(state: &mut ColorPickerDemoState, message: Msg) {
    let event = match message {
        Msg::Picker(event) => event,
        Msg::Preset(color) => ColorPickerEvent::SetColor(color),
        Msg::Reset => ColorPickerEvent::SetColor(INITIAL_COLOR),
        Msg::CopyColor => {
            state.status = "Color not copied: clipboard writer unavailable.".into();
            return;
        }
        Msg::ThemeChanged(theme) => {
            state.theme = theme;
            return;
        }
        Msg::TogglePopup => {
            state.popup_open = !state.popup_open;
            return;
        }
        Msg::DismissPopup => {
            state.popup_open = false;
            return;
        }
    };

    state.status = match state.picker.update(event) {
        Ok(true) => format!("Color updated: {}", state.picker.color().to_hex()),
        Ok(false) => format!("Picker synchronized: {}", state.picker.color().to_hex()),
        Err(error) => format!("Color edit rejected: {error}"),
    };
}

/// Keeps clipboard effects injectable while all other messages use the pure reducer.
fn update_color_picker_demo<E: std::fmt::Display>(
    state: &mut ColorPickerDemoState,
    message: Msg,
    write_text: impl FnOnce(String) -> Result<(), E>,
) {
    match message {
        Msg::CopyColor => {
            // Drafts can be invalid or use a different spelling than the canonical color.
            let hex = state.picker.color().to_hex();
            state.status = match write_text(hex.clone()) {
                Ok(()) => format!("Copied color as HEX: {hex}"),
                Err(error) => format!("Failed to copy color as HEX: {error}"),
            };
        }
        message => reduce_color_picker_demo(state, message),
    }
}

fn picker_callbacks() -> ColorPickerCallbacks<Msg> {
    ColorPickerCallbacks::new(
        |(saturation, value)| Msg::Picker(ColorPickerEvent::SaturationValue { saturation, value }),
        |hue| Msg::Picker(ColorPickerEvent::Hue(hue)),
        |alpha| Msg::Picker(ColorPickerEvent::Alpha(alpha)),
        |index| Msg::Picker(ColorPickerEvent::Format(index)),
        |text| Msg::Picker(ColorPickerEvent::Hex(text)),
        [
            |text| Msg::Picker(ColorPickerEvent::Channel { index: 0, text }),
            |text| Msg::Picker(ColorPickerEvent::Channel { index: 1, text }),
            |text| Msg::Picker(ColorPickerEvent::Channel { index: 2, text }),
            |text| Msg::Picker(ColorPickerEvent::Channel { index: 3, text }),
            |text| Msg::Picker(ColorPickerEvent::Channel { index: 4, text }),
        ],
    )
    .with_copy_button(Msg::CopyColor)
}

fn picker_view(state: &ColorPickerState, first_id: u64) -> Widget<'_, Msg> {
    Widget::color_picker(
        state,
        ColorPickerIds::new(first_id).expect("demo reserves a valid eleven-ID manual range"),
        picker_callbacks(),
        responsive_width(360.0, Dimension::auto()),
    )
}

fn demo_button<'a>(text: &'static str, message: Msg) -> Widget<'a, Msg> {
    Widget::Button {
        text,
        on_press: message,
        style: responsive_width(160.0, Dimension::length(40.0)),
        color: None,
        variant: ButtonVariant::Primary,
    }
}

fn demo_text<'a>(content: impl Into<String>, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        color: None,
        size,
        style: Style {
            flex_shrink: 0.0,
            // Wrapped rows still measure an auto-width text item intrinsically.
            // Cap that item to its host so a long summary wraps within the page.
            max_size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            ..Style::default()
        },
    }
}

fn popup_view(state: &ColorPickerDemoState) -> Widget<'_, Msg> {
    let content = Widget::Column {
        style: Style {
            padding: Rect::length(spacing::MD),
            ..responsive_width(384.0, Dimension::auto())
        },
        children: vec![picker_view(&state.picker, POPUP_FIRST_ID)],
    };
    Widget::popover(
        state.popup_open,
        demo_button(
            if state.popup_open {
                "Close popup"
            } else {
                "Open popup"
            },
            Msg::TogglePopup,
        ),
        content,
        Some(Msg::DismissPopup),
        responsive_width(384.0, Dimension::length(40.0)),
        responsive_width(384.0, Dimension::auto()),
    )
    .with_id(POPOVER_ID)
}

fn demo_content(state: &ColorPickerDemoState) -> Widget<'_, Msg> {
    Widget::Column {
        style: Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            gap: Size::length(spacing::LG),
            flex_shrink: 0.0,
            ..responsive_width(760.0, Dimension::auto())
        },
        children: vec![
            Widget::heading(rutter::HeadingLevel::H2, "Inline picker", Style::default()),
            picker_view(&state.picker, INLINE_FIRST_ID),
            Widget::Row {
                style: Style {
                    flex_wrap: FlexWrap::Wrap,
                    align_items: Some(AlignItems::Center),
                    gap: Size::length(spacing::MD),
                    flex_shrink: 0.0,
                    ..Style::default()
                },
                children: vec![
                    Widget::color_swatch(
                        state.picker.color(),
                        WidgetId::manual(SUMMARY_SWATCH_ID).expect("valid demo swatch ID"),
                        Style {
                            size: Size::from_lengths(64.0, 48.0),
                            flex_shrink: 0.0,
                            ..Style::default()
                        },
                    ),
                    demo_text(
                        format!(
                            "Live HEX: {} | {} | Alpha: {:.1}%",
                            state.picker.color().to_hex(),
                            ColorFormat::LABELS[state.picker.format().index()],
                            state.picker.alpha() * 100.0,
                        ),
                        14.0,
                    ),
                ],
            },
            demo_text(
                "External presets also replace the visible editor text.",
                13.0,
            ),
            Widget::Row {
                style: Style {
                    flex_wrap: FlexWrap::Wrap,
                    gap: Size::length(spacing::MD),
                    flex_shrink: 0.0,
                    ..Style::default()
                },
                children: vec![
                    demo_button("Coral", Msg::Preset(PickerColor::from_rgb8(255, 127, 80))),
                    demo_button(
                        "Cyan 50%",
                        Msg::Preset(PickerColor::from_rgba8(0, 188, 212, 128)),
                    ),
                    demo_button("Reset color", Msg::Reset),
                ],
            },
            Widget::heading(
                rutter::HeadingLevel::H2,
                "Popup picker (shares the inline color and format)",
                Style::default(),
            ),
            popup_view(state),
            demo_text(state.status.clone(), 13.0),
            demo_text(
                "Choose RGB, HEX, HSL, HSV or CMYK. Use arrows on the plane/strips; \
                 click outside the popup or press Escape to dismiss it.",
                13.0,
            ),
        ],
    }
}

impl AppLogic for ColorPickerDemo {
    type State = ColorPickerDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        initial_color_picker_demo_state()
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Msg> {
        super::layout::page(
            "Color picker",
            Widget::Column {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    align_items: Some(AlignItems::Stretch),
                    size: Size {
                        width: Dimension::percent(1.0),
                        height: Dimension::percent(1.0),
                    },
                    padding: Rect::length(spacing::XL),
                    gap: Size::length(spacing::LG),
                    ..Style::default()
                },
                children: vec![
                    example_theme_selector(state.theme, Msg::ThemeChanged),
                    demo_text("Color Picker Demo", 24.0),
                    Widget::ScrollView {
                        id: SCROLL_ID,
                        style: Style {
                            flex_grow: 0.0,
                            // A row's default stretch would constrain the document to the viewport height.
                            align_items: Some(AlignItems::FlexStart),
                            min_size: Size::from_lengths(0.0, 0.0),
                            ..responsive_width(780.0, Dimension::auto())
                        },
                        child: Box::new(demo_content(state)),
                    },
                ],
            },
        )
    }

    fn update(state: &mut Self::State, message: Msg, clipboard: &mut Clipboard) {
        update_color_picker_demo(state, message, |text| clipboard.set_text(text));
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<ColorPickerDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rutter::WidgetIdSnapshot;

    fn picker_event(state: &mut ColorPickerDemoState, event: ColorPickerEvent) {
        reduce_color_picker_demo(state, Msg::Picker(event));
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    fn visit(widget: &Widget<'_, Msg>, inspect: &mut impl FnMut(&Widget<'_, Msg>)) {
        inspect(widget);
        match widget {
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                for child in children {
                    visit(child, inspect);
                }
            }
            Widget::ScrollView { child, .. } | Widget::ButtonContent { child, .. } => {
                visit(child, inspect);
            }
            Widget::Popover {
                anchor, content, ..
            } => {
                visit(anchor, inspect);
                visit(content, inspect);
            }
            _ => {}
        }
    }

    fn editor_message(state: &mut ColorPickerDemoState, editor_id: u64, text: &str) -> Msg {
        let view = ColorPickerDemo::view(state);
        let mut message = None;
        visit(&view, &mut |widget| {
            if let Widget::TextInput { id, on_change, .. } = widget
                && *id == editor_id
            {
                message = Some(on_change(text.to_owned()));
            }
        });
        message.expect("requested editor exists in demo view")
    }

    fn assert_editor_models(state: &mut ColorPickerDemoState) {
        let view = ColorPickerDemo::view(state);
        let mut editors = Vec::new();
        let mut summaries = Vec::new();
        visit(&view, &mut |widget| {
            if let Widget::TextInput {
                id, model_value, ..
            } = widget
            {
                editors.push((
                    *id,
                    model_value
                        .expect("picker inputs are controlled")
                        .to_owned(),
                ));
            }
            if let Widget::Text { content, .. } = widget
                && content.starts_with("Live HEX:")
            {
                summaries.push(content.clone());
            }
        });
        drop(view);
        let mut expected = Vec::new();
        for first_id in [INLINE_FIRST_ID, POPUP_FIRST_ID] {
            if state.picker.format() == ColorFormat::Hex {
                expected.push((first_id + 5, state.picker.hex().to_owned()));
            } else {
                for index in 0..state.picker.channel_count() {
                    expected.push((
                        first_id + 6 + index as u64,
                        state.picker.channel_text(index).into(),
                    ));
                }
            }
        }
        assert_eq!(editors, expected);
        assert_eq!(
            summaries,
            vec![format!(
                "Live HEX: {} | {} | Alpha: {:.1}%",
                state.picker.color().to_hex(),
                ColorFormat::LABELS[state.picker.format().index()],
                state.picker.alpha() * 100.0,
            )]
        );
    }

    #[test]
    fn initial_color_drafts_and_theme_are_coherent() {
        let mut state = initial_color_picker_demo_state();
        assert_eq!(state.theme, ExampleTheme::Dark);
        assert_eq!(
            ColorPickerDemo::theme_for(&state).surface,
            Theme::dark().surface
        );
        assert!(!state.popup_open);
        assert_eq!(state.picker.color(), INITIAL_COLOR);
        assert_eq!(state.picker.hex(), "#6750A4FF");
        assert_eq!(state.picker.format(), ColorFormat::Rgb);
        for (index, expected) in [103.0, 80.0, 164.0, 100.0].into_iter().enumerate() {
            assert_close(state.picker.channel_text(index).parse().unwrap(), expected);
        }
        assert_editor_models(&mut state);

        reduce_color_picker_demo(&mut state, Msg::ThemeChanged(ExampleTheme::Light));
        assert_eq!(
            ColorPickerDemo::theme_for(&state).surface,
            Theme::light().surface
        );
        let Widget::Column { children, .. } = ColorPickerDemo::view(&mut state) else {
            panic!("theme selector must be a root child");
        };
        assert!(children.iter().any(|widget| matches!(
            widget,
            Widget::ButtonContent {
                label: "Switch to Dark theme",
                on_press: Msg::ThemeChanged(ExampleTheme::Dark),
                ..
            }
        )));
    }

    #[test]
    fn both_selectors_expose_and_cycle_all_five_modes_without_changing_color() {
        let mut state = initial_color_picker_demo_state();
        for format in ColorFormat::ALL.into_iter().chain([ColorFormat::Rgb]) {
            let expected_selected = state.picker.format().index();
            let message = {
                let view = ColorPickerDemo::view(&mut state);
                let mut message = None;
                let mut selectors = 0;
                visit(&view, &mut |widget| {
                    if let Widget::Select {
                        id,
                        options,
                        selected_index,
                        on_change,
                        ..
                    } = widget
                    {
                        selectors += 1;
                        assert_eq!(*options, ColorFormat::LABELS.as_slice());
                        assert_eq!(*selected_index, expected_selected);
                        if *id == INLINE_FIRST_ID + 4 {
                            message = Some(on_change(format.index()));
                        }
                    }
                });
                assert_eq!(selectors, 2);
                message.unwrap()
            };
            reduce_color_picker_demo(&mut state, message);
            assert_eq!(state.picker.format(), format);
            assert_eq!(state.picker.color(), INITIAL_COLOR);
            assert_eq!(
                state.picker.channel_count(),
                match format {
                    ColorFormat::Hex => 0,
                    ColorFormat::Cmyk => 5,
                    _ => 4,
                }
            );
            assert_editor_models(&mut state);
        }
    }

    #[test]
    fn sv_hue_and_alpha_events_refresh_numeric_and_hex_models() {
        let mut state = initial_color_picker_demo_state();
        picker_event(
            &mut state,
            ColorPickerEvent::Format(ColorFormat::Hsv.index()),
        );
        picker_event(&mut state, ColorPickerEvent::Hue(120.0));
        picker_event(
            &mut state,
            ColorPickerEvent::SaturationValue {
                saturation: 1.0,
                value: 1.0,
            },
        );
        picker_event(&mut state, ColorPickerEvent::Alpha(0.5));
        assert_eq!(state.picker.color().to_hex(), "#00FF0080");
        assert_close(state.picker.hue(), 120.0);
        assert_close(state.picker.saturation(), 1.0);
        assert_close(state.picker.value(), 1.0);
        assert_close(state.picker.alpha(), 0.5);
        for (index, expected) in [120.0, 100.0, 100.0, 50.0].into_iter().enumerate() {
            assert_close(state.picker.channel_text(index).parse().unwrap(), expected);
        }
        assert_editor_models(&mut state);
        picker_event(&mut state, ColorPickerEvent::Hue(240.0));
        picker_event(
            &mut state,
            ColorPickerEvent::Format(ColorFormat::Rgb.index()),
        );
        assert_eq!(state.picker.hex(), "#0000FF80");
        for (index, expected) in [0.0, 0.0, 255.0, 50.0].into_iter().enumerate() {
            assert_close(state.picker.channel_text(index).parse().unwrap(), expected);
        }
        assert_editor_models(&mut state);
    }

    #[test]
    fn invalid_and_valid_channel_edits_flow_through_real_callbacks_and_status() {
        let mut state = initial_color_picker_demo_state();
        let invalid = editor_message(&mut state, INLINE_FIRST_ID + 6, "-");
        reduce_color_picker_demo(&mut state, invalid);
        assert_eq!(state.picker.color(), INITIAL_COLOR);
        assert_eq!(state.picker.channel_text(0), "-");
        let error = state.picker.channel_error(0).unwrap();
        assert_eq!(state.status, format!("Color edit rejected: {error}"));
        assert_editor_models(&mut state);

        let valid = editor_message(&mut state, POPUP_FIRST_ID + 6, "255");
        reduce_color_picker_demo(&mut state, valid);
        assert_eq!(state.picker.color().to_hex(), "#FF50A4FF");
        assert_eq!(state.picker.channel_text(0), "255");
        assert!(state.picker.channel_error(0).is_none());
        assert!(state.status.starts_with("Color updated:"));
        assert_editor_models(&mut state);
    }

    #[test]
    fn hex_callback_preserves_invalid_draft_then_accepts_rgba() {
        let mut state = initial_color_picker_demo_state();
        picker_event(
            &mut state,
            ColorPickerEvent::Format(ColorFormat::Hex.index()),
        );
        let invalid = editor_message(&mut state, INLINE_FIRST_ID + 5, "#12");
        reduce_color_picker_demo(&mut state, invalid);
        assert_eq!(state.picker.color(), INITIAL_COLOR);
        assert_eq!(state.picker.hex(), "#12");
        assert!(state.status.starts_with("Color edit rejected:"));
        assert!(state.status.contains("HEX"));
        assert_editor_models(&mut state);

        let valid = editor_message(&mut state, POPUP_FIRST_ID + 5, "#12345678");
        reduce_color_picker_demo(&mut state, valid);
        assert_eq!(
            state.picker.color(),
            PickerColor::from_rgba8(18, 52, 86, 120)
        );
        assert_eq!(state.picker.hex(), "#12345678");
        assert!(!state.status.contains("rejected"));
        assert_editor_models(&mut state);
    }

    #[test]
    fn all_five_cmyk_callbacks_address_their_fixed_channel() {
        let mut state = initial_color_picker_demo_state();
        picker_event(
            &mut state,
            ColorPickerEvent::Format(ColorFormat::Cmyk.index()),
        );
        for index in 0..5 {
            let message = editor_message(&mut state, INLINE_FIRST_ID + 6 + index as u64, "025.00");
            assert!(matches!(
                &message,
                Msg::Picker(ColorPickerEvent::Channel { index: actual, text })
                    if *actual == index && text == "025.00"
            ));
            reduce_color_picker_demo(&mut state, message);
            assert_eq!(state.picker.channel_text(index), "025.00");
            assert!(state.picker.channel_error(index).is_none());
        }
        assert_close(state.picker.alpha(), 0.25);
        assert_editor_models(&mut state);
    }

    #[test]
    fn external_presets_and_reset_replace_invalid_drafts_even_for_unchanged_color() {
        let mut state = initial_color_picker_demo_state();
        let cyan = PickerColor::from_rgba8(0, 188, 212, 128);
        for format in ColorFormat::ALL {
            picker_event(&mut state, ColorPickerEvent::Format(format.index()));
            picker_event(&mut state, ColorPickerEvent::Hex("invalid".into()));
            reduce_color_picker_demo(&mut state, Msg::Preset(cyan));
            assert_eq!(state.picker.color(), cyan);
            assert_eq!(state.picker.hex(), "#00BCD480");
            assert_eq!(state.picker.format(), format);
            assert_editor_models(&mut state);

            // Reset must refresh drafts even when the last valid color already matches.
            reduce_color_picker_demo(&mut state, Msg::Reset);
            if state.picker.channel_count() > 0 {
                picker_event(
                    &mut state,
                    ColorPickerEvent::Channel {
                        index: 0,
                        text: "-".into(),
                    },
                );
                assert!(state.picker.channel_error(0).is_some());
            } else {
                picker_event(&mut state, ColorPickerEvent::Hex("#".into()));
            }
            reduce_color_picker_demo(&mut state, Msg::Reset);
            assert_eq!(state.picker.color(), INITIAL_COLOR);
            assert_eq!(state.picker.hex(), "#6750A4FF");
            assert_eq!(state.picker.format(), format);
            assert!(state.status.starts_with("Picker synchronized:"));
            for index in 0..state.picker.channel_count() {
                assert!(state.picker.channel_error(index).is_none());
                assert!(state.picker.channel_text(index).parse::<f32>().is_ok());
            }
            assert_editor_models(&mut state);
        }
    }

    #[test]
    fn popup_dismissal_uses_declared_message_and_preserves_the_color() {
        let mut state = initial_color_picker_demo_state();
        reduce_color_picker_demo(&mut state, Msg::TogglePopup);
        assert!(state.popup_open);
        picker_event(&mut state, ColorPickerEvent::Alpha(0.25));
        let color = state.picker.color();
        let dismiss = {
            let view = ColorPickerDemo::view(&mut state);
            let mut dismiss = None;
            visit(&view, &mut |widget| {
                if let Widget::Popover {
                    id,
                    open,
                    on_dismiss,
                    ..
                } = widget
                {
                    assert_eq!(*id, POPOVER_ID);
                    assert!(*open);
                    dismiss = on_dismiss.clone();
                }
            });
            dismiss.expect("outside click/Escape must dispatch a dismissal message")
        };
        reduce_color_picker_demo(&mut state, dismiss);
        assert!(!state.popup_open);
        assert_eq!(state.picker.color(), color);
        reduce_color_picker_demo(&mut state, Msg::DismissPopup);
        assert!(!state.popup_open);
        reduce_color_picker_demo(&mut state, Msg::TogglePopup);
        reduce_color_picker_demo(&mut state, Msg::TogglePopup);
        assert!(!state.popup_open);
    }

    #[test]
    fn invalid_gesture_or_format_is_reported_without_changing_the_color() {
        let mut state = initial_color_picker_demo_state();
        for event in [ColorPickerEvent::Hue(f32::NAN), ColorPickerEvent::Format(5)] {
            picker_event(&mut state, event);
            assert_eq!(state.picker.color(), INITIAL_COLOR);
            assert_eq!(state.picker.format(), ColorFormat::Rgb);
            assert!(state.status.starts_with("Color edit rejected:"));
        }
    }

    #[test]
    fn manual_ids_are_distinct_and_stable_across_color_edits_and_popup_changes() {
        let mut state = initial_color_picker_demo_state();
        let initial = WidgetIdSnapshot::capture(&ColorPickerDemo::view(&mut state)).unwrap();
        for message in [
            Msg::Picker(ColorPickerEvent::Hue(75.0)),
            Msg::Picker(ColorPickerEvent::Channel {
                index: 0,
                text: "-".into(),
            }),
            Msg::Preset(PickerColor::from_rgb8(255, 127, 80)),
            Msg::Reset,
        ] {
            reduce_color_picker_demo(&mut state, message);
            let view = ColorPickerDemo::view(&mut state);
            let snapshot = WidgetIdSnapshot::capture(&view).unwrap();
            initial.validate_reconstruction(&snapshot).unwrap();
            let mut ids = Vec::new();
            visit(&view, &mut |widget| match widget {
                Widget::Custom { id, .. } => ids.push(id.get()),
                Widget::Select { id, .. }
                | Widget::TextInput { id, .. }
                | Widget::Popover { id, .. }
                | Widget::ScrollView { id, .. } => ids.push(*id),
                _ => {}
            });
            ids.sort_unstable();
            let expected: Vec<_> = (100..=110)
                .chain(200..=210)
                .chain(300..=302)
                .filter(|id| ![105, 110, 205, 210].contains(id))
                .collect();
            assert_eq!(ids, expected);
        }
        for format in ColorFormat::ALL {
            picker_event(&mut state, ColorPickerEvent::Format(format.index()));
            reduce_color_picker_demo(&mut state, Msg::TogglePopup);
            let snapshot = WidgetIdSnapshot::capture(&ColorPickerDemo::view(&mut state)).unwrap();
            initial.validate_transition_to(&snapshot).unwrap();
        }
    }

    mod copy_and_layout {
        use std::{cell::RefCell, collections::HashMap, rc::Rc};

        use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
        use rutter::render::RichTextRenderer;
        use skia_safe::{Point, Rect};
        use winit::dpi::PhysicalSize;

        use super::*;

        #[derive(Default)]
        struct ClipboardWriterFake {
            writes: Vec<String>,
            error: Option<&'static str>,
        }

        impl ClipboardWriterFake {
            fn write_text(&mut self, text: String) -> Result<(), &'static str> {
                self.writes.push(text);
                match self.error {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            }
        }

        fn copy_messages(state: &mut ColorPickerDemoState) -> Vec<Msg> {
            let view = ColorPickerDemo::view(state);
            // Includes the closed popup's declared branch and all automatically keyed buttons.
            WidgetIdSnapshot::capture(&view).expect("copy buttons must not collide with other IDs");
            let mut messages = Vec::new();
            visit(&view, &mut |widget| {
                if let Widget::ButtonContent {
                    label, on_press, ..
                } = widget
                    && (*label == "Copy color as HEX" || matches!(on_press, Msg::CopyColor))
                {
                    assert_eq!(*label, "Copy color as HEX");
                    assert!(matches!(on_press, Msg::CopyColor));
                    messages.push(on_press.clone());
                }
            });
            assert_eq!(
                messages.len(),
                2,
                "inline and popup each expose a copy button"
            );
            messages
        }

        fn assert_picker_unchanged(actual: &ColorPickerState, before: &ColorPickerState) {
            assert_eq!(actual.color(), before.color());
            assert_eq!(actual.format(), before.format());
            assert_eq!(actual.hue(), before.hue());
            assert_eq!(actual.saturation(), before.saturation());
            assert_eq!(actual.value(), before.value());
            assert_eq!(actual.alpha(), before.alpha());
            assert_eq!(actual.hex(), before.hex());
            assert_eq!(actual.hex_error(), before.hex_error());
            assert_eq!(actual.channel_count(), before.channel_count());
            for index in 0..before.channel_count() {
                assert_eq!(actual.channel_text(index), before.channel_text(index));
                assert_eq!(actual.channel_error(index), before.channel_error(index));
            }
        }

        fn state_with_invalid_drafts(format: ColorFormat, alpha: u8) -> ColorPickerDemoState {
            let mut state = initial_color_picker_demo_state();
            reduce_color_picker_demo(
                &mut state,
                Msg::Preset(PickerColor::from_rgba8(18, 52, 86, alpha)),
            );
            picker_event(&mut state, ColorPickerEvent::Format(format.index()));
            // An invalid HEX draft in numeric modes also detects accidental draft copying.
            picker_event(&mut state, ColorPickerEvent::Hex("#12".into()));
            if format != ColorFormat::Hex {
                let message = editor_message(&mut state, INLINE_FIRST_ID + 6, "-");
                reduce_color_picker_demo(&mut state, message);
            }
            state.popup_open = alpha != 0;
            state
        }

        #[test]
        fn both_copy_buttons_write_canonical_rgba_and_report_failures_without_mutating_picker() {
            for format in ColorFormat::ALL {
                for (alpha, expected) in [(0, "#12345600"), (128, "#12345680"), (255, "#123456FF")]
                {
                    let mut state = state_with_invalid_drafts(format, alpha);
                    let before = state.picker.clone();
                    let mut clipboard = ClipboardWriterFake::default();
                    for message in copy_messages(&mut state) {
                        clipboard.error = None;
                        update_color_picker_demo(&mut state, message.clone(), |text| {
                            clipboard.write_text(text)
                        });
                        assert_eq!(state.status, format!("Copied color as HEX: {expected}"));
                        assert_picker_unchanged(&state.picker, &before);
                        assert_eq!(state.popup_open, alpha != 0);
                        clipboard.error = Some("clipboard is busy");
                        update_color_picker_demo(&mut state, message, |text| {
                            clipboard.write_text(text)
                        });
                        assert_eq!(
                            state.status,
                            "Failed to copy color as HEX: clipboard is busy"
                        );
                        assert_picker_unchanged(&state.picker, &before);
                        assert_eq!(state.popup_open, alpha != 0);
                        assert_eq!(state.theme, ExampleTheme::Dark);
                    }
                    assert_eq!(clipboard.writes, vec![expected; 4]);
                    assert_editor_models(&mut state);
                }
            }
        }

        #[test]
        fn non_copy_messages_delegate_without_writing_and_pure_reducer_never_claims_a_copy() {
            let mut state = initial_color_picker_demo_state();
            let mut expected = initial_color_picker_demo_state();
            for message in [
                Msg::Preset(PickerColor::from_rgba8(18, 52, 86, 120)),
                Msg::Picker(ColorPickerEvent::Hex("#12".into())),
                Msg::ThemeChanged(ExampleTheme::Light),
                Msg::TogglePopup,
                Msg::DismissPopup,
                Msg::Reset,
            ] {
                reduce_color_picker_demo(&mut expected, message.clone());
                update_color_picker_demo(&mut state, message, |_| -> Result<(), &'static str> {
                    panic!("non-copy message must not access the clipboard")
                });
                assert_picker_unchanged(&state.picker, &expected.picker);
                assert_eq!(state.status, expected.status);
                assert_eq!(state.popup_open, expected.popup_open);
                assert_eq!(state.theme, expected.theme);
            }
            state.status = "Copied color as HEX: #6750A4FF".into();
            reduce_color_picker_demo(&mut state, Msg::CopyColor);
            assert_eq!(
                state.status,
                "Color not copied: clipboard writer unavailable."
            );
        }

        fn widget_at<'w, 'a>(
            mut widget: &'w Widget<'a, Msg>,
            path: &[usize],
        ) -> &'w Widget<'a, Msg> {
            for index in path {
                widget = match widget {
                    Widget::Column { children, .. } | Widget::Row { children, .. } => {
                        &children[*index]
                    }
                    Widget::ScrollView { child, .. } if *index == 0 => child,
                    Widget::Popover { anchor, .. } if *index == 0 => anchor,
                    Widget::Popover { content, .. } if *index == 1 => content,
                    _ => panic!("unexpected demo widget at {path:?}"),
                };
            }
            widget
        }

        fn node_at(tree: &TaffyTree<RutterContext>, mut node: NodeId, path: &[usize]) -> NodeId {
            for index in path {
                node = tree.children(node).unwrap()[*index];
            }
            node
        }

        fn bounds_at(tree: &TaffyTree<RutterContext>, mut node: NodeId, path: &[usize]) -> Rect {
            let mut origin = Point::new(0.0, 0.0);
            for index in path {
                let layout = tree.layout(node).unwrap();
                origin.x += layout.location.x;
                origin.y += layout.location.y;
                node = tree.children(node).unwrap()[*index];
            }
            let layout = tree.layout(node).unwrap();
            Rect::from_xywh(
                origin.x + layout.location.x,
                origin.y + layout.location.y,
                layout.size.width,
                layout.size.height,
            )
        }

        fn collect_controls(
            widget: &Widget<'_, Msg>,
            tree: &TaffyTree<RutterContext>,
            node: NodeId,
            origin: Point,
            controls: &mut Vec<(u64, Rect)>,
        ) {
            let layout = tree.layout(node).unwrap();
            let origin = Point::new(origin.x + layout.location.x, origin.y + layout.location.y);
            let id = match widget {
                Widget::Custom { id, .. } => Some(id.get()),
                Widget::Select { id, .. } | Widget::TextInput { id, .. } => Some(*id),
                Widget::ButtonContent {
                    on_press: Msg::CopyColor,
                    ..
                } => Some(0),
                _ => None,
            };
            if let Some(id) = id {
                let rect =
                    Rect::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height);
                controls.push((id, rect));
            } else if let Widget::Column { children, .. } | Widget::Row { children, .. } = widget {
                let nodes = tree.children(node).unwrap();
                assert_eq!(children.len(), nodes.len());
                for (child, node) in children.iter().zip(nodes) {
                    collect_controls(child, tree, node, origin, controls);
                }
            }
        }

        fn assert_popup_controls(
            view: &Widget<'_, Msg>,
            tree: &TaffyTree<RutterContext>,
            root: NodeId,
            format: ColorFormat,
        ) {
            let picker_path = [1, 0, 1, 6, 1, 0, 0];
            let picker_node = node_at(tree, root, &picker_path);
            let picker_widget = widget_at(view, &[1, 0, 1, 6, 1, 0]);
            let mut controls = Vec::new();
            let picker = tree.layout(picker_node).unwrap();
            let origin = Point::new(-picker.location.x, -picker.location.y);
            collect_controls(picker_widget, tree, picker_node, origin, &mut controls);
            let bounds = |id| controls.iter().find(|(actual, _)| *actual == id).unwrap().1;
            for (id, height) in [(200, 190.0), (201, 24.0), (202, 24.0), (203, 58.0)] {
                assert_close(bounds(id).height(), height);
            }
            let editor_ids: Vec<_> = if format == ColorFormat::Hex {
                vec![205]
            } else {
                let count = if format == ColorFormat::Cmyk { 5 } else { 4 };
                (206..206 + count).collect()
            };
            assert_eq!(controls.len(), 6 + editor_ids.len());
            for id in editor_ids {
                let field = bounds(id);
                assert!(field.width() >= 60.0);
                assert_close(field.height() - 20.0, 48.0);
                assert!(
                    field.right <= bounds(0).left,
                    "copy button must sit beside the editor group"
                );
            }
            for (index, (id, rect)) in controls.iter().enumerate() {
                assert!(
                    rect.width().is_finite() && rect.width() > 0.0,
                    "control {id}"
                );
                assert!(
                    rect.height().is_finite() && rect.height() > 0.0,
                    "control {id}"
                );
                assert!(
                    rect.left >= 0.0 && rect.right <= picker.size.width + 1.0,
                    "control {id}: {rect:?}"
                );
                assert!(
                    rect.top >= 0.0 && rect.bottom <= picker.size.height + 1.0,
                    "control {id}: {rect:?}"
                );
                for (_, other) in &controls[index + 1..] {
                    assert!(
                        rect.right <= other.left
                            || other.right <= rect.left
                            || rect.bottom <= other.top
                            || other.bottom <= rect.top,
                        "overlapping controls: {rect:?}, {other:?}"
                    );
                }
            }
        }

        #[test]
        fn full_view_popup_fits_desktop_and_narrow_surfaces_and_document_scrolls_in_every_format() {
            let fonts = Rc::new(RefCell::new(FontSystem::new()));
            for width in [800, 320] {
                for format in ColorFormat::ALL {
                    let mut state = initial_color_picker_demo_state();
                    picker_event(&mut state, ColorPickerEvent::Format(format.index()));
                    state.popup_open = true;
                    let view = ColorPickerDemo::view(&mut state);
                    assert!(matches!(
                        widget_at(&view, &[1]),
                        Widget::ScrollView { id: SCROLL_ID, .. }
                    ));
                    assert!(matches!(
                        widget_at(&view, &[1, 0, 1, 6]),
                        Widget::Popover {
                            id: POPOVER_ID,
                            open: true,
                            ..
                        }
                    ));
                    let mut tree = TaffyTree::new();
                    let root = build_taffy_tree(&mut tree, &view, fonts.clone(), &HashMap::new());
                    compute_layout(
                        &mut tree,
                        root,
                        PhysicalSize::new(width, 700),
                        fonts.clone(),
                        &RichTextRenderer::default(),
                    );
                    let host = bounds_at(&tree, root, &[1, 0, 1, 6]);
                    let anchor = bounds_at(&tree, root, &[1, 0, 1, 6, 0]);
                    let popup = bounds_at(&tree, root, &[1, 0, 1, 6, 1]);
                    let content = bounds_at(&tree, root, &[1, 0, 1, 6, 1, 0]);
                    let picker = bounds_at(&tree, root, &[1, 0, 1, 6, 1, 0, 0]);
                    assert_close(host.width(), (width as f32 - 48.0).min(384.0));
                    assert_close(host.height(), 40.0);
                    assert_close(anchor.width(), 160.0);
                    assert_close(popup.width(), host.width());
                    assert_close(content.width(), popup.width());
                    assert_close(picker.width(), popup.width() - 24.0);
                    assert_close(picker.left - content.left, 12.0);
                    assert_close(picker.top - content.top, 12.0);
                    assert_close(content.height(), picker.height() + 24.0);
                    assert!(popup.left >= 24.0 && popup.right <= width as f32 - 24.0);
                    assert_popup_controls(&view, &tree, root, format);
                    let viewport = bounds_at(&tree, root, &[1]);
                    let document = bounds_at(&tree, root, &[1, 0]);
                    let last = bounds_at(&tree, root, &[1, 0, 1, 8]);
                    assert!(viewport.height() > 0.0 && document.height() > viewport.height());
                    assert!(last.height() > 0.0 && last.bottom > viewport.bottom);
                    // Taffy rounds document and text-node dimensions independently.
                    assert!((document.bottom - last.bottom).abs() <= 1.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod page_tests {
    use super::ColorPickerDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<ColorPickerDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<ColorPickerDemo>();
    }
}
