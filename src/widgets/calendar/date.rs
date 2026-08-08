// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

pub(crate) const MIN_CALENDAR_YEAR: i32 = 1;
pub(crate) const MAX_CALENDAR_YEAR: i32 = 9999;

/// Reports an invalid Gregorian calendar value and the expected shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarError {
    InvalidDate { year: i32, month: u8, day: u8 },
    InvalidMonth { year: i32, month: u8 },
}

impl fmt::Display for CalendarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDate { year, month, day } => write!(
                formatter,
                "invalid calendar date year={year}, month={month}, day={day}; expected Gregorian YYYY-MM-DD with year 0001..=9999, month 01..=12, and a day valid for that month"
            ),
            Self::InvalidMonth { year, month } => write!(
                formatter,
                "invalid calendar month year={year}, month={month}; expected Gregorian YYYY-MM with year 0001..=9999 and month 01..=12"
            ),
        }
    }
}

impl std::error::Error for CalendarError {}

/// A validated date in the proleptic Gregorian calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CalendarDate {
    /// Creates a checked Gregorian date.
    ///
    /// ```rust
    /// use rutter::CalendarDate;
    ///
    /// let date = CalendarDate::new(2026, 7, 31).unwrap();
    /// assert_eq!(date.to_string(), "2026-07-31");
    /// ```
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, CalendarError> {
        let valid_month = valid_year(year) && (1..=12).contains(&month);
        if !valid_month || day == 0 || day > gregorian_month_days(year, month) {
            return Err(CalendarError::InvalidDate { year, month, day });
        }
        Ok(Self { year, month, day })
    }

    /// Returns the four-digit Gregorian year.
    ///
    /// ```rust
    /// # use rutter::CalendarDate;
    /// assert_eq!(CalendarDate::new(2026, 7, 31).unwrap().year(), 2026);
    /// ```
    pub const fn year(self) -> i32 {
        self.year
    }

    /// Returns the one-based month number.
    ///
    /// ```rust
    /// # use rutter::CalendarDate;
    /// assert_eq!(CalendarDate::new(2026, 7, 31).unwrap().month(), 7);
    /// ```
    pub const fn month(self) -> u8 {
        self.month
    }

    /// Returns the one-based day of month.
    ///
    /// ```rust
    /// # use rutter::CalendarDate;
    /// assert_eq!(CalendarDate::new(2026, 7, 31).unwrap().day(), 31);
    /// ```
    pub const fn day(self) -> u8 {
        self.day
    }

    /// Returns the containing validated month.
    ///
    /// ```rust
    /// # use rutter::{CalendarDate, CalendarMonth};
    /// let date = CalendarDate::new(2026, 7, 31).unwrap();
    /// assert_eq!(date.calendar_month(), CalendarMonth::new(2026, 7).unwrap());
    /// ```
    pub const fn calendar_month(self) -> CalendarMonth {
        CalendarMonth {
            year: self.year,
            month: self.month,
        }
    }

    /// Reports whether this date's year is a Gregorian leap year.
    ///
    /// ```rust
    /// # use rutter::CalendarDate;
    /// assert!(CalendarDate::new(2024, 2, 29).unwrap().is_leap_year());
    /// ```
    pub const fn is_leap_year(self) -> bool {
        gregorian_leap_year(self.year)
    }

    /// Returns the number of days in this date's month.
    ///
    /// ```rust
    /// # use rutter::CalendarDate;
    /// assert_eq!(CalendarDate::new(2024, 2, 1).unwrap().days_in_month(), 29);
    /// ```
    pub const fn days_in_month(self) -> u8 {
        gregorian_month_days(self.year, self.month)
    }

    pub(crate) fn shifted(self, day_delta: i32) -> Option<Self> {
        let mut shifted = self;
        for _ in 0..day_delta.unsigned_abs() {
            shifted = if day_delta >= 0 {
                shifted.next_day()?
            } else {
                shifted.previous_day()?
            };
        }
        Some(shifted)
    }

    pub(crate) const fn sunday_weekday_index(self) -> usize {
        let elapsed = days_before_year(self.year)
            + days_before_month(self.year, self.month)
            + self.day as i64
            - 1;
        ((elapsed + 1) % 7) as usize
    }

    fn next_day(self) -> Option<Self> {
        if self.day < self.days_in_month() {
            return Some(Self {
                day: self.day + 1,
                ..self
            });
        }
        let month = self.calendar_month().next()?;
        Some(month.first_date())
    }

    fn previous_day(self) -> Option<Self> {
        if self.day > 1 {
            return Some(Self {
                day: self.day - 1,
                ..self
            });
        }
        let month = self.calendar_month().previous()?;
        Some(Self {
            day: month.day_count(),
            year: month.year,
            month: month.month,
        })
    }
}

