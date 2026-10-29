// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{
    ButtonVariant, CloseBehavior, MultiWindowAppLogic, MultiWindowRunner, RichText, RichTextColor,
    RichTextSize, RichTextSpan, RichTextStyle, SurfaceCommand, SurfaceEvent, SurfaceId,
    SurfaceRequest, Theme, Widget, WindowConfig, WindowLevel,
};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const SECOND_WINDOW: SurfaceId = SurfaceId::new(1);

#[derive(Clone, Default)]
pub(super) struct MultiWindowState {
    theme: ExampleTheme,
    second_window_open: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Message {
    ThemeChanged(ExampleTheme),
    OpenSecondWindow,
    CloseCurrentWindow,
}

pub(super) struct MultiWindowDemo;

impl MultiWindowAppLogic for MultiWindowDemo {
    type State = MultiWindowState;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Self::State {
        MultiWindowState::default()
    }

    fn initial_surfaces() -> Vec<SurfaceRequest> {
        vec![main_window_request()]
    }

    fn view<'a>(state: &'a mut Self::State, surface: SurfaceId) -> Widget<'a, Self::Message> {
        match surface {
            SurfaceId::PRIMARY => with_theme_selector(main_window_view(), state.theme),
            SECOND_WINDOW => with_theme_selector(second_window_view(), state.theme),
            _ => unknown_window_view(surface),
        }
    }

    fn surface_created(state: &mut Self::State, surface: SurfaceId) {
        set_second_window_presence(state, surface, true);
    }

    fn surface_closed(state: &mut Self::State, surface: SurfaceId) {
        set_second_window_presence(state, surface, false);
    }

    fn surface_event(
        _: &mut Self::State,
        surface: SurfaceId,
        event: SurfaceEvent,
    ) -> Vec<SurfaceCommand> {
        surface_focus_commands(surface, event)
    }

    fn update(
        state: &mut Self::State,
        surface: SurfaceId,
        message: Self::Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        multi_window_commands(state, surface, message)
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn multi_window_commands(
    state: &mut MultiWindowState,
    surface: SurfaceId,
    message: Message,
) -> Vec<SurfaceCommand> {
    match message {
        Message::ThemeChanged(theme) => {
            state.theme = theme;
            Vec::new()
        }
        Message::OpenSecondWindow => open_second_window_commands(state),
        Message::CloseCurrentWindow => vec![SurfaceCommand::Close(surface)],
    }
}

fn open_second_window_commands(state: &mut MultiWindowState) -> Vec<SurfaceCommand> {
    if state.second_window_open {
        return Vec::new();
    }
    state.second_window_open = true;
    vec![
        SurfaceCommand::Open(second_window_request()),
        SurfaceCommand::SetVisible {
            surface: SECOND_WINDOW,
            visible: true,
        },
        SurfaceCommand::RequestRedraw(SECOND_WINDOW),
    ]
}

fn main_window_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Rutter Multi-Window Demo")
        .with_resizable(true)
        .with_inner_size(520, 320)
        .expect("main window size 520x320 must contain positive dimensions")
        .with_min_inner_size(320, 220)
        .expect("main window minimum size 320x220 must contain positive dimensions")
        .with_close_behavior(CloseBehavior::ExitApplication);
    SurfaceRequest::new(SurfaceId::PRIMARY, window)
}

fn second_window_request() -> SurfaceRequest {
    let window = WindowConfig::default()
        .with_title("Temporary Inspector")
        .with_resizable(true)
        .with_inner_size(560, 260)
        .expect("temporary window size 560x260 must contain positive dimensions")
        .with_min_inner_size(320, 220)
        .expect("temporary minimum size 320x220 must contain positive dimensions")
        .with_max_inner_size(720, 420)
        .expect("temporary maximum size 720x420 must contain positive dimensions")
        .with_position(160, 140)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_visible(false)
        .with_close_on_focus_loss(true);
    SurfaceRequest::new(SECOND_WINDOW, window)
}

