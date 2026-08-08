// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{
    AppLogic, RichText, RichTextColor, RichTextSize, RichTextSpan, RichTextStyle, RutterRunner,
    Theme, Widget,
};

struct RichTextDemo;

impl AppLogic for RichTextDemo {
    type State = ();
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {}

    fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Column {
            style: page_style(),
            children: vec![
                Widget::rich_text(title_content(), line_style()),
                Widget::rich_text(emphasis_content(), line_style()),
                Widget::rich_text(inherited_content(), line_style()),
                Widget::rich_text(multilingual_content(), wrapping_style()),
            ],
        }
    }

    fn update(_: &mut Self::State, _: Self::Message, _: &mut Clipboard) {}

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn title_content() -> RichText<'static> {
    let defaults = RichTextStyle::default()
        .with_size(RichTextSize::new(30.0).unwrap())
        .with_color(RichTextColor::rgb(100, 170, 255));
    RichText::from_span(RichTextSpan::new("RichText spans").bold()).with_default_style(defaults)
}

fn emphasis_content() -> RichText<'static> {
    RichText::from_spans([
        RichTextSpan::new("One leaf can mix "),
        RichTextSpan::new("bold").bold(),
        RichTextSpan::new(", "),
        RichTextSpan::new("italic").italic(),
        RichTextSpan::new(", "),
        RichTextSpan::new("underline").underline(),
        RichTextSpan::new(" and "),
        RichTextSpan::new("color").with_color(RichTextColor::rgb(255, 110, 130)),
        RichTextSpan::new("."),
    ])
}

fn inherited_content() -> RichText<'static> {
    let defaults = RichTextStyle::default()
        .with_size(RichTextSize::new(18.0).unwrap())
        .with_color(RichTextColor::rgb(190, 200, 215));
    RichText::from_spans([
        RichTextSpan::new("Inherited defaults, "),
        RichTextSpan::new("larger text")
            .with_size(RichTextSize::new(24.0).unwrap())
            .bold(),
        RichTextSpan::new(" and explicit span overrides."),
    ])
    .with_default_style(defaults)
}

fn multilingual_content() -> RichText<'static> {
    RichText::from_spans([
        RichTextSpan::new("Advanced shaping: ").bold(),
        RichTextSpan::new("مرحبا بالعالم ").with_color(RichTextColor::rgb(120, 220, 170)),
        RichTextSpan::new("— hello world — "),
        RichTextSpan::new("こんにちは世界 👋").italic(),
    ])
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size::percent(1.0_f32),
        padding: Rect::length(40.0_f32),
        gap: Size::length(24.0_f32),
        ..Style::default()
    }
}

fn line_style() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        ..Style::default()
    }
}

fn wrapping_style() -> Style {
    Style {
        max_size: Size {
            width: Dimension::length(560.0),
            height: Dimension::auto(),
        },
        ..line_style()
    }
}

pub fn run() {
    RutterRunner::<RichTextDemo>::run();
}
