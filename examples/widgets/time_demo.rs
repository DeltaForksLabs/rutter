// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Live timezone-aware clock and controlled time-picker demonstration.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    AppLogic, ClockConfig, ClockFormat, RutterRunner, Theme, TimeOfDay, TimePickerConfig,
    TimePickerLabels, TimeZone, Widget,
};
use taffy::prelude::*;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const TIME_ZONES: &[&str] = &["UTC", "America/Sao_Paulo", "Europe/Lisbon", "Asia/Tokyo"];

pub struct TimeDemoState {
    theme: ExampleTheme,
    selected_time: TimeOfDay,
    selected_time_zone: usize,
    picker_open: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    TogglePicker,
    ClosePicker,
    HourChanged(i64),
    MinuteChanged(i64),
    SecondChanged(i64),
    TimeZoneChanged(usize),
}

pub struct TimeDemo;

impl AppLogic for TimeDemo {
    type State = TimeDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        initial_time_demo_state()
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Column {
            children: time_demo_children(state),
            style: time_demo_root_style(),
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        apply_time_demo_message(state, message);
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn initial_time_demo_state() -> TimeDemoState {
    TimeDemoState {
        theme: ExampleTheme::Dark,
        selected_time: TimeOfDay::new(9, 30, 0).expect("09:30:00 must be a valid demo time"),
        selected_time_zone: 1,
        picker_open: false,
    }
}

fn time_demo_children(state: &TimeDemoState) -> Vec<Widget<'_, Msg>> {
    vec![
        example_theme_selector(state.theme, Msg::ThemeChanged),
        demo_text("Relógio e Time Picker", 24.0),
        demo_text(
            "O relógio atualiza no limite de cada segundo e respeita o fuso selecionado.",
            14.0,
        ),
        live_clock(state),
        time_picker(state),
        demo_text(selected_time_summary(state), 14.0),
    ]
}

fn live_clock(state: &TimeDemoState) -> Widget<'_, Msg> {
    let config = ClockConfig::new(ClockFormat::twelve_hour(), 30.0)
        .expect("30 logical pixels must be a valid clock font size");
    Widget::clock_with_config(
        selected_time_zone(state.selected_time_zone),
        config,
        live_clock_style(),
        "Relógio ao vivo",
    )
    .with_id(1)
}

fn time_picker(state: &TimeDemoState) -> Widget<'_, Msg> {
    Widget::time_picker(
        state.picker_open,
        state.selected_time,
        time_picker_config(state.selected_time_zone),
        time_picker_anchor_style(),
        time_picker_popup_style(),
    )
    .with_id(2)
}

fn time_picker_config(selected_time_zone: usize) -> TimePickerConfig<'static, Msg> {
    TimePickerConfig::new(
        TIME_ZONES,
        selected_time_zone,
        Msg::TogglePicker,
        Msg::ClosePicker,
        Msg::HourChanged,
        Msg::MinuteChanged,
        Msg::SecondChanged,
        Msg::TimeZoneChanged,
        "Horário do evento",
    )
    .expect("the time-demo IANA timezone list must remain valid")
    .with_labels(TimePickerLabels::PORTUGUESE)
}

fn selected_time_summary(state: &TimeDemoState) -> String {
    format!(
        "Horário selecionado: {} {}",
        state.selected_time, TIME_ZONES[state.selected_time_zone]
    )
}

fn selected_time_zone(index: usize) -> TimeZone {
    let name = TIME_ZONES.get(index).copied().unwrap_or_else(|| {
        panic!("time-demo timezone index={index} must be within the configured list")
    });
    TimeZone::iana(name)
        .unwrap_or_else(|error| panic!("time-demo timezone value={name:?} must be valid: {error}"))
}

fn apply_time_demo_message(state: &mut TimeDemoState, message: Msg) {
    match message {
        Msg::ThemeChanged(theme) => state.theme = theme,
        Msg::TogglePicker => state.picker_open = !state.picker_open,
        Msg::ClosePicker => state.picker_open = false,
        Msg::HourChanged(hour) => update_selected_time(
            state,
            hour,
            i64::from(state.selected_time.minute()),
            i64::from(state.selected_time.second()),
        ),
        Msg::MinuteChanged(minute) => update_selected_time(
            state,
            i64::from(state.selected_time.hour()),
            minute,
            i64::from(state.selected_time.second()),
        ),
        Msg::SecondChanged(second) => update_selected_time(
            state,
            i64::from(state.selected_time.hour()),
            i64::from(state.selected_time.minute()),
            second,
        ),
        Msg::TimeZoneChanged(index) => state.selected_time_zone = validated_time_zone_index(index),
    }
}

fn update_selected_time(state: &mut TimeDemoState, hour: i64, minute: i64, second: i64) {
    state.selected_time = TimeOfDay::from_counter_values(hour, minute, second).unwrap_or_else(|error| {
        panic!(
            "time picker callbacks must provide bounded time parts hour={hour}, minute={minute}, second={second}: {error}"
        )
    });
}

fn validated_time_zone_index(index: usize) -> usize {
    TIME_ZONES.get(index).map(|_| index).unwrap_or_else(|| {
        panic!(
            "time picker callback timezone index={index} must be within {} configured choices",
            TIME_ZONES.len()
        )
    })
}

fn demo_text<'a>(content: impl Into<String>, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: Style::default(),
        color: None,
        size,
    }
}

fn time_demo_root_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        size: Size::percent(1.0_f32),
        padding: Rect::length(32.0_f32),
        gap: Size::length(14.0_f32),
        ..Style::default()
    }
}

fn live_clock_style() -> Style {
    Style {
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(56.0),
        },
        ..Style::default()
    }
}

fn time_picker_anchor_style() -> Style {
    responsive_width(360.0, Dimension::length(44.0))
}

fn time_picker_popup_style() -> Style {
    responsive_width(360.0, Dimension::length(210.0))
}

/// Runs the standalone clock and time-picker demonstration.
///
/// Run it with `cargo run -- time`.
pub fn run() {
    RutterRunner::<TimeDemo>::run();
}

#[cfg(test)]
#[path = "../../tests/unit/time_demo_unit_tests.rs"]
mod tests;
