// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

use chrono::{DateTime, LocalResult, NaiveDate, TimeZone as ChronoTimeZone, Timelike, Utc};
use skia_safe::Color as SkiaColor;

use super::{TimeOfDay, TimeZone};
use crate::widgets::calendar::CalendarDate;

/// Selects whether a clock uses 24-hour or 12-hour notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HourCycle {
    Twelve,
    #[default]
    TwentyFour,
}

/// Controls the text shown by a live clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockFormat {
    hour_cycle: HourCycle,
    show_seconds: bool,
    show_time_zone: bool,
}

impl ClockFormat {
    /// Creates a 24-hour clock format.
    ///
    /// ```rust
    /// use rutter::ClockFormat;
    ///
    /// assert!(ClockFormat::twenty_four_hour().show_seconds());
    /// ```
    pub const fn twenty_four_hour() -> Self {
        Self {
            hour_cycle: HourCycle::TwentyFour,
            show_seconds: true,
            show_time_zone: true,
        }
    }

    /// Creates a 12-hour clock format with an AM/PM suffix.
    ///
    /// ```rust
    /// use rutter::{ClockFormat, HourCycle};
    ///
    /// assert_eq!(ClockFormat::twelve_hour().hour_cycle(), HourCycle::Twelve);
    /// ```
    pub const fn twelve_hour() -> Self {
        Self {
            hour_cycle: HourCycle::Twelve,
            ..Self::twenty_four_hour()
        }
    }

    /// Chooses whether seconds appear in the clock text.
    ///
    /// ```rust
    /// use rutter::ClockFormat;
    ///
    /// assert!(!ClockFormat::twenty_four_hour().with_seconds(false).show_seconds());
    /// ```
    pub const fn with_seconds(mut self, show_seconds: bool) -> Self {
        self.show_seconds = show_seconds;
        self
    }

    /// Chooses whether the timezone identifier appears after the clock text.
    ///
    /// ```rust
    /// use rutter::ClockFormat;
    ///
    /// assert!(!ClockFormat::twenty_four_hour().with_time_zone(false).show_time_zone());
    /// ```
    pub const fn with_time_zone(mut self, show_time_zone: bool) -> Self {
        self.show_time_zone = show_time_zone;
        self
    }

    /// Returns the selected hour cycle.
    pub const fn hour_cycle(self) -> HourCycle {
        self.hour_cycle
    }

    /// Returns whether seconds are shown.
    pub const fn show_seconds(self) -> bool {
        self.show_seconds
    }

    /// Returns whether the timezone identifier is shown.
    pub const fn show_time_zone(self) -> bool {
        self.show_time_zone
    }
}

impl Default for ClockFormat {
    fn default() -> Self {
        Self::twenty_four_hour()
    }
}

/// Reports an invalid clock timestamp or presentation configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockError {
    InvalidUnixTimestamp { seconds: i64, nanoseconds: u32 },
    InvalidFontSize { value: f32 },
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUnixTimestamp {
                seconds,
                nanoseconds,
            } => write!(
                formatter,
                "invalid clock timestamp seconds={seconds}, nanoseconds={nanoseconds}; expected a representable Unix timestamp with nanoseconds 0..=999999999"
            ),
            Self::InvalidFontSize { value } => write!(
                formatter,
                "invalid clock font size={value}; expected a positive finite number of logical pixels"
            ),
        }
    }
}

impl std::error::Error for ClockError {}

/// Rendering settings for a live [`Widget::clock`](crate::Widget::clock).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockConfig {
    format: ClockFormat,
    font_size: f32,
    color: Option<SkiaColor>,
}

impl ClockConfig {
    /// Creates validated clock presentation settings.
    ///
    /// ```rust
    /// use rutter::{ClockConfig, ClockFormat};
    ///
    /// assert_eq!(ClockConfig::new(ClockFormat::twenty_four_hour(), 28.0).unwrap().font_size(), 28.0);
    /// ```
    pub fn new(format: ClockFormat, font_size: f32) -> Result<Self, ClockError> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(ClockError::InvalidFontSize { value: font_size });
        }
        Ok(Self {
            format,
            font_size,
            color: None,
        })
    }

    /// Sets an explicit text color, retaining the theme color when `None`.
    ///
    /// ```rust
    /// use rutter::{ClockConfig, ClockFormat};
    ///
    /// let config = ClockConfig::new(ClockFormat::default(), 28.0).unwrap().with_color(None);
    /// assert_eq!(config.color(), None);
    /// ```
    pub const fn with_color(mut self, color: Option<SkiaColor>) -> Self {
        self.color = color;
        self
    }

    /// Returns the clock text format.
    pub const fn format(self) -> ClockFormat {
        self.format
    }

    /// Returns the clock font size in logical pixels.
    pub const fn font_size(self) -> f32 {
        self.font_size
    }

    /// Returns the optional explicit text color.
    pub const fn color(self) -> Option<SkiaColor> {
        self.color
    }
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            format: ClockFormat::default(),
            font_size: 32.0,
            color: None,
        }
    }
}

