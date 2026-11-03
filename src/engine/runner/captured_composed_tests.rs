//! Composed callbacks cross real popup, native edit and custom input paths.
use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::*;
use accesskit::{Action, ActionHandler, ActionRequest, NodeId, TreeId};
use std::sync::Arc;
use taffy::prelude::*;
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, PhysicalKey};

const VIEWPORT: PhysicalSize<u32> = PhysicalSize::new(800, 1000);

#[derive(Clone, Debug, PartialEq)]
struct Message(usize, u64, String);

// Deliberately non-Clone captures, independently owned by every callback.
struct Capture(usize, u64, Arc<()>);
impl Capture {
    fn message(&self, value: impl std::fmt::Debug) -> Message {
        let _owner = &self.2;
        Message(self.0, self.1, format!("{value:?}"))
    }
    fn edit(&self, kind: &str, value: impl std::fmt::Display) -> Message {
        let _owner = &self.2;
        Message(self.0, self.1, format!("{kind}:{value}"))
    }
}

struct Model {
    family: usize,
    epoch: u64,
    open: bool,
    visible: bool,
    owner: Arc<()>,
    colors: [ColorPickerState; 2],
    messages: Vec<Message>,
    registry: activity::ActivityRegistry,
    activity: activity::ActivityId,
}
struct ComposedApp;
fn size() -> Style {
    Style {
        size: Size::from_lengths(320.0, 44.0),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn composed<'a>(slot: usize, model: &'a Model) -> Widget<'a, Message> {
    let capture = || Capture(slot, model.epoch, model.owner.clone());
    let a = capture();
    let b = capture();
    match model.family {
        0 => Widget::date_picker_with(
            model.open,
            CalendarMonth::new(2026, 7).unwrap(),
            None,
            Message(slot, model.epoch, "toggle".into()),
            Message(slot, model.epoch, "close".into()),
            move |v| a.message(v),
            move |v| b.message(v),
            "Date",
            "YYYY-MM-DD",
            size(),
            Style {
                size: Size {
                    width: Dimension::length(300.0),
                    height: Dimension::auto(),
                },
                ..Style::default()
            },
        ),
        1 => {
            let c = capture();
            let d = capture();
            Widget::time_picker_with(
                model.open,
                TimeOfDay::new(9, 30, 0).unwrap(),
                TimePickerConfigWith::new(
                    &["UTC", "America/Sao_Paulo"],
                    0,
                    Message(slot, model.epoch, "toggle".into()),
                    Message(slot, model.epoch, "close".into()),
                    move |v| a.message(v),
                    move |v| b.message(v),
                    move |v| c.message(v),
                    move |v| d.message(v),
                    "Time",
                )
                .unwrap(),
                size(),
                Style {
                    size: Size {
                        width: Dimension::length(300.0),
                        height: Dimension::auto(),
                    },
                    ..Style::default()
                },
            )
        }
        2 => {
            let c = capture();
            let d = capture();
            let e = capture();
            let channels = std::array::from_fn(|index| {
                let context = capture();
                ChangeCallback::new(move |v: String| context.edit(&format!("channel{index}"), v))
            });
            Widget::color_picker_with(
                &model.colors[slot],
                ColorPickerIds::new(100 + slot as u64 * 20).unwrap(),
                ColorPickerCallbacksWith::new(
                    move |v| a.message(v),
                    move |v| b.message(v),
                    move |v| c.message(v),
                    move |v| d.edit("format", v),
                    move |v| e.edit("hex", v),
                    channels,
                )
                .with_copy_button(Message(slot, model.epoch, "copy".into())),
                Style {
                    size: Size {
                        width: Dimension::length(320.0),
                        height: Dimension::auto(),
                    },
                    ..Style::default()
                },
            )
        }
        _ => activity::view::details_with(
            model.registry.get(model.activity).unwrap(),
            move |id, action| a.message((id, action)),
            &Theme::light(),
        ),
    }
}

