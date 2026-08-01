// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

/// Selects which weekday occupies the first calendar column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WeekStart {
    Sunday,
    #[default]
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl WeekStart {
    /// Returns this weekday's zero-based index in Sunday-first order.
    ///
    /// ```rust
    /// use rutter::WeekStart;
    ///
    /// assert_eq!(WeekStart::Monday.sunday_index(), 1);
    /// ```
    pub const fn sunday_index(self) -> usize {
        match self {
            Self::Sunday => 0,
            Self::Monday => 1,
            Self::Tuesday => 2,
            Self::Wednesday => 3,
            Self::Thursday => 4,
            Self::Friday => 5,
            Self::Saturday => 6,
        }
    }
}

/// Localized labels used by composed calendar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarLabels<'a> {
    pub months: [&'a str; 12],
    pub weekdays_sunday_first: [&'a str; 7],
    pub previous_month: &'a str,
    pub next_month: &'a str,
    pub previous_year: &'a str,
    pub next_year: &'a str,
}

impl CalendarLabels<'static> {
    /// Built-in English calendar labels.
    pub const ENGLISH: Self = Self {
        months: [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ],
        weekdays_sunday_first: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        previous_month: "Previous month",
        next_month: "Next month",
        previous_year: "Previous year",
        next_year: "Next year",
    };

    /// Built-in Brazilian Portuguese calendar labels.
    pub const PORTUGUESE: Self = Self {
        months: [
            "Janeiro",
            "Fevereiro",
            "Março",
            "Abril",
            "Maio",
            "Junho",
            "Julho",
            "Agosto",
            "Setembro",
            "Outubro",
            "Novembro",
            "Dezembro",
        ],
        weekdays_sunday_first: ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"],
        previous_month: "Mês anterior",
        next_month: "Próximo mês",
        previous_year: "Ano anterior",
        next_year: "Próximo ano",
    };
}

impl Default for CalendarLabels<'static> {
    fn default() -> Self {
        Self::ENGLISH
    }
}

/// Configures labels and the first weekday for a calendar widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarConfig<'a> {
    pub labels: CalendarLabels<'a>,
    pub week_start: WeekStart,
}

impl<'a> CalendarConfig<'a> {
    /// Creates calendar presentation settings.
    ///
    /// ```rust
    /// use rutter::{CalendarConfig, CalendarLabels, WeekStart};
    ///
    /// let config = CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday);
    /// assert_eq!(config.labels.months[6], "Julho");
    /// ```
    pub const fn new(labels: CalendarLabels<'a>, week_start: WeekStart) -> Self {
        Self { labels, week_start }
    }
}

impl Default for CalendarConfig<'static> {
    fn default() -> Self {
        Self {
            labels: CalendarLabels::ENGLISH,
            week_start: WeekStart::Monday,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CalendarConfig, CalendarLabels, WeekStart};

    #[test]
    fn built_in_labels_and_week_starts_are_explicit() {
        let config = CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Sunday);

        assert_eq!(config.labels.months[6], "Julho");
        assert_eq!(config.labels.weekdays_sunday_first[0], "Dom");
        assert_eq!(config.week_start.sunday_index(), 0);
        assert_eq!(CalendarConfig::default().week_start, WeekStart::Monday);
    }
}
