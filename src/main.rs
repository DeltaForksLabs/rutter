// ============================================================
// Rutter Framework — main.rs
// Demo Fase 2: AppState sem editors (gerenciados pelo engine).
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, InputState, RutterRunner, Theme, Widget};

// ── Estado — apenas dados da aplicação ───────────────────────

#[derive(Default)]
pub struct AppState {
    pub username:       String,
    pub password:       String,
    pub status:         String,
    pub username_state: InputState,
    pub password_state: InputState,
}

// ── Mensagens ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Msg {
    UsernameChanged(String),
    PasswordChanged(String),
    LoginPressed,
    ClearPressed,
}

// ── AppLogic ──────────────────────────────────────────────────

pub struct MyApp;

impl AppLogic for MyApp {
    type State   = AppState;
    type Message = Msg;

    fn new(_fs: &mut FontSystem) -> AppState {
        AppState::default()
    }

    fn view<'a>(s: &'a mut AppState) -> Widget<'a, Msg> {
        let input_style = Style {
            size: Size {
                width:  Dimension::length(280.0),
                height: Dimension::length(40.0),
            },
            margin: Rect {
                top: LengthPercentageAuto::length(16.0),
                bottom: LengthPercentageAuto::length(0.0),
                left: LengthPercentageAuto::length(0.0),
                right: LengthPercentageAuto::length(0.0)
            },
            ..Default::default()
        };

        let btn_primary = Style {
            size: Size {
                width:  Dimension::length(280.0),
                height: Dimension::length(36.0),
            },
            margin: Rect {
                top: LengthPercentageAuto::length(24.0),
                bottom: LengthPercentageAuto::length(0.0),
                left: LengthPercentageAuto::length(0.0),
                right: LengthPercentageAuto::length(0.0)
            },
            ..Default::default()
        };

        let btn_ghost = Style {
            size: Size {
                width:  Dimension::length(132.0),
                height: Dimension::length(30.0),
            },
            margin: Rect {
                top:   LengthPercentageAuto::length(8.0),
                right: LengthPercentageAuto::length(8.0),
                bottom: LengthPercentageAuto::length(0.0),
                left: LengthPercentageAuto::length(0.0),
            },
            ..Default::default()
        };

        let col = Style {
            flex_direction:  FlexDirection::Column,
            align_items:     Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            size: Size {
                width:  Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        };

        let status_color = if s.status.starts_with("Welcome") {
            Some(skia_safe::Color::from_rgb(0x4e, 0xc9, 0xb0)) // success green
        } else if s.status.starts_with("Error") {
            Some(skia_safe::Color::from_rgb(0xf4, 0x47, 0x47)) // error red
        } else {
            None
        };

        Widget::Column {
            style: col,
            children: vec![
                // Título
                Widget::Text {
                    content: "Rutter".into(),
                    color:   None,
                    size:    22.0,
                    style:   Style {
                        margin: Rect {
                            bottom: LengthPercentageAuto::length(4.0),
                            top: LengthPercentageAuto::length(0.0),
                            left: LengthPercentageAuto::length(0.0),
                            right: LengthPercentageAuto::length(0.0)
                        },
                        ..Default::default()
                    },
                },
                // Subtítulo
                Widget::Text {
                    content: "Sign in to continue".into(),
                    color:   Some(skia_safe::Color::from_rgb(0x9d, 0x9d, 0x9d)),
                    size:    13.0,
                    style:   Style {
                        margin: Rect {
                            bottom: LengthPercentageAuto::length(28.0),
                            top: LengthPercentageAuto::length(0.0),
                            left: LengthPercentageAuto::length(0.0),
                            right: LengthPercentageAuto::length(0.0)
                        },
                        ..Default::default()
                    },
                },
                // Username
                Widget::TextInput {
                    on_change:   Msg::UsernameChanged,
                    on_submit:   None,
                    style:       input_style.clone(),
                    id:          1,
                    label:       "Username",
                    placeholder: "Enter your username",
                    state:       s.username_state,
                    error_msg:   if s.username_state == InputState::Error {
                        Some("Username is required".into())
                    } else { None },
                    is_password: false,
                },
                // Password
                Widget::TextInput {
                    on_change:   Msg::PasswordChanged,
                    on_submit:   Some(Msg::LoginPressed),
                    style:       input_style,
                    id:          2,
                    label:       "Password",
                    placeholder: "Enter your password",
                    state:       s.password_state,
                    error_msg:   if s.password_state == InputState::Error {
                        Some("Password is required".into())
                    } else { None },
                    is_password: true,
                },
                // Botão Sign in
                Widget::Button {
                    text:     "Sign in",
                    on_press: Msg::LoginPressed,
                    style:    btn_primary,
                    color:    None,
                    variant:  ButtonVariant::Primary,
                },
                // Linha de ações secundárias
                Widget::Row {
                    style: Style {
                        flex_direction: FlexDirection::Row,
                        ..Default::default()
                    },
                    children: vec![
                        Widget::Button {
                            text:     "Clear",
                            on_press: Msg::ClearPressed,
                            style:    btn_ghost.clone(),
                            color:    None,
                            variant:  ButtonVariant::Ghost,
                        },
                        Widget::Button {
                            text:     "Forgot?",
                            on_press: Msg::ClearPressed, // placeholder
                            style:    btn_ghost,
                            color:    None,
                            variant:  ButtonVariant::Text,
                        },
                    ],
                },
                // Status
                Widget::Text {
                    content: s.status.clone(),
                    color:   status_color,
                    size:    12.0,
                    style:   Style {
                        margin: Rect {
                            top: LengthPercentageAuto::length(12.0),
                            bottom: LengthPercentageAuto::length(0.0),
                            left: LengthPercentageAuto::length(0.0),
                            right: LengthPercentageAuto::length(0.0)
                        },
                        ..Default::default()
                    },
                },
            ],
        }
    }

    fn update(s: &mut AppState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::UsernameChanged(v) => {
                s.username       = v;
                s.username_state = InputState::Idle;
                if !s.status.is_empty() { s.status.clear(); }
            }
            Msg::PasswordChanged(v) => {
                s.password       = v;
                s.password_state = InputState::Idle;
            }
            Msg::LoginPressed => {
                if s.username.is_empty() {
                    s.username_state = InputState::Error;
                    s.status         = "Error: username required".into();
                } else if s.password.is_empty() {
                    s.password_state = InputState::Error;
                    s.status         = "Error: password required".into();
                } else {
                    s.username_state = InputState::Success;
                    s.password_state = InputState::Success;
                    s.status         = format!("Welcome, {}!", s.username);
                }
            }
            Msg::ClearPressed => {
                s.username       = String::new();
                s.password       = String::new();
                s.status         = String::new();
                s.username_state = InputState::Idle;
                s.password_state = InputState::Idle;
            }
        }
    }

    fn theme() -> Theme { Theme::dark() }
}

fn main() {
    RutterRunner::<MyApp>::run();
}
