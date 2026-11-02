//! Native editing and pointer dispatch must use the current view's capture.
use super::*;
use crate::Widget;
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::widget::InputState;
use accesskit::{Action, ActionHandler, ActionRequest, NodeId, TreeId};
use std::sync::Arc;
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, PhysicalKey};

const VIEWPORT: PhysicalSize<u32> = PhysicalSize::new(320, 640);

#[path = "captured_virtual_slider_tests.rs"]
mod virtual_sliders;

#[derive(Clone, Debug, PartialEq)]
struct Context {
    instance: String,
    epoch: u64,
}

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Text(Context, String),
    Slider(Context, f32),
}

struct Model {
    epoch: u64,
    enabled: bool,
    visible: bool,
    controlled: bool,
    password: bool,
    legacy_sliders: bool,
    texts: [String; 3],
    sliders: [f32; 2],
    width: f32,
    owner: Arc<()>,
    messages: Vec<Message>,
}

struct CapturedNativeApp;

fn context(id: u64, epoch: u64) -> Context {
    Context {
        instance: format!("native-{id}"),
        epoch,
    }
}

fn control_style(width: f32, height: f32) -> Style {
    Style {
        size: Size::from_lengths(width, height),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

impl AppLogic for CapturedNativeApp {
    type State = Model;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Model {
        Model {
            epoch: 0,
            enabled: true,
            visible: true,
            controlled: false,
            password: false,
            legacy_sliders: false,
            texts: Default::default(),
            sliders: [0.0; 2],
            width: 240.0,
            owner: Arc::new(()),
            messages: Vec::new(),
        }
    }

    fn view(model: &mut Model) -> Widget<'_, Message> {
        let mut children = Vec::new();
        if model.visible {
            for id in 1..=3 {
                let snapshot = context(id, model.epoch);
                let owner = model.owner.clone();
                let change = move |text| {
                    let _keep_alive = &owner;
                    Message::Text(snapshot.clone(), text)
                };
                let widget = if id == 3 {
                    Widget::text_area_with(
                        change,
                        None,
                        control_style(model.width, 120.0),
                        "Multiline",
                        InputState::Error,
                        "Type here",
                        Some("Required".into()),
                    )
                } else if model.controlled {
                    Widget::controlled_text_input_with(
                        &model.texts[id as usize - 1],
                        change,
                        None,
                        control_style(model.width, 44.0),
                        "Name",
                        "Type here",
                        InputState::Error,
                        Some("Required".into()),
                        model.password,
                    )
                } else {
                    Widget::text_input_with(
                        change,
                        None,
                        control_style(model.width, 44.0),
                        "Name",
                        "Type here",
                        InputState::Error,
                        Some("Required".into()),
                        model.password,
                    )
                };
                children.push(widget.with_id(id).enabled(model.enabled));
            }
            for id in 4..=5 {
                let snapshot = context(id, model.epoch);
                let owner = model.owner.clone();
                let slider = Widget::slider_with(
                    model.sliders[id as usize - 4],
                    0.0,
                    100.0,
                    1.0,
                    move |value| {
                        let _keep_alive = &owner;
                        Message::Slider(snapshot.clone(), value)
                    },
                    control_style(model.width, 44.0),
                    "Amount",
                );
                let slider = if model.legacy_sliders {
                    Widget::slider(
                        model.sliders[id as usize - 4],
                        0.0,
                        100.0,
                        1.0,
                        if id == 4 {
                            legacy_first_slider
                        } else {
                            legacy_second_slider
                        },
                        control_style(model.width, 44.0),
                        "Amount",
                    )
                } else {
                    slider
                };
                children.push(slider.with_id(id).enabled(model.enabled));
            }
        }
        Widget::Column {
            children,
            style: Style::default(),
        }
    }

    fn update(model: &mut Model, message: Message, _: &mut arboard::Clipboard) {
        match &message {
            Message::Text(snapshot, value) => {
                let id = snapshot
                    .instance
                    .strip_prefix("native-")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                model.texts[id - 1] = value.clone();
            }
            Message::Slider(snapshot, value) => {
                let id = snapshot
                    .instance
                    .strip_prefix("native-")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                model.sliders[id - 4] = *value;
            }
        }
        model.messages.push(message);
    }
}

fn runner() -> RutterRunner<CapturedNativeApp> {
    runner_with_password(false)
}

fn runner_with_password(password: bool) -> RutterRunner<CapturedNativeApp> {
    let mut engine = RutterEngine::<CapturedNativeApp>::new().expect("owned Xvfb clipboard");
    engine.app_state.password = password;
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut runner);
    runner
}

