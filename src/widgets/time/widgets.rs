// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

use taffy::prelude::Style;

use super::styles::{
    fill_parent_style, time_picker_anchor_value_style, time_picker_content_style,
    time_picker_counter_style, time_picker_field_style, time_picker_fields_style,
    time_picker_label_style, time_picker_zone_style,
};
use super::{ClockConfig, TimeOfDay, TimePickerLabels, TimeZone, TimeZoneError};
use crate::widget::{ButtonVariant, Widget};

/// Reports an invalid timezone configuration for a time picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimePickerError {
    EmptyTimeZones,
    SelectedTimeZoneOutOfBounds { selected: usize, zone_count: usize },
    InvalidTimeZone { index: usize, value: String },
}

impl fmt::Display for TimePickerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTimeZones => formatter.write_str(
                "invalid time picker timezone list; expected at least one IANA timezone identifier",
            ),
            Self::SelectedTimeZoneOutOfBounds {
                selected,
                zone_count,
            } => write!(
                formatter,
                "invalid selected timezone index={selected}; expected an index within a timezone list of length {zone_count}"
            ),
            Self::InvalidTimeZone { index, value } => write!(
                formatter,
                "invalid time picker timezone index={index}, value={value:?}; expected an IANA timezone name such as \"UTC\" or \"America/Sao_Paulo\""
            ),
        }
    }
}

impl std::error::Error for TimePickerError {}

/// Controlled callbacks and timezone choices for a time picker.
///
/// The timezone strings must be valid IANA identifiers. The picker returns the
/// selected index so applications can preserve their own selection state.
pub struct TimePickerConfig<'a, Msg> {
    time_zone_names: &'a [&'a str],
    selected_time_zone: usize,
    on_toggle: Msg,
    on_dismiss: Msg,
    on_hour_change: fn(i64) -> Msg,
    on_minute_change: fn(i64) -> Msg,
    on_second_change: fn(i64) -> Msg,
    on_time_zone_change: fn(usize) -> Msg,
    accessibility_label: &'a str,
    labels: TimePickerLabels<'a>,
}

impl<'a, Msg> TimePickerConfig<'a, Msg> {
    /// Creates validated controlled time-picker configuration.
    ///
    /// ```rust
    /// use rutter::TimePickerConfig;
    ///
    /// enum Msg { Toggle, Close, Hour(i64), Minute(i64), Second(i64), Zone(usize) }
    /// const ZONES: &[&str] = &["UTC", "America/Sao_Paulo"];
    /// let config = TimePickerConfig::new(
    ///     ZONES, 0, Msg::Toggle, Msg::Close, Msg::Hour, Msg::Minute,
    ///     Msg::Second, Msg::Zone, "Time picker",
    /// ).unwrap();
    /// assert_eq!(config.selected_time_zone(), 0);
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time_zone_names: &'a [&'a str],
        selected_time_zone: usize,
        on_toggle: Msg,
        on_dismiss: Msg,
        on_hour_change: fn(i64) -> Msg,
        on_minute_change: fn(i64) -> Msg,
        on_second_change: fn(i64) -> Msg,
        on_time_zone_change: fn(usize) -> Msg,
        accessibility_label: &'a str,
    ) -> Result<Self, TimePickerError> {
        validate_time_picker_zones(time_zone_names, selected_time_zone)?;
        Ok(Self {
            time_zone_names,
            selected_time_zone,
            on_toggle,
            on_dismiss,
            on_hour_change,
            on_minute_change,
            on_second_change,
            on_time_zone_change,
            accessibility_label,
            labels: TimePickerLabels::default(),
        })
    }

    /// Replaces the default English labels.
    ///
    /// ```rust
    /// # use rutter::{TimePickerConfig, TimePickerLabels};
    /// # enum Msg { Toggle, Close, Hour(i64), Minute(i64), Second(i64), Zone(usize) }
    /// # const ZONES: &[&str] = &["UTC"];
    /// let config = TimePickerConfig::new(ZONES, 0, Msg::Toggle, Msg::Close, Msg::Hour, Msg::Minute, Msg::Second, Msg::Zone, "Time")
    ///     .unwrap().with_labels(TimePickerLabels::PORTUGUESE);
    /// assert_eq!(config.labels().hour, "Hora");
    /// ```
    pub const fn with_labels(mut self, labels: TimePickerLabels<'a>) -> Self {
        self.labels = labels;
        self
    }

    /// Returns the configured IANA timezone identifiers.
    pub const fn time_zone_names(&self) -> &'a [&'a str] {
        self.time_zone_names
    }

    /// Returns the currently selected timezone index.
    pub const fn selected_time_zone(&self) -> usize {
        self.selected_time_zone
    }

    /// Returns the labels used by the picker.
    pub const fn labels(&self) -> TimePickerLabels<'a> {
        self.labels
    }

    fn content_props(&self) -> TimePickerContentProps<'a, Msg> {
        TimePickerContentProps {
            time_zone_names: self.time_zone_names,
            selected_time_zone: self.selected_time_zone,
            on_hour_change: self.on_hour_change,
            on_minute_change: self.on_minute_change,
            on_second_change: self.on_second_change,
            on_time_zone_change: self.on_time_zone_change,
            labels: self.labels,
        }
    }

    fn into_anchor(self, time: TimeOfDay, style: Style) -> (Widget<'a, Msg>, Msg) {
        let time_zone_name = self.time_zone_names[self.selected_time_zone];
        let anchor = time_picker_anchor(
            time,
            time_zone_name,
            self.on_toggle,
            self.accessibility_label,
            style,
        );
        (anchor, self.on_dismiss)
    }
}

