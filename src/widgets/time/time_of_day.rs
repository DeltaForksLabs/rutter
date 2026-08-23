// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

/// Reports an invalid wall-clock time and its expected shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDayError {
    InvalidTime { hour: i64, minute: i64, second: i64 },
}

impl fmt::Display for TimeOfDayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTime {
                hour,
                minute,
                second,
            } => write!(
                formatter,
                "invalid time hour={hour}, minute={minute}, second={second}; expected HH:MM:SS with hour 00..=23 and minute/second 00..=59"
            ),
        }
    }
}

impl std::error::Error for TimeOfDayError {}

/// A validated 24-hour wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
    second: u8,
}

impl TimeOfDay {
    /// Creates a validated 24-hour time.
    ///
    /// ```rust
    /// use rutter::TimeOfDay;
    ///
    /// assert_eq!(TimeOfDay::new(9, 5, 3).unwrap().to_string(), "09:05:03");
    /// ```
    pub fn new(hour: u8, minute: u8, second: u8) -> Result<Self, TimeOfDayError> {
        Self::from_counter_values(i64::from(hour), i64::from(minute), i64::from(second))
    }

    /// Converts Counter callback values into a validated time.
    ///
    /// ```rust
    /// use rutter::TimeOfDay;
    ///
    /// assert_eq!(TimeOfDay::from_counter_values(23, 59, 59).unwrap().hour(), 23);
    /// ```
    pub fn from_counter_values(
        hour: i64,
        minute: i64,
        second: i64,
    ) -> Result<Self, TimeOfDayError> {
        if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=59).contains(&second) {
            return Err(TimeOfDayError::InvalidTime {
                hour,
                minute,
                second,
            });
        }
        Ok(Self {
            hour: hour as u8,
            minute: minute as u8,
            second: second as u8,
        })
    }

    /// Returns the hour in 24-hour notation.
    ///
    /// ```rust
    /// # use rutter::TimeOfDay;
    /// assert_eq!(TimeOfDay::new(17, 0, 0).unwrap().hour(), 17);
    /// ```
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// Returns the minute in the range `0..=59`.
    ///
    /// ```rust
    /// # use rutter::TimeOfDay;
    /// assert_eq!(TimeOfDay::new(17, 42, 0).unwrap().minute(), 42);
    /// ```
    pub const fn minute(self) -> u8 {
        self.minute
    }

    /// Returns the second in the range `0..=59`.
    ///
    /// ```rust
    /// # use rutter::TimeOfDay;
    /// assert_eq!(TimeOfDay::new(17, 42, 9).unwrap().second(), 9);
    /// ```
    pub const fn second(self) -> u8 {
        self.second
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:02}:{:02}:{:02}",
            self.hour, self.minute, self.second
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{TimeOfDay, TimeOfDayError};

    #[test]
    fn time_of_day_accepts_both_day_boundaries() {
        assert_eq!(TimeOfDay::new(0, 0, 0).unwrap().to_string(), "00:00:00");
        assert_eq!(TimeOfDay::new(23, 59, 59).unwrap().to_string(), "23:59:59");
    }

    #[test]
    fn counter_values_reject_out_of_range_time_parts() {
        assert_eq!(
            TimeOfDay::from_counter_values(24, 0, 0),
            Err(TimeOfDayError::InvalidTime {
                hour: 24,
                minute: 0,
                second: 0,
            })
        );
    }
}