fn surface_focus_commands(surface: SurfaceId, event: SurfaceEvent) -> Vec<SurfaceCommand> {
    if surface != SECOND_WINDOW || event != SurfaceEvent::FocusChanged(true) {
        return Vec::new();
    }
    vec![SurfaceCommand::RequestRedraw(surface)]
}

fn set_second_window_presence(state: &mut MultiWindowState, surface: SurfaceId, present: bool) {
    if surface == SECOND_WINDOW {
        state.second_window_open = present;
    }
}

fn with_theme_selector<'a>(view: Widget<'a, Message>, theme: ExampleTheme) -> Widget<'a, Message> {
    match view {
        Widget::Column {
            mut children,
            style,
        } => {
            children.insert(0, example_theme_selector(theme, Message::ThemeChanged));
            super::layout::page("Multi-window", Widget::Column { children, style })
        }
        other => other,
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
        RichTextSpan::new("temporary window")
            .bold()
            .with_color(RichTextColor::rgb(90, 180, 255)),
        RichTextSpan::new(" — it closes after focus moves away.").italic(),
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
            padding: Rect::length(spacing::XL),
            gap: Size::length(spacing::LG),
            ..Style::default()
        },
    }
}

fn centered_button_style() -> Style {
    responsive_width(220.0, Dimension::length(52.0))
}

fn rich_text_layout_style() -> Style {
    responsive_width(480.0, Dimension::auto())
}

