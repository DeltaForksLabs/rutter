// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{CarouselConfig, CarouselPosition, CarouselSizing};
use crate::i18n::LayoutDirection;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselItemFrame {
    pub index: usize,
    pub x: f32,
    pub width: f32,
}

pub(crate) fn carousel_item_frames(
    config: &CarouselConfig,
    position: CarouselPosition,
    viewport_width: f32,
    item_count: usize,
    direction: LayoutDirection,
) -> Vec<CarouselItemFrame> {
    if item_count == 0 || !viewport_width.is_finite() || viewport_width <= 0.0 {
        return Vec::new();
    }
    let frames = match &config.sizing {
        CarouselSizing::Uncontained { item_extent } => {
            fixed_item_frames(*item_extent, position, viewport_width, item_count)
        }
        CarouselSizing::Weighted { flex_weights } => {
            weighted_item_frames(flex_weights, position, viewport_width, item_count)
        }
    };
    orient_item_frames(frames, viewport_width, direction)
}

pub(crate) fn carousel_index_at(
    config: &CarouselConfig,
    position: CarouselPosition,
    viewport_width: f32,
    item_count: usize,
    local_x: f32,
    direction: LayoutDirection,
) -> Option<usize> {
    carousel_item_frames(config, position, viewport_width, item_count, direction)
        .into_iter()
        .find(|frame| local_x >= frame.x && local_x < frame.x + frame.width)
        .map(|frame| frame.index)
}

pub(crate) fn carousel_scroll_step(config: &CarouselConfig, viewport_width: f32) -> f32 {
    match &config.sizing {
        CarouselSizing::Uncontained { item_extent } => item_extent.min(viewport_width).max(1.0),
        CarouselSizing::Weighted { flex_weights } => {
            weighted_base_extents(flex_weights, viewport_width)[0].max(1.0)
        }
    }
}

pub(crate) fn carousel_max_position(
    config: &CarouselConfig,
    viewport_width: f32,
    item_count: usize,
) -> CarouselPosition {
    if item_count == 0 || viewport_width <= 0.0 {
        return CarouselPosition::default();
    }
    match config.sizing {
        CarouselSizing::Uncontained { item_extent } => {
            fixed_max_position(item_extent.min(viewport_width), viewport_width, item_count)
        }
        CarouselSizing::Weighted { .. } => CarouselPosition::exact(item_count.saturating_sub(1)),
    }
}

pub(crate) fn carousel_prominent_index(
    position: CarouselPosition,
    item_count: usize,
) -> Option<usize> {
    position.rounded_index(item_count)
}

fn fixed_item_frames(
    item_extent: f32,
    position: CarouselPosition,
    viewport_width: f32,
    item_count: usize,
) -> Vec<CarouselItemFrame> {
    let extent = item_extent.min(viewport_width);
    let maximum = fixed_max_position(extent, viewport_width, item_count);
    let position = position.clamped(maximum);
    let scroll_x = position.progress * extent;
    let first = position.index.min(item_count);
    let last = fixed_visible_end(first, extent, viewport_width, item_count);
    (first..last)
        .map(|index| fixed_item_frame(index, first, extent, scroll_x))
        .collect()
}

fn fixed_visible_end(first: usize, extent: f32, viewport: f32, count: usize) -> usize {
    let visible = ((viewport / extent).ceil() as usize).min(count);
    first.saturating_add(visible).saturating_add(1).min(count)
}

fn fixed_item_frame(index: usize, first: usize, extent: f32, scroll_x: f32) -> CarouselItemFrame {
    CarouselItemFrame {
        index,
        x: index.saturating_sub(first) as f32 * extent - scroll_x,
        width: extent,
    }
}

fn fixed_max_position(
    item_extent: f32,
    viewport_width: f32,
    item_count: usize,
) -> CarouselPosition {
    if item_extent <= 0.0 {
        return CarouselPosition::default();
    }
    let visible_items = viewport_width / item_extent;
    if !visible_items.is_finite() {
        return CarouselPosition::default();
    }
    CarouselPosition::exact(item_count).shifted(-visible_items)
}

