//! Exercises real publication, keyboard, wheel/drag state and pointer routing.
use super::*;
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::widget::{InputState, Widget};
use taffy::prelude::{Dimension, FlexDirection, Size, Style};
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, ModifiersState, PhysicalKey};

struct ScrollApp;

#[derive(Default)]
struct Model {
    nested: bool,
    popover: bool,
    open: bool,
    overlay_scroll: bool,
    collapsed: bool,
    messages: Vec<u64>,
}

fn fixed(height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(200.0),
            height: Dimension::length(height),
        },
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        ..Style::default()
    }
}

fn column<'a>(children: Vec<Widget<'a, u64>>) -> Widget<'a, u64> {
    Widget::Column {
        children,
        style: Style {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..Style::default()
        },
    }
}

fn spacer<'a>(height: f32) -> Widget<'a, u64> {
    Widget::Spacer {
        style: fixed(height),
    }
}

fn button<'a>(id: u64) -> Widget<'a, u64> {
    Widget::Button {
        text: "Action",
        on_press: id,
        style: fixed(30.0),
        color: None,
        variant: Default::default(),
    }
    .with_id(id)
}

fn input<'a>() -> Widget<'a, u64> {
    Widget::text_input(
        |_| 20,
        None,
        fixed(30.0),
        "Label",
        "",
        InputState::Idle,
        Some("Error".into()),
        false,
    )
    .with_id(2)
}

impl AppLogic for ScrollApp {
    type State = Model;
    type Message = u64;
    fn new(_: &mut FontSystem) -> Model {
        Model::default()
    }
    fn view(model: &mut Model) -> Widget<'_, u64> {
        let content = if model.popover {
            let popup = if model.overlay_scroll {
                Widget::scroll_view(column(vec![button(4), spacer(200.0), input()]), fixed(90.0))
                    .with_id(12)
            } else {
                button(4)
            };
            let popover = Widget::popover(
                model.open,
                button(1),
                popup,
                Some(99),
                fixed(30.0),
                fixed(if model.overlay_scroll { 100.0 } else { 40.0 }),
            )
            .with_id(5);
            let popover = if model.collapsed {
                Widget::Accordion {
                    id: 6,
                    title: "Hidden",
                    expanded: false,
                    on_toggle: 88,
                    child: Box::new(popover),
                    style: fixed(30.0),
                }
            } else {
                popover
            };
            column(vec![spacer(180.0), popover, spacer(300.0), button(3)])
        } else {
            column(vec![
                button(1),
                spacer(180.0),
                input(),
                spacer(180.0),
                button(3),
            ])
        };
        let content = if model.nested {
            column(vec![
                spacer(160.0),
                Widget::scroll_view(content, fixed(100.0)).with_id(11),
                spacer(200.0),
            ])
        } else {
            content
        };
        Widget::scroll_view(content, fixed(120.0)).with_id(10)
    }
    fn update(model: &mut Model, message: u64, _: &mut arboard::Clipboard) {
        model.messages.push(message);
        if message == 99 {
            model.open = false;
        }
    }
}

fn runner(model: Model) -> RutterRunner<ScrollApp> {
    let mut engine = RutterEngine::<ScrollApp>::new().expect("isolated clipboard service");
    engine.app_state = model;
    engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(240, 120),
        scale: 1.0,
        popup: false,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut runner);
    runner
}

fn publish(runner: &mut RutterRunner<ScrollApp>) {
    runner.refresh_layout_before_keyboard_input().unwrap();
    assert!(runner.fatal_error.is_none());
}

fn key(runner: &mut RutterRunner<ScrollApp>, key: NamedKey, shift: bool) {
    runner.engine.modifiers = if shift {
        ModifiersState::SHIFT
    } else {
        ModifiersState::empty()
    }
    .into();
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Tab),
                logical_key: Key::Named(key),
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
                text_with_all_modifiers: None,
                key_without_modifiers: Key::Named(key),
            },
        },
    );
    publish(runner);
}

