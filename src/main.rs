// ============================================================
// Rutter Framework — main.rs  (Fase 3 Demo)
//
// Demonstra todos os novos widgets da Fase 3:
//   Checkbox, Switch, Radio, Slider, ProgressBar, Spinner,
//   Divider, Spacer, ScrollView, Select, Tooltip, Image
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;
use rutter::{AppLogic, ButtonVariant, InputState, RutterRunner, Theme, Widget, widget::Orientation};

// ── Estado ───────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct AppState {
    // TextInputs
    pub username:       String,
    pub password:       String,
    pub username_state: InputState,

    // Checkbox / Switch / Radio
    pub remember_me:    bool,
    pub dark_mode:      bool,
    pub theme_option:   usize,          // 0=System 1=Light 2=Dark

    // Slider
    pub volume:         f32,            // 0.0–100.0
    pub font_size:      f32,            // 10.0–24.0

    // ProgressBar
    pub upload_progress: f32,           // 0.0–1.0
    pub is_loading:     bool,

    // Select
    pub language:       usize,          // índice em LANGUAGES

    // Status
    pub status:         String,
}

const LANGUAGES: &[&str] = &["Rust", "Python", "TypeScript", "Go", "Zig"];
const THEMES:    &[&str] = &["System", "Light", "Dark"];

// ── Mensagens ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Msg {
    UsernameChanged(String),
    PasswordChanged(String),
    RememberMeToggled(bool),
    DarkModeToggled(bool),
    ThemeSelected(usize),
    VolumeChanged(f32),
    FontSizeChanged(f32),
    LanguageChanged(usize),
    LoginPressed,
    SimulateProgress,
    ClearPressed,
}

// ── AppLogic ──────────────────────────────────────────────────

pub struct MyApp;

impl AppLogic for MyApp {
    type State   = AppState;
    type Message = Msg;

    fn new(_fs: &mut FontSystem) -> AppState {
        AppState {
            volume:          60.0,
            font_size:       14.0,
            upload_progress: 0.35,
            ..Default::default()
        }
    }

