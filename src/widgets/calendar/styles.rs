// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use taffy::prelude::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage, Rect, Size, Style,
};

const CALENDAR_GAP: f32 = 4.0;
const CALENDAR_PADDING: f32 = 8.0;
const CALENDAR_NAVIGATION_SIZE: f32 = 34.0;

pub(super) const CALENDAR_WEEKDAY_HEIGHT: f32 = 24.0;
pub(super) const CALENDAR_DAY_HEIGHT: f32 = 36.0;

pub(super) fn calendar_content_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size::percent(1.0_f32),
        padding: Rect::length(CALENDAR_PADDING),
        gap: Size::length(CALENDAR_GAP),
        ..Style::default()
    }
}

pub(super) fn calendar_header_style() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::Center),
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(CALENDAR_NAVIGATION_SIZE),
        },
        gap: Size::length(CALENDAR_GAP),
        ..Style::default()
    }
}

pub(super) fn calendar_heading_style() -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::length(0.0),
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(CALENDAR_NAVIGATION_SIZE),
        },
        ..Style::default()
    }
}

pub(super) fn calendar_heading_content_style() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        size: Size::percent(1.0_f32),
        gap: Size::length(CALENDAR_GAP),
        ..Style::default()
    }
}

pub(super) fn navigation_button_style() -> Style {
    Style {
        size: Size::length(CALENDAR_NAVIGATION_SIZE),
        ..Style::default()
    }
}

pub(super) fn navigation_icon_style() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        padding: Rect {
            left: LengthPercentage::length(11.0),
            ..Rect::zero()
        },
        ..Style::default()
    }
}

pub(super) fn calendar_row_style(height: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(height),
        },
        gap: Size::length(CALENDAR_GAP),
        ..Style::default()
    }
}

pub(super) fn calendar_cell_style(height: f32) -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::length(0.0),
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(height),
        },
        ..Style::default()
    }
}

pub(super) fn calendar_weekday_cell_style() -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::length(0.0),
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(CALENDAR_WEEKDAY_HEIGHT),
        },
        ..Style::default()
    }
}

pub(super) fn calendar_label_text_style(width: f32, font_size: f32) -> Style {
    Style {
        flex_shrink: 0.0,
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(font_size * 1.2),
        },
        ..Style::default()
    }
}

pub(super) fn date_picker_value_style() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        padding: Rect {
            left: LengthPercentage::length(12.0),
            ..Rect::zero()
        },
        ..Style::default()
    }
}

pub(super) fn fill_parent_style() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        ..Style::default()
    }
}
