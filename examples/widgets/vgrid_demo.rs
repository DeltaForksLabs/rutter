// ============================================================
// Rutter Framework — demos/vgrid_demo.rs
// Demo isolada de Widget::VirtualGrid.
// Exercita scroll virtualizado, clique e navegação por teclado.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, VirtualSelection, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

const TOTAL_ITEMS: usize = 1_200;

#[derive(Default)]
pub struct VGridDemoState {
    pub theme: ExampleTheme,
    pub selected: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    SelectionChanged(Vec<usize>),
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
                    content: grid_selection_caption(&s.selected),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::virtual_grid_with_selection(
                    4,
                    72.0,
                    TOTAL_ITEMS,
                    &|i| Some(format!("Card #{:04}", i + 1)),
                    VirtualSelection::multiple(&s.selected, Msg::SelectionChanged),
                    grid_s,
                )
                .with_id(70),
                Widget::Text {
                    content: "Arraste para selecionar um retângulo. Shift+setas estende; Ctrl/Command+A seleciona tudo."
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
            Msg::SelectionChanged(selected) => s.selected = selected,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn grid_selection_caption(selected: &[usize]) -> String {
    match selected {
        [] => "Nenhuma célula selecionada. Use clique ou setas.".into(),
        [index] => format!("Selecionado: card #{:04}", index + 1),
        indices => format!("{} cards selecionados", indices.len()),
    }
}

pub fn run() {
    RutterRunner::<VGridDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_selection_caption_describes_empty_single_and_multiple_sets() {
        assert_eq!(
            grid_selection_caption(&[]),
            "Nenhuma célula selecionada. Use clique ou setas."
        );
        assert_eq!(grid_selection_caption(&[4]), "Selecionado: card #0005");
        assert_eq!(grid_selection_caption(&[1, 3]), "2 cards selecionados");
    }
}