    fn view<'a>(s: &'a mut AppState) -> Widget<'a, Msg> {
        // ── Estilos comuns ───────────────────────────────────
        let col = Style {
            flex_direction:  FlexDirection::Column,
            align_items:     Some(AlignItems::Stretch),
            size: Size { width: Dimension::length(320.0), height: Dimension::auto() },
            gap:  Size { width:  LengthPercentage::length(0.0),
                         height: LengthPercentage::length(20.0) },
            ..Default::default()
        };

        let root_col = Style {
            flex_direction:  FlexDirection::Column,
            align_items:     Some(AlignItems::Center),
            justify_content: Some(JustifyContent::FlexStart),
            size: Size { width: Dimension::percent(1.0), height: Dimension::percent(1.0) },
            padding: Rect {
                top:    LengthPercentage::length(40.0),
                bottom: LengthPercentage::length(40.0),
                left:   LengthPercentage::length(0.0),
                right:  LengthPercentage::length(0.0),
            },
            ..Default::default()
        };

        let input_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(40.0) },
            ..Default::default()
        };

        let btn_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(38.0) },
            ..Default::default()
        };

        let row_style = Style {
            flex_direction: FlexDirection::Row,
            align_items:    Some(AlignItems::Center),
            gap: Size { width: LengthPercentage::length(16.0), height: LengthPercentage::length(0.0) },
            ..Default::default()
        };

        let checkbox_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(28.0) },
            ..Default::default()
        };

        let switch_style = Style {
            size: Size { width: Dimension::length(64.0), height: Dimension::length(28.0) },
            ..Default::default()
        };

        let radio_style = Style {
            size: Size { width: Dimension::length(90.0), height: Dimension::length(28.0) },
            ..Default::default()
        };

        let slider_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(36.0) },
            ..Default::default()
        };

        let progress_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(20.0) },
            ..Default::default()
        };

        let divider_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(1.0) },
            margin: Rect {
                top: LengthPercentageAuto::length(4.0),
                bottom: LengthPercentageAuto::length(4.0),
                left: LengthPercentageAuto::length(0.0),
                right: LengthPercentageAuto::length(0.0)
            },
            ..Default::default()
        };

        let select_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(44.0) },
            ..Default::default()
        };

        let spinner_style = Style {
            size: Size { width: Dimension::length(32.0), height: Dimension::length(32.0) },
            ..Default::default()
        };

        let scroll_style = Style {
            size: Size { width: Dimension::percent(1.0), height: Dimension::length(180.0) },
            ..Default::default()
        };

        // ── Seção de credenciais ─────────────────────────────
        let credentials = Widget::Column {
            style: col.clone(),
            children: vec![
                Widget::Text {
                    content: "Sign in".into(),
                    color:   None,
                    size:    18.0,
                    style:   Style::default(),
                },
                Widget::TextInput {
                    on_change: Msg::UsernameChanged, on_submit: None,
                    style: input_style.clone(), id: 1,
                    label: "Username", placeholder: "Enter username",
                    state: s.username_state, error_msg: None, is_password: false,
                },
                Widget::TextInput {
                    on_change: Msg::PasswordChanged, on_submit: Some(Msg::LoginPressed),
                    style: input_style, id: 2,
                    label: "Password", placeholder: "Enter password",
                    state: InputState::Idle, error_msg: None, is_password: true,
                },
                Widget::Checkbox {
                    checked: s.remember_me, on_change: Msg::RememberMeToggled,
                    label: "Remember me", style: checkbox_style.clone(),
                },
                Widget::Button {
                    text: "Sign in", on_press: Msg::LoginPressed,
                    style: btn_style.clone(), color: None, variant: ButtonVariant::Primary,
                },
            ],
        };

        // ── Divider ──────────────────────────────────────────

        let div1 = Widget::Divider { style: divider_style.clone(), orientation: Orientation::Horizontal };

        // ── Seção de aparência ───────────────────────────────
        let appearance = Widget::Column {
            style: col.clone(),
            children: vec![
                Widget::Text {
                    content: "Appearance".into(), color: None, size: 15.0, style: Style::default(),
                },
                // Switch — Dark Mode
                Widget::Row {
                    style: row_style.clone(),
                    children: vec![
                        Widget::Text {
                            content: "Dark mode".into(), color: None, size: 14.0,
                            style: Style { flex_grow: 1.0, ..Default::default() },
                        },
                        Widget::Switch {
                            checked: s.dark_mode, on_change: Msg::DarkModeToggled,
                            style: switch_style.clone(),
                        },
                    ],
                },
                // Radio group — Tema
                Widget::Text {
                    content: "Theme".into(), color: None, size: 13.0, style: Style::default(),
                },
                Widget::Row {
                    style: row_style.clone(),
                    children: THEMES.iter().enumerate().map(|(i, label)| {
                        Widget::Radio {
                            selected:  s.theme_option == i,
                            on_select: match i {
                                0 => || Msg::ThemeSelected(0),
                                1 => || Msg::ThemeSelected(1),
                                _ => || Msg::ThemeSelected(2),
                            },
                            label,
                            style: radio_style.clone(),
                        }
                    }).collect(),
                },
                // Select — Language
                Widget::Select {
                    id:             10,
                    options:        LANGUAGES,
                    selected_index: s.language,
                    on_change:      Msg::LanguageChanged,
                    style:          select_style,
                    label:          "Language",
                    placeholder:    "Choose...",
                },
            ],
        };

        // ── Divider ──────────────────────────────────────────
        let div2 = Widget::Divider { style: divider_style.clone(), orientation: Orientation::Horizontal };

        // ── Seção de controles ───────────────────────────────
        let controls = Widget::Column {
            style: col.clone(),
            children: vec![
                Widget::Text {
                    content: "Controls".into(), color: None, size: 15.0, style: Style::default(),
                },
                // Slider Volume
                Widget::Text {
                    content: format!("Volume: {:.0}%", s.volume),
                    color: None, size: 13.0, style: Style::default(),
                },
                Widget::Slider {
                    id:        20,
                    value:     s.volume,
                    min:       0.0,
                    max:       100.0,
                    step:      5.0,
                    on_change: Msg::VolumeChanged,
                    style:     slider_style.clone(),
                    label:     "",
                },
                // Slider Font Size
                Widget::Text {
                    content: format!("Font size: {:.0}px", s.font_size),
                    color: None, size: 13.0, style: Style::default(),
                },
                Widget::Slider {
                    id:        21,
                    value:     s.font_size,
                    min:       10.0,
                    max:       24.0,
                    step:      1.0,
                    on_change: Msg::FontSizeChanged,
                    style:     slider_style,
                    label:     "",
                },
            ],
        };

        // ── Divider ──────────────────────────────────────────
        let div3 = Widget::Divider { style: divider_style.clone(), orientation: Orientation::Horizontal };

        // ── Seção de progresso ───────────────────────────────
        let progress_section = Widget::Column {
            style: col.clone(),
            children: vec![
                Widget::Text {
                    content: "Progress".into(), color: None, size: 15.0, style: Style::default(),
                },
                Widget::Text {
                    content: format!("Upload: {:.0}%", s.upload_progress * 100.0),
                    color: None, size: 13.0, style: Style::default(),
                },
                Widget::ProgressBar {
                    value:         s.upload_progress,
                    indeterminate: false,
                    style:         progress_style.clone(),
                },
                // ProgressBar indeterminada + Spinner
                Widget::Row {
                    style: row_style.clone(),
                    children: vec![
                        Widget::ProgressBar {
                            value:         0.0,
                            indeterminate: s.is_loading,
                            style: Style {
                                size: Size { width: Dimension::percent(1.0), height: Dimension::length(20.0) },
                                flex_grow: 1.0,
                                ..Default::default()
                            },
                        },
                        Widget::Spinner {
                            id:    30,
                            style: if s.is_loading { spinner_style.clone() } else {
                                Style { size: Size { width: Dimension::length(0.0), height: Dimension::length(32.0) }, ..Default::default() }
                            },
                        },
                    ],
                },
                // Botões de controle
                Widget::Row {
                    style: row_style.clone(),
                    children: vec![
                        Widget::Button {
                            text: "Simulate", on_press: Msg::SimulateProgress,
                            style: Style {
                                size: Size { width: Dimension::percent(1.0), height: Dimension::length(34.0) },
                                flex_grow: 1.0, ..Default::default()
                            },
                            color: None, variant: ButtonVariant::Ghost,
                        },
                        Widget::Button {
                            text: "Clear", on_press: Msg::ClearPressed,
                            style: Style {
                                size: Size { width: Dimension::percent(1.0), height: Dimension::length(34.0) },
                                flex_grow: 1.0, ..Default::default()
                            },
                            color: None, variant: ButtonVariant::Text,
                        },
                    ],
                },
            ],
        };

        // ── Divider ──────────────────────────────────────────
        let div4 = Widget::Divider { style: divider_style, orientation: Orientation::Horizontal };

        // ── ScrollView com lista longa ───────────────────────
        // Conteúdo mais alto que o container → activa o scroll
        let scroll_content = Widget::Column {
            style: Style {
                flex_direction: FlexDirection::Column,
                size: Size { width: Dimension::percent(1.0), height: Dimension::auto() },
                gap: Size { width: LengthPercentage::length(0.0), height: LengthPercentage::length(4.0) },
                ..Default::default()
            },
            children: (1..=15).map(|i| {
                Widget::Text {
                    content: format!("Log entry #{i:02} — framework event fired"),
                    color:   None,
                    size:    13.0,
                    style:   Style {
                        size: Size { width: Dimension::percent(1.0), height: Dimension::length(26.0) },
                        ..Default::default()
                    },
                }
            }).collect(),
        };

        let scroll_section = Widget::Column {
            style: col.clone(),
            children: vec![
                Widget::Text {
                    content: "Event log (scroll)".into(),
                    color: None, size: 15.0, style: Style::default(),
                },
                Widget::Tooltip {
                    text:  "Scroll with mouse wheel",
                    style: scroll_style.clone(),
                    child: Box::new(Widget::ScrollView {
                        id:    40,
                        style: scroll_style,
                        child: Box::new(scroll_content),
                    }),
                },
            ],
        };

        // ── Status ───────────────────────────────────────────
        let status_color = if s.status.starts_with("Welcome") {
            Some(skia_safe::Color::from_rgb(0x4e, 0xc9, 0xb0))
        } else if s.status.starts_with("Error") {
            Some(skia_safe::Color::from_rgb(0xf4, 0x47, 0x47))
        } else { None };

        // ── Raiz ─────────────────────────────────────────────
        Widget::Column {
            style: root_col,
            children: vec![
                credentials,
                div1,
                appearance,
                div2,
                controls,
                div3,
                progress_section,
                div4,
                scroll_section,
                Widget::Text {
                    content: s.status.clone(),
                    color:   status_color,
                    size:    12.0,
                    style: Style {
                        margin: Rect {
                            top: LengthPercentageAuto::length(12.0),
                            bottom: LengthPercentageAuto::length(0.0),
                            left: LengthPercentageAuto::length(0.0),
                            right: LengthPercentageAuto::length(0.0)
                        },
                        ..Default::default()
                    },
                },
                Widget::Spacer {
                    style: Style { flex_grow: 1.0, ..Default::default() },
                },
            ],
        }
    }

    fn update(s: &mut AppState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::UsernameChanged(v) => {
                s.username       = v;
                s.username_state = InputState::Idle;
                s.status.clear();
            }
            Msg::PasswordChanged(v)        => s.password        = v,
            Msg::RememberMeToggled(v)      => s.remember_me     = v,
            Msg::DarkModeToggled(v)        => s.dark_mode        = v,
            Msg::ThemeSelected(i)          => s.theme_option     = i,
            Msg::VolumeChanged(v)          => s.volume           = v,
            Msg::FontSizeChanged(v)        => s.font_size        = v,
            Msg::LanguageChanged(i)        => s.language         = i,

            Msg::LoginPressed => {
                if s.username.is_empty() {
                    s.username_state = InputState::Error;
                    s.status = "Error: username required".into();
                } else if s.password.is_empty() {
                    s.status = "Error: password required".into();
                } else {
                    s.username_state = InputState::Success;
                    s.status = format!("Welcome, {}! Language: {}", s.username,
                        LANGUAGES.get(s.language).copied().unwrap_or("?"));
                }
            }

            Msg::SimulateProgress => {
                s.upload_progress = (s.upload_progress + 0.1).min(1.0);
                s.is_loading      = s.upload_progress < 1.0;
            }

            Msg::ClearPressed => {
                s.username       = String::new();
                s.password       = String::new();
                s.username_state = InputState::Idle;
                s.upload_progress = 0.0;
                s.is_loading     = false;
                s.status.clear();
            }
        }
    }

    fn theme() -> Theme { Theme::dark() }
}

fn main() {
    RutterRunner::<MyApp>::run();
}
