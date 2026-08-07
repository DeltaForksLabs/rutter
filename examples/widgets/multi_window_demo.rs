// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{
    ButtonVariant, CloseBehavior, MultiWindowAppLogic, MultiWindowRunner, SurfaceCommand,
    SurfaceId, SurfaceRequest, Theme, Widget, WindowConfig,
};

const PANEL: SurfaceId = SurfaceId::new(1);
const SETTINGS: SurfaceId = SurfaceId::new(2);
const POPUP: SurfaceId = SurfaceId::new(3);

#[derive(Clone, Default)]
struct MultiWindowState {
    counter: usize,
    settings_open: bool,
    popup_open: bool,
}

#[derive(Clone, Debug)]
enum Message {
    Increment,
    OpenSettings,
    OpenPopup,
    CloseCurrent,
    Exit,
}

struct MultiWindowDemo;

impl MultiWindowAppLogic for MultiWindowDemo {
    type State = MultiWindowState;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Self::State {
        MultiWindowState::default()
    }

    fn initial_surfaces() -> Vec<SurfaceRequest> {
        vec![panel_request(), settings_request()]
    }

    fn view<'a>(state: &'a mut Self::State, surface: SurfaceId) -> Widget<'a, Self::Message> {
        match surface {
            PANEL => panel_view(state),
            SETTINGS => settings_view(state),
            POPUP => popup_view(state),
            _ => unavailable_surface_view(surface),
        }
    }

    fn surface_created(state: &mut Self::State, surface: SurfaceId) {
        set_surface_presence(state, surface, true);
    }

    fn surface_closed(state: &mut Self::State, surface: SurfaceId) {
        set_surface_presence(state, surface, false);
    }

    fn update(
        state: &mut Self::State,
        surface: SurfaceId,
        message: Self::Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        match message {
            Message::Increment => state.counter += 1,
            Message::OpenSettings if !state.settings_open => {
                return vec![SurfaceCommand::Open(settings_request())];
            }
            Message::OpenPopup if !state.popup_open => {
                return vec![SurfaceCommand::Open(popup_request())];
            }
            Message::CloseCurrent => return vec![SurfaceCommand::Close(surface)],
            Message::Exit => return vec![SurfaceCommand::Exit],
            Message::OpenSettings | Message::OpenPopup => {}
        }
        Vec::new()
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn panel_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Rutter Panel")
        .with_decorations(false)
        .with_resizable(false)
        .with_inner_size(900, 180)
        .unwrap()
        .with_close_behavior(CloseBehavior::ExitApplication);
    SurfaceRequest::new(PANEL, window)
}

fn settings_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Rutter Settings")
        .with_inner_size(520, 360)
        .unwrap();
    SurfaceRequest::new(SETTINGS, window)
}

fn popup_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Rutter Context Surface")
        .with_decorations(false)
        .with_resizable(false)
        .with_inner_size(340, 180)
        .unwrap();
    SurfaceRequest::new(POPUP, window)
}

fn set_surface_presence(state: &mut MultiWindowState, surface: SurfaceId, present: bool) {
    match surface {
        SETTINGS => state.settings_open = present,
        POPUP => state.popup_open = present,
        _ => {}
    }
}

fn panel_view<'a>(state: &'a MultiWindowState) -> Widget<'a, Message> {
    let status = format!(
        "counter={} | settings={} | popup={}",
        state.counter, state.settings_open, state.popup_open
    );
    surface_column(vec![
        text("Independent panel surface", 22.0),
        text(status, 14.0),
        button_row(vec![
            button("Increment shared state", Message::Increment),
            button("Open settings", Message::OpenSettings),
            button("Open popup", Message::OpenPopup),
            button("Exit", Message::Exit),
        ]),
    ])
}

fn settings_view<'a>(state: &'a MultiWindowState) -> Widget<'a, Message> {
    surface_column(vec![
        text("Settings surface", 24.0),
        text(format!("Shared counter: {}", state.counter), 16.0),
        button("Increment from settings", Message::Increment),
        button("Close only settings", Message::CloseCurrent),
    ])
}

fn popup_view<'a>(state: &'a MultiWindowState) -> Widget<'a, Message> {
    surface_column(vec![
        text("Popup surface", 22.0),
        text(format!("Shared counter: {}", state.counter), 14.0),
        button("Close popup", Message::CloseCurrent),
    ])
}

fn unavailable_surface_view<'a>(surface: SurfaceId) -> Widget<'a, Message> {
    surface_column(vec![text(
        format!("Unknown surface ID: {}", surface.get()),
        18.0,
    )])
}

fn surface_column<'a>(children: Vec<Widget<'a, Message>>) -> Widget<'a, Message> {
    Widget::Column {
        style: Style {
            size: Size::percent(1.0_f32),
            padding: Rect::length(28.0_f32),
            gap: Size::length(12.0_f32),
            ..Default::default()
        },
        children,
    }
}

fn button_row<'a>(children: Vec<Widget<'a, Message>>) -> Widget<'a, Message> {
    Widget::Row {
        style: Style {
            gap: Size::length(10.0_f32),
            ..Default::default()
        },
        children,
    }
}

fn text<'a>(content: impl Into<String>, size: f32) -> Widget<'a, Message> {
    Widget::Text {
        content: content.into(),
        color: None,
        size,
        style: Style::default(),
    }
}

fn button<'a>(label: &'a str, message: Message) -> Widget<'a, Message> {
    Widget::Button {
        text: label,
        on_press: message,
        style: Style::default(),
        color: None,
        variant: ButtonVariant::Primary,
    }
}

pub fn run() {
    MultiWindowRunner::<MultiWindowDemo>::run();
}
