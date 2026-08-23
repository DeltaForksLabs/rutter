use taffy::prelude::Style;

use super::{
    ClockConfig, ClockFormat, TimeOfDay, TimePickerConfig, TimePickerError, TimePickerLabels,
    TimeZone,
};
use crate::Widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Toggle,
    Close,
    Hour(i64),
    Minute(i64),
    Second(i64),
    Zone(usize),
}

const ZONES: &[&str] = &["UTC", "America/Sao_Paulo"];

#[test]
fn clock_constructor_retains_timezone_and_config() {
    let config = ClockConfig::new(ClockFormat::twelve_hour(), 28.0).unwrap();
    let widget: Widget<'_, Msg> =
        Widget::clock_with_config(TimeZone::UTC, config, Style::default(), "UTC clock");

    assert!(matches!(
        widget,
        Widget::Clock {
            time_zone: TimeZone::Iana(chrono_tz::UTC),
            config: actual,
            label: "UTC clock",
            ..
        } if actual == config
    ));
}

#[test]
fn time_picker_constructor_keeps_each_control_callback() {
    let config = TimePickerConfig::new(
        ZONES,
        1,
        Msg::Toggle,
        Msg::Close,
        Msg::Hour,
        Msg::Minute,
        Msg::Second,
        Msg::Zone,
        "Time picker",
    )
    .unwrap();
    let widget = Widget::time_picker(
        false,
        TimeOfDay::new(12, 30, 45).unwrap(),
        config,
        Style::default(),
        Style::default(),
    );

    let Widget::Popover {
        anchor, content, ..
    } = widget
    else {
        panic!("time picker expected a Popover root");
    };
    assert!(matches!(
        *anchor,
        Widget::ButtonContent {
            on_press: Msg::Toggle,
            ..
        }
    ));
    assert!(picker_contains_callback(&content, Msg::Hour(12)));
    assert!(picker_contains_callback(&content, Msg::Minute(30)));
    assert!(picker_contains_callback(&content, Msg::Second(45)));
    assert!(picker_contains_callback(&content, Msg::Zone(1)));
}

#[test]
fn time_picker_composes_a_stable_popover_with_all_time_fields() {
    let widget = Widget::time_picker(
        true,
        TimeOfDay::new(9, 30, 15).unwrap(),
        config(),
        Style::default(),
        Style::default(),
    );
    let Widget::Popover {
        anchor, content, ..
    } = widget
    else {
        panic!("time picker expected a Popover root");
    };

    assert!(time_picker_contains_text(&anchor, "09:30:15 UTC"));
    assert_eq!(counter_count(&content), 3);
    assert!(time_picker_contains_text(&content, "Time zone"));
}

#[test]
fn time_picker_configuration_rejects_invalid_iana_zone_names() {
    let result = TimePickerConfig::new(
        &["UTC", "Mars/Olympus"],
        0,
        Msg::Toggle,
        Msg::Close,
        Msg::Hour,
        Msg::Minute,
        Msg::Second,
        Msg::Zone,
        "Time",
    );

    match result {
        Err(TimePickerError::InvalidTimeZone { index, value }) => {
            assert_eq!(index, 1);
            assert_eq!(value, "Mars/Olympus");
        }
        _ => panic!("expected an invalid IANA time-zone error"),
    }
}

#[test]
fn time_picker_configuration_requires_a_timezone_choice() {
    let result = TimePickerConfig::new(
        &[],
        0,
        Msg::Toggle,
        Msg::Close,
        Msg::Hour,
        Msg::Minute,
        Msg::Second,
        Msg::Zone,
        "Time",
    );

    assert!(matches!(result, Err(TimePickerError::EmptyTimeZones)));
}

#[test]
fn time_picker_configuration_rejects_out_of_range_selected_timezones() {
    let result = TimePickerConfig::new(
        ZONES,
        2,
        Msg::Toggle,
        Msg::Close,
        Msg::Hour,
        Msg::Minute,
        Msg::Second,
        Msg::Zone,
        "Time",
    );

    assert!(matches!(
        result,
        Err(TimePickerError::SelectedTimeZoneOutOfBounds {
            selected: 2,
            zone_count: 2,
        })
    ));
}

#[test]
fn time_picker_configuration_uses_localized_labels() {
    let config = config().with_labels(TimePickerLabels::PORTUGUESE);

    assert_eq!(config.labels().time_zone, "Fuso horário");
}

fn config() -> TimePickerConfig<'static, Msg> {
    TimePickerConfig::new(
        ZONES,
        0,
        Msg::Toggle,
        Msg::Close,
        Msg::Hour,
        Msg::Minute,
        Msg::Second,
        Msg::Zone,
        "Time picker",
    )
    .unwrap()
}

fn picker_contains_callback(widget: &Widget<'_, Msg>, expected: Msg) -> bool {
    match widget {
        Widget::Counter {
            value, on_change, ..
        } => on_change(*value) == expected,
        Widget::Select {
            selected_index,
            on_change,
            ..
        } => on_change(*selected_index) == expected,
        Widget::Column { children, .. } | Widget::Row { children, .. } => children
            .iter()
            .any(|child| picker_contains_callback(child, expected)),
        Widget::Container { child, .. } | Widget::ButtonContent { child, .. } => {
            picker_contains_callback(child, expected)
        }
        Widget::Popover {
            anchor, content, ..
        } => {
            picker_contains_callback(anchor, expected)
                || picker_contains_callback(content, expected)
        }
        _ => false,
    }
}

fn time_picker_contains_text<Msg>(widget: &Widget<'_, Msg>, expected: &str) -> bool {
    match widget {
        Widget::Text { content, .. } => content == expected,
        Widget::Column { children, .. } | Widget::Row { children, .. } => children
            .iter()
            .any(|child| time_picker_contains_text(child, expected)),
        Widget::Container { child, .. } | Widget::ButtonContent { child, .. } => {
            time_picker_contains_text(child, expected)
        }
        Widget::Popover {
            anchor, content, ..
        } => {
            time_picker_contains_text(anchor, expected)
                || time_picker_contains_text(content, expected)
        }
        _ => false,
    }
}

fn counter_count<Msg>(widget: &Widget<'_, Msg>) -> usize {
    match widget {
        Widget::Counter { .. } => 1,
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            children.iter().map(counter_count).sum()
        }
        Widget::Container { child, .. } | Widget::ButtonContent { child, .. } => {
            counter_count(child)
        }
        Widget::Popover {
            anchor, content, ..
        } => counter_count(anchor) + counter_count(content),
        _ => 0,
    }
}
