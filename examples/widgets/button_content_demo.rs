// ============================================================
// Rutter Framework — demos/button_content_demo.rs
// Demo isolada de ButtonContent com texto, imagem e composicao.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use skia_safe::Color;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

const ICON_DATA: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x18, 0x08, 0x02, 0x00, 0x00, 0x00, 0x6f, 0x15, 0xaa,
    0xaf, 0x00, 0x00, 0x00, 0x1f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xb8, 0xe3, 0xe0, 0x41,
    0x15, 0xc4, 0x30, 0x6a, 0xd0, 0xa8, 0x41, 0xa3, 0x06, 0x8d, 0x1a, 0x34, 0x6a, 0xd0, 0xa8, 0x41,
    0x03, 0x6f, 0x10, 0x00, 0xe0, 0x82, 0x21, 0x2e, 0x6a, 0xcd, 0x6f, 0xbc, 0x00, 0x00, 0x00, 0x00,
    0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Default)]
pub struct ButtonContentDemoState {
    pub clicks: u32,
    pub selected: u8,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Save,
    SelectPrimary,
    SelectGhost,
    Reset,
}

pub struct ButtonContentDemo;

impl AppLogic for ButtonContentDemo {
    type State = ButtonContentDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ButtonContentDemoState::default()
    }

    fn view<'a>(s: &'a mut ButtonContentDemoState) -> Widget<'a, Msg> {
        Widget::Column {
            style: page_style(),
            children: vec![
                heading("ButtonContent"),
                label(format!(
                    "Cliques: {} | Opcao: {}",
                    s.clicks,
                    selected_label(s.selected)
                )),
                action_row(),
                preview_row(),
            ],
        }
    }

    fn update(s: &mut ButtonContentDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Save => s.clicks += 1,
            Msg::SelectPrimary => s.selected = 1,
            Msg::SelectGhost => s.selected = 2,
            Msg::Reset => *s = ButtonContentDemoState::default(),
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

fn action_row<'a>() -> Widget<'a, Msg> {
    Widget::Row {
        style: row_style(),
        children: vec![
            rich_button("Save changes", "Save", Msg::Save, ButtonVariant::Primary),
            rich_button(
                "Select primary",
                "Primary",
                Msg::SelectPrimary,
                ButtonVariant::Ghost,
            ),
            rich_button(
                "Select ghost",
                "Ghost",
                Msg::SelectGhost,
                ButtonVariant::Ghost,
            ),
            text_button("Reset", Msg::Reset),
        ],
    }
}

fn preview_row<'a>() -> Widget<'a, Msg> {
    Widget::Row {
        style: row_style(),
        children: vec![
            swatch_button("Icon only", Msg::Save),
            heading("Imagem, texto e click no mesmo ButtonContent"),
        ],
    }
}

fn rich_button<'a>(
    label: &'a str,
    text: &'a str,
    msg: Msg,
    variant: ButtonVariant,
) -> Widget<'a, Msg> {
    Widget::button_content(label, button_body(text), msg, button_style(), None, variant)
}

fn swatch_button<'a>(label: &'a str, msg: Msg) -> Widget<'a, Msg> {
    Widget::button_content(
        label,
        image_icon(28.0),
        msg,
        icon_button_style(),
        None,
        ButtonVariant::Ghost,
    )
}

fn text_button<'a>(text: &'a str, msg: Msg) -> Widget<'a, Msg> {
    Widget::Button {
        text,
        on_press: msg,
        style: button_style(),
        color: None,
        variant: ButtonVariant::Text,
    }
}

fn button_body<'a>(text: &'a str) -> Widget<'a, Msg> {
    Widget::Row {
        style: button_body_style(),
        children: vec![image_icon(18.0), button_label(text)],
    }
}

fn image_icon<'a>(side: f32) -> Widget<'a, Msg> {
    Widget::Image {
        data: ICON_DATA,
        style: fixed_size(side, side),
        radius: 4.0,
    }
}

fn button_label<'a>(text: &'a str) -> Widget<'a, Msg> {
    Widget::Text {
        content: text.into(),
        style: Style::default(),
        color: Some(Color::WHITE),
        size: 14.0,
    }
}

fn heading<'a>(text: &'a str) -> Widget<'a, Msg> {
    Widget::Text {
        content: text.into(),
        style: Style::default(),
        color: None,
        size: 18.0,
    }
}

fn label<'a>(content: String) -> Widget<'a, Msg> {
    Widget::Text {
        content,
        style: Style::default(),
        color: None,
        size: 14.0,
    }
}

fn selected_label(selected: u8) -> &'static str {
    match selected {
        1 => "primary",
        2 => "ghost",
        _ => "nenhuma",
    }
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FlexStart),
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::percent(1.0),
        },
        padding: Rect::length(40.0_f32),
        gap: gap_size(16.0, 16.0),
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
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        size: Size::from_lengths(150.0, 40.0),
        ..Default::default()
    }
}

fn icon_button_style() -> Style {
    Style {
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        size: Size::from_lengths(48.0, 40.0),
        ..Default::default()
    }
}

fn button_body_style() -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::Center),
        gap: gap_size(8.0, 0.0),
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
    RutterRunner::<ButtonContentDemo>::run();
}