fn weighted_item_frames(
    flex_weights: &[u16],
    position: CarouselPosition,
    viewport_width: f32,
    item_count: usize,
) -> Vec<CarouselItemFrame> {
    let maximum = CarouselPosition::exact(item_count.saturating_sub(1));
    let position = position.clamped(maximum);
    let prominent = position.index;
    let largest_slot = largest_weight_slot(flex_weights);
    let extents = transitioned_extents(flex_weights, viewport_width, position.progress);
    let layout = WeightedFrameLayout {
        prominent,
        largest_slot,
        item_count,
        viewport_width,
    };
    collect_weighted_frames(&layout, &extents)
}

struct WeightedFrameLayout {
    prominent: usize,
    largest_slot: usize,
    item_count: usize,
    viewport_width: f32,
}

fn collect_weighted_frames(
    layout: &WeightedFrameLayout,
    extents: &[f32],
) -> Vec<CarouselItemFrame> {
    let mut frames = Vec::with_capacity(extents.len());
    let mut x = 0.0;
    for (slot, width) in extents.iter().copied().enumerate() {
        if let Some(frame) = weighted_item_frame(layout, slot, x, width.min(layout.viewport_width))
        {
            frames.push(frame);
        }
        x += width;
    }
    frames
}

fn weighted_item_frame(
    layout: &WeightedFrameLayout,
    slot: usize,
    x: f32,
    width: f32,
) -> Option<CarouselItemFrame> {
    let index = weighted_item_index(layout.prominent, layout.largest_slot, slot)?;
    (index < layout.item_count && width > f32::EPSILON).then_some(CarouselItemFrame {
        index,
        x,
        width,
    })
}

fn weighted_item_index(prominent: usize, largest_slot: usize, slot: usize) -> Option<usize> {
    if slot >= largest_slot {
        return prominent.checked_add(slot - largest_slot);
    }
    prominent.checked_sub(largest_slot - slot)
}

fn transitioned_extents(flex_weights: &[u16], viewport_width: f32, progress: f32) -> Vec<f32> {
    let base = weighted_base_extents(flex_weights, viewport_width);
    let mut extents = Vec::with_capacity(base.len() + 1);
    extents.push(base[0] * (1.0 - progress));
    for slot in 1..base.len() {
        let extent = base[slot] + (base[slot - 1] - base[slot]) * progress;
        extents.push(extent);
    }
    extents.push(base[base.len() - 1] * progress);
    extents
}

fn weighted_base_extents(flex_weights: &[u16], viewport_width: f32) -> Vec<f32> {
    let total = flex_weights
        .iter()
        .map(|weight| *weight as u64)
        .sum::<u64>() as f32;
    flex_weights
        .iter()
        .map(|weight| viewport_width * *weight as f32 / total)
        .collect()
}

fn largest_weight_slot(flex_weights: &[u16]) -> usize {
    let largest = flex_weights.iter().copied().max().unwrap_or(1);
    flex_weights
        .iter()
        .position(|weight| *weight == largest)
        .unwrap_or(0)
}