impl AppLogic for ComposedApp {
    type State = Model;
    type Message = Message;
    fn new(_: &mut FontSystem) -> Model {
        let mut registry = activity::ActivityRegistry::new();
        let owner = registry.register_owner().unwrap();
        let activity = registry
            .start(
                owner,
                activity::Transfer::new(
                    "Download 東京",
                    activity::TransferDirection::Download,
                    25,
                    Some(100),
                    activity::TransferState::Running,
                )
                .unwrap(),
            )
            .unwrap();
        Model {
            family: 0,
            epoch: 0,
            open: false,
            visible: true,
            owner: Arc::new(()),
            colors: std::array::from_fn(|_| {
                ColorPickerState::new(PickerColor::from_rgb8(103, 80, 164))
            }),
            messages: Vec::new(),
            registry,
            activity,
        }
    }
    fn view(model: &mut Model) -> Widget<'_, Message> {
        Widget::Row {
            children: if model.visible {
                (0..2).map(|slot| composed(slot, model)).collect()
            } else {
                Vec::new()
            },
            style: Style {
                gap: Size::length(24.0),
                ..Style::default()
            },
        }
    }
    fn update(model: &mut Model, message: Message, _: &mut arboard::Clipboard) {
        match message.2.as_str() {
            "toggle" => model.open = !model.open,
            "close" => model.open = false,
            _ => {}
        }
        if model.family == 2 {
            let edit = if let Some(value) = message.2.strip_prefix("format:") {
                Some(ColorPickerEvent::Format(value.parse().unwrap()))
            } else if let Some(value) = message.2.strip_prefix("hex:") {
                Some(ColorPickerEvent::Hex(value.into()))
            } else if let Some(value) = message.2.strip_prefix("channel") {
                let (index, text) = value.split_once(':').unwrap();
                Some(ColorPickerEvent::Channel {
                    index: index.parse().unwrap(),
                    text: text.into(),
                })
            } else {
                None
            };
            if let Some(edit) = edit {
                // Invalid drafts deliberately return an error while updating text.
                if let Err(error) = model.colors[message.0].update(edit) {
                    assert!(!error.to_string().is_empty());
                }
            }
        }
        model.messages.push(message);
        model.epoch += 1;
    }
}

fn runner(family: usize) -> RutterRunner<ComposedApp> {
    let mut engine = RutterEngine::<ComposedApp>::new().expect("owned Xvfb clipboard");
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    engine.app_state.family = family;
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut runner);
    runner
}
fn publish(r: &mut RutterRunner<ComposedApp>) {
    r.engine.layout_dirty = true;
    r.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}
fn tree(r: &mut RutterRunner<ComposedApp>) -> accesskit::TreeUpdate {
    r.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
    let view = ComposedApp::view(&mut r.engine.app_state);
    build_accessibility_update(
        &r.engine.taffy,
        &view,
        r.engine.last_root_node,
        AccessibilityInputs {
            input_states: &r.engine.input_states,
            widget_states: &r.engine.widget_states,
            focused_widget_id: r.engine.focused_widget_id,
            viewport: (800.0, 1000.0),
            direction: LayoutDirection::Ltr,
        },
    )
}
fn named(r: &mut RutterRunner<ComposedApp>, label: &str) -> Vec<u64> {
    tree(r)
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some(label))
        .map(|(id, _)| id.0)
        .collect()
}
fn action(r: &mut RutterRunner<ComposedApp>, id: u64, action: Action) {
    r.engine
        .accessibility_actions
        .handler()
        .do_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: NodeId(id),
            data: None,
        });
    r.process_accessibility_actions();
    assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
}
fn event(r: &mut RutterRunner<ComposedApp>, event: WindowEvent) {
    r.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
}
fn key(r: &mut RutterRunner<ComposedApp>, key: NamedKey) {
    event(
        r,
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

#[test]
fn captured_composed_date_and_time_popup_edits_use_current_context() {
    for family in [0, 1] {
        let mut r = runner(family);
        let label = if family == 0 { "Date" } else { "Time" };
        let anchors = named(&mut r, label);
        assert_eq!(anchors.len(), 2);
        action(&mut r, anchors[1], Action::Click);
        assert!(r.engine.app_state.open);
        if family == 0 {
            let next = named(&mut r, "Next month");
            assert!(!next.is_empty());
            action(&mut r, *next.last().unwrap(), Action::Click);
            assert_eq!(r.engine.app_state.messages.last().unwrap().0, 1);
            assert_eq!(r.engine.app_state.messages.last().unwrap().1, 1);
            let days = named(&mut r, "15");
            action(&mut r, *days.last().unwrap(), Action::Click);
            assert_eq!(r.engine.app_state.messages.last().unwrap().1, 2);
        } else {
            let hours = named(&mut r, "Hour");
            action(&mut r, *hours.last().unwrap(), Action::Increment);
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message(1, 1, "10".into()))
            );
            let minutes = named(&mut r, "Minute");
            action(&mut r, *minutes.last().unwrap(), Action::Focus);
            key(&mut r, NamedKey::ArrowUp);
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message(1, 2, "31".into()))
            );
            let seconds = named(&mut r, "Second");
            action(&mut r, *seconds.last().unwrap(), Action::Increment);
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message(1, 3, "1".into()))
            );
            let zones = named(&mut r, "Time zone");
            action(&mut r, *zones.last().unwrap(), Action::Focus);
            key(&mut r, NamedKey::Enter);
            key(&mut r, NamedKey::ArrowDown);
            key(&mut r, NamedKey::Enter);
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message(1, 4, "1".into()))
            );
        }
        for (epoch, message) in r.engine.app_state.messages.iter().enumerate() {
            assert_eq!(message.1, epoch as u64);
        }
        let old = Arc::downgrade(&r.engine.app_state.owner);
        r.engine.app_state.owner = Arc::new(());
        r.engine.app_state.visible = false;
        publish(&mut r);
        assert!(old.upgrade().is_none());
    }
}

