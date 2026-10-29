// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! `cargo run --example native_popup_clock -- top` (also bottom, left, right).
//! Press the clock icon. See docs/NATIVE_POPUPS.md for Wayland input-grab rules.

use arboard::Clipboard;
use chrono::{Datelike, Utc};
use cosmic_text::FontSystem;
use rutter::spacing;
use rutter::{
    ButtonVariant, CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, ClockConfig,
    ClockFormat, MultiWindowAppLogic, MultiWindowRunner, PanelConfig, PanelEdge, PanelFocus,
    PopupAnchor, PopupConfig, PopupConfigError, PopupConstraints, PopupDirection, SurfaceCommand,
    SurfaceId, SurfaceRequest, TimeZone, WeekStart, Widget, WindowConfig,
};
use taffy::prelude::*;

const CLOCK_POPUP: SurfaceId = SurfaceId::new(1);

#[derive(Clone)]
struct ClockModel {
    direction: PopupDirection,
    month: CalendarMonth,
    selected: Option<CalendarDate>,
    popup_open: bool,
}

impl ClockModel {
    fn new(direction: PopupDirection) -> Self {
        let today = Utc::now().date_naive();
        let date = CalendarDate::new(today.year(), today.month() as u8, today.day() as u8)
            .expect("UTC today must be a valid calendar date");
        Self {
            direction,
            month: date.calendar_month(),
            selected: Some(date),
            popup_open: false,
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    Toggle,
    Close,
    Navigate(CalendarMonth),
    Select(CalendarDate),
}

struct ClockCalendar;

impl MultiWindowAppLogic for ClockCalendar {
    type State = ClockModel;
    type Message = Message;

    fn new(_: &mut FontSystem) -> ClockModel {
        ClockModel::new(PopupDirection::Below)
    }

    fn view<'a>(state: &'a mut ClockModel, surface: SurfaceId) -> Widget<'a, Message> {
        if surface == SurfaceId::PRIMARY {
            return Widget::button_content(
                "Open clock and calendar",
                Widget::Text {
                    content: "◷".into(),
                    style: Style::default(),
                    color: None,
                    size: 22.0,
                },
                Message::Toggle,
                Style {
                    size: Size::length(32.0_f32),
                    flex_shrink: 0.0,
                    ..Style::default()
                },
                None,
                ButtonVariant::Text,
            );
        }
        Widget::ScrollView {
            id: 21,
            style: Style {
                size: Size::percent(1.0_f32),
                align_items: Some(AlignItems::FlexStart),
                min_size: Size::from_lengths(0.0, 0.0),
                ..Style::default()
            },
            child: Box::new(Widget::Column {
                children: vec![
                    Widget::clock_with_config(
                        TimeZone::UTC,
                        ClockConfig::new(
                            ClockFormat::twenty_four_hour().with_time_zone(false),
                            28.0,
                        )
                        .expect("28 logical pixels is a valid clock font size"),
                        Style {
                            flex_shrink: 0.0,
                            size: Size {
                                width: Dimension::percent(1.0),
                                height: Dimension::length(38.0),
                            },
                            ..Style::default()
                        },
                        "UTC time, hours minutes seconds",
                    )
                    .with_id(20),
                    Widget::calendar_with_config(
                        state.month,
                        state.selected,
                        Message::Select,
                        Message::Navigate,
                        CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday),
                        Style {
                            size: Size {
                                width: Dimension::percent(1.0),
                                height: Dimension::length(330.0),
                            },
                            flex_shrink: 0.0,
                            ..Style::default()
                        },
                    ),
                    Widget::Button {
                        text: "Close",
                        on_press: Message::Close,
                        style: Style {
                            size: Size {
                                width: Dimension::percent(1.0),
                                height: Dimension::length(36.0),
                            },
                            flex_shrink: 0.0,
                            ..Style::default()
                        },
                        color: None,
                        variant: ButtonVariant::Text,
                    },
                ],
                style: Style {
                    size: Size {
                        width: Dimension::percent(1.0),
                        height: Dimension::auto(),
                    },
                    flex_shrink: 0.0,
                    padding: Rect::length(spacing::MD),
                    gap: Size::length(spacing::SM),
                    ..Style::default()
                },
            }),
        }
    }

    fn surface_created(state: &mut ClockModel, surface: SurfaceId) {
        if surface == CLOCK_POPUP {
            state.popup_open = true;
        }
    }

    fn surface_closed(state: &mut ClockModel, surface: SurfaceId) {
        if surface == CLOCK_POPUP {
            state.popup_open = false;
        }
    }

    fn update(
        state: &mut ClockModel,
        _: SurfaceId,
        message: Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        match message {
            Message::Toggle if state.popup_open => vec![SurfaceCommand::Close(CLOCK_POPUP)],
            Message::Toggle => vec![SurfaceCommand::Open(
                popup_request(state.direction).expect("fixed clock popup geometry is valid"),
            )],
            Message::Close => vec![SurfaceCommand::Close(CLOCK_POPUP)],
            Message::Navigate(month) => {
                state.month = month;
                Vec::new()
            }
            Message::Select(date) => {
                state.selected = Some(date);
                Vec::new()
            }
        }
    }
}

