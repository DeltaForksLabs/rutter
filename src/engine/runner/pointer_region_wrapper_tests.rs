//! Pointer wrappers must preserve the event routes owned by their descendants.
use super::*;
use crate::widget::{ToastKind, ToastPosition};
use winit::event::{MouseScrollDelta, TouchPhase};

#[derive(Clone, Debug, PartialEq)]
enum RouteMessage {
    Ignore,
    Opening(u64),
    Popup,
}
struct RouteModel {
    captured: bool,
    popover: bool,
    messages: Vec<RouteMessage>,
}
struct RouteApp;
const MENU: &[ContextMenuEntry<'static, RouteMessage>] = &[ContextMenuEntry::Item {
    label: "Open",
    on_select: Some(RouteMessage::Ignore),
}];

fn fixed(height: f32) -> Style {
    Style {
        size: Size::from_lengths(240.0, height),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn wrap<'a>(
    captured: bool,
    child: Widget<'a, RouteMessage>,
    height: f32,
) -> Widget<'a, RouteMessage> {
    if captured {
        Widget::pointer_region_with(
            WidgetId::manual(17).unwrap(),
            child,
            PointerRegionConfigWith::new(|_| RouteMessage::Ignore)
                .with_hover(|_| RouteMessage::Ignore),
            fixed(height),
        )
    } else {
        Widget::pointer_region(
            WidgetId::manual(17).unwrap(),
            child,
            PointerRegionConfig::new(|_| RouteMessage::Ignore).with_hover(|_| RouteMessage::Ignore),
            fixed(height),
        )
    }
}

impl AppLogic for RouteApp {
    type State = RouteModel;
    type Message = RouteMessage;
    fn new(_: &mut FontSystem) -> RouteModel {
        RouteModel {
            captured: false,
            popover: false,
            messages: Vec::new(),
        }
    }
    fn view<'a>(model: &'a mut RouteModel) -> Widget<'a, RouteMessage> {
        if model.popover {
            return wrap(
                model.captured,
                Widget::popover(
                    true,
                    Widget::Button {
                        text: "Anchor",
                        on_press: RouteMessage::Ignore,
                        style: fixed(40.0),
                        color: None,
                        variant: Default::default(),
                    },
                    Widget::Button {
                        text: "Popup",
                        on_press: RouteMessage::Popup,
                        style: fixed(40.0),
                        color: None,
                        variant: Default::default(),
                    },
                    None,
                    fixed(40.0),
                    fixed(40.0),
                )
                .with_id(33),
                40.0,
            );
        }
        wrap(
            model.captured,
            Widget::ScrollView {
                id: 31,
                child: Box::new(Widget::ContextMenu {
                    id: 32,
                    child: Box::new(Widget::Spacer {
                        style: fixed(400.0),
                    }),
                    entries: MENU,
                    style: fixed(400.0),
                }),
                style: fixed(80.0),
            },
            80.0,
        )
    }
    fn update(model: &mut RouteModel, message: RouteMessage, _: &mut Clipboard) {
        model.messages.push(message);
    }
    fn context_menu_opening(_: &RouteModel, target: ContextMenuTarget) -> Option<RouteMessage> {
        Some(RouteMessage::Opening(target.id()))
    }
}

fn route_runner(captured: bool, popover: bool) -> RutterRunner<RouteApp> {
    let mut r = RutterRunner::with_engine(crate::engine::RutterEngine::<RouteApp>::new().unwrap());
    r.engine.app_state.captured = captured;
    r.engine.app_state.popover = popover;
    r.engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(320, 160),
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    r.active_window_id = Some(WindowId::from_raw(1));
    r.engine
        .try_ensure_layout_for_input(PhysicalSize::new(320, 160))
        .unwrap();
    r
}

#[test]
fn pointer_region_review_wrapper_wheel_and_secondary_button_route_through_both_families() {
    for captured in [false, true] {
        let mut r = route_runner(captured, false);
        motion(&mut r, 24.0, 24.0);
        event(
            &mut r,
            WindowEvent::MouseWheel {
                device_id: None,
                delta: MouseScrollDelta::LineDelta(0.0, -2.0),
                phase: TouchPhase::Moved,
            },
        );
        assert_eq!(r.engine.active_scroll_id, Some(31));
        assert!(r.engine.widget_states[&31].as_scroll().unwrap().offset_y > 0.0);
        event(
            &mut r,
            WindowEvent::PointerButton {
                device_id: None,
                state: ElementState::Pressed,
                primary: true,
                position: PhysicalPosition::new(24.0, 24.0),
                button: ButtonSource::Mouse(MouseButton::Right),
                is_macos_activation_click: false,
            },
        );
        assert!(
            r.engine
                .app_state
                .messages
                .contains(&RouteMessage::Opening(32))
        );
        assert!(r.engine.any_context_menu_open());
        assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
    }
}

#[test]
fn pointer_region_review_wrapped_popover_receives_real_click_outside_wrapper_bounds() {
    for captured in [false, true] {
        let mut r = route_runner(captured, true);
        button(&mut r, 24.0, 64.0, ElementState::Pressed);
        button(&mut r, 24.0, 64.0, ElementState::Released);
        assert!(r.engine.app_state.messages.contains(&RouteMessage::Popup));
        assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
    }
}

#[test]
fn pointer_region_review_other_wrapper_lookups_retain_descendant_paths() {
    use crate::render::hit_test::{find_input_geometry, find_toast_dismiss_msg, find_vlist_props};
    for captured in [false, true] {
        let list = wrap(
            captured,
            Widget::virtual_list(
                40.0,
                12,
                &|index| Some(index.to_string()),
                |_| RouteMessage::Ignore,
                fixed(80.0),
            )
            .with_id(41),
            80.0,
        );
        assert_eq!(find_vlist_props(&list, 41), Some((40.0, 12)));
        let toast = wrap(
            captured,
            Widget::Toast {
                id: 42,
                visible: true,
                message: "Dismiss",
                kind: ToastKind::Info,
                position: ToastPosition::BottomRight,
                duration_ms: 1000,
                on_dismiss: Some(RouteMessage::Ignore),
            },
            80.0,
        );
        assert_eq!(
            find_toast_dismiss_msg(&toast, 42),
            Some(RouteMessage::Ignore)
        );
        let input = wrap(
            captured,
            Widget::text_input(
                |_| RouteMessage::Ignore,
                None,
                fixed(40.0),
                "",
                "",
                InputState::Idle,
                None,
                false,
            )
            .with_id(43),
            40.0,
        );
        let mut tree = TaffyTree::new();
        let root = crate::layout::build_layout_tree(&mut tree, &input, LayoutDirection::Ltr);
        crate::layout::compute_layout_with_renderer(
            &mut tree,
            root,
            PhysicalSize::new(320, 160),
            &crate::render::rich_text::RichTextRenderer::new(),
        );
        let geometry = find_input_geometry(&input, &tree, root, 43).unwrap();
        assert!(geometry.width > 0.0 && geometry.height > 0.0);
    }
}
