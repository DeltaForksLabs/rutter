// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::time::{Duration, Instant};

use winit::keyboard::{Key, NamedKey};

use super::RutterRunner;
use crate::app::AppLogic;

const COUNTER_HOLD_INITIAL_DELAY: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug)]
struct CounterValueRange {
    value: i64,
    min: i64,
    max: i64,
    step: i64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CounterHoldRepeat {
    id: u64,
    increment: bool,
    range: CounterValueRange,
    last_observed_value: i64,
    next_repeat_at: Instant,
    repetition_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CounterRepeatAdvance {
    Waiting(Instant),
    Changed {
        value: i64,
        next_deadline: Option<Instant>,
    },
    Finished,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn begin_counter_hold(&mut self, id: u64, increment: bool) {
        self.counter_hold_repeat = None;
        let Some(counter) = self.engine.runtime_caches.counters.get(&id).cloned() else {
            return;
        };
        let range = CounterValueRange {
            value: counter.value,
            min: counter.min,
            max: counter.max,
            step: counter.step,
        };
        let Some((value, repeat)) = counter_hold_plan(id, range, increment, Instant::now()) else {
            return;
        };
        self.counter_hold_repeat = repeat;
        self.apply_counter_value(counter.on_change, value);
    }

    pub(super) fn counter_hold_deadline(&mut self, now: Instant) -> Option<Instant> {
        if !self.mouse_down {
            self.counter_hold_repeat = None;
            return None;
        }
        let mut repeat = self.counter_hold_repeat.take()?;
        let (value, min, max, step, on_change) = self
            .engine
            .runtime_caches
            .counters
            .get(&repeat.id)
            .map(|counter| {
                (
                    counter.value,
                    counter.min,
                    counter.max,
                    counter.step,
                    counter.on_change,
                )
            })?;
        sync_counter_hold_range(&mut repeat, value, min, max, step);
        self.apply_counter_hold_advance(repeat, on_change, now)
    }

    fn apply_counter_hold_advance(
        &mut self,
        mut repeat: CounterHoldRepeat,
        on_change: fn(i64) -> A::Message,
        now: Instant,
    ) -> Option<Instant> {
        match advance_counter_hold(&mut repeat, now) {
            CounterRepeatAdvance::Waiting(deadline) => {
                self.counter_hold_repeat = Some(repeat);
                Some(deadline)
            }
            CounterRepeatAdvance::Changed {
                value,
                next_deadline,
            } => {
                self.counter_hold_repeat = next_deadline.map(|_| repeat);
                self.apply_counter_value(on_change, value);
                self.redraw();
                next_deadline
            }
            CounterRepeatAdvance::Finished => None,
        }
    }

    pub(super) fn adjust_focused_counter_key(&mut self, id: u64, key: &Key) -> bool {
        let Some(counter) = self.engine.runtime_caches.counters.get(&id).cloned() else {
            return false;
        };
        let Some(value) =
            counter_value_for_key(counter.value, counter.min, counter.max, counter.step, key)
        else {
            return false;
        };
        self.apply_counter_value(counter.on_change, value);
        true
    }

    fn apply_counter_value(&mut self, on_change: fn(i64) -> A::Message, value: i64) {
        A::update(
            &mut self.engine.app_state,
            on_change(value),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
    }
}

fn counter_hold_plan(
    id: u64,
    mut range: CounterValueRange,
    increment: bool,
    now: Instant,
) -> Option<(i64, Option<CounterHoldRepeat>)> {
    let observed_value = range.value;
    let value = counter_adjusted_value(range.value, range.min, range.max, range.step, increment)?;
    range.value = value;
    let can_repeat =
        counter_adjusted_value(value, range.min, range.max, range.step, increment).is_some();
    let repeat = can_repeat.then_some(CounterHoldRepeat {
        id,
        increment,
        range,
        last_observed_value: observed_value,
        next_repeat_at: now + COUNTER_HOLD_INITIAL_DELAY,
        repetition_count: 0,
    });
    Some((value, repeat))
}

fn sync_counter_hold_range(
    repeat: &mut CounterHoldRepeat,
    value: i64,
    min: i64,
    max: i64,
    step: i64,
) {
    if value != repeat.last_observed_value && value != repeat.range.value {
        repeat.range.value = value;
    }
    repeat.last_observed_value = value;
    repeat.range.min = min;
    repeat.range.max = max;
    repeat.range.step = step;
}

fn advance_counter_hold(repeat: &mut CounterHoldRepeat, now: Instant) -> CounterRepeatAdvance {
    if now < repeat.next_repeat_at {
        return CounterRepeatAdvance::Waiting(repeat.next_repeat_at);
    }
    let range = repeat.range;
    let Some(value) = counter_adjusted_value(
        range.value,
        range.min,
        range.max,
        range.step,
        repeat.increment,
    ) else {
        return CounterRepeatAdvance::Finished;
    };
    let next_deadline = schedule_counter_repeat(repeat, value, now);
    CounterRepeatAdvance::Changed {
        value,
        next_deadline,
    }
}

fn schedule_counter_repeat(
    repeat: &mut CounterHoldRepeat,
    value: i64,
    now: Instant,
) -> Option<Instant> {
    repeat.range.value = value;
    counter_adjusted_value(
        value,
        repeat.range.min,
        repeat.range.max,
        repeat.range.step,
        repeat.increment,
    )?;
    repeat.repetition_count = repeat.repetition_count.saturating_add(1);
    repeat.next_repeat_at = now + counter_repeat_interval(repeat.repetition_count);
    Some(repeat.next_repeat_at)
}

fn counter_repeat_interval(repetition_count: u32) -> Duration {
    match repetition_count {
        0..=3 => Duration::from_millis(180),
        4..=11 => Duration::from_millis(100),
        _ => Duration::from_millis(50),
    }
}

fn counter_adjusted_value(
    value: i64,
    min: i64,
    max: i64,
    step: i64,
    increment: bool,
) -> Option<i64> {
    if min > max || step <= 0 {
        return None;
    }
    let bounded = value.clamp(min, max);
    if bounded != value {
        return Some(bounded);
    }
    let next = if increment {
        value.saturating_add(step).min(max)
    } else {
        value.saturating_sub(step).max(min)
    };
    (next != value).then_some(next)
}

#[derive(Clone, Copy)]
enum CounterKeyAction {
    Decrement,
    Increment,
    Minimum,
    Maximum,
    PageDecrement,
    PageIncrement,
}

fn counter_value_for_key(value: i64, min: i64, max: i64, step: i64, key: &Key) -> Option<i64> {
    if min > max || step <= 0 {
        return None;
    }
    match counter_key_action(key)? {
        CounterKeyAction::Decrement => counter_adjusted_value(value, min, max, step, false),
        CounterKeyAction::Increment => counter_adjusted_value(value, min, max, step, true),
        CounterKeyAction::Minimum => (value != min).then_some(min),
        CounterKeyAction::Maximum => (value != max).then_some(max),
        CounterKeyAction::PageDecrement => {
            counter_adjusted_value(value, min, max, step.saturating_mul(10), false)
        }
        CounterKeyAction::PageIncrement => {
            counter_adjusted_value(value, min, max, step.saturating_mul(10), true)
        }
    }
}

fn counter_key_action(key: &Key) -> Option<CounterKeyAction> {
    match key {
        Key::Character(value) if value == "-" => Some(CounterKeyAction::Decrement),
        Key::Character(value) if value == "+" => Some(CounterKeyAction::Increment),
        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowDown) => {
            Some(CounterKeyAction::Decrement)
        }
        Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowUp) => {
            Some(CounterKeyAction::Increment)
        }
        Key::Named(NamedKey::Home) => Some(CounterKeyAction::Minimum),
        Key::Named(NamedKey::End) => Some(CounterKeyAction::Maximum),
        Key::Named(NamedKey::PageDown) => Some(CounterKeyAction::PageDecrement),
        Key::Named(NamedKey::PageUp) => Some(CounterKeyAction::PageIncrement),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_adjustment_clamps_uneven_steps_and_integer_extremes() {
        assert_eq!(counter_adjusted_value(8, 0, 10, 3, true), Some(10));
        assert_eq!(counter_adjusted_value(0, 0, 10, 3, false), None);
        assert_eq!(
            counter_adjusted_value(i64::MAX - 1, i64::MIN, i64::MAX, 2, true),
            Some(i64::MAX)
        );
        assert_eq!(counter_adjusted_value(1, 4, 3, 1, true), None);
    }

    #[test]
    fn counter_keyboard_targets_use_bounds_and_page_steps() {
        assert_eq!(
            counter_value_for_key(1, 0, 20, 2, &Key::Named(NamedKey::PageUp)),
            Some(20)
        );
        assert_eq!(
            counter_value_for_key(7, 0, 20, 2, &Key::Named(NamedKey::Home)),
            Some(0)
        );
        assert_eq!(
            counter_value_for_key(20, 0, 20, 2, &Key::Named(NamedKey::ArrowUp)),
            None
        );
        assert_eq!(
            counter_value_for_key(4, 0, 20, 2, &Key::Character("+".into())),
            Some(6)
        );
    }

    #[test]
    fn counter_hold_waits_then_accelerates_until_the_boundary() {
        let now = Instant::now();
        let range = CounterValueRange {
            value: 0,
            min: 0,
            max: 4,
            step: 1,
        };
        let (value, repeat) = counter_hold_plan(7, range, true, now).unwrap();
        let mut repeat = repeat.unwrap();

        assert_eq!(value, 1);
        assert_eq!(
            advance_counter_hold(&mut repeat, now),
            CounterRepeatAdvance::Waiting(now + COUNTER_HOLD_INITIAL_DELAY)
        );
        let first_deadline = now + COUNTER_HOLD_INITIAL_DELAY;
        assert!(matches!(
            advance_counter_hold(&mut repeat, first_deadline),
            CounterRepeatAdvance::Changed { value: 2, .. }
        ));
        assert_eq!(counter_repeat_interval(1), Duration::from_millis(180));
        assert_eq!(counter_repeat_interval(4), Duration::from_millis(100));
        assert_eq!(counter_repeat_interval(12), Duration::from_millis(50));
    }

    #[test]
    fn counter_hold_stops_as_soon_as_it_reaches_a_boundary() {
        let now = Instant::now();
        let range = CounterValueRange {
            value: 1,
            min: 0,
            max: 3,
            step: 1,
        };
        let (_, repeat) = counter_hold_plan(8, range, true, now).unwrap();
        let mut repeat = repeat.unwrap();

        assert!(matches!(
            advance_counter_hold(&mut repeat, now + COUNTER_HOLD_INITIAL_DELAY),
            CounterRepeatAdvance::Changed {
                value: 3,
                next_deadline: None,
            }
        ));
    }

    #[test]
    fn counter_hold_sync_uses_only_new_application_values() {
        let now = Instant::now();
        let range = CounterValueRange {
            value: 3,
            min: 0,
            max: 10,
            step: 1,
        };
        let (_, repeat) = counter_hold_plan(9, range, true, now).unwrap();
        let mut repeat = repeat.unwrap();

        sync_counter_hold_range(&mut repeat, 3, 0, 10, 1);
        assert_eq!(repeat.range.value, 4);
        sync_counter_hold_range(&mut repeat, 4, 0, 10, 1);
        assert_eq!(repeat.range.value, 4);
        sync_counter_hold_range(&mut repeat, 7, 0, 8, 1);
        assert_eq!(repeat.range.value, 7);
        assert_eq!(repeat.range.max, 8);
    }
}