#[test]
fn captured_composed_color_pointer_keyboard_accessibility_drafts_and_release() {
    let mut r = runner(2);
    let ids = ColorPickerIds::new(120).unwrap();
    assert!(
        tree(&mut r)
            .nodes
            .iter()
            .any(|(id, _)| id.0 == ids.plane().get())
    );
    action(&mut r, ids.hue().get(), Action::Increment);
    assert_eq!(r.engine.app_state.messages.last().unwrap().0, 1);
    action(&mut r, ids.alpha().get(), Action::Focus);
    key(&mut r, NamedKey::Home);
    assert_eq!(
        r.engine.app_state.messages.last(),
        Some(&Message(1, 1, "0.0".into()))
    );
    let plane = tree(&mut r)
        .nodes
        .into_iter()
        .find(|(id, _)| id.0 == ids.plane().get())
        .unwrap()
        .1
        .bounds()
        .unwrap();
    let position = PhysicalPosition::new((plane.x0 + plane.x1) / 2.0, (plane.y0 + plane.y1) / 2.0);
    for state in [ElementState::Pressed, ElementState::Released] {
        event(
            &mut r,
            WindowEvent::PointerButton {
                device_id: None,
                state,
                primary: true,
                position,
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
    }
    assert_eq!(r.engine.app_state.messages.last().unwrap().0, 1);
    assert!(r.engine.app_state.messages.last().unwrap().1 >= 2);
    action(&mut r, ids.channel(0).unwrap().get(), Action::Focus);
    event(&mut r, WindowEvent::Ime(Ime::Commit("z".into())));
    assert!(r.engine.app_state.messages.last().unwrap().2.contains('z'));
    assert_eq!(r.engine.app_state.messages.last().unwrap().0, 1);
    assert!(r.engine.app_state.colors[1].channel_text(0).contains('z'));
    assert!(r.engine.app_state.colors[1].channel_error(0).is_some());
    action(&mut r, ids.format().get(), Action::Focus);
    key(&mut r, NamedKey::Enter);
    key(&mut r, NamedKey::ArrowDown);
    key(&mut r, NamedKey::Enter);
    assert_eq!(r.engine.app_state.colors[1].format(), ColorFormat::Hex);
    action(&mut r, ids.hex().get(), Action::Focus);
    event(&mut r, WindowEvent::Ime(Ime::Commit("z".into())));
    assert!(r.engine.app_state.colors[1].hex().contains('z'));
    assert!(r.engine.app_state.colors[1].hex_error().is_some());
    action(&mut r, ids.format().get(), Action::Focus);
    key(&mut r, NamedKey::Enter);
    key(&mut r, NamedKey::End);
    key(&mut r, NamedKey::Enter);
    assert_eq!(r.engine.app_state.colors[1].format(), ColorFormat::Cmyk);
    for index in 0..5 {
        let id = ids.channel(index).unwrap().get();
        assert!(tree(&mut r).nodes.iter().any(|(node, _)| node.0 == id));
        action(&mut r, id, Action::Focus);
        event(&mut r, WindowEvent::Ime(Ime::Commit("z".into())));
        assert!(
            r.engine.app_state.colors[1]
                .channel_text(index)
                .contains('z')
        );
        assert!(r.engine.app_state.colors[1].channel_error(index).is_some());
        assert!(
            r.engine
                .app_state
                .messages
                .last()
                .unwrap()
                .2
                .starts_with(&format!("channel{index}:"))
        );
    }
    assert_eq!(r.engine.app_state.colors[0].format(), ColorFormat::Rgb);
    let copies = named(&mut r, "Copy color as HEX");
    action(&mut r, copies[0], Action::Click);
    assert_eq!(r.engine.app_state.messages.last().unwrap().0, 0);
    assert_eq!(r.engine.app_state.messages.last().unwrap().2, "copy");
    for (epoch, message) in r.engine.app_state.messages.iter().enumerate() {
        assert_eq!(message.1, epoch as u64);
    }
    let old = Arc::downgrade(&r.engine.app_state.owner);
    r.engine.app_state.owner = Arc::new(());
    r.engine.app_state.visible = false;
    publish(&mut r);
    assert!(old.upgrade().is_none());
}

#[test]
fn captured_composed_activity_actions_refresh_snapshot_and_identity() {
    let mut r = runner(3);
    let pause = named(&mut r, "Pause");
    assert_eq!(pause.len(), 2);
    action(&mut r, pause[0], Action::Click);
    action(&mut r, pause[1], Action::Click);
    assert_eq!(r.engine.app_state.messages[0].0, 0);
    assert_eq!(r.engine.app_state.messages[1].0, 1);
    assert_eq!(r.engine.app_state.messages[1].1, 1);
    assert!(r.engine.app_state.messages[1].2.contains("Pause"));
}

#[test]
fn captured_composed_eager_callbacks_release_captures_and_keep_legacy_constants() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    const CONFIG: CalendarConfig<'static> =
        CalendarConfig::new(CalendarLabels::ENGLISH, WeekStart::Monday);
    const CALLBACKS: ColorPickerCallbacks<()> =
        ColorPickerCallbacks::new(|_| (), |_| (), |_| (), |_| (), |_| (), [|_| (); 5]);
    fn copy<T: Copy>(value: T) -> (T, T) {
        (value, value)
    }
    let _ = copy(CONFIG);
    let _ = CALLBACKS;
    let owner = Arc::new(());
    let weak = Arc::downgrade(&owner);
    let calls = Arc::new(AtomicUsize::new(0));
    let days = calls.clone();
    let navigation = calls.clone();
    let day_capture = Capture(7, 8, owner);
    let widget = Widget::date_picker_with_config_with(
        false,
        CalendarMonth::new(2026, 7).unwrap(),
        None,
        Message(7, 8, "toggle".into()),
        Message(7, 8, "close".into()),
        move |date| {
            days.fetch_add(1, Ordering::SeqCst);
            day_capture.message(date)
        },
        move |month| {
            navigation.fetch_add(1, Ordering::SeqCst);
            Message(7, 8, format!("{month:?}"))
        },
        "Date",
        "Date",
        CONFIG,
        size(),
        Style::default(),
    );
    assert_eq!(calls.load(Ordering::SeqCst), 46); // Four navigation and 42 day messages, even closed.
    assert!(weak.upgrade().is_none()); // Only messages remain in the composed tree.
    assert!(matches!(widget, Widget::Popover { .. }));
    let model = ComposedApp::new(&mut FontSystem::new());
    let owner = Arc::new(());
    let weak = Arc::downgrade(&owner);
    let capture = Capture(1, 2, owner);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let widget = activity::view::details_with(
        model.registry.get(model.activity).unwrap(),
        move |id, action| {
            observed.fetch_add(1, Ordering::SeqCst);
            capture.message((id, action))
        },
        &Theme::light(),
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        model
            .registry
            .get(model.activity)
            .unwrap()
            .supported_actions()
            .len()
    );
    assert!(weak.upgrade().is_none());
    assert!(matches!(widget, Widget::Column { .. }));
}

#[test]
fn captured_composed_time_validation_and_color_ids_retain_contracts() {
    fn config(
        names: &'static [&'static str],
        selected: usize,
    ) -> Result<TimePickerConfigWith<'static, ()>, TimePickerError> {
        TimePickerConfigWith::new(
            names,
            selected,
            (),
            (),
            |_| (),
            |_| (),
            |_| (),
            |_| (),
            "Time",
        )
    }
    assert!(matches!(
        config(&[], 0),
        Err(TimePickerError::EmptyTimeZones)
    ));
    assert!(matches!(
        config(&["UTC"], 1),
        Err(TimePickerError::SelectedTimeZoneOutOfBounds {
            selected: 1,
            zone_count: 1
        })
    ));
    assert!(matches!(
        config(&["not/a/zone"], 0),
        Err(TimePickerError::InvalidTimeZone { index: 0, .. })
    ));
    let config = config(&["UTC"], 0)
        .unwrap()
        .with_labels(TimePickerLabels::PORTUGUESE);
    assert_eq!(config.time_zone_names(), &["UTC"]);
    assert_eq!(config.selected_time_zone(), 0);
    assert_eq!(config.labels().hour, "Hora");
    assert!(ColorPickerIds::new((1 << 63) - 10).is_err());
    let ids = ColorPickerIds::new(100).unwrap();
    assert_eq!(ids.channel(4).unwrap().get(), 110);
    assert!(ids.channel(5).is_none());
}
