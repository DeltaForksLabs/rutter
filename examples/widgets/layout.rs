use taffy::prelude::*;

/// Fills the available width on narrow surfaces without exceeding the desktop limit.
pub(super) fn responsive_width(max_width: f32, height: Dimension) -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height,
        },
        max_size: Size {
            width: Dimension::length(max_width),
            height: Dimension::auto(),
        },
        ..Style::default()
    }
}
