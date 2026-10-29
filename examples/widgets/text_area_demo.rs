// ============================================================
// Rutter Framework — demos/text_area_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{AppLogic, InputState, RutterRunner, Theme, Widget};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

#[derive(Default)]
pub struct TextAreaDemoState {
    pub theme: ExampleTheme,
    pub notes: String,
    pub description: String,
    pub validation_text: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Theme(ExampleTheme),
    Notes(String),
    Description(String),
    Validation(String),
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
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(spacing::XL),
            gap: Size::length(spacing::LG),
            ..Default::default()
        };

        let area_style = |height: f32| responsive_width(560.0, Dimension::length(height));

        super::layout::page("Text areas", Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::Theme),
                Widget::Text {
                    content: "TextArea Demo".into(),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content:
                        "Preserva quebras de linha normalizadas, converte tabs em espaços e remove controles não imprimíveis."
                            .into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::TextArea {
                    id: 201,
                    on_change: Msg::Notes,
                    on_submit: None,
                    style: area_style(120.0),
                    label: "Observações",
                    placeholder: "Digite observações longas...",
                    state: InputState::Idle,
                    error_msg: None,
                },
                Widget::TextArea {
                    id: 202,
                    on_change: Msg::Description,
                    on_submit: None,
                    style: area_style(160.0),
                    label: "Descrição técnica",
                    placeholder: "Descreva os detalhes do problema, contexto e resultado esperado.",
                    state: InputState::Idle,
                    error_msg: None,
                },
                Widget::TextArea {
                    id: 203,
                    on_change: Msg::Validation,
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
                        padding: Rect::length(spacing::LG),
                        ..responsive_width(560.0, Dimension::auto())
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
        })
    }

    fn update(s: &mut TextAreaDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Theme(theme) => s.theme = theme,
            Msg::Notes(v) => s.notes = v,
            Msg::Description(v) => s.description = v,
            Msg::Validation(v) => s.validation_text = v,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<TextAreaDemo>::run();
}

#[cfg(test)]
mod page_tests {
    use super::TextAreaDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<TextAreaDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<TextAreaDemo>();
    }
}
