use super::*;
use rutter::{RichTextWeight, WindowLevel, WindowPosition};

#[test]
fn demo_starts_with_only_the_centered_primary_window() {
    let requests = <MultiWindowDemo as MultiWindowAppLogic>::initial_surfaces();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].surface, SurfaceId::PRIMARY);
    assert_eq!(requests[0].window.title(), "Rutter Multi-Window Demo");

    let Widget::Column { children, style } = main_window_view() else {
        panic!("main demo view must be a centered Column")
    };
    assert_eq!(style.align_items, Some(AlignItems::Center));
    assert_eq!(style.justify_content, Some(JustifyContent::Center));
    assert_eq!(style.size, Size::percent(1.0_f32));
    assert!(matches!(&children[0], Widget::Button { text, .. } if *text == "Open Second Window"));
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
        multi_window_commands(&mut state, SurfaceId::PRIMARY, Message::OpenSecondWindow).is_empty()
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
    assert_eq!(request.window.min_inner_size().unwrap().width(), 480);
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
    assert!(surface_focus_commands(SECOND_WINDOW, SurfaceEvent::FocusChanged(false)).is_empty());
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
    assert_eq!(style.size.height, Dimension::length(72.0));

    let Widget::Column { children, .. } = unknown_window_view(SurfaceId::new(99)) else {
        panic!("unknown window view must be a centered Column")
    };
    assert!(
        matches!(&children[0], Widget::RichText { content, .. } if content.plain_text() == "Unknown window ID: 99")
    );
}
