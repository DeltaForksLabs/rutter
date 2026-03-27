// ============================================================
// Rutter Framework — demos/scroll_demo.rs
// Demo isolada de Widget::ScrollView.
// FIX-4: scroll agora funciona com:
//   • Roda do mouse (já funcionava)
//   • Teclas ↑↓ / PageUp / PageDown (quando o ScrollView tem foco)
//   • Arrastar o polegar da scrollbar com o mouse
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

const ITEM_COUNT: usize = 60;

#[derive(Default)]
pub struct ScrollDemoState {
    pub selected: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ItemSelected(usize),
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
            align_items: Some(AlignItems::Center),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(24.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(16.0),
            },
            ..Default::default()
        };
        // ScrollView de altura fixa com conteúdo longo
        let scroll_s = Style {
            size: Size {
                width: Dimension::length(400.0),
                height: Dimension::length(300.0),
            },
            ..Default::default()
        };
        // Coluna interna — mais alta que o viewport para forçar scroll
        let inner_col = Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(ITEM_COUNT as f32 * 44.0),
            },
            ..Default::default()
        };

        // Gera itens como botões clicáveis dentro do scroll
        let items: Vec<Widget<Msg>> = (0..ITEM_COUNT)
            .map(|i| Widget::Button {
                text: if Some(i) == s.selected {
                    "✓ Item selecionado"
                } else {
                    "Item da lista"
                },
                on_press: Msg::ItemSelected(i),
                style: Style {
                    size: Size {
                        width: Dimension::percent(1.0),
                        height: Dimension::length(40.0),
                    },
                    ..Default::default()
                },
                color: None,
                variant: if Some(i) == s.selected {
                    rutter::ButtonVariant::Primary
                } else {
                    rutter::ButtonVariant::Ghost
                },
            })
            .collect();

        Widget::Column {
            style: root,
            children: vec![
                Widget::Text {
                    content: "ScrollView — roda do mouse, ↑↓, arrastar scrollbar (FIX-4)".into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: s
                        .selected
                        .map(|i| format!("Selecionado: item #{}", i))
                        .unwrap_or_else(|| "Nenhum selecionado".into()),
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
                    content: "Clique na área de scroll para ativar o foco, depois use ↑↓".into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut ScrollDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ItemSelected(i) => s.selected = Some(i),
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<ScrollDemo>::run();
}
