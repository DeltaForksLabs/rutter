// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::rc::Rc;

use accesskit::{Action, ActionHandler, ActionRequest, NodeId, TreeId};
use cosmic_text::FontSystem;
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;

use super::*;
use crate::Widget;
use crate::engine::{RutterEngine, test_surface::MemoryWindow};

#[test]
fn fractional_adjustments_preserve_off_grid_values_and_clamp_uneven_bounds() {
    assert_eq!(
        counter_adjusted_value(0.5, -1.0, 1.0, 0.125, true),
        Some(0.625)
    );
    assert_eq!(
        counter_adjusted_value(-0.125, -1.0, 1.0, 0.25, true),
        Some(0.125)
    );
    assert_eq!(
        counter_adjusted_value(0.125, -1.0, 1.0, 0.25, false),
        Some(-0.125)
    );
    assert_eq!(counter_adjusted_value(0.9, 0.0, 1.0, 0.25, true), Some(1.0));
    assert_eq!(
        counter_adjusted_value(-0.9, -1.0, 1.0, 0.25, false),
        Some(-1.0)
    );
    assert_eq!(
        counter_adjusted_value(2.0, -1.0, 1.0, 0.25, false),
        Some(1.0)
    );
    assert_eq!(counter_adjusted_value(0.5, 0.5, 0.5, 0.125, true), None);

    let value = counter_adjusted_value(0.2, 0.0, 1.0, 0.1, true).unwrap();
    assert!((value - 0.3).abs() <= f64::EPSILON);
    // The float extension must not coerce existing integer counters to f64.
    assert_eq!(
        counter_adjusted_value(9_007_199_254_740_992_i64, 0, i64::MAX, 1, true),
        Some(9_007_199_254_740_993)
    );
}

#[test]
fn float_keyboard_supports_fractional_steps_pages_and_endpoints() {
    for key in [
        Key::Character("+".into()),
        Key::Named(NamedKey::ArrowUp),
        Key::Named(NamedKey::ArrowRight),
    ] {
        assert_eq!(
            counter_value_for_key(0.0, -2.0, 2.0, 0.125, &key),
            Some(0.125)
        );
    }
    for key in [
        Key::Character("-".into()),
        Key::Named(NamedKey::ArrowDown),
        Key::Named(NamedKey::ArrowLeft),
    ] {
        assert_eq!(
            counter_value_for_key(0.0, -2.0, 2.0, 0.125, &key),
            Some(-0.125)
        );
    }
    for (key, expected) in [
        (NamedKey::Home, -2.0),
        (NamedKey::End, 2.0),
        (NamedKey::PageUp, 1.25),
        (NamedKey::PageDown, -1.25),
    ] {
        assert_eq!(
            counter_value_for_key(0.0, -2.0, 2.0, 0.125, &Key::Named(key)),
            Some(expected)
        );
    }
    assert_eq!(
        counter_value_for_key(2.0, -2.0, 2.0, 0.125, &Key::Named(NamedKey::End)),
        None
    );
    assert_eq!(
        counter_value_for_key(0.0, -2.0, 2.0, 0.125, &Key::Named(NamedKey::Enter)),
        None
    );
}

#[test]
fn float_overflow_clamps_and_unrepresentable_steps_do_not_repeat() {
    assert_eq!(
        counter_adjusted_value(f64::MAX / 2.0, -f64::MAX, f64::MAX, f64::MAX, true),
        Some(f64::MAX)
    );
    assert_eq!(
        counter_adjusted_value(-f64::MAX / 2.0, -f64::MAX, f64::MAX, f64::MAX, false),
        Some(-f64::MAX)
    );
    assert_eq!(
        counter_value_for_key(
            -f64::MAX,
            -f64::MAX,
            f64::MAX,
            f64::MAX,
            &Key::Named(NamedKey::PageUp)
        ),
        Some(f64::MAX)
    );
    assert_eq!(
        counter_value_for_key(
            f64::MAX,
            -f64::MAX,
            f64::MAX,
            f64::MAX,
            &Key::Named(NamedKey::PageDown)
        ),
        Some(-f64::MAX)
    );
    let tiny = f64::from_bits(1);
    assert_eq!(
        counter_adjusted_value(0.0, 0.0, 1.0, tiny, true),
        Some(tiny)
    );
    let range = CounterValueRange {
        value: 1.0,
        min: 0.0,
        max: 2.0,
        step: tiny,
    };
    assert!(counter_hold_plan(2, range, true, Instant::now()).is_none());
}

