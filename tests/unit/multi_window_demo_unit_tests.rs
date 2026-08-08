use super::*;
use rutter::RichTextWeight;

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
    assert!(
        matches!(&commands[..], [SurfaceCommand::Open(request)] if request.surface == SECOND_WINDOW && request.window.title() == "Second Window")
    );
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
    assert!(
        matches!(&reopened[..], [SurfaceCommand::Open(request)] if request.surface == SECOND_WINDOW)
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
        "Hello from the second window — rendered with RichText!"
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
