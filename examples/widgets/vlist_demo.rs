// ============================================================
// Rutter Framework — demos/vlist_demo.rs
// Demo isolada de Widget::VirtualList.
// Renderiza 1000+ itens sem instanciar todos — lazy via viewport.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, VirtualSelection, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

const TOTAL_ITEMS: usize = 1_000;

#[derive(Default)]
pub struct VListDemoState {
    pub theme: ExampleTheme,
    pub selected: Vec<usize>,
    pub filter: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Theme(ExampleTheme),
    Selection(Vec<usize>),
    Filter(String),
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

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::Theme),
                Widget::Text {
                    content: format!("{} itens — renderização lazy (VirtualList)", TOTAL_ITEMS),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::TextInput {
                    id: 1,
                    on_change: Msg::Filter,
                    on_submit: None,
                    style: inp_s,
                    label: "",
                    placeholder: "Buscar item...",
                    state: rutter::InputState::Idle,
                    error_msg: None,
                    is_password: false,
                },
                Widget::Text {
                    content: list_selection_caption(&s.selected),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::virtual_list_with_selection(
                    32.0,
                    TOTAL_ITEMS,
                    &|i| {
                        Some(format!(
                            "Item #{:04} — evento de log do framework #{}",
                            i + 1,
                            i * 7 + 1
                        ))
                    },
                    VirtualSelection::multiple(&s.selected, Msg::Selection),
                    list_s,
                )
                .with_id(60),
                Widget::Text {
                    content: "Arraste para selecionar um intervalo. Shift estende; Ctrl/Command alterna ou adiciona."
                        .into(),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut VListDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Theme(theme) => s.theme = theme,
            Msg::Selection(selected) => s.selected = selected,
            Msg::Filter(v) => s.filter = v,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn list_selection_caption(selected: &[usize]) -> String {
    match selected {
        [] => "Nenhum selecionado".into(),
        [index] => format!("Selecionado: item #{:04}", index + 1),
        indices => format!("{} itens selecionados", indices.len()),
    }
}

pub fn run() {
    RutterRunner::<VListDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_selection_caption_describes_empty_single_and_multiple_sets() {
        assert_eq!(list_selection_caption(&[]), "Nenhum selecionado");
        assert_eq!(list_selection_caption(&[4]), "Selecionado: item #0005");
        assert_eq!(list_selection_caption(&[1, 3]), "2 itens selecionados");
    }
}
