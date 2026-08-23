// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

pub(crate) const COUNTER_DEFAULT_HEIGHT: f32 = 40.0;

const COUNTER_CHARACTER_WIDTH: f32 = 10.0;
const COUNTER_VALUE_PADDING: f32 = 12.0;
const COUNTER_MIN_ACTION_WIDTH: f32 = 24.0;
const COUNTER_MAX_ACTION_WIDTH: f32 = 32.0;

pub(crate) fn counter_action_width(height: f32) -> f32 {
    (height * 0.7).clamp(COUNTER_MIN_ACTION_WIDTH, COUNTER_MAX_ACTION_WIDTH)
}

pub(crate) fn counter_value_width(value: i64) -> f32 {
    let characters = value.to_string().chars().count().max(1);
    characters as f32 * COUNTER_CHARACTER_WIDTH + COUNTER_VALUE_PADDING
}

pub(crate) fn counter_preferred_width(value: i64, height: f32) -> f32 {
    counter_action_width(height) * 2.0 + counter_value_width(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_width_grows_with_digits_and_includes_a_negative_sign() {
        assert!(counter_value_width(1000) > counter_value_width(1));
        assert!(counter_value_width(-1) > counter_value_width(1));
        assert_eq!(counter_preferred_width(1, 40.0), 78.0);
    }
}
