// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use taffy::prelude::Style;

use super::styles::{
    CALENDAR_DAY_HEIGHT, CALENDAR_WEEKDAY_HEIGHT, calendar_cell_style, calendar_content_style,
    calendar_header_style, calendar_heading_content_style, calendar_heading_style,
    calendar_label_text_style, calendar_row_style, calendar_weekday_cell_style,
    date_picker_value_style, fill_parent_style, navigation_button_style, navigation_icon_style,
};
use super::{
    CalendarConfig, CalendarDate, CalendarGridCell, CalendarMonth, calendar_grid, day_number_label,
    weekday_label,
};
use crate::widget::{ButtonVariant, Widget};
use crate::widgets::rich_text::{RichText, RichTextSpan};

impl<'a, Msg> Widget<'a, Msg> {
    /// Creates a standalone single-date calendar with English labels.
    ///
    /// The visible month and selected date remain controlled by application state.
    ///
    /// ```rust
    /// use rutter::{CalendarDate, CalendarMonth, Widget};
    /// use taffy::prelude::Style;
    ///
    /// #[derive(Clone)]
    /// enum Msg { Select(CalendarDate), Navigate(CalendarMonth) }
    /// let calendar = Widget::calendar(
    ///     CalendarMonth::new(2026, 7).unwrap(),
    ///     None,
    ///     Msg::Select,
    ///     Msg::Navigate,
    ///     Style::default(),
    /// );
    /// assert!(matches!(calendar, Widget::Container { .. }));
    /// ```
    pub fn calendar(
        visible_month: CalendarMonth,
        selected: Option<CalendarDate>,
        on_select: fn(CalendarDate) -> Msg,
        on_month_change: fn(CalendarMonth) -> Msg,
        style: Style,
    ) -> Self {
        Self::calendar_with_config(
            visible_month,
            selected,
            on_select,
            on_month_change,
            CalendarConfig::default(),
            style,
        )
    }

    /// Creates a standalone calendar with explicit localization and week start.
    ///
    /// ```rust
    /// use rutter::{CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, WeekStart, Widget};
    /// use taffy::prelude::Style;
    ///
    /// #[derive(Clone)]
    /// enum Msg { Select(CalendarDate), Navigate(CalendarMonth) }
    /// let config = CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday);
    /// let calendar = Widget::calendar_with_config(
    ///     CalendarMonth::new(2026, 7).unwrap(), None,
    ///     Msg::Select, Msg::Navigate, config, Style::default(),
    /// );
    /// assert!(matches!(calendar, Widget::Container { .. }));
    /// ```
    pub fn calendar_with_config(
        visible_month: CalendarMonth,
        selected: Option<CalendarDate>,
        on_select: fn(CalendarDate) -> Msg,
        on_month_change: fn(CalendarMonth) -> Msg,
        config: CalendarConfig<'a>,
        style: Style,
    ) -> Self {
        let children =
            calendar_sections(visible_month, selected, on_select, on_month_change, config);
        Self::Container {
            child: Box::new(Self::Column {
                children,
                style: calendar_content_style(),
            }),
            style,
            color: None,
            radius: 8.0,
        }
    }

    /// Creates a controlled date picker backed by the same calendar widget.
    ///
    /// The caller toggles `open`, updates `visible_month`, and closes the picker
    /// after receiving a selected date. `accessibility_label` should include the
    /// current value when assistive technology must announce it.
    ///
    /// ```rust
    /// use rutter::{CalendarDate, CalendarMonth, Widget};
    /// use taffy::prelude::Style;
    /// #[derive(Clone)]
    /// enum Msg { Toggle, Close, Select(CalendarDate), Navigate(CalendarMonth) }
    /// let picker = Widget::date_picker(
    ///     false, CalendarMonth::new(2026, 7).unwrap(), None,
    ///     Msg::Toggle, Msg::Close, Msg::Select, Msg::Navigate,
    ///     "Date", "YYYY-MM-DD", Style::default(), Style::default(),
    /// );
    /// assert!(matches!(picker, Widget::Popover { .. }));
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn date_picker(
        open: bool,
        visible_month: CalendarMonth,
        selected: Option<CalendarDate>,
        on_toggle: Msg,
        on_dismiss: Msg,
        on_select: fn(CalendarDate) -> Msg,
        on_month_change: fn(CalendarMonth) -> Msg,
        accessibility_label: &'a str,
        placeholder: &'a str,
        style: Style,
        popup_style: Style,
    ) -> Self {
        Self::date_picker_with_config(
            open,
            visible_month,
            selected,
            on_toggle,
            on_dismiss,
            on_select,
            on_month_change,
            accessibility_label,
            placeholder,
            CalendarConfig::default(),
            style,
            popup_style,
        )
    }

