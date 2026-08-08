// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{
    ButtonVariant, CloseBehavior, MultiWindowAppLogic, MultiWindowRunner, RichText, RichTextColor,
    RichTextSize, RichTextSpan, RichTextStyle, SurfaceCommand, SurfaceId, SurfaceRequest, Theme,
    Widget, WindowConfig,
};

const SECOND_WINDOW: SurfaceId = SurfaceId::new(1);

#[derive(Clone, Default)]
struct MultiWindowState {
    second_window_open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Message {
    OpenSecondWindow,
    CloseCurrentWindow,
}

struct MultiWindowDemo;

impl MultiWindowAppLogic for MultiWindowDemo {
    type State = MultiWindowState;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Self::State {
        MultiWindowState::default()
    }

    fn initial_surfaces() -> Vec<SurfaceRequest> {
        vec![main_window_request()]
    }

    fn view<'a>(_: &'a mut Self::State, surface: SurfaceId) -> Widget<'a, Self::Message> {
        match surface {
            SurfaceId::PRIMARY => main_window_view(),
            SECOND_WINDOW => second_window_view(),
            _ => unknown_window_view(surface),
        }
    }

    fn surface_created(state: &mut Self::State, surface: SurfaceId) {
        set_second_window_presence(state, surface, true);
    }

    fn surface_closed(state: &mut Self::State, surface: SurfaceId) {
        set_second_window_presence(state, surface, false);
    }

    fn update(
        state: &mut Self::State,
        surface: SurfaceId,
        message: Self::Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        multi_window_commands(state, surface, message)
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn multi_window_commands(
    state: &mut MultiWindowState,
    surface: SurfaceId,
    message: Message,
) -> Vec<SurfaceCommand> {
    match message {
        Message::OpenSecondWindow if !state.second_window_open => {
            state.second_window_open = true;
            vec![SurfaceCommand::Open(second_window_request())]
        }
        Message::OpenSecondWindow => Vec::new(),
        Message::CloseCurrentWindow => vec![SurfaceCommand::Close(surface)],
    }
}

fn main_window_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Rutter Multi-Window Demo")
        .with_resizable(false)
        .with_inner_size(520, 320)
        .expect("main window size 520x320 must contain positive dimensions")
        .with_close_behavior(CloseBehavior::ExitApplication);
    SurfaceRequest::new(SurfaceId::PRIMARY, window)
}

fn second_window_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Second Window")
        .with_resizable(false)
        .with_inner_size(560, 260)
        .expect("second window size 560x260 must contain positive dimensions");
    SurfaceRequest::new(SECOND_WINDOW, window)
}

fn set_second_window_presence(state: &mut MultiWindowState, surface: SurfaceId, present: bool) {
    if surface == SECOND_WINDOW {
        state.second_window_open = present;
    }
}

fn main_window_view<'a>() -> Widget<'a, Message> {
    centered_surface_column(vec![Widget::Button {
        text: "Open Second Window",
        on_press: Message::OpenSecondWindow,
        style: centered_button_style(),
        color: None,
        variant: ButtonVariant::Primary,
    }])
}

fn second_window_view<'a>() -> Widget<'a, Message> {
    centered_surface_column(vec![
        second_window_rich_text(),
        Widget::Button {
            text: "Close Second Window",
            on_press: Message::CloseCurrentWindow,
            style: centered_button_style(),
            color: None,
            variant: ButtonVariant::Text,
        },
    ])
}

fn second_window_rich_text<'a>() -> Widget<'a, Message> {
    let font_size =
        RichTextSize::new(22.0).expect("rich-text font size 22 must be finite and within 1..=512");
    let content = RichText::from_spans([
        RichTextSpan::new("Hello from the "),
        RichTextSpan::new("second window")
            .bold()
            .with_color(RichTextColor::rgb(90, 180, 255)),
        RichTextSpan::new(" — rendered with RichText!").italic(),
    ])
    .with_default_style(RichTextStyle::default().with_size(font_size));
    Widget::rich_text(content, rich_text_layout_style())
}

fn unknown_window_view<'a>(surface: SurfaceId) -> Widget<'a, Message> {
    let message = format!("Unknown window ID: {}", surface.get());
    centered_surface_column(vec![Widget::rich_text(
        RichText::plain(message),
        rich_text_layout_style(),
    )])
}

fn centered_surface_column<'a>(children: Vec<Widget<'a, Message>>) -> Widget<'a, Message> {
    Widget::Column {
        children,
        style: Style {
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            size: Size::percent(1.0_f32),
            padding: Rect::length(28.0_f32),
            gap: Size::length(20.0_f32),
            ..Style::default()
        },
    }
}

fn centered_button_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(220.0),
            height: Dimension::length(52.0),
        },
        ..Style::default()
    }
}

fn rich_text_layout_style() -> Style {
    Style {
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(72.0),
        },
        ..Style::default()
    }
}

pub fn run() {
    MultiWindowRunner::<MultiWindowDemo>::run();
}

#[cfg(test)]
#[path = "../../tests/unit/multi_window_demo_unit_tests.rs"]
mod tests;
