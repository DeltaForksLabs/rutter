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
#[path = "../../tests/unit/color_picker_demo_unit_tests.rs"]
mod tests;
