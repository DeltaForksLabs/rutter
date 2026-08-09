// ============================================================
// Rutter Framework — demos/carousel_demo.rs
// Demonstrates lazy fixed and dynamically weighted carousels.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use skia_safe::Color;
use taffy::prelude::*;

use rutter::{AppLogic, CarouselConfig, RutterRunner, Theme, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

const ITEM_COUNT: usize = 2_000;

#[derive(Default)]
pub struct CarouselDemoState {
    theme: ExampleTheme,
    weighted_selection: Option<usize>,
    fixed_selection: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    SelectWeighted(usize),
    SelectFixed(usize),
}

pub struct CarouselDemo;

impl AppLogic for CarouselDemo {
    type State = CarouselDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        CarouselDemoState::default()
    }

    fn view<'a>(state: &'a mut CarouselDemoState) -> Widget<'a, Msg> {
        Widget::Column {
            style: root_style(),
            children: carousel_demo_children(state),
        }
    }

    fn update(state: &mut CarouselDemoState, message: Msg, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
            Msg::SelectWeighted(index) => state.weighted_selection = Some(index),
            Msg::SelectFixed(index) => state.fixed_selection = Some(index),
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn carousel_demo_children<'a>(state: &CarouselDemoState) -> Vec<Widget<'a, Msg>> {
    vec![
        example_theme_selector(state.theme, Msg::ThemeChanged),
        heading("CarouselView", 26.0),
        status_text("Weighted [1, 6, 2]", state.weighted_selection),
        weighted_carousel(),
        status_text("Uncontained 220 px + snapping", state.fixed_selection),
        fixed_carousel(),
        heading(
            "Use the wheel or trackpad over a carousel. Click a card, then use Left/Right, Home, or End.",
            12.0,
        ),
    ]
}

fn weighted_carousel<'a>() -> Widget<'a, Msg> {
    let config = CarouselConfig::weighted([1, 6, 2])
        .expect("demo weights must contain positive integers")
        .with_item_snapping(false)
        .with_accessibility_label("Dynamically weighted projects");
    Widget::carousel_view(
        ITEM_COUNT,
        weighted_card,
        Msg::SelectWeighted,
        config,
        carousel_style(230.0),
    )
    .with_id(810)
}

fn fixed_carousel<'a>() -> Widget<'a, Msg> {
    let config = CarouselConfig::uncontained(220.0)
        .expect("demo item extent must be finite and >= 1 logical pixel")
        .with_item_snapping(true)
        .with_accessibility_label("Recent projects");
    Widget::carousel_view(
        ITEM_COUNT,
        fixed_card,
        Msg::SelectFixed,
        config,
        carousel_style(150.0),
    )
    .with_id(811)
}

fn weighted_card<'a>(index: usize) -> Option<Widget<'a, Msg>> {
    Some(project_card(index, "Dynamic extent"))
}

fn fixed_card<'a>(index: usize) -> Option<Widget<'a, Msg>> {
    Some(project_card(index, "Fixed extent"))
}

fn project_card<'a>(index: usize, subtitle: &'static str) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Column {
            style: card_content_style(),
            children: vec![
                heading(&format!("Project #{:04}", index + 1), 18.0),
                heading(subtitle, 12.0),
                heading("Built lazily from the visible range + overscan", 11.0),
            ],
        }),
        style: fill_style(),
        color: Some(card_color(index)),
        radius: 12.0,
    }
}

fn status_text<'a>(layout: &str, selection: Option<usize>) -> Widget<'a, Msg> {
    let selection = selection
        .map(|index| format!("selected #{:04}", index + 1))
        .unwrap_or_else(|| "no selection".into());
    heading(&format!("{layout} — {selection}"), 14.0)
}

fn heading<'a>(content: &str, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: Style::default(),
        color: None,
        size,
    }
}

fn card_color(index: usize) -> Color {
    const COLORS: [Color; 5] = [
        Color::from_rgb(46, 79, 122),
        Color::from_rgb(84, 58, 125),
        Color::from_rgb(39, 105, 91),
        Color::from_rgb(128, 72, 54),
        Color::from_rgb(91, 91, 112),
    ];
    COLORS[index % COLORS.len()]
}

fn root_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size::percent(1.0_f32),
        padding: Rect::length(24.0_f32),
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(10.0),
        },
        ..Style::default()
    }
}

fn carousel_style(height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(height),
        },
        ..Style::default()
    }
}

fn card_content_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        justify_content: Some(JustifyContent::Center),
        padding: Rect::length(16.0_f32),
        gap: Size::length(8.0_f32),
        size: Size::percent(1.0_f32),
        ..Style::default()
    }
}

fn fill_style() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        ..Style::default()
    }
}

pub fn run() {
    RutterRunner::<CarouselDemo>::run();
}