#[test]
fn float_page_cancellation_preserves_finite_in_range_targets() {
    assert_eq!(
        counter_value_for_key(
            -f64::MAX,
            -f64::MAX,
            f64::MAX,
            f64::MAX / 8.0,
            &Key::Named(NamedKey::PageUp),
        ),
        Some(f64::MAX / 4.0),
    );
    assert_eq!(
        counter_value_for_key(
            f64::MAX,
            -f64::MAX,
            f64::MAX,
            f64::MAX / 8.0,
            &Key::Named(NamedKey::PageDown),
        ),
        Some(-f64::MAX / 4.0),
    );
}

#[test]
fn unchecked_nonfinite_and_invalid_float_configurations_never_adjust() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for index in 0..4 {
            let mut args = [0.5, 0.0, 1.0, 0.25];
            args[index] = invalid;
            let [value, min, max, step] = args;
            assert_eq!(counter_adjusted_value(value, min, max, step, true), None);
            assert_eq!(
                counter_value_for_key(value, min, max, step, &Key::Named(NamedKey::Home)),
                None
            );
            let range = CounterValueRange {
                value,
                min,
                max,
                step,
            };
            assert!(counter_hold_plan(2, range, true, Instant::now()).is_none());
        }
    }
    for (min, max, step) in [(1.0, -1.0, 0.25), (0.0, 1.0, 0.0), (0.0, 1.0, -0.25)] {
        assert_eq!(counter_adjusted_value(0.5, min, max, step, false), None);
        assert_eq!(
            counter_value_for_key(0.5, min, max, step, &Key::Named(NamedKey::End)),
            None
        );
    }
}

#[test]
fn fractional_hold_uses_updated_application_values_bounds_and_step() {
    let now = Instant::now();
    let range = CounterValueRange {
        value: 0.0,
        min: -1.0,
        max: 1.0,
        step: 0.125,
    };
    let (value, repeat) = counter_hold_plan(2, range, true, now).unwrap();
    assert_eq!(value, 0.125);
    let mut repeat = repeat.unwrap();
    assert_eq!(
        advance_counter_hold(&mut repeat, now),
        CounterRepeatAdvance::Waiting(now + COUNTER_HOLD_INITIAL_DELAY)
    );
    // A stale cache must not overwrite the immediate click's pending value.
    sync_counter_hold_range(&mut repeat, 0.0, -1.0, 1.0, 0.125);
    assert_eq!(repeat.range.value, 0.125);
    // An application edit, including a new off-grid value, is authoritative.
    sync_counter_hold_range(&mut repeat, -0.375, -0.5, 0.0, 0.25);
    assert!(matches!(
        advance_counter_hold(&mut repeat, now + COUNTER_HOLD_INITIAL_DELAY),
        CounterRepeatAdvance::Changed {
            value: -0.125,
            next_deadline: Some(_)
        }
    ));
    let deadline = repeat.next_repeat_at;
    assert_eq!(
        advance_counter_hold(&mut repeat, deadline),
        CounterRepeatAdvance::Changed {
            value: 0.0,
            next_deadline: None
        }
    );
}

struct CounterApp;

struct CounterModel {
    integer: i64,
    amount: f64,
    enabled: bool,
    visible: bool,
    fractional: bool,
    changes: Vec<CounterMessage>,
}

#[derive(Clone, Debug, PartialEq)]
enum CounterMessage {
    Integer(i64),
    Float(f64),
}

