// ============================================================
// Rutter Framework — demos/text_input_demo.rs
// Demo isolada do Widget::TextInput.
// Exercita: digitação, placeholder, label, senha, erro, sucesso,
// Ctrl+A, Ctrl+C, Ctrl+V, Shift+Setas, Backspace c/ seleção.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, InputState, RutterRunner, Theme, Widget};

// ── Estado ───────────────────────────────────────────────────

#[derive(Default)]
pub struct TextInputDemoState {
    pub text_normal: String,
    pub text_password: String,
    pub text_error: String,
    pub submitted: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    NormalChanged(String),
    PasswordChanged(String),
    ErrorChanged(String),
    Submit,
}

pub struct TextInputDemo;

impl AppLogic for TextInputDemo {
    type State = TextInputDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TextInputDemoState::default()
    }

    fn view<'a>(s: &'a mut TextInputDemoState) -> Widget<'a, Msg> {
        let col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(32.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(20.0),
            },
            ..Default::default()
        };
        let inp = Style {
            size: Size {
                width: Dimension::length(320.0),
                height: Dimension::length(44.0),
            },
            ..Default::default()
        };

        Widget::Column {
            style: col,
            children: vec![
                // Campo normal
                Widget::TextInput {
                    id: 1,
                    on_change: Msg::NormalChanged,
                    on_submit: Some(Msg::Submit),
                    style: inp.clone(),
                    label: "Nome",
                    placeholder: "Digite seu nome",
                    state: InputState::Idle,
                    error_msg: None,
                    is_password: false,
                },
                // Campo senha
                Widget::TextInput {
                    id: 2,
                    on_change: Msg::PasswordChanged,
                    on_submit: Some(Msg::Submit),
                    style: inp.clone(),
                    label: "Senha",
                    placeholder: "••••••••",
                    state: InputState::Idle,
                    error_msg: None,
                    is_password: true,
                },
                // Campo com estado de erro
                Widget::TextInput {
                    id: 3,
                    on_change: Msg::ErrorChanged,
                    on_submit: None,
                    style: inp.clone(),
                    label: "E-mail",
                    placeholder: "exemplo@email.com",
                    state: if s.text_error.is_empty() {
                        InputState::Error
                    } else {
                        InputState::Success
                    },
                    error_msg: if s.text_error.is_empty() {
                        Some("Campo obrigatório".into())
                    } else {
                        None
                    },
                    is_password: false,
                },
                // Preview do que foi digitado
                Widget::Text {
                    content: format!(
                        "Normal: \"{}\"\nSenha: {} chars",
                        s.text_normal,
                        s.text_password.len()
                    ),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut TextInputDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::NormalChanged(v) => s.text_normal = v,
            Msg::PasswordChanged(v) => s.text_password = v,
            Msg::ErrorChanged(v) => s.text_error = v,
            Msg::Submit => s.submitted = true,
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<TextInputDemo>::run();
}