struct TimePickerContentProps<'a, Msg> {
    time_zone_names: &'a [&'a str],
    selected_time_zone: usize,
    on_hour_change: fn(i64) -> Msg,
    on_minute_change: fn(i64) -> Msg,
    on_second_change: fn(i64) -> Msg,
    on_time_zone_change: fn(usize) -> Msg,
    labels: TimePickerLabels<'a>,
}

impl<'a, Msg> Widget<'a, Msg> {
    /// Creates a live clock that redraws at each second boundary.
    ///
    /// ```rust
    /// use rutter::{TimeZone, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let clock: Widget<'_, ()> = Widget::clock(TimeZone::UTC, Style::default(), "UTC clock");
    /// assert!(matches!(clock, Widget::Clock { .. }));
    /// ```
    pub fn clock(time_zone: TimeZone, style: Style, accessibility_label: &'a str) -> Self {
        Self::clock_with_config(
            time_zone,
            ClockConfig::default(),
            style,
            accessibility_label,
        )
    }

    /// Creates a live clock with explicit formatting, font size, and color.
    ///
    /// ```rust
    /// use rutter::{ClockConfig, ClockFormat, TimeZone, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let config = ClockConfig::new(ClockFormat::twelve_hour(), 24.0).unwrap();
    /// let clock: Widget<'_, ()> = Widget::clock_with_config(TimeZone::UTC, config, Style::default(), "UTC clock");
    /// assert!(matches!(clock, Widget::Clock { .. }));
    /// ```
    pub fn clock_with_config(
        time_zone: TimeZone,
        config: ClockConfig,
        style: Style,
        accessibility_label: &'a str,
    ) -> Self {
        Self::Clock {
            id: crate::widget::AUTO_ID,
            time_zone,
            config,
            style,
            label: accessibility_label,
        }
    }

    /// Creates a controlled time picker with hour, minute, second, and timezone controls.
    ///
    /// Keep `open`, `TimeOfDay`, and the selected timezone index in application
    /// state. Counter callbacks are intentionally granular so the application
    /// can apply its own carry/wrapping policy when a field changes.
    ///
    /// ```rust
    /// use rutter::{TimeOfDay, TimePickerConfig, Widget};
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { Toggle, Close, Hour(i64), Minute(i64), Second(i64), Zone(usize) }
    /// const ZONES: &[&str] = &["UTC", "America/Sao_Paulo"];
    /// let config = TimePickerConfig::new(ZONES, 0, Msg::Toggle, Msg::Close, Msg::Hour, Msg::Minute, Msg::Second, Msg::Zone, "Time")?;
    /// let picker = Widget::time_picker(false, TimeOfDay::new(9, 30, 0)?, config, Style::default(), Style::default());
    /// assert!(matches!(picker, Widget::Popover { .. }));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn time_picker(
        open: bool,
        time: TimeOfDay,
        config: TimePickerConfig<'a, Msg>,
        style: Style,
        popup_style: Style,
    ) -> Self {
        let content = time_picker_content(time, config.content_props());
        let (anchor, on_dismiss) = config.into_anchor(time, style.clone());
        Self::popover(open, anchor, content, Some(on_dismiss), style, popup_style)
    }
}

