// ============================================================
// Rutter Framework — demos/text_area_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, InputState, RutterRunner, Theme, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

#[derive(Default)]
pub struct TextAreaDemoState {
    pub theme: ExampleTheme,
    pub notes: String,
    pub description: String,
    pub validation_text: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    NotesChanged(String),
    DescriptionChanged(String),
    ValidationChanged(String),
}

pub struct TextAreaDemo;

impl AppLogic for TextAreaDemo {
    type State = TextAreaDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TextAreaDemoState::default()
    }

    fn view<'a>(s: &'a mut TextAreaDemoState) -> Widget<'a, Msg> {
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
                height: LengthPercentage::length(18.0),
            },
            ..Default::default()
        };

        let area_style = |height: f32| Style {
            size: Size {
                width: Dimension::length(560.0),
                height: Dimension::length(height),
            },
            ..Default::default()
        };

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: "TextArea Demo".into(),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content:
                        "Exercita edição multilinha, placeholder, estado visual e validação básica."
                            .into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::TextArea {
                    id: 201,
                    on_change: Msg::NotesChanged,
                    on_submit: None,
                    style: area_style(120.0),
                    label: "Observações",
                    placeholder: "Digite observações longas...",
                    state: InputState::Idle,
                    error_msg: None,
                },
                Widget::TextArea {
                    id: 202,
                    on_change: Msg::DescriptionChanged,
                    on_submit: None,
                    style: area_style(160.0),
                    label: "Descrição técnica",
                    placeholder: "Descreva os detalhes do problema, contexto e resultado esperado.",
                    state: InputState::Idle,
                    error_msg: None,
                },
                Widget::TextArea {
                    id: 203,
                    on_change: Msg::ValidationChanged,
                    on_submit: None,
                    style: area_style(120.0),
                    label: "Resumo obrigatório",
                    placeholder: "Escreva ao menos um resumo curto.",
                    state: if s.validation_text.trim().is_empty() {
                        InputState::Error
                    } else {
                        InputState::Success
                    },
                    error_msg: if s.validation_text.trim().is_empty() {
                        Some("Este campo não pode ficar vazio.".into())
                    } else {
                        None
                    },
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
                            "Preview:\n- Observações: {} chars\n- Descrição: {} chars\n- Resumo: {} chars",
                            s.notes.len(),
                            s.description.len(),
                            s.validation_text.len()
                        ),
                        color: None,
                        size: 13.0,
                        style: Style::default(),
                    }),
                },
            ],
        }
    }

    fn update(s: &mut TextAreaDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::NotesChanged(v) => s.notes = v,
            Msg::DescriptionChanged(v) => s.description = v,
            Msg::ValidationChanged(v) => s.validation_text = v,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<TextAreaDemo>::run();
}