impl AppLogic for CounterApp {
    type State = CounterModel;
    type Message = CounterMessage;

    fn new(_: &mut FontSystem) -> Self::State {
        CounterModel {
            integer: 9_007_199_254_740_992,
            amount: 0.5,
            enabled: true,
            visible: true,
            fractional: true,
            changes: Vec::new(),
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, CounterMessage> {
        let style = Style {
            size: Size::from_lengths(160.0, 40.0),
            ..Style::default()
        };
        let mut children = vec![
            Widget::counter(
                state.integer,
                0,
                i64::MAX,
                1,
                CounterMessage::Integer,
                style.clone(),
                "Integer",
            )
            .with_id(1),
        ];
        if state.visible {
            let counter = if state.fractional {
                Widget::counter(
                    state.amount,
                    -1.0,
                    1.0,
                    0.25,
                    CounterMessage::Float,
                    style,
                    "Amount",
                )
            } else {
                Widget::counter(
                    state.integer,
                    0,
                    i64::MAX,
                    1,
                    CounterMessage::Integer,
                    style,
                    "Amount",
                )
            };
            children.push(counter.with_id(2).enabled(state.enabled));
        }
        Widget::Row {
            children,
            style: Style::default(),
        }
    }

    fn update(state: &mut Self::State, message: CounterMessage, _: &mut arboard::Clipboard) {
        match message {
            CounterMessage::Integer(value) => state.integer = value,
            CounterMessage::Float(value) => state.amount = value,
        }
        state.changes.push(message);
    }
}

// Like other engine tests, these exercise the real controlled update/cache paths
// with in-memory windows, but engine construction requires a clipboard service.
fn counter_runner() -> RutterRunner<CounterApp> {
    let mut engine = RutterEngine::<CounterApp>::new().expect("counter test clipboard service");
    engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(400, 80),
        scale: 1.0,
        popup: false,
        popup_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    refresh(&mut runner);
    runner
}

fn refresh(runner: &mut RutterRunner<CounterApp>) {
    runner.engine.layout_dirty = true;
    runner
        .engine
        .try_ensure_layout_for_input(PhysicalSize::new(400, 80))
        .unwrap();
}

#[test]
fn mouse_keyboard_and_accessibility_deliver_fractional_application_messages() {
    let mut runner = counter_runner();
    runner.begin_counter_hold(2, true);
    assert_eq!(runner.engine.app_state.amount, 0.75);
    assert_eq!(runner.counter_hold_deadline(Instant::now()), None); // released
    refresh(&mut runner);
    runner.focus_widget(Some(2));
    assert!(runner.handle_focused_widget_key(&Key::Character("-".into()), false));
    assert_eq!(runner.engine.app_state.amount, 0.5);
    refresh(&mut runner);

    runner
        .engine
        .accessibility_actions
        .handler()
        .do_action(ActionRequest {
            action: Action::Decrement,
            target_tree: TreeId::ROOT,
            target_node: NodeId(2),
            data: None,
        });
    runner.process_accessibility_actions();
    assert_eq!(runner.engine.app_state.amount, 0.25);
    assert_eq!(
        runner.engine.app_state.changes,
        vec![
            CounterMessage::Float(0.75),
            CounterMessage::Float(0.5),
            CounterMessage::Float(0.25)
        ]
    );

    refresh(&mut runner);
    assert!(runner.adjust_focused_counter_key(1, &Key::Character("+".into())));
    assert_eq!(runner.engine.app_state.integer, 9_007_199_254_740_993);
}

#[test]
fn float_runner_hold_reaches_boundary_without_cache_refresh_or_extra_messages() {
    let mut runner = counter_runner();
    runner.mouse_down = true;
    runner.begin_counter_hold(2, true);
    let Some(CounterHoldRepeat::Float(repeat)) = runner.counter_hold_repeat else {
        panic!("fractional hold must be scheduled")
    };
    assert_eq!(runner.counter_hold_deadline(repeat.next_repeat_at), None);
    assert!(runner.counter_hold_repeat.is_none());
    assert_eq!(runner.engine.app_state.amount, 1.0);
    assert_eq!(
        runner.engine.app_state.changes,
        vec![CounterMessage::Float(0.75), CounterMessage::Float(1.0)]
    );
    refresh(&mut runner);
    assert!(!runner.adjust_focused_counter_key(2, &Key::Named(NamedKey::ArrowUp)));
    assert_eq!(runner.engine.app_state.changes.len(), 2);
}

#[test]
fn disabled_or_removed_float_counters_cancel_active_hold_and_callbacks() {
    for remove in [false, true] {
        let mut runner = counter_runner();
        runner.mouse_down = true;
        runner.begin_counter_hold(2, true);
        let Some(CounterHoldRepeat::Float(repeat)) = runner.counter_hold_repeat else {
            panic!("fractional hold must be scheduled")
        };
        if remove {
            runner.engine.app_state.visible = false;
        } else {
            runner.engine.app_state.enabled = false;
        }
        refresh(&mut runner);
        assert_eq!(runner.counter_hold_deadline(repeat.next_repeat_at), None);
        assert!(runner.counter_hold_repeat.is_none());
        assert!(!runner.adjust_focused_counter_key(2, &Key::Named(NamedKey::ArrowUp)));
        assert_eq!(
            runner.engine.app_state.changes,
            vec![CounterMessage::Float(0.75)]
        );
    }
}

#[test]
fn changing_numeric_mode_with_the_same_counter_id_cancels_the_old_hold() {
    for start_fractional in [true, false] {
        let mut runner = counter_runner();
        runner.engine.app_state.fractional = start_fractional;
        refresh(&mut runner);
        runner.mouse_down = true;
        runner.begin_counter_hold(2, true);
        let deadline = match runner.counter_hold_repeat.expect("hold should start") {
            CounterHoldRepeat::Integer(repeat) => repeat.next_repeat_at,
            CounterHoldRepeat::Float(repeat) => repeat.next_repeat_at,
        };
        runner.engine.app_state.fractional = !start_fractional;
        // Publish a mode change without delivering the queued parent redraw.
        runner.engine.layout_dirty = true;
        assert_eq!(runner.counter_hold_deadline(deadline), None);
        assert!(runner.counter_hold_repeat.is_none());
        assert_eq!(runner.engine.app_state.changes.len(), 1);
        assert!(runner.adjust_focused_counter_key(2, &Key::Character("+".into())));
        let expected = if start_fractional {
            CounterMessage::Integer(9_007_199_254_740_993)
        } else {
            CounterMessage::Float(0.75)
        };
        assert_eq!(runner.engine.app_state.changes.last(), Some(&expected));
    }
}

#[test]
fn round_trip_numeric_mode_changes_do_not_revive_an_old_hold() {
    for start_fractional in [true, false] {
        let mut runner = counter_runner();
        runner.engine.app_state.fractional = start_fractional;
        refresh(&mut runner);
        runner.mouse_down = true;
        runner.begin_counter_hold(2, true);
        let deadline = match runner.counter_hold_repeat.expect("hold should start") {
            CounterHoldRepeat::Integer(repeat) => repeat.next_repeat_at,
            CounterHoldRepeat::Float(repeat) => repeat.next_repeat_at,
        };

        runner.engine.app_state.fractional = !start_fractional;
        refresh(&mut runner);
        runner.engine.app_state.fractional = start_fractional;
        refresh(&mut runner);

        assert_eq!(runner.counter_hold_deadline(deadline), None);
        assert!(runner.counter_hold_repeat.is_none());
        assert_eq!(runner.engine.app_state.changes.len(), 1);
        assert!(runner.adjust_focused_counter_key(2, &Key::Character("+".into())));
        assert_eq!(runner.engine.app_state.changes.len(), 2);
    }
}
