// ============================================================
// Rutter Framework — demos/controls_demo.rs
// Demo isolada de Checkbox, Switch, Radio e Select.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

const LANGUAGES: &[&str] = &["Rust", "Python", "TypeScript", "Go", "Zig", "C++"];

#[derive(Default)]
pub struct ControlsDemoState {
    pub remember: bool,
    pub dark_mode: bool,
    pub newsletter: bool,
    pub selected_lang: usize,
    pub radio_sel: u8,
}

#[derive(Debug, Clone)]
pub enum Msg {
    RememberToggled(bool),
    DarkModeToggled(bool),
    NewsletterToggled(bool),
    LanguageChanged(usize),
    RadioA,
    RadioB,
    RadioC,
}

pub struct ControlsDemo;

impl AppLogic for ControlsDemo {
    type State = ControlsDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ControlsDemoState::default()
    }

    fn view<'a>(s: &'a mut ControlsDemoState) -> Widget<'a, Msg> {
        let col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(40.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(18.0),
            },
            ..Default::default()
        };
        let item_row = Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::length(320.0),
                height: Dimension::length(32.0),
            },
            gap: Size {
                width: LengthPercentage::length(12.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };
        let check_s = Style {
            flex_grow: 1.0,
            size: Size {
                width: Dimension::length(0.0),
                height: Dimension::length(28.0),
            },
            ..Default::default()
        };
        let switch_s = Style {
            size: Size {
                width: Dimension::length(50.0),
                height: Dimension::length(28.0),
            },
            ..Default::default()
        };
        let select_s = Style {
            size: Size {
                width: Dimension::length(320.0),
                height: Dimension::length(44.0),
            },
            ..Default::default()
        };
        let radio_s = Style {
            size: Size {
                width: Dimension::length(160.0),
                height: Dimension::length(28.0),
            },
            ..Default::default()
        };

        let col_rows: Style = Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Start),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            // padding: Rect::length(10.0),
            // gap: Size {
            //     width: LengthPercentage::length(0.0),
            //     height: LengthPercentage::length(18.0),
            // },
            ..Default::default()
        };

        Widget::Column {
            style: col.clone(),
            children: vec![
                // ── Checkboxes ───────────────────────────────────────
                Widget::Text {
                    content: "Checkboxes".into(),
                    color: None,
                    size: 18.0,
                    style: col_rows.clone(),
                },
                Widget::Row {
                    style: col_rows.clone(),
                    children: vec![
                        Widget::Checkbox {
                            checked: s.remember,
                            on_change: Msg::RememberToggled,
                            label: "Lembrar de mim",
                            style: check_s.clone(),
                        },
                        Widget::Checkbox {
                            checked: s.newsletter,
                            on_change: Msg::NewsletterToggled,
                            label: "Receber newsletter",
                            style: check_s.clone(),
                        },
                    ],
                },
                // Widget::Checkbox {
                //     checked: s.remember,
                //     on_change: Msg::RememberToggled,
                //     label: "Lembrar de mim",
                //     style: check_s.clone(),
                // },
                // Widget::Checkbox {
                //     checked: s.newsletter,
                //     on_change: Msg::NewsletterToggled,
                //     label: "Receber newsletter",
                //     style: check_s.clone(),
                // },
                // ── Switch ───────────────────────────────────────────
                Widget::Text {
                    content: "Switch".into(),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::Row {
                    style: item_row,
                    children: vec![
                        Widget::Text {
                            content: "Modo escuro".into(),
                            color: None,
                            size: 14.0,
                            style: Style {
                                flex_grow: 1.0,
                                ..Default::default()
                            },
                        },
                        Widget::Switch {
                            checked: s.dark_mode,
                            on_change: Msg::DarkModeToggled,
                            style: switch_s,
                        },
                    ],
                },
                // ── Radio ────────────────────────────────────────────
                Widget::Text {
                    content: "Radio".into(),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::Radio {
                    selected: s.radio_sel == 0,
                    on_select: || Msg::RadioA,
                    label: "Opção A",
                    style: radio_s.clone(),
                },
                Widget::Radio {
                    selected: s.radio_sel == 1,
                    on_select: || Msg::RadioB,
                    label: "Opção B",
                    style: radio_s.clone(),
                },
                Widget::Radio {
                    selected: s.radio_sel == 2,
                    on_select: || Msg::RadioC,
                    label: "Opção C",
                    style: radio_s.clone(),
                },
                // ── Select ───────────────────────────────────────────
                Widget::Text {
                    content: "Select (dropdown)".into(),
                    color: None,
                    size: 14.0,
                    style: Style::default(),
                },
                Widget::Select {
                    id: 10,
                    options: LANGUAGES,
                    selected_index: s.selected_lang,
                    on_change: Msg::LanguageChanged,
                    style: select_s,
                    label: "Linguagem favorita",
                    placeholder: "Escolha...",
                },
                // ── Resumo ───────────────────────────────────────────
                Widget::Text {
                    content: format!(
                        "Lembrar={} | Newsletter={} | DarkMode={} | Radio={} | Lang={}",
                        s.remember,
                        s.newsletter,
                        s.dark_mode,
                        s.radio_sel,
                        LANGUAGES.get(s.selected_lang).copied().unwrap_or("-")
                    ),
                    color: None,
                    size: 11.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut ControlsDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::RememberToggled(v) => s.remember = v,
            Msg::DarkModeToggled(v) => s.dark_mode = v,
            Msg::NewsletterToggled(v) => s.newsletter = v,
            Msg::LanguageChanged(i) => s.selected_lang = i,
            Msg::RadioA => s.radio_sel = 0,
            Msg::RadioB => s.radio_sel = 1,
            Msg::RadioC => s.radio_sel = 2,
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<ControlsDemo>::run();
}
