// ============================================================
// Rutter Framework — demos/search_bar_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

const MOVIES: &[&str] = &[
    "O Poderoso Chefão",
    "Matrix",
    "Interestelar",
    "O Senhor dos Anéis",
    "Vingadores",
    "Clube da Luta",
    "A Origem",
    "Coringa",
    "Pulp Fiction",
    "Forrest Gump",
    "Star Wars",
    "De Volta para o Futuro",
];

#[derive(Default)]
pub struct SearchBarDemoState {
    pub theme: ExampleTheme,
    pub query: String,
    pub last_search: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    QueryChanged(String),
    SubmitSearch,
    ClearSearch,
    Suggest(&'static str),
}

pub struct SearchBarDemo;

impl AppLogic for SearchBarDemo {
    type State = SearchBarDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        SearchBarDemoState {
            status: "Digite algo e pressione Enter ou o botão de busca.".into(),
            ..Default::default()
        }
    }

    fn view<'a>(s: &'a mut SearchBarDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(32.0_f32),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(16.0),
            },
            ..Default::default()
        };

        let row = Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: Size {
                width: LengthPercentage::length(10.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };

        let btn = |w: f32| Style {
            size: Size {
                width: Dimension::length(w),
                height: Dimension::length(38.0),
            },
            ..Default::default()
        };

        let mut children = vec![
            example_theme_selector(s.theme, Msg::ThemeChanged),
            Widget::Text {
                content: "SearchBar Demo".into(),
                color: None,
                size: 20.0,
                style: Style::default(),
            },
            Widget::SearchBar {
                id: 301,
                on_change: Msg::QueryChanged,
                on_submit: Some(Msg::SubmitSearch),
                on_search: Some(Msg::SubmitSearch),
                on_clear: Some(Msg::ClearSearch),
                style: Style {
                    size: Size {
                        width: Dimension::length(560.0),
                        height: Dimension::length(44.0),
                    },
                    ..Default::default()
                },
                placeholder: "Buscar por filmes...",
            },
            Widget::Row {
                style: row.clone(),
                children: vec![
                    Widget::Button {
                        text: "Buscar",
                        on_press: Msg::SubmitSearch,
                        style: btn(110.0),
                        color: None,
                        variant: ButtonVariant::Primary,
                    },
                    Widget::Button {
                        text: "Limpar",
                        on_press: Msg::ClearSearch,
                        style: btn(110.0),
                        color: None,
                        variant: ButtonVariant::Ghost,
                    },
                ],
            },
            Widget::Text {
                content: "Sugestões rápidas:".into(),
                color: None,
                size: 13.0,
                style: Style::default(),
            },
            Widget::Row {
                style: row,
                children: vec![
                    Widget::Button {
                        text: "Matrix",
                        on_press: Msg::Suggest("Matrix"),
                        style: btn(100.0),
                        color: None,
                        variant: ButtonVariant::Ghost,
                    },
                    Widget::Button {
                        text: "Star Wars",
                        on_press: Msg::Suggest("Star Wars"),
                        style: btn(110.0),
                        color: None,
                        variant: ButtonVariant::Ghost,
                    },
                    Widget::Button {
                        text: "Coringa",
                        on_press: Msg::Suggest("Coringa"),
                        style: btn(110.0),
                        color: None,
                        variant: ButtonVariant::Ghost,
                    },
                ],
            },
            Widget::Container {
                style: Style {
                    padding: Rect::length(16.0_f32),
                    size: Size {
                        width: Dimension::length(560.0),
                        height: Dimension::auto(),
                    },
                    ..Default::default()
                },
                color: Some(skia_safe::Color::from_argb(32, 255, 255, 255)),
                radius: 10.0,
                child: Box::new(Widget::Text {
                    content: format!(
                        "Consulta atual: \"{}\"\nÚltima busca: \"{}\"\nStatus: {}",
                        s.query, s.last_search, s.status
                    ),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                }),
            },
        ];

        if !s.query.is_empty() {
            let filtered: Vec<_> = MOVIES
                .iter()
                .filter(|m| m.to_lowercase().contains(&s.query.to_lowercase()))
                .collect();
            children.push(Widget::Text {
                content: "Resultados:".into(),
                color: None,
                size: 14.0,
                style: Style::default(),
            });
            for m in filtered {
                children.push(Widget::Text {
                    content: m.to_string(),
                    color: None,
                    size: 14.0,
                    style: Style {
                        padding: Rect::length(8.0_f32),
                        ..Default::default()
                    },
                });
            }
        }

        Widget::Column {
            style: root,
            children,
        }
    }

    fn update(s: &mut SearchBarDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::QueryChanged(v) => {
                s.query = v;
                s.status = "Editando consulta...".into();
            }
            Msg::SubmitSearch => {
                s.last_search = s.query.clone();
                s.status = if s.query.trim().is_empty() {
                    "Nada para buscar.".into()
                } else {
                    format!("Busca disparada para: {}", s.query)
                };
            }
            Msg::ClearSearch => {
                s.query.clear();
                s.last_search.clear();
                s.status = "Busca limpa.".into();
            }
            Msg::Suggest(term) => {
                s.query = term.into();
                s.last_search = term.into();
                s.status = format!("Sugestão aplicada: {}", term);
            }
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<SearchBarDemo>::run();
}