impl fmt::Display for CalendarDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// A validated year and month in the proleptic Gregorian calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarMonth {
    year: i32,
    month: u8,
}

impl CalendarMonth {
    /// Creates a checked Gregorian month.
    ///
    /// ```rust
    /// use rutter::CalendarMonth;
    ///
    /// let month = CalendarMonth::new(2026, 7).unwrap();
    /// assert_eq!(month.to_string(), "2026-07");
    /// ```
    pub fn new(year: i32, month: u8) -> Result<Self, CalendarError> {
        if !valid_year(year) || !(1..=12).contains(&month) {
            return Err(CalendarError::InvalidMonth { year, month });
        }
        Ok(Self { year, month })
    }

    /// Returns the four-digit Gregorian year.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// assert_eq!(CalendarMonth::new(2026, 7).unwrap().year(), 2026);
    /// ```
    pub const fn year(self) -> i32 {
        self.year
    }

    /// Returns the one-based month number.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// assert_eq!(CalendarMonth::new(2026, 7).unwrap().month(), 7);
    /// ```
    pub const fn month(self) -> u8 {
        self.month
    }

    /// Returns the first date in this month.
    ///
    /// ```rust
    /// # use rutter::{CalendarDate, CalendarMonth};
    /// let month = CalendarMonth::new(2026, 7).unwrap();
    /// assert_eq!(month.first_date(), CalendarDate::new(2026, 7, 1).unwrap());
    /// ```
    pub const fn first_date(self) -> CalendarDate {
        CalendarDate {
            year: self.year,
            month: self.month,
            day: 1,
        }
    }

    /// Returns the number of days in this month.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// assert_eq!(CalendarMonth::new(2024, 2).unwrap().day_count(), 29);
    /// ```
    pub const fn day_count(self) -> u8 {
        gregorian_month_days(self.year, self.month)
    }

    /// Returns the previous month, or `None` before January 0001.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// let january = CalendarMonth::new(2026, 1).unwrap();
    /// assert_eq!(january.previous().unwrap().to_string(), "2025-12");
    /// ```
    pub const fn previous(self) -> Option<Self> {
        if self.month > 1 {
            return Some(Self {
                month: self.month - 1,
                ..self
            });
        }
        if self.year > MIN_CALENDAR_YEAR {
            return Some(Self {
                year: self.year - 1,
                month: 12,
            });
        }
        None
    }

    /// Returns the next month, or `None` after December 9999.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// let december = CalendarMonth::new(2025, 12).unwrap();
    /// assert_eq!(december.next().unwrap().to_string(), "2026-01");
    /// ```
    pub const fn next(self) -> Option<Self> {
        if self.month < 12 {
            return Some(Self {
                month: self.month + 1,
                ..self
            });
        }
        if self.year < MAX_CALENDAR_YEAR {
            return Some(Self {
                year: self.year + 1,
                month: 1,
            });
        }
        None
    }

    /// Returns the same month in the previous year when representable.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// let month = CalendarMonth::new(2026, 7).unwrap();
    /// assert_eq!(month.previous_year().unwrap().year(), 2025);
    /// ```
    pub const fn previous_year(self) -> Option<Self> {
        if self.year == MIN_CALENDAR_YEAR {
            return None;
        }
        Some(Self {
            year: self.year - 1,
            ..self
        })
    }

