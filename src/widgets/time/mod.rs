// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Timezone-aware clock values and composed time-picker controls.

mod clock;
mod labels;
mod styles;
mod time_of_day;
mod time_zone;
mod widgets;

pub use clock::{ClockConfig, ClockError, ClockFormat, ClockTime, HourCycle, LocalTimeResolution};
pub use labels::TimePickerLabels;
pub use time_of_day::{TimeOfDay, TimeOfDayError};
pub use time_zone::{TimeZone, TimeZoneError};
pub use widgets::{TimePickerConfig, TimePickerError};

pub(crate) use clock::{clock_layout_text, current_clock_text};

#[cfg(test)]
mod widget_tests;
