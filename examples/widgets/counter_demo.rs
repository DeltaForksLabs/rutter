// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use rutter::{AppLogic, RutterRunner, Theme, Widget};
use taffy::prelude::*;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

#[derive(Default)]
pub struct CounterDemoState {
    theme: ExampleTheme,
    quantity: i64,
    amount: f64,
}

#[derive(Clone, Debug)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    QuantityChanged(i64),
    Amount(f64),
}

pub struct CounterDemo;

impl AppLogic for CounterDemo {
    type State = CounterDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        CounterDemoState {
            theme: ExampleTheme::Dark,
            quantity: 1,
            amount: 0.5,
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Msg> {
        super::layout::page(
            "Counter",
            Widget::Column {
                children: counter_demo_children(state),
                style: counter_demo_column_style(),
            },
        )
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
            Msg::QuantityChanged(quantity) => state.quantity = quantity,
            Msg::Amount(amount) => state.amount = amount,
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
        demo_text("- 1 + with bounded, accelerating hold updates", 14.0),
        super::layout::section(vec![
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
            demo_text(format!("Quantity: {}", state.quantity), 14.0),
        ]),
        super::layout::section(vec![
            Widget::counter(
                state.amount,
                -10.0,
                10.0,
                0.125,
                Msg::Amount,
                counter_demo_style(),
                "Fractional amount",
            )
            .with_id(2),
            demo_text(
                format!("Fractional amount: {} (step 0.125)", state.amount),
                14.0,
            ),
        ]),
        demo_text(
            "Hold an on-screen - or + to accelerate, or use +/- keys, Arrows, Home, End, and Page keys.",
            12.0,
        ),
    ]
}

fn counter_demo_column_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FlexStart),
        size: Size::percent(1.0_f32),
        padding: Rect::length(spacing::XL),
        gap: Size::length(spacing::LG),
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
        style: responsive_width(560.0, Dimension::auto()),
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
        assert_eq!(state.amount, 0.5);
    }

    #[test]
    fn demo_includes_integer_and_fractional_counters() {
        let mut fonts = FontSystem::new();
        let state = CounterDemo::new(&mut fonts);
        let children = counter_demo_children(&state);
        let controls: Vec<_> = children
            .iter()
            .flat_map(|widget| match widget {
                Widget::Column { children, .. } => children.iter().collect::<Vec<_>>(),
                other => vec![other],
            })
            .collect();
        assert!(
            controls
                .iter()
                .any(|widget| matches!(widget, Widget::Counter { id: 1, .. }))
        );
        let counter = controls
            .iter()
            .find_map(|widget| match widget {
                Widget::Counter {
                    id: 2,
                    config: rutter::CounterConfig::Float(parameters),
                    ..
                } => Some(parameters),
                _ => None,
            })
            .expect("demo contains a fractional counter");
        assert_eq!((counter.value, counter.step), (0.5, 0.125));
        assert!(matches!((counter.on_change)(0.625), Msg::Amount(0.625)));
    }
}