fn legacy_first_slider(value: f32) -> Message {
    Message::Slider(context(4, 0), value)
}
fn legacy_second_slider(value: f32) -> Message {
    Message::Slider(context(5, 0), value)
}

fn publish(runner: &mut RutterRunner<CapturedNativeApp>) {
    runner.engine.layout_dirty = true;
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}

fn event(runner: &mut RutterRunner<CapturedNativeApp>, event: WindowEvent) {
    runner.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
}

fn ime(runner: &mut RutterRunner<CapturedNativeApp>, text: &str) {
    event(runner, WindowEvent::Ime(Ime::Commit(text.into())));
}

fn key(runner: &mut RutterRunner<CapturedNativeApp>, key: NamedKey) {
    event(
        runner,
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Space),
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
}

fn press_slider(runner: &mut RutterRunner<CapturedNativeApp>, id: u64, fraction: f32) {
    publish(runner);
    let frame = runner.engine.runtime_caches.viewport_geometry.sliders[&id];
    let point = Point::new(
        frame.left + 16.0 + fraction * (frame.width() - 32.0),
        frame.center_y(),
    );
    event(
        runner,
        WindowEvent::PointerButton {
            device_id: None,
            state: ElementState::Pressed,
            primary: true,
            position: PhysicalPosition::new(point.x as f64, point.y as f64),
            button: ButtonSource::Mouse(MouseButton::Left),
            is_macos_activation_click: false,
        },
    );
    assert_eq!(runner.engine.drag_slider_id, Some(id));
}

fn accessibility(runner: &mut RutterRunner<CapturedNativeApp>, id: u64, action: Action) {
    runner
        .engine
        .accessibility_actions
        .handler()
        .do_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: NodeId(id),
            data: None,
        });
    runner.process_accessibility_actions();
    assert!(runner.fatal_error.is_none());
}

#[test]
fn captured_native_ime_refreshes_before_redraw_and_old_clones_keep_their_epoch() {
    let mut runner = runner();
    for id in 1..=3 {
        let old = runner.engine.runtime_caches.inputs[&id].on_change.clone();
        runner.focus_widget(Some(id));
        runner.engine.app_state.epoch = id + 10;
        runner.engine.layout_dirty = true;
        ime(&mut runner, "é東京");
        assert_eq!(
            runner.engine.app_state.messages.last(),
            Some(&Message::Text(context(id, id + 10), "é東京".into()))
        );
        assert_eq!(
            old.call("old".into()),
            Message::Text(context(id, if id == 1 { 0 } else { id + 9 }), "old".into())
        );
    }
}

#[test]
fn captured_native_ime_cannot_edit_disabled_or_removed_input_before_redraw() {
    for removed in [false, true] {
        let mut runner = runner();
        runner.focus_widget(Some(1));
        if removed {
            runner.engine.app_state.visible = false;
        } else {
            runner.engine.app_state.enabled = false;
        }
        runner.engine.layout_dirty = true;
        ime(&mut runner, "must not dispatch");
        assert!(runner.engine.app_state.messages.is_empty());
        assert!(runner.engine.runtime_caches.inputs.is_empty());
    }
}

