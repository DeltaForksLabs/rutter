// ============================================================
// Rutter Framework — demos/vlist_demo.rs
// Demo isolada de Widget::VirtualList.
// Renderiza 1000+ itens sem instanciar todos — lazy via viewport.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

const TOTAL_ITEMS: usize = 1_000;

#[derive(Default)]
pub struct VListDemoState {
    pub theme: ExampleTheme,
    pub selected: Option<usize>,
    pub filter: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    ItemSelected(usize),
    FilterChanged(String),
}

pub struct VListDemo;

impl AppLogic for VListDemo {
    type State = VListDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        VListDemoState::default()
    }

    fn view<'a>(s: &'a mut VListDemoState) -> Widget<'a, Msg> {
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
        let list_s = Style {
            size: Size {
                width: Dimension::length(420.0),
                height: Dimension::length(400.0),
            },
            ..Default::default()
        };
        let inp_s = Style {
            size: Size {
                width: Dimension::length(420.0),
                height: Dimension::length(40.0),
            },
            ..Default::default()
        };

        // Captura o filtro para uso no closure (lifetime seguro)
        // let filter_ref = &s.filter;

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: format!("{} itens — renderização lazy (VirtualList)", TOTAL_ITEMS),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::TextInput {
                    id: 1,
                    on_change: Msg::FilterChanged,
                    on_submit: None,
                    style: inp_s,
                    label: "",
                    placeholder: "Buscar item...",
                    state: rutter::InputState::Idle,
                    error_msg: None,
                    is_password: false,
                },
                Widget::Text {
                    content: s
                        .selected
                        .map(|i| format!("Selecionado: item #{:04}", i + 1))
                        .unwrap_or_else(|| "Nenhum selecionado".into()),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::VirtualList {
                    id: 60,
                    item_height: 32.0,
                    item_count: TOTAL_ITEMS,
                    items: &|i| {
                        Some(format!(
                            "Item #{:04} — evento de log do framework #{}",
                            i + 1,
                            i * 7 + 1
                        ))
                    },
                    on_select: Msg::ItemSelected,
                    style: list_s,
                },
                Widget::Text {
                    content: "Roda do mouse para rolar. Clique para selecionar.".into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut VListDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::ItemSelected(i) => s.selected = Some(i),
            Msg::FilterChanged(v) => s.filter = v,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<VListDemo>::run();
}