    /// Returns the same month in the next year when representable.
    ///
    /// ```rust
    /// # use rutter::CalendarMonth;
    /// let month = CalendarMonth::new(2026, 7).unwrap();
    /// assert_eq!(month.next_year().unwrap().year(), 2027);
    /// ```
    pub const fn next_year(self) -> Option<Self> {
        if self.year == MAX_CALENDAR_YEAR {
            return None;
        }
        Some(Self {
            year: self.year + 1,
            ..self
        })
    }
}

impl From<CalendarDate> for CalendarMonth {
    fn from(date: CalendarDate) -> Self {
        date.calendar_month()
    }
}

impl fmt::Display for CalendarMonth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:04}-{:02}", self.year, self.month)
    }
}

const fn valid_year(year: i32) -> bool {
    year >= MIN_CALENDAR_YEAR && year <= MAX_CALENDAR_YEAR
}

const fn gregorian_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn gregorian_month_days(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if gregorian_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

const fn days_before_year(year: i32) -> i64 {
    let previous = year as i64 - 1;
    previous * 365 + previous / 4 - previous / 100 + previous / 400
}

const fn days_before_month(year: i32, month: u8) -> i64 {
    let ordinary = [0_i64, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let elapsed = ordinary[month as usize];
    if month > 2 && gregorian_leap_year(year) {
        elapsed + 1
    } else {
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::{CalendarDate, CalendarMonth};

    #[test]
    fn calendar_date_validates_leap_years_and_reports_offending_values() {
        assert!(CalendarDate::new(2000, 2, 29).is_ok());
        assert!(CalendarDate::new(2024, 2, 29).is_ok());
        for year in [1900, 2023, 2100] {
            let error = CalendarDate::new(year, 2, 29).unwrap_err().to_string();
            assert!(error.contains(&format!("year={year}, month=2, day=29")));
            assert!(error.contains("YYYY-MM-DD"));
        }
    }

    #[test]
    fn calendar_date_formats_iso_and_exposes_components() {
        let date = CalendarDate::new(42, 3, 7).unwrap();

        assert_eq!((date.year(), date.month(), date.day()), (42, 3, 7));
        assert_eq!(date.to_string(), "0042-03-07");
        assert_eq!(date.days_in_month(), 31);
        assert!(!date.is_leap_year());
    }

    #[test]
    fn calendar_month_navigation_crosses_year_boundaries() {
        let january = CalendarMonth::new(2026, 1).unwrap();
        let december = CalendarMonth::new(2025, 12).unwrap();

        assert_eq!(january.previous(), Some(december));
        assert_eq!(december.next(), Some(january));
        assert_eq!(january.previous_year().unwrap().year(), 2025);
        assert_eq!(january.next_year().unwrap().year(), 2027);
        assert_eq!(january.first_date().day(), 1);
    }

    #[test]
    fn shifted_dates_preserve_gregorian_month_lengths() {
        let leap_day = CalendarDate::new(2024, 2, 28).unwrap().shifted(1).unwrap();
        let march = leap_day.shifted(1).unwrap();

        assert_eq!(leap_day, CalendarDate::new(2024, 2, 29).unwrap());
        assert_eq!(march, CalendarDate::new(2024, 3, 1).unwrap());
        assert_eq!(
            march.shifted(-2),
            Some(CalendarDate::new(2024, 2, 28).unwrap())
        );
    }

    #[test]
    fn known_weekdays_use_sunday_first_indices() {
        assert_eq!(
            CalendarDate::new(2026, 7, 31)
                .unwrap()
                .sunday_weekday_index(),
            5
        );
        assert_eq!(
            CalendarDate::new(2000, 1, 1)
                .unwrap()
                .sunday_weekday_index(),
            6
        );
    }
}
