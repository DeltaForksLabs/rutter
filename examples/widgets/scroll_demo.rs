// ============================================================
// Rutter Framework — demos/scroll_demo.rs
// Demo isolada de Widget::ScrollView.
// Scroll por roda, navegação no ancestral do controle focado e trilha/polegar.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const ITEM_COUNT: usize = 60;

#[derive(Default)]
pub struct ScrollDemoState {
    pub theme: ExampleTheme,
    pub selected: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    ItemToggled(usize),
}

pub struct ScrollDemo;

impl AppLogic for ScrollDemo {
    type State = ScrollDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ScrollDemoState::default()
    }

    fn view<'a>(s: &'a mut ScrollDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(spacing::XL),
            gap: Size::length(spacing::LG),
            ..Default::default()
        };
        // ScrollView de altura fixa com conteúdo longo
        let scrollbar_gutter = rutter::layout::SCROLLBAR_W + 2.0 + spacing::SM;
        let scroll_s = Style {
            // The outer page also scrolls on short windows. Keep the inner
            // track clear of its physical-right track, including narrow widths.
            size: Size {
                width: Dimension::auto(),
                height: Dimension::length(300.0),
            },
            margin: Rect {
                right: LengthPercentageAuto::length(scrollbar_gutter),
                ..Rect::zero()
            },
            ..responsive_width(400.0, Dimension::length(300.0))
        };
        // Intrinsic document height includes the real item gaps, not a guessed extent.
        let inner_col = Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            flex_shrink: 0.0,
            gap: Size::length(spacing::XS),
            // The track occupies 8px, inset 2px from the physical right edge
            // even in RTL. Reserve its lane plus an SM gap in the document.
            padding: Rect {
                right: LengthPercentage::length(scrollbar_gutter),
                ..Rect::zero()
            },
            ..Default::default()
        };

        // Generate independently toggleable items inside the scroll view.
        let items: Vec<Widget<Msg>> = (0..ITEM_COUNT)
            .map(|index| {
                let selected = s.selected.binary_search(&index).is_ok();
                Widget::Button {
                    text: if selected {
                        "✓ Item selecionado"
                    } else {
                        "Item da lista"
                    },
                    on_press: Msg::ItemToggled(index),
                    style: Style {
                        flex_shrink: 0.0,
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::length(40.0),
                        },
                        ..Default::default()
                    },
                    color: None,
                    variant: if selected {
                        rutter::ButtonVariant::Primary
                    } else {
                        rutter::ButtonVariant::Ghost
                    },
                }
            })
            .collect();

        super::layout::page("ScrollView", Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: "ScrollView — roda do mouse, foco pelo teclado e seleção múltipla"
                        .into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: scroll_selection_caption(&s.selected),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::ScrollView {
                    id: 70,
                    style: scroll_s,
                    child: Box::new(Widget::Column {
                        style: inner_col,
                        children: items,
                    }),
                },
                Widget::Text {
                    content: "Clique em itens para alternar a seleção; use ↑↓, PageUp/PageDown e Home/End para navegar."
                        .into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        })
    }

    fn update(s: &mut ScrollDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::ItemToggled(index) => toggle_scroll_selection(&mut s.selected, index),
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn toggle_scroll_selection(selected: &mut Vec<usize>, index: usize) {
    match selected.binary_search(&index) {
        Ok(position) => {
            selected.remove(position);
        }
        Err(position) => selected.insert(position, index),
    }
}

fn scroll_selection_caption(selected: &[usize]) -> String {
    match selected {
        [] => "Nenhum item selecionado".into(),
        [index] => format!("Selecionado: item #{}", index + 1),
        indices => format!("{} itens selecionados", indices.len()),
    }
}

pub fn run() {
    RutterRunner::<ScrollDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::{scroll_selection_caption, toggle_scroll_selection};

    #[test]
    fn scroll_selection_toggles_and_stays_sorted() {
        let mut selected = vec![1, 4];

        toggle_scroll_selection(&mut selected, 3);
        toggle_scroll_selection(&mut selected, 4);

        assert_eq!(selected, vec![1, 3]);
        assert_eq!(scroll_selection_caption(&selected), "2 itens selecionados");
    }
}

#[cfg(test)]
#[path = "scroll_demo_geometry_tests.rs"]
mod geometry_tests;
