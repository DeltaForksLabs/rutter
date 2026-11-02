// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache-2.0.

use std::rc::Rc;
use std::sync::Arc;

use accesskit::{Action, ActionHandler, ActionRequest, NodeId, TreeId};
use cosmic_text::FontSystem;
use skia_safe::Point;
use taffy::prelude::{Size, Style};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ButtonSource, ElementState, MouseButton, WindowEvent},
    window::WindowId,
};

use super::*;
use crate::engine::{
    RutterEngine,
    test_surface::{MemoryEventLoop, MemoryWindow},
};
use crate::render::select_overlay::{
    collector::collect_open_select_overlays, popup_layout_for_focus,
};
use crate::{CounterValue, Widget, WidgetConfigError};

const VIEWPORT: PhysicalSize<u32> = PhysicalSize::new(400, 500);

// Owned strings deliberately exercise a non-Copy capture. Identity and epoch
// belong to a view snapshot, never a mutable/global lookup at dispatch time.
#[derive(Clone, Debug, PartialEq)]
struct Context {
    panel: String,
    instance: String,
    draft_epoch: u64,
}

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Counter(Arc<Context>, CounterValue),
    Select(Arc<Context>, usize),
}

struct Model {
    integer: i64,
    amount: f64,
    selected: [usize; 2],
    draft_epoch: u64,
    enabled: bool,
    visible: bool,
    fractional: bool,
    messages: Vec<Message>,
}

struct CapturedApp;

fn context(instance: &str, draft_epoch: u64) -> Arc<Context> {
    Arc::new(Context {
        panel: "apps-panel".to_owned(),
        instance: instance.to_owned(),
        draft_epoch,
    })
}

fn style() -> Style {
    Style {
        size: Size::from_lengths(160.0, 64.0),
        ..Style::default()
    }
}

impl AppLogic for CapturedApp {
    type State = Model;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Model {
        Model {
            integer: 9_007_199_254_740_992,
            amount: 0.5,
            selected: [0, 0],
            draft_epoch: 0,
            enabled: true,
            visible: true,
            fractional: true,
            messages: Vec::new(),
        }
    }

    fn view<'a>(state: &'a mut Model) -> Widget<'a, Message> {
        let integer_context = context("integer", state.draft_epoch);
        let mut children = vec![
            Widget::try_counter_with(
                state.integer,
                0,
                i64::MAX,
                1,
                move |value| {
                    Message::Counter(integer_context.clone(), CounterValue::Integer(value))
                },
                style(),
                "",
            )
            .unwrap()
            .with_id(1),
        ];
        if state.visible {
            let amount_context = context("amount", state.draft_epoch);
            let counter = if state.fractional {
                Widget::try_counter_with(
                    state.amount,
                    0.0,
                    10.0,
                    0.25,
                    move |value| {
                        Message::Counter(amount_context.clone(), CounterValue::Float(value))
                    },
                    style(),
                    "",
                )
                .unwrap()
            } else {
                Widget::counter_with(
                    1_i64,
                    0,
                    10,
                    1,
                    move |value| {
                        Message::Counter(amount_context.clone(), CounterValue::Integer(value))
                    },
                    style(),
                    "",
                )
            };
            children.push(counter.with_id(2).enabled(state.enabled));
        }
        for (index, instance) in ["first-select", "second-select"].into_iter().enumerate() {
            let select_context = context(instance, state.draft_epoch);
            children.push(
                Widget::try_select_with(
                    &["Zero", "One", "Two"],
                    state.selected[index],
                    move |value| Message::Select(select_context.clone(), value),
                    style(),
                    "",
                    "Pick",
                )
                .unwrap()
                .with_id(3 + index as u64),
            );
        }
        Widget::Column {
            children,
            style: Style::default(),
        }
    }

    fn update(state: &mut Model, message: Message, _: &mut arboard::Clipboard) {
        match &message {
            Message::Counter(_, CounterValue::Integer(value)) => state.integer = *value,
            Message::Counter(_, CounterValue::Float(value)) => state.amount = *value,
            Message::Select(context, value) => {
                let index = usize::from(context.instance == "second-select");
                state.selected[index] = *value;
            }
        }
        state.messages.push(message);
    }
}

fn runner() -> RutterRunner<CapturedApp> {
    let mut engine = RutterEngine::<CapturedApp>::new().expect("captured callback test clipboard");
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    refresh(&mut runner);
    runner
}

fn refresh(runner: &mut RutterRunner<CapturedApp>) {
    runner.engine.layout_dirty = true;
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}

