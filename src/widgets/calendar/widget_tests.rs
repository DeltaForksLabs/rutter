use taffy::prelude::{JustifyContent, Style};

use super::{CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, WeekStart};
use crate::{RichTextWeight, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Toggle,
    Close,
    Select(CalendarDate),
    Navigate(CalendarMonth),
}

#[test]
fn calendar_constructor_keeps_a_stable_six_week_structure() {
    let widget = Widget::calendar(
        CalendarMonth::new(2026, 7).unwrap(),
        Some(CalendarDate::new(2026, 7, 31).unwrap()),
        Msg::Select,
        Msg::Navigate,
        Style::default(),
    );
    let Widget::Container { child, .. } = widget else {
        panic!("calendar constructor expected Container root");
    };
    let Widget::Column { children, .. } = *child else {
        panic!("calendar root expected Column content");
    };

    assert_eq!(children.len(), 8);
    assert!(matches!(&children[0], Widget::Row { children, .. } if children.len() == 5));
    assert!(
        children[2..]
            .iter()
            .all(|week| matches!(week, Widget::Row { children, .. } if children.len() == 7))
    );
}

#[test]
fn configured_calendar_uses_localized_heading() {
    let widget = Widget::calendar_with_config(
        CalendarMonth::new(2026, 7).unwrap(),
        None,
        Msg::Select,
        Msg::Navigate,
        CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday),
        Style::default(),
    );

    assert!(calendar_contains_text(&widget, "Julho"));
    assert!(calendar_contains_bold_rich_text(&widget, "2026"));
}

#[test]
fn date_picker_composes_anchor_and_calendar_popover() {
    let selected = CalendarDate::new(2026, 7, 31).unwrap();
    let widget = Widget::date_picker(
        true,
        selected.calendar_month(),
        Some(selected),
        Msg::Toggle,
        Msg::Close,
        Msg::Select,
        Msg::Navigate,
        "Date",
        "YYYY-MM-DD",
        Style::default(),
        Style::default(),
    );
    let Widget::Popover {
        open,
        anchor,
        content,
        on_dismiss,
        ..
    } = widget
    else {
        panic!("date picker expected Popover root");
    };

    assert!(open);
    assert_eq!(on_dismiss, Some(Msg::Close));
    assert!(matches!(*anchor, Widget::ButtonContent { .. }));
    assert!(calendar_contains_text(&content, "July"));
    assert!(calendar_contains_bold_rich_text(&content, "2026"));
    assert!(calendar_contains_text(&anchor, "2026-07-31"));
}

#[test]
fn configured_date_picker_uses_custom_weekday_labels() {
    let widget = Widget::date_picker_with_config(
        false,
        CalendarMonth::new(2026, 7).unwrap(),
        None,
        Msg::Toggle,
        Msg::Close,
        Msg::Select,
        Msg::Navigate,
        "Data",
        "AAAA-MM-DD",
        CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Sunday),
        Style::default(),
        Style::default(),
    );

    assert!(calendar_contains_text(&widget, "Dom"));
}

#[test]
fn calendar_centers_heading_and_weekday_content() {
    let widget = Widget::calendar(
        CalendarMonth::new(2026, 7).unwrap(),
        None,
        Msg::Select,
        Msg::Navigate,
        Style::default(),
    );
    let sections = calendar_sections(&widget);
    let Widget::Row {
        children: header, ..
    } = &sections[0]
    else {
        panic!("calendar header expected Row");
    };
    let Widget::Container { child: heading, .. } = &header[2] else {
        panic!("calendar heading expected Container");
    };
    let Widget::Row { children, style } = heading.as_ref() else {
        panic!("calendar heading content expected Row");
    };

    assert_eq!(style.justify_content, Some(JustifyContent::Center));
    assert!(
        matches!(&children[1], Widget::RichText { content, .. } if content.plain_text() == "2026")
    );
    assert!(
        matches!(&sections[1], Widget::Row { children, .. } if children.iter().all(|child| matches!(child, Widget::Container { .. })))
    );
}

fn calendar_sections<'widget, 'content, Msg>(
    widget: &'widget Widget<'content, Msg>,
) -> &'widget [Widget<'content, Msg>] {
    let Widget::Container { child, .. } = widget else {
        panic!("calendar constructor expected Container root");
    };
    let Widget::Column { children, .. } = child.as_ref() else {
        panic!("calendar root expected Column content");
    };
    children
}

fn calendar_contains_text<Msg>(widget: &Widget<'_, Msg>, expected: &str) -> bool {
    match widget {
        Widget::Text { content, .. } => content == expected,
        Widget::RichText { content, .. } => content.plain_text() == expected,
        Widget::Column { children, .. } | Widget::Row { children, .. } => children
            .iter()
            .any(|child| calendar_contains_text(child, expected)),
        Widget::Container { child, .. } | Widget::ButtonContent { child, .. } => {
            calendar_contains_text(child, expected)
        }
        Widget::Popover {
            anchor, content, ..
        } => calendar_contains_text(anchor, expected) || calendar_contains_text(content, expected),
        _ => false,
    }
}

fn calendar_contains_bold_rich_text<Msg>(widget: &Widget<'_, Msg>, expected: &str) -> bool {
    match widget {
        Widget::RichText { content, .. } => {
            content.plain_text() == expected
                && content
                    .spans()
                    .iter()
                    .any(|span| span.style().weight() == Some(RichTextWeight::BOLD))
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => children
            .iter()
            .any(|child| calendar_contains_bold_rich_text(child, expected)),
        Widget::Container { child, .. } | Widget::ButtonContent { child, .. } => {
            calendar_contains_bold_rich_text(child, expected)
        }
        Widget::Popover {
            anchor, content, ..
        } => {
            calendar_contains_bold_rich_text(anchor, expected)
                || calendar_contains_bold_rich_text(content, expected)
        }
        _ => false,
    }
}
