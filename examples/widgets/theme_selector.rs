// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use rutter::{ButtonVariant, Theme, Widget, spacing};
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
            right: LengthPercentageAuto::length(spacing::LG),
            top: LengthPercentageAuto::length(spacing::LG),
            bottom: LengthPercentageAuto::auto(),
        },
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        size: Size {
            width: Dimension::length(super::layout::THEME_BUTTON_SIZE),
            height: Dimension::length(super::layout::THEME_BUTTON_SIZE),
        },
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestMessage {
        Select(ExampleTheme),
    }

    #[test]
    fn example_theme_defaults_to_dark_and_resolves_both_palettes() {
        assert_eq!(ExampleTheme::default(), ExampleTheme::Dark);
        assert_eq!(ExampleTheme::Dark.resolve().surface, Theme::dark().surface);
        assert_eq!(
            ExampleTheme::Light.resolve().surface,
            Theme::light().surface
        );
    }

    #[test]
    fn dark_selector_shows_sun_and_switches_to_light() {
        let Widget::ButtonContent {
            label,
            child,
            on_press,
            style,
            variant,
            ..
        } = example_theme_selector(ExampleTheme::Dark, TestMessage::Select)
        else {
            panic!("dark theme selector must be a ButtonContent")
        };
        assert_eq!(label, "Switch to Light theme");
        assert_eq!(on_press, TestMessage::Select(ExampleTheme::Light));
        assert_eq!(variant, ButtonVariant::Ghost);
        assert_theme_toggle_position(&style);
        assert!(matches!(*child, Widget::Image { data, .. } if data == SUN_ICON_SVG));
    }

    #[test]
    fn light_selector_shows_moon_and_switches_to_dark() {
        let Widget::ButtonContent {
            label,
            child,
            on_press,
            ..
        } = example_theme_selector(ExampleTheme::Light, TestMessage::Select)
        else {
            panic!("light theme selector must be a ButtonContent")
        };
        assert_eq!(label, "Switch to Dark theme");
        assert_eq!(on_press, TestMessage::Select(ExampleTheme::Dark));
        assert!(matches!(*child, Widget::Image { data, .. } if data == MOON_ICON_SVG));
    }

    #[test]
    fn theme_toggle_svg_icons_are_parseable() {
        use skia_safe::{FontMgr, svg::Dom};

        assert!(Dom::from_bytes(SUN_ICON_SVG, FontMgr::empty()).is_ok());
        assert!(Dom::from_bytes(MOON_ICON_SVG, FontMgr::empty()).is_ok());
    }

    fn assert_theme_toggle_position(style: &Style) {
        assert_eq!(style.position, Position::Absolute);
        assert_eq!(style.inset.top, LengthPercentageAuto::length(16.0));
        assert_eq!(style.inset.right, LengthPercentageAuto::length(16.0));
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use cosmic_text::FontSystem;
    use rutter::{AppLogic, Theme, Widget};

    pub(crate) fn assert_single_window_theme<Application: AppLogic>() {
        let mut fonts = FontSystem::new();
        let mut state = Application::new(&mut fonts);
        assert_eq!(
            Application::theme_for(&state).surface,
            Theme::dark().surface
        );
        assert_theme_toggle(&Application::view(&mut state));
    }

    pub(crate) fn assert_theme_toggle<Message>(widget: &Widget<'_, Message>) {
        assert!(root_contains_theme_toggle(widget, "Switch to Light theme"));
    }

    fn root_contains_theme_toggle<Message>(widget: &Widget<'_, Message>, expected: &str) -> bool {
        match widget {
            Widget::Column { children, .. } | Widget::Row { children, .. } => children.iter().any(
                |child| matches!(child, Widget::ButtonContent { label, .. } if *label == expected),
            ),
            _ => false,
        }
    }
}