fn orient_item_frames(
    mut frames: Vec<CarouselItemFrame>,
    viewport_width: f32,
    direction: LayoutDirection,
) -> Vec<CarouselItemFrame> {
    if direction == LayoutDirection::Rtl {
        for frame in &mut frames {
            frame.x = viewport_width - frame.x - frame.width;
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weighted() -> CarouselConfig {
        CarouselConfig::weighted([1, 6, 1]).unwrap()
    }

    fn frames(
        config: &CarouselConfig,
        position: f32,
        viewport_width: f32,
        count: usize,
    ) -> Vec<CarouselItemFrame> {
        carousel_item_frames(
            config,
            test_position(position),
            viewport_width,
            count,
            LayoutDirection::Ltr,
        )
    }

    fn test_position(value: f32) -> CarouselPosition {
        CarouselPosition::with_progress(value.floor() as usize, value.fract())
    }

    #[test]
    fn fixed_frames_materialize_only_visible_items() {
        let config = CarouselConfig::uncontained(200.0).unwrap();
        let frames = frames(&config, 2.0, 500.0, 100);
        assert_eq!(
            frames.iter().map(|frame| frame.index).collect::<Vec<_>>(),
            [2, 3, 4, 5]
        );
    }

    #[test]
    fn fixed_maximum_preserves_counts_above_f32_integer_precision() {
        let maximum = fixed_max_position(1.0, 16_777_216.0, 16_777_217);
        assert_eq!(maximum, CarouselPosition::exact(1));
    }

    #[test]
    fn fixed_maximum_preserves_usize_end_boundary() {
        let maximum = fixed_max_position(1.0, 3.0, usize::MAX);
        assert_eq!(maximum, CarouselPosition::exact(usize::MAX - 3));
    }

    #[test]
    fn fixed_maximum_retains_fractional_trailing_offset() {
        let maximum = fixed_max_position(200.0, 500.0, 10);
        assert_eq!(maximum.index, 7);
        assert!((maximum.progress - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn weighted_start_places_first_item_in_largest_slot() {
        let frames = frames(&weighted(), 0.0, 800.0, 10);
        assert_eq!(frames[0].index, 0);
        assert!((frames[0].x - 100.0).abs() < 0.01);
        assert!((frames[0].width - 600.0).abs() < 0.01);
    }

    #[test]
    fn weighted_half_step_interpolates_adjacent_items() {
        let frames = frames(&weighted(), 0.5, 800.0, 10);
        assert!((frames[0].width - 350.0).abs() < 0.01);
        assert!((frames[1].width - 350.0).abs() < 0.01);
    }

    #[test]
    fn weighted_next_step_moves_largest_extent_to_next_item() {
        let frames = frames(&weighted(), 1.0, 800.0, 10);
        let second = frames.iter().find(|frame| frame.index == 1).unwrap();
        assert!((second.width - 600.0).abs() < 0.01);
    }

    #[test]
    fn weighted_geometry_never_builds_more_than_transition_window() {
        let frames = frames(&weighted(), 40.25, 800.0, 10_000);
        assert!(frames.len() <= 4);
    }

    #[test]
    fn weighted_layout_uses_first_largest_slot_for_equal_weights() {
        let config = CarouselConfig::weighted([5, 1, 5]).unwrap();
        let frames = frames(&config, 0.0, 1100.0, 10);
        assert!((frames[0].x - 0.0).abs() < f32::EPSILON);
        assert!((frames[0].width - 500.0).abs() < 0.01);
    }

    #[test]
    fn index_at_respects_dynamic_weighted_bounds() {
        let config = weighted();
        assert_eq!(
            carousel_index_at(
                &config,
                CarouselPosition::default(),
                800.0,
                10,
                120.0,
                LayoutDirection::Ltr,
            ),
            Some(0)
        );
        assert_eq!(
            carousel_index_at(
                &config,
                CarouselPosition::default(),
                800.0,
                10,
                750.0,
                LayoutDirection::Ltr,
            ),
            Some(1)
        );
    }

    #[test]
    fn empty_carousel_has_no_frames_or_prominent_item() {
        assert!(frames(&weighted(), 0.0, 800.0, 0).is_empty());
        assert_eq!(
            carousel_prominent_index(CarouselPosition::default(), 0),
            None
        );
    }

    #[test]
    fn rtl_geometry_mirrors_frames_and_hit_testing() {
        let config = weighted();
        let rtl = carousel_item_frames(
            &config,
            CarouselPosition::default(),
            800.0,
            10,
            LayoutDirection::Rtl,
        );
        assert!((rtl[0].x - 100.0).abs() < 0.01);
        assert_eq!(rtl[0].index, 0);
        assert_eq!(
            carousel_index_at(
                &config,
                CarouselPosition::default(),
                800.0,
                10,
                50.0,
                LayoutDirection::Rtl,
            ),
            Some(1)
        );
    }

    #[test]
    fn large_adjacent_indices_keep_distinct_prominent_frames() {
        let requested = 16_777_217;
        let frames = carousel_item_frames(
            &weighted(),
            CarouselPosition::exact(requested),
            800.0,
            20_000_000,
            LayoutDirection::Ltr,
        );
        let prominent = frames
            .iter()
            .find(|frame| frame.index == requested)
            .unwrap();
        assert!((prominent.width - 600.0).abs() < 0.01);
    }

    #[test]
    fn maximum_item_count_keeps_last_valid_index_exact() {
        let requested = usize::MAX - 1;
        let frames = carousel_item_frames(
            &weighted(),
            CarouselPosition::exact(requested),
            800.0,
            usize::MAX,
            LayoutDirection::Ltr,
        );
        let prominent = frames
            .iter()
            .find(|frame| frame.index == requested)
            .unwrap();
        assert!((prominent.width - 600.0).abs() < 0.01);
    }
}