/// An instant displayed in a specific timezone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockTime {
    utc: DateTime<Utc>,
    time_zone: TimeZone,
}

impl ClockTime {
    /// Captures the current instant for a timezone.
    ///
    /// ```rust
    /// use rutter::{ClockTime, TimeZone};
    ///
    /// assert_eq!(ClockTime::now(TimeZone::UTC).time_zone(), TimeZone::UTC);
    /// ```
    pub fn now(time_zone: TimeZone) -> Self {
        Self {
            utc: Utc::now(),
            time_zone,
        }
    }

    /// Creates a clock instant from a Unix timestamp.
    ///
    /// ```rust
    /// use rutter::{ClockTime, TimeZone};
    ///
    /// assert_eq!(ClockTime::from_unix_timestamp(0, 0, TimeZone::UTC).unwrap().time_of_day().to_string(), "00:00:00");
    /// ```
    pub fn from_unix_timestamp(
        seconds: i64,
        nanoseconds: u32,
        time_zone: TimeZone,
    ) -> Result<Self, ClockError> {
        let Some(utc) = DateTime::from_timestamp(seconds, nanoseconds) else {
            return Err(ClockError::InvalidUnixTimestamp {
                seconds,
                nanoseconds,
            });
        };
        Ok(Self { utc, time_zone })
    }

    /// Returns this instant's clock time in the selected timezone.
    ///
    /// ```rust
    /// use rutter::{ClockTime, TimeZone};
    ///
    /// let time = ClockTime::from_unix_timestamp(0, 0, TimeZone::fixed_offset(3_600).unwrap()).unwrap();
    /// assert_eq!(time.time_of_day().to_string(), "01:00:00");
    /// ```
    pub fn time_of_day(&self) -> TimeOfDay {
        match self.time_zone {
            TimeZone::Iana(zone) => time_of_day_from(self.utc.with_timezone(&zone)),
            TimeZone::FixedOffset(offset) => time_of_day_from(self.utc.with_timezone(&offset)),
        }
    }

    /// Returns the timezone used to display this instant.
    pub const fn time_zone(&self) -> TimeZone {
        self.time_zone
    }

    /// Returns whole seconds since the Unix epoch.
    pub const fn unix_timestamp(&self) -> i64 {
        self.utc.timestamp()
    }

    /// Formats this instant using the requested clock presentation.
    ///
    /// ```rust
    /// use rutter::{ClockFormat, ClockTime, TimeZone};
    ///
    /// let time = ClockTime::from_unix_timestamp(0, 0, TimeZone::UTC).unwrap();
    /// assert_eq!(time.format(ClockFormat::default()), "00:00:00 UTC");
    /// ```
    pub fn format(&self, format: ClockFormat) -> String {
        format_time(self.time_of_day(), self.time_zone, format)
    }
}

/// Resolves a wall-clock time against a calendar date and timezone.
///
/// Daylight-saving changes can make a local time ambiguous or nonexistent, so
/// callers must handle all variants instead of silently changing the instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalTimeResolution {
    Single(ClockTime),
    Ambiguous {
        earlier: ClockTime,
        later: ClockTime,
    },
    Nonexistent,
}

impl TimeOfDay {
    /// Resolves this wall-clock time for a date and timezone.
    ///
    /// ```rust
    /// use rutter::{CalendarDate, LocalTimeResolution, TimeOfDay, TimeZone};
    ///
    /// let resolution = TimeOfDay::new(12, 0, 0).unwrap()
    ///     .resolve(CalendarDate::new(2026, 7, 1).unwrap(), TimeZone::UTC);
    /// assert!(matches!(resolution, LocalTimeResolution::Single(_)));
    /// ```
    pub fn resolve(self, date: CalendarDate, time_zone: TimeZone) -> LocalTimeResolution {
        let Some(local_date) =
            NaiveDate::from_ymd_opt(date.year(), u32::from(date.month()), u32::from(date.day()))
        else {
            return LocalTimeResolution::Nonexistent;
        };
        let Some(local) = local_date.and_hms_opt(
            u32::from(self.hour()),
            u32::from(self.minute()),
            u32::from(self.second()),
        ) else {
            return LocalTimeResolution::Nonexistent;
        };
        match time_zone {
            TimeZone::Iana(zone) => {
                resolve_local_result(zone.from_local_datetime(&local), time_zone)
            }
            TimeZone::FixedOffset(offset) => {
                resolve_local_result(offset.from_local_datetime(&local), time_zone)
            }
        }
    }
}