fn pointer(runner: &mut RutterRunner<CapturedApp>, point: Point) {
    for state in [ElementState::Pressed, ElementState::Released] {
        runner.window_event(
            &MemoryEventLoop::default(),
            WindowId::from_raw(1),
            WindowEvent::PointerButton {
                device_id: None,
                state,
                primary: true,
                position: PhysicalPosition::new(f64::from(point.x), f64::from(point.y)),
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
        assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
    }
}

fn accessibility(runner: &mut RutterRunner<CapturedApp>, id: u64, action: Action) {
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
}

#[test]
fn captured_counter_pointer_keyboard_and_accessibility_keep_distinct_contexts() {
    let mut runner = runner();
    pointer(&mut runner, Point::new(150.0, 32.0));
    pointer(&mut runner, Point::new(150.0, 96.0));
    runner.focus_widget(Some(1));
    assert!(runner.handle_focused_widget_key(&Key::Named(NamedKey::ArrowUp), false));
    refresh(&mut runner);
    runner.focus_widget(Some(2));
    assert!(runner.handle_focused_widget_key(&Key::Named(NamedKey::ArrowDown), false));
    refresh(&mut runner);
    accessibility(&mut runner, 1, Action::Decrement);
    refresh(&mut runner);
    accessibility(&mut runner, 2, Action::Increment);
    assert_eq!(
        runner.engine.app_state.messages,
        vec![
            Message::Counter(
                context("integer", 0),
                CounterValue::Integer(9_007_199_254_740_993)
            ),
            Message::Counter(context("amount", 0), CounterValue::Float(0.75)),
            Message::Counter(
                context("integer", 0),
                CounterValue::Integer(9_007_199_254_740_994)
            ),
            Message::Counter(context("amount", 0), CounterValue::Float(0.5)),
            Message::Counter(
                context("integer", 0),
                CounterValue::Integer(9_007_199_254_740_993)
            ),
            Message::Counter(context("amount", 0), CounterValue::Float(0.75)),
        ]
    );
}

#[test]
fn captured_select_pointer_keyboard_accessibility_and_rebuild_keep_contexts() {
    let mut runner = runner();
    pointer(&mut runner, Point::new(80.0, 160.0));
    let point = {
        let widget = CapturedApp::view(&mut runner.engine.app_state);
        let overlays = collect_open_select_overlays(
            &widget,
            &runner.engine.taffy,
            runner.engine.last_root_node,
            &runner.engine.widget_states,
            (400.0, 500.0),
        );
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].id, 3);
        let popup = popup_layout_for_focus(overlays[0].anchor, 3, 0, (400.0, 500.0));
        Point::new(
            popup.rect.center_x(),
            popup.rect.top + crate::layout::OPTION_HEIGHT * 1.5,
        )
    };
    pointer(&mut runner, point);
    runner.engine.app_state.draft_epoch = 7;
    refresh(&mut runner);
    runner.focus_widget(Some(4));
    assert!(runner.handle_focused_widget_key(&Key::Named(NamedKey::End), false));
    refresh(&mut runner);
    accessibility(&mut runner, 3, Action::Decrement);
    refresh(&mut runner);
    accessibility(&mut runner, 4, Action::Decrement);
    assert_eq!(
        runner.engine.app_state.messages,
        vec![
            Message::Select(context("first-select", 0), 1),
            Message::Select(context("second-select", 7), 2),
            Message::Select(context("first-select", 7), 0),
            Message::Select(context("second-select", 7), 1),
        ]
    );
}

fn hold_deadline(runner: &RutterRunner<CapturedApp>) -> Instant {
    match runner.counter_hold_repeat.expect("active captured hold") {
        CounterHoldRepeat::Integer(repeat) => repeat.next_repeat_at,
        CounterHoldRepeat::Float(repeat) => repeat.next_repeat_at,
    }
}

#[test]
fn captured_hold_repeat_refreshes_epoch_without_mutating_prior_callback_snapshots() {
    for id in [1, 2] {
        let mut runner = runner();
        let old_select = runner.engine.runtime_caches.selects[&3].on_change.clone();
        runner.mouse_down = true;
        runner.begin_counter_hold(id, true);
        let deadline = hold_deadline(&runner);
        runner.engine.app_state.draft_epoch = 8;
        // Exercise scheduled work before redraw: it must rebuild dirty metadata.
        runner.engine.layout_dirty = true;
        assert!(runner.counter_hold_deadline(deadline).is_some());
        let expected = if id == 1 {
            Message::Counter(
                context("integer", 8),
                CounterValue::Integer(9_007_199_254_740_994),
            )
        } else {
            Message::Counter(context("amount", 8), CounterValue::Float(1.0))
        };
        assert_eq!(runner.engine.app_state.messages.last(), Some(&expected));
        assert_eq!(
            old_select.call(2),
            Message::Select(context("first-select", 0), 2)
        );
        assert_eq!(
            runner.engine.runtime_caches.selects[&3].on_change.call(2),
            Message::Select(context("first-select", 8), 2)
        );
    }
}

#[test]
fn captured_hold_cannot_resume_after_disable_removal_or_numeric_mode_round_trip() {
    for change in ["disabled", "removed", "mode"] {
        let mut runner = runner();
        runner.mouse_down = true;
        runner.begin_counter_hold(2, true);
        let deadline = hold_deadline(&runner);
        match change {
            "disabled" => runner.engine.app_state.enabled = false,
            "removed" => runner.engine.app_state.visible = false,
            "mode" => runner.engine.app_state.fractional = false,
            _ => unreachable!(),
        }
        refresh(&mut runner);
        runner.engine.app_state.enabled = true;
        runner.engine.app_state.visible = true;
        runner.engine.app_state.fractional = true;
        refresh(&mut runner);
        assert_eq!(runner.counter_hold_deadline(deadline), None, "{change}");
        assert!(runner.counter_hold_repeat.is_none());
        assert_eq!(runner.engine.app_state.messages.len(), 1);
    }
}

