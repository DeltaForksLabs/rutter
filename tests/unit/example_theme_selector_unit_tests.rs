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