fn offset(runner: &RutterRunner<ScrollApp>, id: u64) -> f32 {
    runner.engine.widget_states[&id]
        .as_scroll()
        .unwrap()
        .offset_y
}

fn button_focus(runner: &RutterRunner<ScrollApp>, message: u64) -> u64 {
    *runner
        .engine
        .runtime_caches
        .buttons
        .iter()
        .find(|(_, value)| **value == message)
        .unwrap()
        .0
}

fn scroll(runner: &mut RutterRunner<ScrollApp>, id: u64, delta: f32) {
    runner.engine.active_scroll_id = Some(id);
    assert!(runner.scroll_active_target(0.0, delta));
    publish(runner);
}

fn anchor(runner: &RutterRunner<ScrollApp>) -> super::super::PopoverState {
    runner.engine.widget_states[&5]
        .as_popover()
        .unwrap()
        .clone()
}

fn click(runner: &mut RutterRunner<ScrollApp>, x: f64, y: f64) {
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::PointerButton {
            device_id: None,
            state: ElementState::Pressed,
            position: PhysicalPosition::new(x, y),
            primary: true,
            button: ButtonSource::Mouse(MouseButton::Left),
            is_macos_activation_click: false,
        },
    );
    assert!(runner.fatal_error.is_none());
}

#[test]
fn task61_short_page_tab_and_shift_tab_reveal_and_activate_controls() {
    let mut runner = runner(Model::default());
    key(&mut runner, NamedKey::Tab, false);
    assert_eq!(
        runner.engine.focused_widget_id,
        Some(button_focus(&runner, 1))
    );
    assert_eq!(offset(&runner, 10), 0.0);
    key(&mut runner, NamedKey::Tab, false);
    assert_eq!(runner.engine.focused_widget_id, Some(2));
    assert!(offset(&runner, 10) > 0.0);
    let body = runner.focused_input_rect.unwrap();
    assert!(body.top >= 20.0 && body.bottom <= 100.0);
    let widget = ScrollApp::view(&mut runner.engine.app_state);
    assert!(
        matches!(hit_test(&widget, &runner.engine.taffy, runner.engine.last_root_node,
        Point::new(body.left + 18.0, body.top + 9.0), Point::new(0.0, 0.0), &runner.engine.widget_states),
        Some(HitResult::InputFocus { id: 2, local_x, local_y, width, height })
        if local_x == 18.0 && local_y == 9.0 && width == body.width() && height == body.height())
    );
    drop(widget);
    key(&mut runner, NamedKey::Tab, false);
    assert_eq!(
        runner.engine.focused_widget_id,
        Some(button_focus(&runner, 3))
    );
    let bottom_offset = offset(&runner, 10);
    key(&mut runner, NamedKey::Enter, false);
    assert_eq!(runner.engine.app_state.messages, vec![3]);
    key(&mut runner, NamedKey::Tab, true);
    assert_eq!(runner.engine.focused_widget_id, Some(2));
    assert!(offset(&runner, 10) < bottom_offset);
    key(&mut runner, NamedKey::Tab, true);
    assert_eq!(
        runner.engine.focused_widget_id,
        Some(button_focus(&runner, 1))
    );
    assert_eq!(offset(&runner, 10), 0.0);
}

#[test]
fn task61_nested_scroll_focus_reveals_inside_out_and_reverse_wraps() {
    let mut runner = runner(Model {
        nested: true,
        ..Model::default()
    });
    key(&mut runner, NamedKey::Tab, true);
    assert_eq!(
        runner.engine.focused_widget_id,
        Some(button_focus(&runner, 3))
    );
    assert!(offset(&runner, 10) > 0.0 && offset(&runner, 11) > 0.0);
    key(&mut runner, NamedKey::Tab, true);
    assert_eq!(runner.engine.focused_widget_id, Some(2));
    let rect = runner.focused_input_rect.unwrap();
    assert!(rect.top >= 0.0 && rect.bottom <= 120.0);
    key(&mut runner, NamedKey::Tab, true);
    assert_eq!(offset(&runner, 11), 0.0);
    assert!(offset(&runner, 10) > 0.0);
}

