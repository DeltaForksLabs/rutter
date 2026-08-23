// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::{fmt, str::FromStr};

use chrono::FixedOffset;
use chrono_tz::Tz;

/// Reports an invalid named or fixed timezone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeZoneError {
    InvalidIanaName { value: String },
    InvalidOffsetSeconds { value: i32 },
}

impl fmt::Display for TimeZoneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIanaName { value } => write!(
                formatter,
                "invalid timezone value={value:?}; expected an IANA timezone name such as \"UTC\" or \"America/Sao_Paulo\""
            ),
            Self::InvalidOffsetSeconds { value } => write!(
                formatter,
                "invalid timezone offset seconds={value}; expected an offset in -86399..=86399"
            ),
        }
    }
}

impl std::error::Error for TimeZoneError {}

/// A named IANA timezone or explicit fixed UTC offset.
///
/// IANA zones retain daylight-saving rules through `chrono-tz`; fixed offsets
/// deliberately have no daylight-saving transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZone {
    Iana(Tz),
    FixedOffset(FixedOffset),
}

impl TimeZone {
    /// The Coordinated Universal Time timezone.
    pub const UTC: Self = Self::Iana(chrono_tz::UTC);

    /// Parses an IANA timezone identifier.
    ///
    /// ```rust
    /// use rutter::TimeZone;
    ///
    /// assert_eq!(TimeZone::iana("America/Sao_Paulo").unwrap().to_string(), "America/Sao_Paulo");
    /// ```
    pub fn iana(value: &str) -> Result<Self, TimeZoneError> {
        value.parse()
    }

    /// Creates a fixed timezone offset measured east of UTC.
    ///
    /// ```rust
    /// use rutter::TimeZone;
    ///
    /// assert_eq!(TimeZone::fixed_offset(-10_800).unwrap().to_string(), "UTC-03:00");
    /// ```
    pub fn fixed_offset(seconds_east_of_utc: i32) -> Result<Self, TimeZoneError> {
        let Some(offset) = FixedOffset::east_opt(seconds_east_of_utc) else {
            return Err(TimeZoneError::InvalidOffsetSeconds {
                value: seconds_east_of_utc,
            });
        };
        Ok(Self::FixedOffset(offset))
    }

    /// Returns the wrapped IANA zone when this timezone has daylight-saving rules.
    ///
    /// ```rust
    /// use rutter::TimeZone;
    ///
    /// assert!(TimeZone::UTC.as_iana().is_some());
    /// ```
    pub const fn as_iana(self) -> Option<Tz> {
        match self {
            Self::Iana(zone) => Some(zone),
            Self::FixedOffset(_) => None,
        }
    }
}

impl From<Tz> for TimeZone {
    fn from(value: Tz) -> Self {
        Self::Iana(value)
    }
}

impl FromStr for TimeZone {
    type Err = TimeZoneError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let zone = value
            .parse::<Tz>()
            .map_err(|_| TimeZoneError::InvalidIanaName {
                value: value.to_owned(),
            })?;
        Ok(Self::Iana(zone))
    }
}

impl fmt::Display for TimeZone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Iana(zone) => zone.fmt(formatter),
            Self::FixedOffset(offset) => write_fixed_offset(formatter, offset.local_minus_utc()),
        }
    }
}

fn write_fixed_offset(formatter: &mut fmt::Formatter<'_>, seconds: i32) -> fmt::Result {
    let sign = if seconds < 0 { '-' } else { '+' };
    let absolute = seconds.unsigned_abs();
    let hours = absolute / 3_600;
    let minutes = absolute % 3_600 / 60;
    write!(formatter, "UTC{sign}{hours:02}:{minutes:02}")
}

#[cfg(test)]
mod tests {
    use super::{TimeZone, TimeZoneError};

    #[test]
    fn timezone_parses_iana_names_and_formats_fixed_offsets() {
        assert_eq!(
            TimeZone::iana("America/Sao_Paulo").unwrap().to_string(),
            "America/Sao_Paulo"
        );
        assert_eq!(
            TimeZone::fixed_offset(19_800).unwrap().to_string(),
            "UTC+05:30"
        );
    }

    #[test]
    fn timezone_reports_the_invalid_identifier() {
        assert_eq!(
            TimeZone::iana("Mars/Olympus"),
            Err(TimeZoneError::InvalidIanaName {
                value: String::from("Mars/Olympus"),
            })
        );
    }
}
