// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Validated Gregorian values and composed calendar/date-picker widgets.

mod date;
mod grid;
mod labels;
mod styles;
mod widgets;

pub use date::{CalendarDate, CalendarError, CalendarMonth};
pub use labels::{CalendarConfig, CalendarLabels, WeekStart};

pub(crate) use grid::{CalendarGridCell, calendar_grid, day_number_label, weekday_label};

#[cfg(test)]
mod widget_tests;