#[test]
fn task61_popover_open_after_scroll_and_while_open_tracks_live_anchor() {
    let mut runner = runner(Model {
        popover: true,
        ..Model::default()
    });
    scroll(&mut runner, 10, 160.0);
    assert_eq!(anchor(&runner).anchor_y, 20.0);
    runner.engine.app_state.open = true;
    runner.engine.layout_dirty = true;
    publish(&mut runner);
    assert!(anchor(&runner).is_open);
    assert_eq!(anchor(&runner).anchor_y, 20.0);
    scroll(&mut runner, 10, 10.0);
    assert_eq!(anchor(&runner).anchor_y, 10.0);
    assert!(anchor(&runner).is_open);
    // Pointer routing uses the same anchor as painting, with no parent redraw.
    click(&mut runner, 40.0, 65.0);
    assert_eq!(runner.engine.app_state.messages, vec![4]);
}

#[test]
fn task61_hidden_popover_suppresses_paint_and_absorbs_underlying_click() {
    let mut runner = runner(Model {
        popover: true,
        open: true,
        ..Model::default()
    });
    assert!(!anchor(&runner).is_open);
    assert!(runner.engine.any_popover_open());
    // Scrolling it into view restores presentation without another publication.
    scroll(&mut runner, 10, 180.0);
    assert!(anchor(&runner).is_open);
    let revision = runner.engine.layout_revision;
    scroll(&mut runner, 10, 1000.0);
    assert_eq!(runner.engine.layout_revision, revision);
    assert!(!anchor(&runner).is_open);
    assert!(runner.engine.any_popover_open());
    // The bottom button is visible under the logically open, clipped popup.
    click(&mut runner, 40.0, 105.0);
    assert_eq!(runner.engine.app_state.messages, vec![99]);
    publish(&mut runner);
    assert!(!runner.engine.any_popover_open());
}

#[test]
fn task61_nested_clips_and_drag_offsets_reconcile_popovers_without_layout() {
    let mut runner = runner(Model {
        nested: true,
        popover: true,
        open: true,
        ..Model::default()
    });
    scroll(&mut runner, 11, 180.0);
    assert!(!anchor(&runner).is_open); // Inner trigger is still outside outer viewport.
    scroll(&mut runner, 10, 150.0);
    assert!(anchor(&runner).is_open);
    assert_eq!(anchor(&runner).anchor_y, 10.0);
    let revision = runner.engine.layout_revision;
    apply_scrollbar_drag_offset(
        runner.engine.widget_states.get_mut(&10).unwrap(),
        140.0,
        ScrollbarAxis::Vertical,
    );
    publish(&mut runner);
    assert_eq!(runner.engine.layout_revision, revision);
    assert_eq!(anchor(&runner).anchor_y, 20.0);
    scroll(&mut runner, 11, -100.0);
    assert!(!anchor(&runner).is_open); // Clipped by inner even though it overlaps the window.
    key(&mut runner, NamedKey::Escape, false);
    assert_eq!(runner.engine.app_state.messages, vec![99]);
}