fn validate_time_picker_zones(
    time_zone_names: &[&str],
    selected_time_zone: usize,
) -> Result<(), TimePickerError> {
    if time_zone_names.is_empty() {
        return Err(TimePickerError::EmptyTimeZones);
    }
    if selected_time_zone >= time_zone_names.len() {
        return Err(TimePickerError::SelectedTimeZoneOutOfBounds {
            selected: selected_time_zone,
            zone_count: time_zone_names.len(),
        });
    }
    for (index, value) in time_zone_names.iter().enumerate() {
        TimeZone::iana(value).map_err(|error| invalid_time_picker_zone(index, error))?;
    }
    Ok(())
}

fn invalid_time_picker_zone(index: usize, error: TimeZoneError) -> TimePickerError {
    match error {
        TimeZoneError::InvalidIanaName { value } => {
            TimePickerError::InvalidTimeZone { index, value }
        }
        TimeZoneError::InvalidOffsetSeconds { value } => TimePickerError::InvalidTimeZone {
            index,
            value: value.to_string(),
        },
    }
}

fn time_picker_anchor<'a, Msg>(
    time: TimeOfDay,
    time_zone_name: &str,
    on_toggle: Msg,
    accessibility_label: &'a str,
    style: Style,
) -> Widget<'a, Msg> {
    Widget::ButtonContent {
        label: accessibility_label,
        child: Box::new(time_picker_anchor_value(format!("{time} {time_zone_name}"))),
        on_press: on_toggle,
        style,
        color: None,
        variant: ButtonVariant::Ghost,
    }
}

fn time_picker_anchor_value<'a, Msg>(display: String) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Text {
            content: display,
            style: fill_parent_style(),
            color: None,
            size: 14.0,
        }),
        style: time_picker_anchor_value_style(),
        color: None,
        radius: 0.0,
    }
}

fn time_picker_content<'a, Msg>(
    time: TimeOfDay,
    properties: TimePickerContentProps<'a, Msg>,
) -> Widget<'a, Msg> {
    time_picker_content_container(time_picker_content_children(time, &properties))
}

fn time_picker_content_children<'a, Msg>(
    time: TimeOfDay,
    properties: &TimePickerContentProps<'a, Msg>,
) -> Vec<Widget<'a, Msg>> {
    let time_zone = Widget::select(
        properties.time_zone_names,
        properties.selected_time_zone,
        properties.on_time_zone_change,
        time_picker_zone_style(),
        properties.labels.time_zone,
        "",
    );
    vec![
        time_picker_fields(time, properties),
        time_picker_label(properties.labels.time_zone),
        time_zone,
    ]
}

fn time_picker_content_container<'a, Msg>(children: Vec<Widget<'a, Msg>>) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Column {
            children,
            style: time_picker_content_style(),
        }),
        style: fill_parent_style(),
        color: None,
        radius: 8.0,
    }
}

fn time_picker_fields<'a, Msg>(
    time: TimeOfDay,
    properties: &TimePickerContentProps<'a, Msg>,
) -> Widget<'a, Msg> {
    let fields = [
        (
            properties.labels.hour,
            i64::from(time.hour()),
            23,
            properties.on_hour_change,
        ),
        (
            properties.labels.minute,
            i64::from(time.minute()),
            59,
            properties.on_minute_change,
        ),
        (
            properties.labels.second,
            i64::from(time.second()),
            59,
            properties.on_second_change,
        ),
    ];
    Widget::Row {
        children: fields
            .into_iter()
            .map(|(label, value, maximum, on_change)| {
                time_picker_field(label, value, maximum, on_change)
            })
            .collect(),
        style: time_picker_fields_style(),
    }
}

fn time_picker_field<'a, Msg>(
    label: &'a str,
    value: i64,
    maximum: i64,
    on_change: fn(i64) -> Msg,
) -> Widget<'a, Msg> {
    Widget::Column {
        children: vec![
            time_picker_label(label),
            Widget::counter(
                value,
                0,
                maximum,
                1,
                on_change,
                time_picker_counter_style(),
                label,
            ),
        ],
        style: time_picker_field_style(),
    }
}

fn time_picker_label<'a, Msg>(label: &'a str) -> Widget<'a, Msg> {
    Widget::Text {
        content: label.to_owned(),
        style: time_picker_label_style(),
        color: None,
        size: 12.0,
    }
}
