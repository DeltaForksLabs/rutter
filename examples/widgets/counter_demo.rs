// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{AppLogic, RutterRunner, Theme, Widget};
use taffy::prelude::*;

use super::theme_selector::{ExampleTheme, example_theme_selector};

#[derive(Default)]
pub struct CounterDemoState {
    theme: ExampleTheme,
    quantity: i64,
}

#[derive(Clone, Debug)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    QuantityChanged(i64),
}

pub struct CounterDemo;

impl AppLogic for CounterDemo {
    type State = CounterDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        CounterDemoState {
            theme: ExampleTheme::Dark,
            quantity: 1,
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Msg> {
        Widget::Column {
            children: counter_demo_children(state),
            style: counter_demo_column_style(),
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
            Msg::QuantityChanged(quantity) => state.quantity = quantity,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn counter_demo_children<'a>(state: &CounterDemoState) -> Vec<Widget<'a, Msg>> {
    vec![
        example_theme_selector(state.theme, Msg::ThemeChanged),
        demo_text("Counter", 24.0),
        demo_text("- 1 + with bounded integer updates", 14.0),
        Widget::counter(
            state.quantity,
            0,
            1000,
            1,
            Msg::QuantityChanged,
            counter_demo_style(),
            "Quantity",
        )
        .with_id(1),
        demo_text(&format!("Quantity: {}", state.quantity), 14.0),
        demo_text(
            "Use the buttons or Arrow keys, Home, End, Page Up, and Page Down.",
            12.0,
        ),
    ]
}

fn counter_demo_column_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FlexStart),
        size: Size::percent(1.0_f32),
        padding: Rect::length(40.0_f32),
        gap: Size::length(20.0_f32),
        ..Style::default()
    }
}

fn counter_demo_style() -> Style {
    Style {
        size: Size {
            width: Dimension::auto(),
            height: Dimension::length(40.0),
        },
        ..Style::default()
    }
}

fn demo_text<'a>(content: impl Into<String>, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: Style::default(),
        color: None,
        size,
    }
}

pub fn run() {
    RutterRunner::<CounterDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_starts_with_one_quantity() {
        let mut fonts = FontSystem::new();
        let state = CounterDemo::new(&mut fonts);

        assert_eq!(state.quantity, 1);
    }
}
