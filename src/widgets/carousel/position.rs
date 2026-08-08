// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct CarouselPosition {
    pub index: usize,
    pub progress: f32,
}

impl CarouselPosition {
    pub(crate) const fn exact(index: usize) -> Self {
        Self {
            index,
            progress: 0.0,
        }
    }

    pub(crate) fn with_progress(index: usize, progress: f32) -> Self {
        if !progress.is_finite() || progress <= 0.0 {
            return Self::exact(index);
        }
        let whole = progress.floor() as usize;
        Self {
            index: index.saturating_add(whole),
            progress: progress.fract(),
        }
    }

    pub(crate) fn clamped(self, maximum: Self) -> Self {
        if self.index > maximum.index {
            return maximum;
        }
        if self.index == maximum.index && self.progress > maximum.progress {
            return maximum;
        }
        self
    }

    pub(crate) fn shifted(self, delta: f32) -> Self {
        if !delta.is_finite() || delta == 0.0 {
            return self;
        }
        if delta > 0.0 {
            return self.shifted_forward(delta);
        }
        self.shifted_backward(-delta)
    }

    pub(crate) fn snapped(self, delta: f32) -> Self {
        if delta > 0.0 {
            return Self::exact(self.index.saturating_add(1));
        }
        if self.progress > 0.0 {
            return Self::exact(self.index);
        }
        Self::exact(self.index.saturating_sub(1))
    }

    pub(crate) fn rounded_index(self, item_count: usize) -> Option<usize> {
        if item_count == 0 {
            return None;
        }
        let rounded = if self.progress >= 0.5 {
            self.index.saturating_add(1)
        } else {
            self.index
        };
        Some(rounded.min(item_count - 1))
    }

    fn shifted_forward(self, delta: f32) -> Self {
        let whole = delta.floor() as usize;
        let mut index = self.index.saturating_add(whole);
        let mut progress = self.progress + delta.fract();
        if progress >= 1.0 {
            index = index.saturating_add(1);
            progress -= 1.0;
        }
        Self { index, progress }
    }

    fn shifted_backward(self, delta: f32) -> Self {
        let whole = delta.floor() as usize;
        if whole > self.index {
            return Self::default();
        }
        let mut index = self.index - whole;
        let fraction = delta.fract();
        if fraction <= self.progress {
            return Self::with_progress(index, self.progress - fraction);
        }
        if index == 0 {
            return Self::default();
        }
        index -= 1;
        Self::with_progress(index, 1.0 + self.progress - fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_normalization_preserves_exact_integer_anchor() {
        let position = CarouselPosition::with_progress(usize::MAX - 2, 1.25);
        assert_eq!(position.index, usize::MAX - 1);
        assert!((position.progress - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn clamping_compares_integer_anchor_before_progress() {
        let maximum = CarouselPosition::with_progress(10, 0.5);
        assert_eq!(CarouselPosition::exact(11).clamped(maximum), maximum);
        assert_eq!(
            CarouselPosition::with_progress(10, 0.25)
                .clamped(maximum)
                .progress,
            0.25
        );
    }

    #[test]
    fn shifting_crosses_boundaries_without_converting_anchor_to_float() {
        let start = CarouselPosition::with_progress(16_777_217, 0.75);
        let shifted = start.shifted(0.5);
        assert_eq!(shifted.index, 16_777_218);
        assert!((shifted.progress - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn backward_shift_clamps_at_collection_start() {
        let start = CarouselPosition::with_progress(2, 0.25);
        assert_eq!(start.shifted(-3.0), CarouselPosition::default());
    }

    #[test]
    fn snapping_uses_the_next_boundary_in_input_direction() {
        let position = CarouselPosition::with_progress(4, 0.25);
        assert_eq!(position.snapped(1.0), CarouselPosition::exact(5));
        assert_eq!(position.snapped(-1.0), CarouselPosition::exact(4));
    }

    #[test]
    fn backward_snap_preserves_boundary_for_tiny_positive_progress() {
        let position = CarouselPosition::with_progress(4, 1.0e-8);
        assert_eq!(position.snapped(-1.0), CarouselPosition::exact(4));
    }

    #[test]
    fn rounded_index_keeps_adjacent_large_indices_distinct() {
        let position = CarouselPosition::exact(16_777_217);
        assert_eq!(position.rounded_index(20_000_000), Some(16_777_217));
    }
}