    /// Creates a localized controlled date picker in an anchored popover.
    ///
    /// ```rust
    /// use rutter::{CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, WeekStart, Widget};
    /// use taffy::prelude::Style;
    ///
    /// #[derive(Clone)]
    /// enum Msg { Toggle, Close, Select(CalendarDate), Navigate(CalendarMonth) }
    /// let picker = Widget::date_picker_with_config(
    ///     false, CalendarMonth::new(2026, 7).unwrap(), None,
    ///     Msg::Toggle, Msg::Close, Msg::Select, Msg::Navigate,
    ///     "Date", "YYYY-MM-DD",
    ///     CalendarConfig::new(CalendarLabels::ENGLISH, WeekStart::Monday),
    ///     Style::default(), Style::default(),
    /// );
    /// assert!(matches!(picker, Widget::Popover { .. }));
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn date_picker_with_config(
        open: bool,
        visible_month: CalendarMonth,
        selected: Option<CalendarDate>,
        on_toggle: Msg,
        on_dismiss: Msg,
        on_select: fn(CalendarDate) -> Msg,
        on_month_change: fn(CalendarMonth) -> Msg,
        accessibility_label: &'a str,
        placeholder: &'a str,
        config: CalendarConfig<'a>,
        style: Style,
        popup_style: Style,
    ) -> Self {
        let anchor = date_picker_anchor(
            selected,
            on_toggle,
            accessibility_label,
            placeholder,
            style.clone(),
        );
        let content = Self::calendar_with_config(
            visible_month,
            selected,
            on_select,
            on_month_change,
            config,
            fill_parent_style(),
        );
        Self::popover(open, anchor, content, Some(on_dismiss), style, popup_style)
    }
}

fn calendar_sections<'a, Msg>(
    visible_month: CalendarMonth,
    selected: Option<CalendarDate>,
    on_select: fn(CalendarDate) -> Msg,
    on_month_change: fn(CalendarMonth) -> Msg,
    config: CalendarConfig<'a>,
) -> Vec<Widget<'a, Msg>> {
    let mut sections = Vec::with_capacity(8);
    sections.push(calendar_header(visible_month, on_month_change, config));
    sections.push(calendar_weekdays(config));
    sections.extend(calendar_weeks(
        visible_month,
        selected,
        on_select,
        on_month_change,
        config,
    ));
    sections
}

fn calendar_header<'a, Msg>(
    visible_month: CalendarMonth,
    on_month_change: fn(CalendarMonth) -> Msg,
    config: CalendarConfig<'a>,
) -> Widget<'a, Msg> {
    let [previous_year, previous_month] =
        previous_navigation(visible_month, on_month_change, config);
    let [next_month, next_year] = next_navigation(visible_month, on_month_change, config);
    Widget::Row {
        children: vec![
            previous_year,
            previous_month,
            calendar_month_heading(visible_month, config),
            next_month,
            next_year,
        ],
        style: calendar_header_style(),
    }
}

fn previous_navigation<'a, Msg>(
    visible_month: CalendarMonth,
    on_month_change: fn(CalendarMonth) -> Msg,
    config: CalendarConfig<'a>,
) -> [Widget<'a, Msg>; 2] {
    let previous_year = visible_month.previous_year().unwrap_or(visible_month);
    let previous_month = visible_month.previous().unwrap_or(visible_month);
    [
        calendar_navigation_button(
            "«",
            config.labels.previous_year,
            on_month_change(previous_year),
        ),
        calendar_navigation_button(
            "‹",
            config.labels.previous_month,
            on_month_change(previous_month),
        ),
    ]
}

fn next_navigation<'a, Msg>(
    visible_month: CalendarMonth,
    on_month_change: fn(CalendarMonth) -> Msg,
    config: CalendarConfig<'a>,
) -> [Widget<'a, Msg>; 2] {
    let next_month = visible_month.next().unwrap_or(visible_month);
    let next_year = visible_month.next_year().unwrap_or(visible_month);
    [
        calendar_navigation_button("›", config.labels.next_month, on_month_change(next_month)),
        calendar_navigation_button("»", config.labels.next_year, on_month_change(next_year)),
    ]
}

fn calendar_navigation_button<'a, Msg>(
    icon: &'static str,
    accessibility_label: &'a str,
    message: Msg,
) -> Widget<'a, Msg> {
    Widget::ButtonContent {
        label: accessibility_label,
        child: Box::new(Widget::Container {
            child: Box::new(Widget::Text {
                content: icon.into(),
                style: Style::default(),
                color: None,
                size: 16.0,
            }),
            style: navigation_icon_style(),
            color: None,
            radius: 0.0,
        }),
        on_press: message,
        style: navigation_button_style(),
        color: None,
        variant: ButtonVariant::Text,
    }
}

