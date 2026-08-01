// ============================================================
// Rutter Framework — examples/widgets/calendar_demo.rs
// Standalone calendar and date-picker popover demonstration.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    AppLogic, CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, RutterRunner, Theme,
    WeekStart, Widget,
};
use taffy::prelude::*;

pub struct CalendarDemoState {
    calendar_month: CalendarMonth,
    calendar_date: Option<CalendarDate>,
    picker_month: CalendarMonth,
    picker_date: Option<CalendarDate>,
    picker_open: bool,
    picker_accessibility_label: String,
    status: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    SelectCalendarDate(CalendarDate),
    NavigateCalendar(CalendarMonth),
    TogglePicker,
    ClosePicker,
    SelectPickerDate(CalendarDate),
    NavigatePicker(CalendarMonth),
}

pub struct CalendarDemo;

impl AppLogic for CalendarDemo {
    type State = CalendarDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        CalendarDemoState {
            calendar_month: CalendarMonth::new(2026, 7).unwrap(),
            calendar_date: Some(CalendarDate::new(2026, 7, 31).unwrap()),
            picker_month: CalendarMonth::new(2026, 8).unwrap(),
            picker_date: None,
            picker_open: false,
            picker_accessibility_label: "Data do evento: nenhuma data selecionada".into(),
            status: "Escolha uma data em qualquer calendário.".into(),
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Column {
            children: vec![
                demo_text("Calendário e Date Picker", 24.0),
                demo_text("Navegue por mês ou ano e selecione uma única data.", 14.0),
                calendar_showcase(state),
            ],
            style: root_style(),
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        match message {
            Msg::SelectCalendarDate(date) => select_standalone_date(state, date),
            Msg::NavigateCalendar(month) => state.calendar_month = month,
            Msg::TogglePicker => state.picker_open = !state.picker_open,
            Msg::ClosePicker => state.picker_open = false,
            Msg::SelectPickerDate(date) => select_picker_date(state, date),
            Msg::NavigatePicker(month) => state.picker_month = month,
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn calendar_showcase(state: &CalendarDemoState) -> Widget<'_, Msg> {
    Widget::Row {
        children: vec![standalone_calendar(state), picker_column(state)],
        style: showcase_style(),
    }
}

fn standalone_calendar(state: &CalendarDemoState) -> Widget<'_, Msg> {
    Widget::calendar_with_config(
        state.calendar_month,
        state.calendar_date,
        Msg::SelectCalendarDate,
        Msg::NavigateCalendar,
        portuguese_calendar_config(),
        calendar_style(),
    )
}

fn picker_column(state: &CalendarDemoState) -> Widget<'_, Msg> {
    Widget::Column {
        children: vec![
            demo_text("Date picker", 18.0),
            date_picker(state),
            demo_text(picker_summary(state), 14.0),
            demo_text(state.status.clone(), 13.0),
        ],
        style: picker_column_style(),
    }
}

fn date_picker(state: &CalendarDemoState) -> Widget<'_, Msg> {
    Widget::date_picker_with_config(
        state.picker_open,
        state.picker_month,
        state.picker_date,
        Msg::TogglePicker,
        Msg::ClosePicker,
        Msg::SelectPickerDate,
        Msg::NavigatePicker,
        &state.picker_accessibility_label,
        "AAAA-MM-DD",
        portuguese_calendar_config(),
        picker_anchor_style(),
        picker_popup_style(),
    )
    .with_id(920)
}

fn portuguese_calendar_config() -> CalendarConfig<'static> {
    CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday)
}

fn select_standalone_date(state: &mut CalendarDemoState, date: CalendarDate) {
    state.calendar_date = Some(date);
    state.calendar_month = date.calendar_month();
    state.status = format!("Calendário selecionou {date}.");
}

fn select_picker_date(state: &mut CalendarDemoState, date: CalendarDate) {
    state.picker_date = Some(date);
    state.picker_month = date.calendar_month();
    state.picker_open = false;
    state.picker_accessibility_label = format!("Data do evento: {date}");
    state.status = format!("Date picker selecionou {date}.");
}

fn picker_summary(state: &CalendarDemoState) -> String {
    state
        .picker_date
        .map(|date| format!("Data atual: {date}"))
        .unwrap_or_else(|| "Nenhuma data selecionada.".into())
}

fn demo_text<'a>(content: impl Into<String>, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: Style::default(),
        color: None,
        size,
    }
}

fn root_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size::percent(1.0_f32),
        padding: Rect::length(32.0_f32),
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(12.0),
        },
        ..Style::default()
    }
}

fn showcase_style() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::FlexStart),
        gap: Size {
            width: LengthPercentage::length(28.0),
            height: LengthPercentage::length(0.0),
        },
        ..Style::default()
    }
}

fn calendar_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(340.0),
            height: Dimension::length(330.0),
        },
        ..Style::default()
    }
}

fn picker_column_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size {
            width: Dimension::length(300.0),
            height: Dimension::auto(),
        },
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(14.0),
        },
        ..Style::default()
    }
}

fn picker_anchor_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(230.0),
            height: Dimension::length(44.0),
        },
        ..Style::default()
    }
}

fn picker_popup_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(340.0),
            height: Dimension::length(330.0),
        },
        ..Style::default()
    }
}

pub fn run() {
    RutterRunner::<CalendarDemo>::run();
}
