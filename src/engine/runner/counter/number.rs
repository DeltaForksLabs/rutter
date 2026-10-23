// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

/// Keeps integer arithmetic exact while sharing counter input and hold scheduling.
pub(super) trait CounterNumber: Copy + PartialEq {
    fn valid_range(value: Self, min: Self, max: Self, step: Self) -> bool;
    fn bounded(self, min: Self, max: Self) -> Self;
    fn stepped(self, step: Self, increment: bool) -> Self;
    fn page_target(self, step: Self, increment: bool) -> Self;
}

impl CounterNumber for i64 {
    fn valid_range(_: Self, min: Self, max: Self, step: Self) -> bool {
        min <= max && step > 0
    }

    fn bounded(self, min: Self, max: Self) -> Self {
        self.clamp(min, max)
    }

    fn stepped(self, step: Self, increment: bool) -> Self {
        if increment {
            self.saturating_add(step)
        } else {
            self.saturating_sub(step)
        }
    }

    fn page_target(self, step: Self, increment: bool) -> Self {
        self.stepped(step.saturating_mul(10), increment)
    }
}

impl CounterNumber for f64 {
    fn valid_range(value: Self, min: Self, max: Self, step: Self) -> bool {
        value.is_finite()
            && min.is_finite()
            && max.is_finite()
            && step.is_finite()
            && min <= max
            && step > 0.0
    }

    fn bounded(self, min: Self, max: Self) -> Self {
        self.clamp(min, max)
    }

    fn stepped(self, step: Self, increment: bool) -> Self {
        // Overflow becomes infinity and is clamped to a finite bound by the caller.
        if increment { self + step } else { self - step }
    }

    fn page_target(self, step: Self, increment: bool) -> Self {
        // Multiplying first can overflow even when cancellation with the value
        // makes the ten-step target finite and strictly inside the bounds.
        let signed_step = if increment { step } else { -step };
        signed_step.mul_add(10.0, self)
    }
}
