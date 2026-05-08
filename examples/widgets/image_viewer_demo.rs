// ============================================================
// Rutter Framework — demos/image_viewer_demo.rs
// Demo isolada de Widget::Image com imagens PNG embutidas.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

const RED_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x18, 0x08, 0x02, 0x00, 0x00, 0x00, 0x6f, 0x15, 0xaa,
    0xaf, 0x00, 0x00, 0x00, 0x1f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xb8, 0xe3, 0xe0, 0x41,
    0x15, 0xc4, 0x30, 0x6a, 0xd0, 0xa8, 0x41, 0xa3, 0x06, 0x8d, 0x1a, 0x34, 0x6a, 0xd0, 0xa8, 0x41,
    0x03, 0x6f, 0x10, 0x00, 0xe0, 0x82, 0x21, 0x2e, 0x6a, 0xcd, 0x6f, 0xbc, 0x00, 0x00, 0x00, 0x00,
    0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];
const GREEN_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x18, 0x08, 0x02, 0x00, 0x00, 0x00, 0x6f, 0x15, 0xaa,
    0xaf, 0x00, 0x00, 0x00, 0x1f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xd0, 0x9a, 0xdb, 0x4f,
    0x15, 0xc4, 0x30, 0x6a, 0xd0, 0xa8, 0x41, 0xa3, 0x06, 0x8d, 0x1a, 0x34, 0x6a, 0xd0, 0xa8, 0x41,
    0x03, 0x6f, 0x10, 0x00, 0xdf, 0xd0, 0x01, 0xae, 0xaa, 0xf7, 0x99, 0xa2, 0x00, 0x00, 0x00, 0x00,
    0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];
const BLUE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x18, 0x08, 0x02, 0x00, 0x00, 0x00, 0x6f, 0x15, 0xaa,
    0xaf, 0x00, 0x00, 0x00, 0x1f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x70, 0xad, 0x9e, 0x4b,
    0x15, 0xc4, 0x30, 0x6a, 0xd0, 0xa8, 0x41, 0xa3, 0x06, 0x8d, 0x1a, 0x34, 0x6a, 0xd0, 0xa8, 0x41,
    0x03, 0x6f, 0x10, 0x00, 0xe5, 0x3a, 0x11, 0x6e, 0x6e, 0x55, 0x8f, 0x38, 0x00, 0x00, 0x00, 0x00,
    0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];
const SAMPLES: &[ImageSample] = &[
    ImageSample {
        name: "Red",
        data: RED_PNG,
    },
    ImageSample {
        name: "Green",
        data: GREEN_PNG,
    },
    ImageSample {
        name: "Blue",
        data: BLUE_PNG,
    },
];

pub struct ImageSample {
    pub name: &'static str,
    pub data: &'static [u8],
}

#[derive(Default)]
pub struct ImageViewerDemoState {
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Previous,
    Next,
    Select(usize),
}

pub struct ImageViewerDemo;

impl AppLogic for ImageViewerDemo {
    type State = ImageViewerDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ImageViewerDemoState::default()
    }

    fn view<'a>(s: &'a mut ImageViewerDemoState) -> Widget<'a, Msg> {
        let selected = sample(s.selected);
        Widget::Column {
            style: page_style(),
            children: vec![
                title(selected.name),
                large_image(selected.data),
                controls(),
                thumbnails(),
            ],
        }
    }

    fn update(s: &mut ImageViewerDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Previous => s.selected = previous_index(s.selected),
            Msg::Next => s.selected = next_index(s.selected),
            Msg::Select(index) => s.selected = index.min(SAMPLES.len() - 1),
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn sample(index: usize) -> &'static ImageSample {
    &SAMPLES[index.min(SAMPLES.len() - 1)]
}

fn previous_index(index: usize) -> usize {
    if index == 0 {
        SAMPLES.len() - 1
    } else {
        index - 1
    }
}

fn next_index(index: usize) -> usize {
    (index + 1) % SAMPLES.len()
}

fn title<'a>(name: &'a str) -> Widget<'a, Msg> {
    Widget::Text {
        content: format!("Image viewer: {name}"),
        style: Style::default(),
        color: None,
        size: 18.0,
    }
}

fn large_image<'a>(data: &'a [u8]) -> Widget<'a, Msg> {
    Widget::Image {
        data,
        style: fixed_size(240.0, 240.0),
        radius: 12.0,
    }
}

fn controls<'a>() -> Widget<'a, Msg> {
    Widget::Row {
        style: row_style(),
        children: vec![button("Previous", Msg::Previous), button("Next", Msg::Next)],
    }
}

fn thumbnails<'a>() -> Widget<'a, Msg> {
    Widget::Row {
        style: row_style(),
        children: SAMPLES
            .iter()
            .enumerate()
            .map(|(index, sample)| thumbnail(sample, index))
            .collect(),
    }
}

fn thumbnail<'a>(sample: &'a ImageSample, index: usize) -> Widget<'a, Msg> {
    Widget::button_content(
        sample.name,
        Widget::Image {
            data: sample.data,
            style: fixed_size(52.0, 52.0),
            radius: 8.0,
        },
        Msg::Select(index),
        thumbnail_style(),
        None,
        ButtonVariant::Ghost,
    )
}

fn button<'a>(text: &'a str, msg: Msg) -> Widget<'a, Msg> {
    Widget::Button {
        text,
        on_press: msg,
        style: button_style(),
        color: None,
        variant: ButtonVariant::Primary,
    }
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FlexStart),
        size: Size::from_percent(1.0, 1.0),
        padding: Rect::length(40.0_f32),
        gap: gap_size(18.0, 18.0),
        ..Default::default()
    }
}

fn row_style() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::Center),
        gap: gap_size(12.0, 0.0),
        ..Default::default()
    }
}

fn button_style() -> Style {
    Style {
        size: Size::from_lengths(120.0, 36.0),
        ..Default::default()
    }
}

fn thumbnail_style() -> Style {
    Style {
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        size: Size::from_lengths(68.0, 68.0),
        ..Default::default()
    }
}

fn fixed_size(width: f32, height: f32) -> Style {
    Style {
        size: Size::from_lengths(width, height),
        ..Default::default()
    }
}

fn gap_size(width: f32, height: f32) -> Size<LengthPercentage> {
    Size {
        width: LengthPercentage::length(width),
        height: LengthPercentage::length(height),
    }
}

pub fn run() {
    RutterRunner::<ImageViewerDemo>::run();
}
