// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::time::{Duration, Instant};

use winit::keyboard::{Key, NamedKey};

use super::RutterRunner;
use crate::CounterConfig;
use crate::app::AppLogic;
use crate::engine::CounterRuntime;

mod number;
use number::CounterNumber;

#[cfg(test)]
mod float_tests;

const COUNTER_HOLD_INITIAL_DELAY: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, Debug)]
struct CounterValueRange<T = i64> {
    value: T,
    min: T,
    max: T,
    step: T,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CounterHoldRepeat {
    Integer(CounterRepeat<i64>),
    Float(CounterRepeat<f64>),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CounterRepeat<T> {
    id: u64,
    generation: u64,
    increment: bool,
    range: CounterValueRange<T>,
    last_observed_value: T,
    next_repeat_at: Instant,
    repetition_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CounterRepeatAdvance<T> {
    Waiting(Instant),
    Changed {
        value: T,
        next_deadline: Option<Instant>,
    },
    Finished,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn begin_counter_hold(&mut self, id: u64, increment: bool) {
        self.counter_hold_repeat = None;
        let Some(entry) = self.engine.runtime_caches.counters.get(&id).cloned() else {
            return;
        };
        match entry.config {
            CounterConfig::Integer(counter) => self.start_counter_repeat(
                id,
                entry.generation,
                counter,
                increment,
                CounterHoldRepeat::Integer,
            ),
            CounterConfig::Float(counter) => self.start_counter_repeat(
                id,
                entry.generation,
                counter,
                increment,
                CounterHoldRepeat::Float,
            ),
        }
    }

    fn start_counter_repeat<T: CounterNumber>(
        &mut self,
        id: u64,
        generation: u64,
        counter: CounterRuntime<A::Message, T>,
        increment: bool,
        wrap: fn(CounterRepeat<T>) -> CounterHoldRepeat,
    ) {
        let range = CounterValueRange::from_counter(&counter);
        if let Some((value, mut repeat)) = counter_hold_plan(id, range, increment, Instant::now()) {
            if let Some(repeat) = repeat.as_mut() {
                repeat.generation = generation;
            }
            self.counter_hold_repeat = repeat.map(wrap);
            self.apply_counter_value(counter.on_change, value);
        }
    }

    pub(super) fn counter_hold_deadline(&mut self, now: Instant) -> Option<Instant> {
        if !self.mouse_down {
            self.counter_hold_repeat = None;
            return None;
        }
        let repeat = self.counter_hold_repeat.take()?;
        // Scheduled work can precede the queued redraw. Refresh the published
        // model before using callbacks, including when the numeric mode changes.
        if self.engine.layout_dirty {
            let size = self.engine.window.as_ref()?.surface_size();
            if let Err(error) = self.engine.try_ensure_layout_for_input(size) {
                self.fatal_error.get_or_insert_with(|| error.into());
                return None;
            }
        }
        let (id, generation) = match repeat {
            CounterHoldRepeat::Integer(repeat) => (repeat.id, repeat.generation),
            CounterHoldRepeat::Float(repeat) => (repeat.id, repeat.generation),
        };
        let entry = self.engine.runtime_caches.counters.get(&id)?.clone();
        if entry.generation != generation {
            return None;
        }
        // A published numeric-mode change must not retarget an active hold.
        match (repeat, entry.config) {
            (CounterHoldRepeat::Integer(repeat), CounterConfig::Integer(counter)) => {
                self.apply_counter_hold_advance(repeat, counter, CounterHoldRepeat::Integer, now)
            }
            (CounterHoldRepeat::Float(repeat), CounterConfig::Float(counter)) => {
                self.apply_counter_hold_advance(repeat, counter, CounterHoldRepeat::Float, now)
            }
            _ => None,
        }
    }

    fn apply_counter_hold_advance<T: CounterNumber>(
        &mut self,
        mut repeat: CounterRepeat<T>,
        counter: CounterRuntime<A::Message, T>,
        wrap: fn(CounterRepeat<T>) -> CounterHoldRepeat,
        now: Instant,
    ) -> Option<Instant> {
        sync_counter_hold_range(
            &mut repeat,
            counter.value,
            counter.min,
            counter.max,
            counter.step,
        );
        match advance_counter_hold(&mut repeat, now) {
            CounterRepeatAdvance::Waiting(deadline) => {
                self.counter_hold_repeat = Some(wrap(repeat));
                Some(deadline)
            }
            CounterRepeatAdvance::Changed {
                value,
                next_deadline,
            } => {
                self.counter_hold_repeat = next_deadline.map(|_| wrap(repeat));
                self.apply_counter_value(counter.on_change, value);
                self.redraw();
                next_deadline
            }
            CounterRepeatAdvance::Finished => None,
        }
    }

    pub(super) fn adjust_focused_counter_key(&mut self, id: u64, key: &Key) -> bool {
        match self
            .engine
            .runtime_caches
            .counters
            .get(&id)
            .map(|entry| entry.config.clone())
        {
            Some(CounterConfig::Integer(counter)) => self.adjust_counter_key(counter, key),
            Some(CounterConfig::Float(counter)) => self.adjust_counter_key(counter, key),
            None => false,
        }
    }

    fn adjust_counter_key<T: CounterNumber>(
        &mut self,
        counter: CounterRuntime<A::Message, T>,
        key: &Key,
    ) -> bool {
        let Some(value) =
            counter_value_for_key(counter.value, counter.min, counter.max, counter.step, key)
        else {
            return false;
        };
        self.apply_counter_value(counter.on_change, value);
        true
    }

    fn apply_counter_value<T>(&mut self, on_change: fn(T) -> A::Message, value: T) {
        A::update(
            &mut self.engine.app_state,
            on_change(value),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
    }
}

impl<T: Copy> CounterValueRange<T> {
    fn from_counter<Msg>(counter: &CounterRuntime<Msg, T>) -> Self {
        Self {
            value: counter.value,
            min: counter.min,
            max: counter.max,
            step: counter.step,
        }
    }
}

fn counter_hold_plan<T: CounterNumber>(
    id: u64,
    mut range: CounterValueRange<T>,
    increment: bool,
    now: Instant,
) -> Option<(T, Option<CounterRepeat<T>>)> {
    let observed_value = range.value;
    let value = counter_adjusted_value(range.value, range.min, range.max, range.step, increment)?;
    range.value = value;
    let can_repeat =
        counter_adjusted_value(value, range.min, range.max, range.step, increment).is_some();
    let repeat = can_repeat.then_some(CounterRepeat {
        id,
        generation: 0,
        increment,
        range,
        last_observed_value: observed_value,
        next_repeat_at: now + COUNTER_HOLD_INITIAL_DELAY,
        repetition_count: 0,
    });
    Some((value, repeat))
}

fn sync_counter_hold_range<T: CounterNumber>(
    repeat: &mut CounterRepeat<T>,
    value: T,
    min: T,
    max: T,
    step: T,
) {
    if value != repeat.last_observed_value && value != repeat.range.value {
        repeat.range.value = value;
    }
    repeat.last_observed_value = value;
    repeat.range.min = min;
    repeat.range.max = max;
    repeat.range.step = step;
}

fn advance_counter_hold<T: CounterNumber>(
    repeat: &mut CounterRepeat<T>,
    now: Instant,
) -> CounterRepeatAdvance<T> {
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

fn schedule_counter_repeat<T: CounterNumber>(
    repeat: &mut CounterRepeat<T>,
    value: T,
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

fn counter_adjusted_value<T: CounterNumber>(
    value: T,
    min: T,
    max: T,
    step: T,
    increment: bool,
) -> Option<T> {
    if !T::valid_range(value, min, max, step) {
        return None;
    }
    counter_bounded_target(value, min, max, value.stepped(step, increment))
}

// Validate the inputs before computing a target. An overflowing final target
// clamps to a finite endpoint; unchecked out-of-range values are corrected first.
fn counter_bounded_target<T: CounterNumber>(value: T, min: T, max: T, target: T) -> Option<T> {
    let bounded = value.bounded(min, max);
    if bounded != value {
        return Some(bounded);
    }
    let next = target.bounded(min, max);
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

fn counter_value_for_key<T: CounterNumber>(
    value: T,
    min: T,
    max: T,
    step: T,
    key: &Key,
) -> Option<T> {
    if !T::valid_range(value, min, max, step) {
        return None;
    }
    match counter_key_action(key)? {
        CounterKeyAction::Decrement => counter_adjusted_value(value, min, max, step, false),
        CounterKeyAction::Increment => counter_adjusted_value(value, min, max, step, true),
        CounterKeyAction::Minimum => (value != min).then_some(min),
        CounterKeyAction::Maximum => (value != max).then_some(max),
        CounterKeyAction::PageDecrement => {
            counter_bounded_target(value, min, max, value.page_target(step, false))
        }
        CounterKeyAction::PageIncrement => {
            counter_bounded_target(value, min, max, value.page_target(step, true))
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
        assert_eq!(counter_adjusted_value::<i64>(8, 0, 10, 3, true), Some(10));
        assert_eq!(counter_adjusted_value::<i64>(0, 0, 10, 3, false), None);
        assert_eq!(
            counter_adjusted_value(i64::MAX - 1, i64::MIN, i64::MAX, 2, true),
            Some(i64::MAX)
        );
        assert_eq!(counter_adjusted_value::<i64>(1, 4, 3, 1, true), None);
    }

    #[test]
    fn counter_keyboard_targets_use_bounds_and_page_steps() {
        assert_eq!(
            counter_value_for_key::<i64>(1, 0, 20, 2, &Key::Named(NamedKey::PageUp)),
            Some(20)
        );
        assert_eq!(
            counter_value_for_key::<i64>(7, 0, 20, 2, &Key::Named(NamedKey::Home)),
            Some(0)
        );
        assert_eq!(
            counter_value_for_key::<i64>(20, 0, 20, 2, &Key::Named(NamedKey::ArrowUp)),
            None
        );
        assert_eq!(
            counter_value_for_key::<i64>(4, 0, 20, 2, &Key::Character("+".into())),
            Some(6)
        );
    }

    #[test]
    fn counter_hold_waits_then_accelerates_until_the_boundary() {
        let now = Instant::now();
        let range: CounterValueRange = CounterValueRange {
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
        let range: CounterValueRange = CounterValueRange {
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
        let range: CounterValueRange = CounterValueRange {
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
