// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::geometry::{
    carousel_index_at, carousel_max_position, carousel_prominent_index, carousel_scroll_step,
};
use super::{CarouselConfig, CarouselPosition};
use crate::i18n::LayoutDirection;

/// Runtime scroll and selection state retained for one carousel instance.
#[derive(Debug, Clone, Default)]
pub struct CarouselState {
    pub(crate) position: CarouselPosition,
    pub(crate) viewport_width: f32,
    pub(crate) selected_item: Option<usize>,
}

impl CarouselState {
    pub(crate) fn sync_viewport(&mut self, width: f32, config: &CarouselConfig, count: usize) {
        self.viewport_width = width.max(0.0);
        let maximum = carousel_max_position(config, self.viewport_width, count);
        self.position = self.position.clamped(maximum);
        self.selected_item = self.selected_item.filter(|index| *index < count);
    }

    pub(crate) fn scroll_by_pixels(
        &mut self,
        delta: f32,
        config: &CarouselConfig,
        count: usize,
    ) -> bool {
        if count == 0 || !delta.is_finite() || delta == 0.0 {
            return false;
        }
        let previous = self.position;
        self.position = self.scroll_target(delta, config, count);
        self.position != previous
    }

    pub(crate) fn select(&mut self, index: usize, config: &CarouselConfig, count: usize) {
        if index >= count {
            return;
        }
        self.selected_item = Some(index);
        self.position = self.position_for_index(index, config, count);
    }

    pub(crate) fn index_at(
        &self,
        local_x: f32,
        config: &CarouselConfig,
        count: usize,
        direction: LayoutDirection,
    ) -> Option<usize> {
        carousel_index_at(
            config,
            self.position,
            self.viewport_width,
            count,
            local_x,
            direction,
        )
    }

    pub(crate) fn current_index(&self, count: usize) -> Option<usize> {
        self.selected_item
            .filter(|index| *index < count)
            .or_else(|| carousel_prominent_index(self.position, count))
    }

    fn scroll_target(&self, delta: f32, config: &CarouselConfig, count: usize) -> CarouselPosition {
        let maximum = carousel_max_position(config, self.viewport_width, count);
        if config.item_snapping {
            return self.position.snapped(delta).clamped(maximum);
        }
        let step = carousel_scroll_step(config, self.viewport_width);
        self.position.shifted(delta / step).clamped(maximum)
    }

    fn position_for_index(
        &self,
        index: usize,
        config: &CarouselConfig,
        count: usize,
    ) -> CarouselPosition {
        let maximum = carousel_max_position(config, self.viewport_width, count);
        CarouselPosition::exact(index).clamped(maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(snapping: bool) -> CarouselConfig {
        CarouselConfig::uncontained(200.0)
            .unwrap()
            .with_item_snapping(snapping)
    }

    #[test]
    fn viewport_sync_clamps_position_after_resize() {
        let mut state = CarouselState {
            position: CarouselPosition::exact(8),
            viewport_width: 200.0,
            selected_item: None,
        };
        state.sync_viewport(600.0, &fixed(false), 10);
        assert_eq!(state.position, CarouselPosition::exact(7));
    }

    #[test]
    fn free_scroll_uses_fractional_item_position() {
        let mut state = CarouselState::default();
        state.sync_viewport(600.0, &fixed(false), 10);
        assert!(state.scroll_by_pixels(100.0, &fixed(false), 10));
        assert_eq!(state.position.index, 0);
        assert!((state.position.progress - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn snapping_advances_one_boundary_for_small_delta() {
        let mut state = CarouselState::default();
        state.sync_viewport(600.0, &fixed(true), 10);
        assert!(state.scroll_by_pixels(1.0, &fixed(true), 10));
        assert_eq!(state.position, CarouselPosition::exact(1));
    }

    #[test]
    fn selection_scrolls_item_into_reachable_position() {
        let mut state = CarouselState::default();
        state.sync_viewport(600.0, &fixed(false), 10);
        state.select(9, &fixed(false), 10);
        assert_eq!(state.selected_item, Some(9));
        assert_eq!(state.position, CarouselPosition::exact(7));
    }

    #[test]
    fn dynamic_hit_test_uses_current_position() {
        let config = CarouselConfig::weighted([1, 6, 1]).unwrap();
        let mut state = CarouselState::default();
        state.sync_viewport(800.0, &config, 10);
        assert_eq!(
            state.index_at(400.0, &config, 10, LayoutDirection::Ltr),
            Some(0)
        );
    }

    #[test]
    fn current_index_prefers_explicit_selection() {
        let state = CarouselState {
            position: CarouselPosition::exact(4),
            viewport_width: 800.0,
            selected_item: Some(2),
        };
        assert_eq!(state.current_index(10), Some(2));
    }
}