#[test]
fn captured_native_editing_selection_paste_history_and_multiline_enter_use_current_capture() {
    let mut runner = runner();
    runner.focus_widget(Some(1));
    ime(&mut runner, "selected");
    runner.do_select_all();
    runner.engine.app_state.epoch = 7;
    runner.engine.layout_dirty = true;
    key(&mut runner, NamedKey::Delete);
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Text(context(1, 7), String::new()))
    );

    runner.engine.clipboard.set_text("pasted").unwrap();
    runner.refresh_layout_before_keyboard_input().unwrap();
    runner.do_paste();
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Text(context(1, 7), "pasted".into()))
    );
    runner.engine.app_state.epoch = 8;
    runner.engine.layout_dirty = true;
    runner.refresh_layout_before_keyboard_input().unwrap();
    runner.do_undo();
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Text(context(1, 8), String::new()))
    );
    runner.engine.app_state.epoch = 9;
    runner.engine.layout_dirty = true;
    runner.refresh_layout_before_keyboard_input().unwrap();
    runner.do_redo();
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Text(context(1, 9), "pasted".into()))
    );

    runner.focus_widget(Some(3));
    ime(&mut runner, "line");
    runner.engine.app_state.epoch = 10;
    runner.engine.layout_dirty = true;
    key(&mut runner, NamedKey::Enter);
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Text(context(3, 10), "line\n".into()))
    );
    key(&mut runner, NamedKey::Backspace);
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Text(context(3, 10), "line".into()))
    );
}

#[test]
fn captured_native_slider_pointer_drag_keyboard_accessibility_refresh_context_and_geometry() {
    let mut runner = runner();
    let old = runner.engine.runtime_caches.sliders[&4].on_change.clone();
    press_slider(&mut runner, 4, 0.25);
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Slider(context(4, 0), 25.0))
    );
    runner.engine.app_state.epoch = 7;
    runner.engine.app_state.width = 160.0;
    runner.engine.layout_dirty = true;
    // This x is 50% of the NEW track (16..144), not the press-time track.
    let y = runner.engine.runtime_caches.viewport_geometry.sliders[&4].center_y();
    event(
        &mut runner,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(80.0, y as f64),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Slider(context(4, 7), 50.0))
    );
    assert_eq!(old.call(3.0), Message::Slider(context(4, 0), 3.0));
    runner.focus_widget(Some(5));
    runner.engine.app_state.epoch = 8;
    runner.engine.layout_dirty = true;
    key(&mut runner, NamedKey::ArrowRight);
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Slider(context(5, 8), 1.0))
    );
    runner.engine.app_state.epoch = 9;
    runner.engine.layout_dirty = true;
    accessibility(&mut runner, 4, Action::Increment);
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Slider(context(4, 9), 51.0))
    );
}

#[test]
fn captured_native_slider_capture_cancels_on_disable_remove_focus_exit_and_teardown() {
    for cancellation in [
        "disable",
        "remove",
        "focus",
        "exit",
        "teardown",
        "widget-focus",
    ] {
        let mut runner = runner();
        press_slider(&mut runner, 4, 0.25);
        match cancellation {
            "disable" => {
                runner.engine.app_state.enabled = false;
                publish(&mut runner);
            }
            "remove" => {
                runner.engine.app_state.visible = false;
                publish(&mut runner);
            }
            "focus" => event(&mut runner, WindowEvent::Focused(false)),
            "exit" => event(
                &mut runner,
                WindowEvent::PointerLeft {
                    device_id: None,
                    kind: PointerKind::Mouse,
                    primary: true,
                    position: None,
                },
            ),
            "teardown" => runner.release_surface(),
            "widget-focus" => runner.focus_widget(Some(1)),
            _ => unreachable!(),
        }
        assert_eq!(runner.engine.drag_slider_id, None, "{cancellation}");
        assert!(
            !runner
                .engine
                .widget_states
                .get(&4)
                .and_then(WidgetState::as_slider)
                .is_some_and(|state| state.dragging)
        );
        if matches!(cancellation, "disable" | "remove") {
            runner.engine.app_state.enabled = true;
            runner.engine.app_state.visible = true;
            publish(&mut runner);
            // A retained event for the old ID cannot acquire the new lifetime.
            runner.update_slider_drag(4, 200.0).unwrap();
            assert_eq!(runner.engine.app_state.messages.len(), 1);
        }
    }
}

#[test]
fn captured_native_drag_publishes_disable_and_removal_before_callback_lookup() {
    for removed in [false, true] {
        let mut runner = runner();
        press_slider(&mut runner, 4, 0.25);
        if removed {
            runner.engine.app_state.visible = false;
        } else {
            runner.engine.app_state.enabled = false;
        }
        runner.engine.layout_dirty = true;
        runner.update_slider_drag(4, 100.0).unwrap();
        assert_eq!(runner.engine.app_state.messages.len(), 1);
        assert_eq!(runner.engine.drag_slider_id, None);
    }
}