fn popup_request(direction: PopupDirection) -> Result<SurfaceRequest, PopupConfigError> {
    let offset = match direction {
        PopupDirection::Above => (0, -(spacing::XS as i32)),
        PopupDirection::Below => (0, spacing::XS as i32),
        PopupDirection::Left => (-(spacing::XS as i32), 0),
        PopupDirection::Right => (spacing::XS as i32, 0),
    };
    let popup = PopupConfig::new(
        SurfaceId::PRIMARY,
        PopupAnchor::new(0, 0, 32, 32)?,
        384,
        448,
        direction,
    )?
    .with_offset(offset.0, offset.1)?
    .with_constraints(PopupConstraints::SLIDE_X | PopupConstraints::SLIDE_Y)?;
    Ok(SurfaceRequest::new(
        CLOCK_POPUP,
        WindowConfig::default()
            .with_title("Clock and calendar")
            .with_popup(popup),
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let edge = std::env::args().nth(1).unwrap_or_else(|| "top".into());
    let (edge, direction) = match edge.as_str() {
        "top" => (PanelEdge::Top, PopupDirection::Below),
        "bottom" => (PanelEdge::Bottom, PopupDirection::Above),
        "left" => (PanelEdge::Left, PopupDirection::Right),
        "right" => (PanelEdge::Right, PopupDirection::Left),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "expected top, bottom, left or right",
            )
            .into());
        }
    };
    // On-demand keyboard input permits Escape without a keyboard-exclusive panel.
    let panel = PanelConfig::new("rutter-clock-popup", edge, 0, 32)?.with_focus(PanelFocus::Normal);
    MultiWindowRunner::<ClockCalendar>::try_run_with_state(
        ClockModel::new(direction),
        vec![SurfaceRequest::new(
            SurfaceId::PRIMARY,
            WindowConfig::default().with_panel(panel),
        )],
    )?;
    Ok(())
}

#[cfg(test)]
mod layout_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_calendar_is_immediate_content_and_native_close_resets_presence() {
        let date = CalendarDate::new(2026, 10, 22).unwrap();
        let mut state = ClockModel {
            direction: PopupDirection::Below,
            month: date.calendar_month(),
            selected: Some(date),
            popup_open: false,
        };
        {
            let widget = ClockCalendar::view(&mut state, CLOCK_POPUP);
            let Widget::ScrollView { child, .. } = widget else {
                panic!("popup must retain a scrollable document on short surfaces");
            };
            let Widget::Column { children, .. } = *child else {
                panic!("popup must be a column");
            };
            assert!(
                matches!(children[0], Widget::Clock { config, .. } if config.format().show_seconds())
            );
            let Widget::Container { child, .. } = &children[1] else {
                panic!("calendar must be immediately visible, not inside a popover");
            };
            assert!(
                matches!(child.as_ref(), Widget::Column { children, .. } if children.len() == 8)
            );
        }
        ClockCalendar::surface_created(&mut state, CLOCK_POPUP);
        assert!(state.popup_open);
        ClockCalendar::surface_closed(&mut state, CLOCK_POPUP);
        assert!(!state.popup_open);
    }

    #[test]
    fn each_panel_edge_opens_a_parented_popup_with_slide_only_constraints() {
        for direction in [
            PopupDirection::Above,
            PopupDirection::Below,
            PopupDirection::Left,
            PopupDirection::Right,
        ] {
            let request = popup_request(direction).unwrap();
            let popup = request.window.popup().unwrap();
            assert_eq!(popup.parent(), SurfaceId::PRIMARY);
            assert_eq!(popup.direction(), direction);
            assert_eq!(
                popup.constraints(),
                PopupConstraints::SLIDE_X | PopupConstraints::SLIDE_Y
            );
            assert!(request.window.panel().is_none());
        }
    }
}
