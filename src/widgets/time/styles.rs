// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use taffy::prelude::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage, Rect, Size, Style,
};

const TIME_PICKER_GAP: f32 = 8.0;
const TIME_PICKER_PADDING: f32 = 12.0;
const TIME_FIELD_HEIGHT: f32 = 40.0;
const TIME_FIELD_LABEL_HEIGHT: f32 = 16.0;

pub(super) fn time_picker_content_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size::percent(1.0_f32),
        padding: Rect::length(TIME_PICKER_PADDING),
        gap: Size::length(TIME_PICKER_GAP),
        ..Style::default()
    }
}

pub(super) fn time_picker_fields_style() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::FlexStart),
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(
                TIME_FIELD_HEIGHT + TIME_FIELD_LABEL_HEIGHT + TIME_PICKER_GAP,
            ),
        },
        gap: Size::length(TIME_PICKER_GAP),
        ..Style::default()
    }
}

pub(super) fn time_picker_field_style() -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::length(0.0),
        flex_direction: FlexDirection::Column,
        size: Size {
            width: Dimension::auto(),
            height: Dimension::percent(1.0),
        },
        gap: Size::length(TIME_PICKER_GAP),
        ..Style::default()
    }
}

pub(super) fn time_picker_counter_style() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(TIME_FIELD_HEIGHT),
        },
        ..Style::default()
    }
}

pub(super) fn time_picker_zone_style() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(TIME_FIELD_HEIGHT),
        },
        ..Style::default()
    }
}

pub(super) fn time_picker_label_style() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(TIME_FIELD_LABEL_HEIGHT),
        },
        ..Style::default()
    }
}

pub(super) fn time_picker_anchor_value_style() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        padding: Rect {
            left: LengthPercentage::length(12.0),
            ..Rect::zero()
        },
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        ..Style::default()
    }
}

pub(super) fn fill_parent_style() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use taffy::prelude::Dimension;

    use super::{time_picker_counter_style, time_picker_field_style, time_picker_zone_style};

    #[test]
    fn time_picker_fields_share_available_width_without_fixed_overflow() {
        let field = time_picker_field_style();
        let counter = time_picker_counter_style();
        let zone = time_picker_zone_style();

        assert_eq!(field.flex_grow, 1.0);
        assert_eq!(field.size.width, Dimension::auto());
        assert_eq!(counter.size.width, Dimension::percent(1.0));
        assert_eq!(zone.size.width, Dimension::percent(1.0));
    }
}
