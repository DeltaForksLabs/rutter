// ============================================================
// Rutter Framework — demos/vgrid_demo.rs
// Demo isolada de Widget::VirtualGrid.
// Exercita scroll virtualizado, clique e navegação por teclado.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

const TOTAL_ITEMS: usize = 1_200;

#[derive(Default)]
pub struct VGridDemoState {
    pub theme: ExampleTheme,
    pub selected: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    CellSelected(usize),
}

pub struct VGridDemo;

impl AppLogic for VGridDemo {
    type State = VGridDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        VGridDemoState::default()
    }

    fn view<'a>(s: &'a mut VGridDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(24.0_f32),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(14.0),
            },
            ..Default::default()
        };
        let grid_s = Style {
            size: Size {
                width: Dimension::length(720.0),
                height: Dimension::length(420.0),
            },
            ..Default::default()
        };

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: format!(
                        "{} células — virtualização lazy em grade nativa",
                        TOTAL_ITEMS
                    ),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: s
                        .selected
                        .map(|i| format!("Selecionado: card #{:04}", i + 1))
                        .unwrap_or_else(|| {
                            "Nenhuma célula selecionada. Use clique ou setas.".into()
                        }),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::virtual_grid(
                    4,
                    72.0,
                    TOTAL_ITEMS,
                    &|i| Some(format!("Card #{:04}", i + 1)),
                    Msg::CellSelected,
                    grid_s,
                )
                .with_id(70),
                Widget::Text {
                    content:
                        "Setas movem a seleção, PageUp/PageDown rolam, Enter confirma a célula."
                            .into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut VGridDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::CellSelected(i) => s.selected = Some(i),
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<VGridDemo>::run();
}
