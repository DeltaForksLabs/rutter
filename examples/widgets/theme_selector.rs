// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use rutter::{ButtonVariant, Theme, Widget};
use taffy::prelude::*;

const SUN_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#F4B942" stroke-width="2" stroke-linecap="round">
<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.42 1.42M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.42-1.42M17.66 6.34l1.41-1.41"/>
</svg>"##;
const MOON_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#334155" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
</svg>"##;

/// Theme choices shared by every widget demonstration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExampleTheme {
    #[default]
    Dark,
    Light,
}

impl ExampleTheme {
    /// Resolves the selected framework theme.
    pub fn resolve(self) -> Theme {
        match self {
            Self::Dark => Theme::dark(),
            Self::Light => Theme::light(),
        }
    }
}

/// Builds an accessible icon toggle that switches an example's theme.
///
/// Example: `example_theme_selector(ExampleTheme::Dark, Message::ThemeChanged)`.
pub fn example_theme_selector<'a, Message: Clone>(
    selected: ExampleTheme,
    on_select: fn(ExampleTheme) -> Message,
) -> Widget<'a, Message> {
    let (icon, label, next_theme) = match selected {
        ExampleTheme::Dark => (SUN_ICON_SVG, "Switch to Light theme", ExampleTheme::Light),
        ExampleTheme::Light => (MOON_ICON_SVG, "Switch to Dark theme", ExampleTheme::Dark),
    };
    Widget::ButtonContent {
        label,
        child: Box::new(theme_toggle_icon(icon)),
        on_press: on_select(next_theme),
        style: theme_toggle_button_style(),
        color: None,
        variant: ButtonVariant::Ghost,
    }
}

fn theme_toggle_icon<'a, Message>(icon: &'static [u8]) -> Widget<'a, Message> {
    Widget::Image {
        data: icon,
        style: Style {
            size: Size::length(24.0_f32),
            ..Style::default()
        },
        radius: 0.0,
    }
}

fn theme_toggle_button_style() -> Style {
    Style {
        position: Position::Absolute,
        inset: Rect {
            left: LengthPercentageAuto::auto(),
            right: LengthPercentageAuto::length(16.0),
            top: LengthPercentageAuto::length(16.0),
            bottom: LengthPercentageAuto::auto(),
        },
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        size: Size {
            width: Dimension::length(44.0),
            height: Dimension::length(44.0),
        },
        ..Style::default()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/example_theme_selector_unit_tests.rs"]
mod tests;
