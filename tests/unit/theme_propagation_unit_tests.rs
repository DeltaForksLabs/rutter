use arboard::Clipboard;
use cosmic_text::FontSystem;
use skia_safe::Color;
use taffy::prelude::Style;

use super::Theme;
use crate::{AppLogic, MultiWindowAppLogic, SurfaceCommand, SurfaceId, Widget};

struct StaticThemeApp;

impl AppLogic for StaticThemeApp {
    type State = bool;
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {
        false
    }

    fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Spacer {
            style: Style::default(),
        }
    }

    fn update(_: &mut Self::State, _: Self::Message, _: &mut Clipboard) {}

    fn theme() -> Theme {
        Theme::dark()
    }
}

struct StateAwareThemeApp;

impl AppLogic for StateAwareThemeApp {
    type State = bool;
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {
        false
    }

    fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Spacer {
            style: Style::default(),
        }
    }

    fn update(_: &mut Self::State, _: Self::Message, _: &mut Clipboard) {}

    fn theme_for(dark: &Self::State) -> Theme {
        if *dark {
            return Theme::dark();
        }
        Theme::light()
    }
}

struct StaticThemeMultiWindowApp;

impl MultiWindowAppLogic for StaticThemeMultiWindowApp {
    type State = bool;
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {
        false
    }

    fn view<'a>(_: &'a mut Self::State, _: SurfaceId) -> Widget<'a, Self::Message> {
        Widget::Spacer {
            style: Style::default(),
        }
    }

    fn update(
        _: &mut Self::State,
        _: SurfaceId,
        _: Self::Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        Vec::new()
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

#[test]
fn app_theme_for_falls_back_to_static_theme() {
    let resolved = StaticThemeApp::theme_for(&false);

    assert_eq!(resolved.surface, Theme::dark().surface);
}

#[test]
fn app_theme_for_can_select_theme_from_state() {
    assert_eq!(
        StateAwareThemeApp::theme_for(&false).surface,
        Theme::light().surface
    );
    assert_eq!(
        StateAwareThemeApp::theme_for(&true).surface,
        Theme::dark().surface
    );
}

#[test]
fn multi_window_theme_for_falls_back_to_static_theme() {
    let resolved = StaticThemeMultiWindowApp::theme_for(&false);

    assert_eq!(resolved.surface, Theme::dark().surface);
}

#[test]
fn light_constructor_matches_the_unchanged_default() {
    let light = Theme::light();
    let default = Theme::default();

    assert_eq!(light.primary, default.primary);
    assert_eq!(light.on_primary, default.on_primary);
    assert_eq!(light.surface, Color::WHITE);
    assert_eq!(light.surface, default.surface);
    assert_eq!(light.on_surface, default.on_surface);
    assert_eq!(light.error, default.error);
    assert_eq!(light.success, default.success);
    assert_eq!(light.font_body, default.font_body);
    assert_eq!(light.font_label, default.font_label);
    assert_eq!(light.font_small, default.font_small);
    assert_eq!(light.radius_sm, default.radius_sm);
    assert_eq!(light.radius_md, default.radius_md);
    assert_eq!(light.spacing, default.spacing);
}