pub(crate) fn clock_layout_text(time_zone: TimeZone, format: ClockFormat) -> String {
    format_time(
        TimeOfDay::new(23, 59, 59).expect("23:59:59 must be a valid clock time"),
        time_zone,
        format,
    )
}

pub(crate) fn current_clock_text(time_zone: TimeZone, format: ClockFormat) -> String {
    ClockTime::now(time_zone).format(format)
}

fn format_time(time: TimeOfDay, time_zone: TimeZone, format: ClockFormat) -> String {
    let time_text = match format.hour_cycle() {
        HourCycle::TwentyFour => format_twenty_four_hour(time, format.show_seconds()),
        HourCycle::Twelve => format_twelve_hour(time, format.show_seconds()),
    };
    if !format.show_time_zone() {
        return time_text;
    }
    format!("{time_text} {time_zone}")
}

fn format_twenty_four_hour(time: TimeOfDay, show_seconds: bool) -> String {
    if show_seconds {
        return time.to_string();
    }
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn format_twelve_hour(time: TimeOfDay, show_seconds: bool) -> String {
    let suffix = if time.hour() < 12 { "AM" } else { "PM" };
    let hour = match time.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    if show_seconds {
        return format!("{hour}:{:02}:{:02} {suffix}", time.minute(), time.second());
    }
    format!("{hour}:{:02} {suffix}", time.minute())
}

fn time_of_day_from<T: ChronoTimeZone>(date_time: DateTime<T>) -> TimeOfDay {
    TimeOfDay::new(
        date_time.hour() as u8,
        date_time.minute() as u8,
        date_time.second() as u8,
    )
    .expect("chrono time components must form a valid clock time")
}

fn resolve_local_result<T: ChronoTimeZone>(
    result: LocalResult<DateTime<T>>,
    time_zone: TimeZone,
) -> LocalTimeResolution {
    match result {
        LocalResult::Single(value) => LocalTimeResolution::Single(ClockTime {
            utc: value.with_timezone(&Utc),
            time_zone,
        }),
        LocalResult::Ambiguous(earlier, later) => LocalTimeResolution::Ambiguous {
            earlier: ClockTime {
                utc: earlier.with_timezone(&Utc),
                time_zone,
            },
            later: ClockTime {
                utc: later.with_timezone(&Utc),
                time_zone,
            },
        },
        LocalResult::None => LocalTimeResolution::Nonexistent,
    }
}

#[cfg(test)]
mod tests {
    use super::{ClockConfig, ClockError, ClockFormat, ClockTime, LocalTimeResolution};
    use crate::{CalendarDate, TimeOfDay, TimeZone};

    #[test]
    fn clock_formats_fixed_offset_time_in_both_hour_cycles() {
        let clock =
            ClockTime::from_unix_timestamp(0, 0, TimeZone::fixed_offset(-10_800).unwrap()).unwrap();

        assert_eq!(clock.format(ClockFormat::default()), "21:00:00 UTC-03:00");
        assert_eq!(
            clock.format(ClockFormat::twelve_hour().with_time_zone(false)),
            "9:00:00 PM"
        );
    }

    #[test]
    fn clock_config_rejects_non_positive_font_sizes() {
        assert_eq!(
            ClockConfig::new(ClockFormat::default(), 0.0),
            Err(ClockError::InvalidFontSize { value: 0.0 })
        );
    }

    #[test]
    fn local_time_resolution_exposes_dst_gap_and_overlap() {
        let zone = TimeZone::iana("America/New_York").unwrap();
        let spring_gap = TimeOfDay::new(2, 30, 0)
            .unwrap()
            .resolve(CalendarDate::new(2026, 3, 8).unwrap(), zone);
        let autumn_overlap = TimeOfDay::new(1, 30, 0)
            .unwrap()
            .resolve(CalendarDate::new(2026, 11, 1).unwrap(), zone);

        assert_eq!(spring_gap, LocalTimeResolution::Nonexistent);
        assert!(matches!(
            autumn_overlap,
            LocalTimeResolution::Ambiguous { .. }
        ));
    }
}