fn calendar_month_heading<'a, Msg>(
    visible_month: CalendarMonth,
    config: CalendarConfig<'a>,
) -> Widget<'a, Msg> {
    let month_name = config.labels.months[visible_month.month() as usize - 1];
    Widget::Container {
        child: Box::new(Widget::Row {
            children: calendar_heading_text(month_name, visible_month.year()),
            style: calendar_heading_content_style(),
        }),
        style: calendar_heading_style(),
        color: None,
        radius: 0.0,
    }
}

fn calendar_heading_text<'a, Msg>(month_name: &str, year: i32) -> Vec<Widget<'a, Msg>> {
    let year = year.to_string();
    let year_style = measured_calendar_text_style(&year, 16.0);
    vec![
        Widget::Text {
            content: month_name.into(),
            style: measured_calendar_text_style(month_name, 16.0),
            color: None,
            size: 16.0,
        },
        Widget::rich_text(
            RichText::from_span(RichTextSpan::owned(year).bold()),
            year_style,
        ),
    ]
}

fn calendar_weekdays<'a, Msg>(config: CalendarConfig<'a>) -> Widget<'a, Msg> {
    let children = (0..7)
        .map(|column| weekday_label(config.labels, config.week_start, column))
        .map(calendar_weekday_heading)
        .collect();
    Widget::Row {
        children,
        style: calendar_row_style(CALENDAR_WEEKDAY_HEIGHT),
    }
}

fn calendar_weekday_heading<'a, Msg>(label: &str) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Text {
            content: label.into(),
            style: measured_calendar_text_style(label, 11.0),
            color: None,
            size: 11.0,
        }),
        style: calendar_weekday_cell_style(),
        color: None,
        radius: 0.0,
    }
}

fn measured_calendar_text_style(text: &str, font_size: f32) -> Style {
    let estimated_width = text.chars().count() as f32 * font_size * 0.62 + 2.0;
    calendar_label_text_style(estimated_width.max(font_size), font_size)
}

fn calendar_weeks<'a, Msg>(
    visible_month: CalendarMonth,
    selected: Option<CalendarDate>,
    on_select: fn(CalendarDate) -> Msg,
    on_month_change: fn(CalendarMonth) -> Msg,
    config: CalendarConfig<'a>,
) -> Vec<Widget<'a, Msg>> {
    calendar_grid(visible_month, config.week_start)
        .chunks(7)
        .map(|week| Widget::Row {
            children: week
                .iter()
                .map(|cell| {
                    calendar_day_button(*cell, visible_month, selected, on_select, on_month_change)
                })
                .collect(),
            style: calendar_row_style(CALENDAR_DAY_HEIGHT),
        })
        .collect()
}

fn calendar_day_button<'a, Msg>(
    cell: CalendarGridCell,
    visible_month: CalendarMonth,
    selected: Option<CalendarDate>,
    on_select: fn(CalendarDate) -> Msg,
    on_month_change: fn(CalendarMonth) -> Msg,
) -> Widget<'a, Msg> {
    let variant = if cell.date.is_some_and(|date| selected == Some(date)) {
        ButtonVariant::Primary
    } else if cell.is_visible_month {
        ButtonVariant::Ghost
    } else {
        ButtonVariant::Text
    };
    let (text, on_press) = match cell.date {
        Some(date) => (day_number_label(date.day()), on_select(date)),
        None => ("", on_month_change(visible_month)),
    };
    Widget::Button {
        text,
        on_press,
        style: calendar_cell_style(CALENDAR_DAY_HEIGHT),
        color: None,
        variant,
    }
}

fn date_picker_anchor<'a, Msg>(
    selected: Option<CalendarDate>,
    on_toggle: Msg,
    accessibility_label: &'a str,
    placeholder: &'a str,
    style: Style,
) -> Widget<'a, Msg> {
    let display = selected
        .map(|date| date.to_string())
        .unwrap_or_else(|| placeholder.to_owned());
    Widget::ButtonContent {
        label: accessibility_label,
        child: Box::new(date_picker_value(display)),
        on_press: on_toggle,
        style,
        color: None,
        variant: ButtonVariant::Ghost,
    }
}

fn date_picker_value<'a, Msg>(display: String) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Text {
            content: display,
            style: fill_parent_style(),
            color: None,
            size: 14.0,
        }),
        style: date_picker_value_style(),
        color: None,
        radius: 0.0,
    }
}