#[test]
fn captured_constructors_preserve_validation_errors() {
    let invalid_integer: Result<Widget<'_, ()>, _> =
        Widget::try_counter_with(11_i64, 0, 10, 1, move |_| (), style(), "Count");
    assert!(matches!(
        invalid_integer,
        Err(crate::CounterConfigError::ValueOutOfRange { .. })
    ));
    let invalid_float: Result<Widget<'_, ()>, _> =
        Widget::try_counter_with(0.5, 0.0, 1.0, f64::NAN, move |_| (), style(), "Amount");
    assert!(matches!(
        invalid_float,
        Err(crate::CounterConfigError::InvalidStep { .. })
    ));
    let invalid_select: Result<Widget<'_, ()>, _> =
        Widget::try_select_with(&["Only"], 1, move |_| (), style(), "Pick", "");
    assert!(matches!(
        invalid_select,
        Err(WidgetConfigError::SelectedIndexOutOfBounds)
    ));
    let empty_select: Result<Widget<'_, ()>, _> =
        Widget::try_select_with(&[], 0, move |_| (), style(), "Pick", "");
    assert!(matches!(
        empty_select,
        Err(WidgetConfigError::SelectedIndexOutOfBounds)
    ));
}

#[test]
fn captured_callback_ownership_releases_context_after_last_retained_clone() {
    for select in [false, true] {
        let snapshot = context("retained", 17);
        let weak = Arc::downgrade(&snapshot);
        let widget = if select {
            Widget::select_with(
                &["Zero", "One"],
                0,
                move |index| Message::Select(snapshot.clone(), index),
                style(),
                "",
                "",
            )
        } else {
            Widget::counter_with(
                0_i64,
                0,
                10,
                1,
                move |value| Message::Counter(snapshot.clone(), CounterValue::Integer(value)),
                style(),
                "",
            )
        };
        let retained: Box<dyn Fn() -> Message> = match &widget {
            Widget::Counter {
                config: crate::CounterConfig::IntegerWith(parameters),
                ..
            } => {
                let callback = parameters.on_change.clone();
                Box::new(move || callback.call(1))
            }
            Widget::SelectWith { on_change, .. } => {
                let callback = on_change.clone();
                Box::new(move || callback.call(1))
            }
            _ => unreachable!("test constructs only captured Counter/Select"),
        };
        drop(widget);
        assert!(weak.upgrade().is_some());
        drop(retained);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn captured_select_lookup_preserves_context_and_legacy_function_pointer_api() {
    use crate::render::hit_test::{find_select_callback, find_select_callback_with};

    let snapshot = context("lookup", 17);
    let widget = Widget::Column {
        style: Style::default(),
        children: vec![
            Widget::select_with(
                &["Zero", "One"],
                0,
                move |index| Message::Select(snapshot.clone(), index),
                style(),
                "",
                "",
            )
            .with_id(3),
            Widget::select(
                &["Zero", "One"],
                0,
                |index| Message::Select(context("legacy", 0), index),
                style(),
                "",
                "",
            )
            .with_id(4),
        ],
    };
    assert!(find_select_callback(&widget, 3).is_none());
    assert_eq!(
        find_select_callback_with(&widget, 3).unwrap().call(1),
        Message::Select(context("lookup", 17), 1)
    );
    assert_eq!(
        find_select_callback(&widget, 4).unwrap()(1),
        Message::Select(context("legacy", 0), 1)
    );
    assert_eq!(
        find_select_callback_with(&widget, 4).unwrap().call(1),
        Message::Select(context("legacy", 0), 1)
    );
}

#[test]
fn captured_select_lookup_traverses_hover_pointer_region_with_auto_ids() {
    use crate::render::hit_test::{find_select_callback, find_select_callback_with};

    let snapshot = context("wrapped-select", 23);
    let select = Widget::select_with(
        &["Zero", "One"],
        0,
        move |index| Message::Select(snapshot.clone(), index),
        style(),
        "Pick",
        "",
    );
    let id = select.resolved_id(&[0, 0]).unwrap();
    let widget = Widget::Column {
        children: vec![Widget::pointer_region(
            crate::WidgetId::manual(99).unwrap(),
            select,
            crate::PointerRegionConfig::new(|_| Message::Select(context("pointer", 0), 0))
                .with_hover(|_| Message::Select(context("hover", 0), 0)),
            Style::default(),
        )],
        style: Style::default(),
    };
    let callback = find_select_callback_with(&widget, id).expect("Select inside PointerRegion");
    assert_eq!(
        callback.call(1),
        Message::Select(context("wrapped-select", 23), 1)
    );
    assert!(find_select_callback(&widget, id).is_none());
    assert!(find_select_callback_with(&widget.enabled(false), id).is_none());
}