#[test]
fn task61_overlay_focus_reveals_only_overlay_scroll_ancestors() {
    let mut runner = runner(Model {
        popover: true,
        open: true,
        overlay_scroll: true,
        ..Model::default()
    });
    scroll(&mut runner, 10, 180.0);
    key(&mut runner, NamedKey::Tab, false); // Anchor.
    key(&mut runner, NamedKey::Tab, false); // Popup button.
    let body_offset = offset(&runner, 10);
    key(&mut runner, NamedKey::Tab, false); // Popup input below its viewport.
    assert_eq!(runner.engine.focused_widget_id, Some(2));
    assert!(offset(&runner, 12) > 0.0);
    assert_eq!(offset(&runner, 10), body_offset);
    let body = runner.focused_input_rect.unwrap();
    assert!(body.top >= 0.0 && body.bottom <= 120.0);
    let widget = ScrollApp::view(&mut runner.engine.app_state);
    assert!(
        matches!(hit_test_popover_overlay(&widget, &runner.engine.taffy, runner.engine.last_root_node,
        Point::new(body.left + 18.0, body.top + 9.0), (240.0, 120.0), &runner.engine.widget_states),
        Some(PopoverOverlayHit::Content(HitResult::InputFocus { id: 2, local_x, local_y, .. }))
        if local_x == 18.0 && local_y == 9.0)
    );
    drop(widget);
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::Ime(Ime::Commit("x".into())),
    );
    assert_eq!(runner.engine.input_states[&2].text(), "x");
    assert_eq!(runner.engine.app_state.messages, vec![20]);
    assert_eq!(offset(&runner, 10), body_offset);
}

#[test]
fn task61_collapsed_trigger_cannot_publish_popup_or_keyboard_content() {
    let mut runner = runner(Model {
        popover: true,
        open: true,
        collapsed: true,
        ..Model::default()
    });
    scroll(&mut runner, 10, 180.0);
    assert!(!anchor(&runner).is_open);
    key(&mut runner, NamedKey::Tab, false);
    assert_eq!(runner.engine.focused_widget_id, Some(6));
    key(&mut runner, NamedKey::Enter, false);
    assert_eq!(runner.engine.app_state.messages, vec![88]);
}

#[test]
fn task61_wheel_and_page_keys_reconcile_before_the_next_pointer_event() {
    let mut runner = runner(Model {
        popover: true,
        ..Model::default()
    });
    runner.update_cursor_position(PhysicalPosition::new(40.0, 8.0));
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::LineDelta(0.0, -4.0),
            phase: winit::event::TouchPhase::Moved,
        },
    );
    assert_eq!(offset(&runner, 10), 160.0);
    runner.engine.app_state.open = true;
    runner.engine.layout_dirty = true;
    publish(&mut runner);
    assert_eq!(anchor(&runner).anchor_y, 20.0);
    let revision = runner.engine.layout_revision;
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -10.0)),
            phase: winit::event::TouchPhase::Moved,
        },
    );
    // No redraw/publication between the wheel and popup press.
    click(&mut runner, 40.0, 65.0);
    assert_eq!(runner.engine.layout_revision, revision);
    assert_eq!(runner.engine.app_state.messages, vec![4]);

    runner.engine.app_state.open = false;
    runner.engine.layout_dirty = true;
    runner.focus_widget(None);
    publish(&mut runner);
    runner.engine.active_scroll_id = Some(10);
    key(&mut runner, NamedKey::PageUp, false);
    assert_eq!(anchor(&runner).anchor_y, 180.0 - offset(&runner, 10));
}

#[test]
fn task61_smooth_scroll_updates_anchor_and_keyboard_reveal_cancels_animation() {
    let mut runner = runner(Model {
        popover: true,
        open: true,
        ..Model::default()
    });
    let now = Instant::now();
    runner.engine.start_smooth_scroll(10, 180.0, now);
    assert!(
        runner
            .engine
            .tick_smooth_scrolls(now + Duration::from_millis(125))
    );
    publish(&mut runner);
    assert_eq!(anchor(&runner).anchor_y, 90.0);
    assert!(anchor(&runner).is_open);
    key(&mut runner, NamedKey::Tab, true); // Last control is below the trigger.
    assert_eq!(
        runner.engine.focused_widget_id,
        Some(button_focus(&runner, 3))
    );
    assert!(!runner.engine.has_pending_smooth_scroll());
    assert!(!anchor(&runner).is_open);
}
