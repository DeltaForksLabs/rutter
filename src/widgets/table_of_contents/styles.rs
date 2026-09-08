// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use taffy::prelude::{Dimension, FlexDirection, LengthPercentage, Rect, Size, Style};

use super::{TABLE_OF_CONTENTS_ACCORDION_HEADER_HEIGHT, TABLE_OF_CONTENTS_NAVIGATION_PADDING};

const TABLE_OF_CONTENTS_ENTRY_HEIGHT: f32 = 28.0;
const TABLE_OF_CONTENTS_MIN_VIEWPORT_HEIGHT: f32 = 44.0;

pub(crate) fn root_style(style: &Style) -> Style {
    let mut root = style.clone();
    root.flex_direction = FlexDirection::Column;
    root.flex_shrink = 0.0;
    root.max_size.height = Dimension::auto();
    if style.size.height.into_option().is_some() {
        root.size.height = Dimension::auto();
        root.min_size.height = Dimension::auto();
    } else if !style.size.height.is_auto() {
        root.min_size.height = style.size.height;
        root.size.height = Dimension::auto();
    }
    root
}

pub(crate) fn navigation_style(has_accordion: bool) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: automatic_navigation_size(),
        min_size: Size::zero(),
        padding: Rect::length(navigation_padding(has_accordion)),
        gap: navigation_gap(has_accordion),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

pub(crate) fn navigation_header_style(has_accordion: bool) -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: header_height(has_accordion),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

pub(crate) fn navigation_entries_style(has_accordion: bool) -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        size: automatic_navigation_size(),
        min_size: Size::zero(),
        padding: Rect::length(entries_padding(has_accordion)),
        gap: Size {
            width: LengthPercentage::length(16.0),
            height: LengthPercentage::length(0.0),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

pub(crate) fn navigation_column_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size::auto(),
        min_size: Size::zero(),
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(4.0),
        },
        flex_grow: 1.0,
        flex_shrink: 1.0,
        flex_basis: Dimension::length(0.0),
        ..Style::default()
    }
}

pub(crate) fn entry_style(depth: usize) -> Style {
    let mut style = navigation_header_style(false);
    style.size.height = Dimension::length(TABLE_OF_CONTENTS_ENTRY_HEIGHT);
    style.padding = Rect::length(4.0_f32);
    style.padding.left = LengthPercentage::length(8.0 + depth as f32 * 16.0);
    style
}

pub(crate) fn viewport_style(table_style: &Style) -> Style {
    if table_style.size.height.into_option().is_some() {
        return fixed_viewport_style(table_style);
    }
    if !table_style.size.height.is_auto() || table_style.flex_grow > 0.0 {
        return bounded_viewport_style();
    }
    intrinsic_viewport_style()
}

fn fixed_viewport_style(table_style: &Style) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size {
            width: Dimension::percent(1.0),
            height: table_style.size.height,
        },
        min_size: height_limit(table_style.min_size.height),
        max_size: height_limit(table_style.max_size.height),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn bounded_viewport_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(0.0),
        },
        min_size: height_limit(Dimension::length(TABLE_OF_CONTENTS_MIN_VIEWPORT_HEIGHT)),
        flex_grow: 1.0,
        flex_shrink: 0.0,
        flex_basis: Dimension::length(0.0),
        ..Style::default()
    }
}

fn intrinsic_viewport_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: automatic_navigation_size(),
        min_size: Size::zero(),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Style::default()
    }
}

fn automatic_navigation_size() -> Size<Dimension> {
    Size {
        width: Dimension::percent(1.0),
        height: Dimension::auto(),
    }
}

fn navigation_padding(has_accordion: bool) -> f32 {
    if has_accordion {
        return 0.0;
    }
    TABLE_OF_CONTENTS_NAVIGATION_PADDING
}

fn entries_padding(has_accordion: bool) -> f32 {
    if has_accordion {
        return TABLE_OF_CONTENTS_NAVIGATION_PADDING;
    }
    0.0
}

fn navigation_gap(has_accordion: bool) -> Size<LengthPercentage> {
    Size {
        width: LengthPercentage::length(0.0),
        height: LengthPercentage::length(if has_accordion { 0.0 } else { 4.0 }),
    }
}

fn header_height(has_accordion: bool) -> Dimension {
    if has_accordion {
        return Dimension::length(TABLE_OF_CONTENTS_ACCORDION_HEADER_HEIGHT);
    }
    Dimension::auto()
}

fn height_limit(height: Dimension) -> Size<Dimension> {
    Size {
        width: Dimension::auto(),
        height,
    }
}