#[test]
fn captured_native_drag_keeps_its_native_lifetime_across_callback_forms() {
    let mut runner = runner();
    press_slider(&mut runner, 4, 0.25);
    let generation = runner.engine.runtime_caches.sliders[&4].generation;
    runner.engine.app_state.legacy_sliders = true;
    runner.engine.layout_dirty = true;
    runner.update_slider_drag(4, 120.0).unwrap();
    assert_eq!(
        runner.engine.runtime_caches.sliders[&4].generation,
        generation
    );
    assert_eq!(runner.engine.drag_slider_id, Some(4));
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Slider(context(4, 0), 50.0))
    );
    runner.engine.app_state.legacy_sliders = false;
    runner.engine.app_state.epoch = 11;
    runner.engine.layout_dirty = true;
    runner.update_slider_drag(4, 172.0).unwrap();
    assert_eq!(
        runner.engine.runtime_caches.sliders[&4].generation,
        generation
    );
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&Message::Slider(context(4, 11), 75.0))
    );
}

#[test]
fn captured_native_controlled_echo_preserves_caret_selection_and_undo_without_changes() {
    let mut runner = runner();
    runner.engine.app_state.controlled = true;
    publish(&mut runner);
    runner.focus_widget(Some(1));
    ime(&mut runner, "draft");
    runner.engine.input_states.get_mut(&1).unwrap().snapshot();
    runner.do_select_all();
    let cursor = runner.engine.input_states[&1].editor.cursor();
    let selection = runner.engine.input_states[&1].editor.selection();
    publish(&mut runner);
    assert_eq!(runner.engine.app_state.messages.len(), 1);
    assert_eq!(runner.engine.input_states[&1].editor.cursor(), cursor);
    assert_eq!(runner.engine.input_states[&1].editor.selection(), selection);
    assert!(runner.engine.input_states[&1].undo.can_undo());
    runner.engine.app_state.texts[0] = "replacement".into();
    publish(&mut runner);
    assert_eq!(runner.engine.input_states[&1].text(), "replacement");
    assert!(!runner.engine.input_states[&1].undo.can_undo());
    assert_eq!(runner.engine.app_state.messages.len(), 1);
}

#[test]
fn captured_native_controlled_password_sync_and_callback_ownership() {
    let mut runner = runner_with_password(true);
    runner.engine.app_state.controlled = true;
    runner.engine.app_state.texts[0] = "secret".into();
    publish(&mut runner);
    assert!(runner.engine.app_state.messages.is_empty());
    assert_eq!(runner.engine.input_states[&1].text(), "secret");
    assert!(runner.engine.input_states[&1].is_sensitive());
    let update = {
        let widget = CapturedNativeApp::view(&mut runner.engine.app_state);
        crate::accessibility::build_accessibility_update(
            &runner.engine.taffy,
            &widget,
            runner.engine.last_root_node,
            crate::accessibility::AccessibilityInputs {
                input_states: &runner.engine.input_states,
                widget_states: &runner.engine.widget_states,
                focused_widget_id: None,
                viewport: (320.0, 640.0),
                direction: LayoutDirection::Ltr,
            },
        )
    };
    let password = update.nodes.iter().find(|(id, _)| id.0 == 1).unwrap();
    assert_eq!(password.1.role(), accesskit::Role::PasswordInput);
    assert_eq!(password.1.value(), None);

    let weak = Arc::downgrade(&runner.engine.app_state.owner);
    let retained = runner.engine.runtime_caches.inputs[&1].on_change.clone();
    let retained_area = runner.engine.runtime_caches.inputs[&3].on_change.clone();
    let retained_slider = runner.engine.runtime_caches.sliders[&4].on_change.clone();
    runner.engine.app_state.owner = Arc::new(());
    runner.engine.app_state.enabled = false;
    publish(&mut runner);
    assert!(weak.upgrade().is_some());
    drop(retained);
    assert!(weak.upgrade().is_some());
    drop(retained_area);
    assert!(weak.upgrade().is_some());
    drop(retained_slider);
    assert!(
        weak.upgrade().is_none(),
        "disabled callbacks and scratch cache must release captures"
    );
}
