// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{CalendarDate, CalendarLabels, CalendarMonth, WeekStart};

pub(crate) const CALENDAR_COLUMN_COUNT: usize = 7;
pub(crate) const CALENDAR_CELL_COUNT: usize = 42;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CalendarGridCell {
    pub date: Option<CalendarDate>,
    pub is_visible_month: bool,
}

pub(crate) fn calendar_grid(
    visible_month: CalendarMonth,
    week_start: WeekStart,
) -> [CalendarGridCell; CALENDAR_CELL_COUNT] {
    let first = visible_month.first_date();
    let leading = first
        .sunday_weekday_index()
        .wrapping_add(CALENDAR_COLUMN_COUNT)
        .wrapping_sub(week_start.sunday_index())
        % CALENDAR_COLUMN_COUNT;
    std::array::from_fn(|slot| grid_cell(first, visible_month, slot as i32 - leading as i32))
}

fn grid_cell(
    first: CalendarDate,
    visible_month: CalendarMonth,
    day_delta: i32,
) -> CalendarGridCell {
    let date = first.shifted(day_delta);
    CalendarGridCell {
        date,
        is_visible_month: date.is_some_and(|date| date.calendar_month() == visible_month),
    }
}

pub(crate) fn weekday_label<'a>(
    labels: CalendarLabels<'a>,
    week_start: WeekStart,
    column: usize,
) -> &'a str {
    let sunday_index = (week_start.sunday_index() + column) % CALENDAR_COLUMN_COUNT;
    labels.weekdays_sunday_first[sunday_index]
}

pub(crate) const fn day_number_label(day: u8) -> &'static str {
    const DAY_LABELS: [&str; 31] = [
        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
    ];
    DAY_LABELS[day.saturating_sub(1) as usize]
}

#[cfg(test)]
mod tests {
    use super::{calendar_grid, weekday_label};
    use crate::calendar::{CalendarDate, CalendarLabels, CalendarMonth, WeekStart};

    #[test]
    fn calendar_grid_is_stable_and_includes_adjacent_month_dates() {
        let july = CalendarMonth::new(2026, 7).unwrap();
        let grid = calendar_grid(july, WeekStart::Monday);

        assert_eq!(grid.len(), 42);
        assert_eq!(grid[2].date, Some(CalendarDate::new(2026, 7, 1).unwrap()));
        assert_eq!(grid[41].date, Some(CalendarDate::new(2026, 8, 9).unwrap()));
        assert!(!grid[0].is_visible_month);
        assert!(grid[2].is_visible_month);
    }

    #[test]
    fn calendar_grid_uses_blank_cells_beyond_supported_year_boundaries() {
        let first_month = CalendarMonth::new(1, 1).unwrap();
        let last_month = CalendarMonth::new(9999, 12).unwrap();

        assert!(
            calendar_grid(first_month, WeekStart::Sunday)[0]
                .date
                .is_none()
        );
        assert!(
            calendar_grid(last_month, WeekStart::Monday)
                .iter()
                .any(|cell| cell.date.is_none())
        );
    }

    #[test]
    fn weekday_headings_rotate_from_the_configured_start() {
        assert_eq!(
            weekday_label(CalendarLabels::ENGLISH, WeekStart::Monday, 0),
            "Mon"
        );
        assert_eq!(
            weekday_label(CalendarLabels::PORTUGUESE, WeekStart::Sunday, 6),
            "Sáb"
        );
    }
}
