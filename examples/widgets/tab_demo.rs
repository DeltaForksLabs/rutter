// ============================================================
// Rutter Framework — demos/tab_demo.rs
// Demo isolada de Widget::TabBar.
// FIX-2: o underline agora acompanha corretamente o nome da aba
//        (calculado em render-time com a largura real da TabBar).
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

const TABS_3: &[&str] = &["Perfil", "Configurações", "Segurança"];
const TABS_5: &[&str] = &["Home", "Explore", "Library", "History", "Settings"];

#[derive(Default)]
pub struct TabDemoState {
    pub active3: usize,
    pub active5: usize,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Tab3Changed(usize),
    Tab5Changed(usize),
}

pub struct TabDemo;

impl AppLogic for TabDemo {
    type State = TabDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TabDemoState::default()
    }

    fn view<'a>(s: &'a mut TabDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        };
        let tab_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(44.0),
            },
            ..Default::default()
        };
        let content_col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            padding: Rect::length(32.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(32.0),
            },
            flex_grow: 1.0,
            ..Default::default()
        };

        Widget::Column {
            style: root,
            children: vec![
                // ── TabBar de 3 abas ─────────────────────────────────
                Widget::TabBar {
                    id: 50,
                    tabs: TABS_3,
                    active: s.active3,
                    on_change: Msg::Tab3Changed,
                    style: tab_s.clone(),
                },
                Widget::Column {
                    style: content_col.clone(),
                    children: vec![
                        Widget::Text {
                            content: format!("Aba ativa (3): \"{}\"", TABS_3[s.active3]),
                            color: None,
                            size: 16.0,
                            style: Style::default(),
                        },
                        Widget::Text {
                            content: "O underline deve estar centralizado sob o nome (FIX-2)"
                                .into(),
                            color: None,
                            size: 12.0,
                            style: Style::default(),
                        },
                    ],
                },
                // ── TabBar de 5 abas ─────────────────────────────────
                Widget::TabBar {
                    id: 51,
                    tabs: TABS_5,
                    active: s.active5,
                    on_change: Msg::Tab5Changed,
                    style: tab_s,
                },
                Widget::Column {
                    style: content_col,
                    children: vec![Widget::Text {
                        content: format!("Aba ativa (5): \"{}\"", TABS_5[s.active5]),
                        color: None,
                        size: 16.0,
                        style: Style::default(),
                    }],
                },
            ],
        }
    }

    fn update(s: &mut TabDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Tab3Changed(i) => s.active3 = i,
            Msg::Tab5Changed(i) => s.active5 = i,
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<TabDemo>::run();
}
