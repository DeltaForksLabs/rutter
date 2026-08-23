// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::keyboard::{Key, NamedKey};

use super::RutterRunner;
use crate::app::AppLogic;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn adjust_counter(&mut self, id: u64, increment: bool) {
        let Some(counter) = self.engine.runtime_caches.counters.get(&id).cloned() else {
            return;
        };
        let Some(value) = counter_adjusted_value(
            counter.value,
            counter.min,
            counter.max,
            counter.step,
            increment,
        ) else {
            return;
        };
        self.apply_counter_value(counter.on_change, value);
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
    }
}