pub fn run() {
    MultiWindowRunner::<MultiWindowDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rutter::{RichTextWeight, WindowLevel, WindowPosition};

    #[test]
    fn demo_starts_with_only_the_centered_primary_window() {
        let requests = <MultiWindowDemo as MultiWindowAppLogic>::initial_surfaces();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].surface, SurfaceId::PRIMARY);
        assert_eq!(requests[0].window.title(), "Rutter Multi-Window Demo");
        assert!(requests[0].window.is_resizable());
        assert_eq!(requests[0].window.min_inner_size().unwrap().width(), 320);

        let Widget::Column { children, style } = main_window_view() else {
            panic!("main demo view must be a centered Column")
        };
        assert_eq!(style.align_items, Some(AlignItems::Center));
        assert_eq!(style.justify_content, Some(JustifyContent::Center));
        assert_eq!(style.size, Size::percent(1.0_f32));
        assert!(
            matches!(&children[0], Widget::Button { text, .. } if *text == "Open Second Window")
        );
    }

    #[test]
    fn open_message_requests_one_second_window_until_it_closes() {
        let mut state = MultiWindowState::default();
        let Widget::Column { children, .. } = main_window_view() else {
            panic!("main demo view must be a centered Column")
        };
        let Widget::Button { on_press, .. } = &children[0] else {
            panic!("main demo action must be a Button")
        };
        let commands = multi_window_commands(&mut state, SurfaceId::PRIMARY, on_press.clone());
        assert!(matches!(
            &commands[..],
            [
                SurfaceCommand::Open(request),
                SurfaceCommand::SetVisible { surface, visible: true },
                SurfaceCommand::RequestRedraw(redraw),
            ] if request.surface == SECOND_WINDOW
                && request.window.title() == "Temporary Inspector"
                && *surface == SECOND_WINDOW
                && *redraw == SECOND_WINDOW
        ));
        assert!(state.second_window_open);
        assert!(
            multi_window_commands(&mut state, SurfaceId::PRIMARY, Message::OpenSecondWindow)
                .is_empty()
        );
        assert_eq!(
            multi_window_commands(&mut state, SECOND_WINDOW, Message::CloseCurrentWindow),
            vec![SurfaceCommand::Close(SECOND_WINDOW)]
        );

        <MultiWindowDemo as MultiWindowAppLogic>::surface_closed(&mut state, SECOND_WINDOW);
        assert!(!state.second_window_open);
        <MultiWindowDemo as MultiWindowAppLogic>::surface_created(&mut state, SECOND_WINDOW);
        assert!(state.second_window_open);
        <MultiWindowDemo as MultiWindowAppLogic>::surface_closed(&mut state, SECOND_WINDOW);
        let reopened = multi_window_commands(&mut state, SurfaceId::PRIMARY, on_press.clone());
        assert!(matches!(
            &reopened[..],
            [SurfaceCommand::Open(request), ..] if request.surface == SECOND_WINDOW
        ));
    }

    #[test]
    fn temporary_window_uses_constrained_topmost_hidden_configuration() {
        let request = second_window_request();

        assert_eq!(
            request.window.position(),
            Some(WindowPosition::new(160, 140))
        );
        assert!(request.window.is_resizable());
        assert_eq!(request.window.min_inner_size().unwrap().width(), 320);
        assert_eq!(request.window.max_inner_size().unwrap().height(), 420);
        assert_eq!(request.window.window_level(), WindowLevel::AlwaysOnTop);
        assert!(!request.window.is_visible());
        assert!(request.window.closes_on_focus_loss());
    }

    #[test]
    fn temporary_window_requests_redraw_only_on_focus_gain() {
        assert_eq!(
            surface_focus_commands(SECOND_WINDOW, SurfaceEvent::FocusChanged(true)),
            vec![SurfaceCommand::RequestRedraw(SECOND_WINDOW)]
        );
        assert!(
            surface_focus_commands(SECOND_WINDOW, SurfaceEvent::FocusChanged(false)).is_empty()
        );
        assert!(
            surface_focus_commands(SurfaceId::PRIMARY, SurfaceEvent::FocusChanged(true)).is_empty()
        );
    }

    #[test]
    fn second_and_unknown_windows_render_rich_text() {
        let Widget::Column { children, .. } = second_window_view() else {
            panic!("second window view must be a centered Column")
        };
        let Widget::RichText { content, style } = &children[0] else {
            panic!("second window phrase must use RichText")
        };
        assert_eq!(
            content.plain_text(),
            "Hello from the temporary window — it closes after focus moves away."
        );
        assert_eq!(
            content.spans()[1].style().weight(),
            Some(RichTextWeight::BOLD)
        );
        assert_eq!(style.size.width, Dimension::percent(1.0));
        assert_eq!(style.max_size.width, Dimension::length(480.0));
        assert_eq!(style.size.height, Dimension::auto());

        let Widget::Column { children, .. } = unknown_window_view(SurfaceId::new(99)) else {
            panic!("unknown window view must be a centered Column")
        };
        assert!(
            matches!(&children[0], Widget::RichText { content, .. } if content.plain_text() == "Unknown window ID: 99")
        );
    }
}

#[cfg(test)]
mod page_tests {
    use super::super::theme_selector::test_support::assert_theme_toggle;
    use super::*;

    #[test]
    fn multi_window_example_starts_dark_with_selectors_on_both_windows() {
        let mut fonts = FontSystem::new();
        let mut state = <MultiWindowDemo as MultiWindowAppLogic>::new(&mut fonts);
        assert_eq!(
            MultiWindowDemo::theme_for(&state).surface,
            Theme::dark().surface
        );
        let primary = MultiWindowDemo::view(&mut state, SurfaceId::PRIMARY);
        assert_theme_toggle(&primary);
        drop(primary);
        let second = MultiWindowDemo::view(&mut state, SurfaceId::new(1));
        assert_theme_toggle(&second);
    }

    #[test]
    fn pages_reflow_and_retain_reachable_documents() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(FontSystem::new()));
        let mut state = MultiWindowDemo::new(&mut fonts.borrow_mut());
        for surface in [SurfaceId::PRIMARY, SurfaceId::new(1)] {
            let view = MultiWindowDemo::view(&mut state, surface);
            super::super::layout::test_support::assert_page(&view, &fonts, "multi_window_demo");
        }
    }
}
