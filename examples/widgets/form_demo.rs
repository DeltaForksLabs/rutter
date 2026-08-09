// ============================================================
// Rutter Framework — demos/form_demo.rs
// Demo completa integrada (original main.rs refatorado).
// Exercita todos os widgets juntos: Login, Controls, Log.
// FIX-6: Slider com step: 1.0 (padrão linear).
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{
    AppLogic, ButtonVariant, InputState, RutterRunner, Theme, Widget,
    widget::{Orientation, ToastKind, ToastPosition},
};

use super::theme_selector::{ExampleTheme, example_theme_selector};

// ── Estado ───────────────────────────────────────────────────

#[derive(Default)]
pub struct AppState {
    pub username: String,
    pub password: String,
    pub username_state: InputState,
    pub active_tab: usize,
    pub modal_open: bool,
    pub modal_confirmed: bool,
    pub toast_visible: bool,
    pub toast_msg: String,
    pub list_selected: Option<usize>,
    pub volume: f32,
    pub language: usize,
    pub remember: bool,
    pub theme: ExampleTheme,
    pub upload_progress: f32,
    pub is_loading: bool,
}

const LANGUAGES: &[&str] = &["Rust", "Python", "TypeScript", "Go", "Zig"];
const TABS: &[&str] = &["Login", "Controls", "Log"];
const LOG_ITEMS: usize = 200;

#[derive(Debug, Clone)]
pub enum Msg {
    UsernameChanged(String),
    PasswordChanged(String),
    RememberToggled(bool),
    DarkModeToggled(bool),
    ThemeChanged(ExampleTheme),
    VolumeChanged(f32),
    LanguageChanged(usize),
    TabChanged(usize),
    LoginPressed,
    OpenModal,
    ConfirmModal,
    CloseModal,
    DismissToast,
    ListSelected(usize),
    SimulateProgress,
    ClearPressed,
}

pub struct MyApp;

impl AppLogic for MyApp {
    type State = AppState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> AppState {
        AppState {
            volume: 60.0,
            upload_progress: 0.35,
            ..Default::default()
        }
    }

    fn view<'a>(s: &'a mut AppState) -> Widget<'a, Msg> {
        let _full_w: Style = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            ..Default::default()
        };
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        };
        let content_col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            padding: Rect {
                top: LengthPercentage::length(32.0),
                left: LengthPercentage::length(0.0),
                right: LengthPercentage::length(0.0),
                bottom: LengthPercentage::length(32.0),
            },
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(16.0),
            },
            flex_grow: 1.0,
            ..Default::default()
        };
        let card_col = Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::length(320.0),
                height: Dimension::auto(),
            },
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(14.0),
            },
            ..Default::default()
        };
        let input_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(40.0),
            },
            ..Default::default()
        };
        let btn_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(38.0),
            },
            ..Default::default()
        };
        let row_s = Style {
            flex_direction: FlexDirection::Row,
            gap: Size {
                width: LengthPercentage::length(12.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };
        let slider_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(36.0),
            },
            ..Default::default()
        };
        let select_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(44.0),
            },
            ..Default::default()
        };
        let progress_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(20.0),
            },
            ..Default::default()
        };
        let divider_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(1.0),
            },
            ..Default::default()
        };
        let vlist_s = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(240.0),
            },
            ..Default::default()
        };
        let switch_s = Style {
            size: Size {
                width: Dimension::length(60.0),
                height: Dimension::length(28.0),
            },
            ..Default::default()
        };

        // ── TabBar ───────────────────────────────────────────
        let tabbar = Widget::TabBar {
            id: 50,
            tabs: TABS,
            active: s.active_tab,
            on_change: Msg::TabChanged,
            style: Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::length(44.0),
                },
                ..Default::default()
            },
        };

        // ── Conteúdo por aba ─────────────────────────────────
        let tab_content = match s.active_tab {
            0 => Widget::Column {
                style: card_col.clone(),
                children: vec![
                    Widget::Text {
                        content: "Sign in".into(),
                        color: None,
                        size: 16.0,
                        style: Style::default(),
                    },
                    Widget::TextInput {
                        on_change: Msg::UsernameChanged,
                        on_submit: None,
                        style: input_s.clone(),
                        id: 1,
                        label: "Username",
                        placeholder: "Enter username",
                        state: s.username_state,
                        error_msg: if s.username_state == InputState::Error {
                            Some("Required".into())
                        } else {
                            None
                        },
                        is_password: false,
                    },
                    Widget::TextInput {
                        on_change: Msg::PasswordChanged,
                        on_submit: Some(Msg::LoginPressed),
                        style: input_s,
                        id: 2,
                        label: "Password",
                        placeholder: "Enter password",
                        state: InputState::Idle,
                        error_msg: None,
                        is_password: true,
                    },
                    Widget::Row {
                        style: row_s.clone(),
                        children: vec![Widget::Checkbox {
                            checked: s.remember,
                            on_change: Msg::RememberToggled,
                            label: "Remember me",
                            style: Style {
                                flex_grow: 1.0,
                                size: Size {
                                    height: Dimension::length(28.0),
                                    width: Dimension::length(0.0),
                                },
                                ..Default::default()
                            },
                        }],
                    },
                    Widget::Button {
                        text: "Sign in",
                        on_press: Msg::LoginPressed,
                        style: btn_s.clone(),
                        color: None,
                        variant: ButtonVariant::Primary,
                    },
                    Widget::Row {
                        style: row_s.clone(),
                        children: vec![
                            Widget::Button {
                                text: "Open Modal",
                                on_press: Msg::OpenModal,
                                style: Style {
                                    size: Size {
                                        width: Dimension::percent(1.0),
                                        height: Dimension::length(30.0),
                                    },
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                                color: None,
                                variant: ButtonVariant::Ghost,
                            },
                            Widget::Button {
                                text: "Show Toast",
                                on_press: Msg::SimulateProgress,
                                style: Style {
                                    size: Size {
                                        width: Dimension::percent(1.0),
                                        height: Dimension::length(34.0),
                                    },
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                                color: None,
                                variant: ButtonVariant::Text,
                            },
                        ],
                    },
                    if s.modal_confirmed {
                        Widget::Text {
                            content: "✓ Modal confirmed".into(),
                            color: None,
                            size: 12.0,
                            style: Style::default(),
                        }
                    } else {
                        Widget::Spacer {
                            style: Style {
                                size: Size {
                                    height: Dimension::length(0.0),
                                    width: Dimension::length(0.0),
                                },
                                ..Default::default()
                            },
                        }
                    },
                ],
            },

            1 => Widget::Column {
                style: card_col.clone(),
                children: vec![
                    Widget::Text {
                        content: "Controls".into(),
                        color: None,
                        size: 16.0,
                        style: Style::default(),
                    },
                    Widget::Text {
                        content: format!("Volume: {:.0}%", s.volume),
                        color: None,
                        size: 13.0,
                        style: Style::default(),
                    },
                    Widget::Slider {
                        id: 20,
                        value: s.volume,
                        min: 0.0,
                        max: 100.0,
                        step: 1.0, // FIX-6: era 5.0
                        on_change: Msg::VolumeChanged,
                        style: slider_s.clone(),
                        label: "",
                    },
                    Widget::Select {
                        id: 10,
                        options: LANGUAGES,
                        selected_index: s.language,
                        on_change: Msg::LanguageChanged,
                        style: select_s,
                        label: "Language",
                        placeholder: "Choose...",
                    },
                    Widget::Row {
                        style: row_s.clone(),
                        children: vec![
                            Widget::Text {
                                content: "Dark mode".into(),
                                color: None,
                                size: 14.0,
                                style: Style {
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                            },
                            Widget::Switch {
                                checked: s.theme == ExampleTheme::Dark,
                                on_change: Msg::DarkModeToggled,
                                style: switch_s,
                            },
                        ],
                    },
                    Widget::Divider {
                        style: divider_s,
                        orientation: Orientation::Horizontal,
                    },
                    Widget::Text {
                        content: format!("Upload: {:.0}%", s.upload_progress * 100.0),
                        color: None,
                        size: 13.0,
                        style: Style::default(),
                    },
                    Widget::ProgressBar {
                        id: 31,
                        value: s.upload_progress,
                        indeterminate: false,
                        style: progress_s.clone(),
                    },
                    Widget::ProgressBar {
                        id: 32,
                        value: 0.0,
                        indeterminate: s.is_loading,
                        style: progress_s,
                    },
                    Widget::Row {
                        style: row_s,
                        children: vec![
                            Widget::Button {
                                text: "Simulate",
                                on_press: Msg::SimulateProgress,
                                style: Style {
                                    size: Size {
                                        width: Dimension::percent(1.0),
                                        height: Dimension::length(34.0),
                                    },
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                                color: None,
                                variant: ButtonVariant::Ghost,
                            },
                            Widget::Button {
                                text: "Clear",
                                on_press: Msg::ClearPressed,
                                style: Style {
                                    size: Size {
                                        width: Dimension::percent(1.0),
                                        height: Dimension::length(34.0),
                                    },
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                                color: None,
                                variant: ButtonVariant::Text,
                            },
                        ],
                    },
                ],
            },

            _ => Widget::Column {
                style: card_col.clone(),
                children: vec![
                    Widget::Text {
                        content: "Event Log".into(),
                        color: None,
                        size: 16.0,
                        style: Style::default(),
                    },
                    Widget::Text {
                        content: s
                            .list_selected
                            .map(|i| format!("Selected: item #{i}"))
                            .unwrap_or_default(),
                        color: None,
                        size: 12.0,
                        style: Style::default(),
                    },
                    Widget::VirtualList {
                        id: 60,
                        item_height: 30.0,
                        item_count: LOG_ITEMS,
                        items: &|i| Some(format!("Log #{:03} — framework event fired", i + 1)),
                        on_select: Msg::ListSelected,
                        style: vlist_s,
                    },
                ],
            },
        };

        // ── Modal ─────────────────────────────────────────────
        let modal = Widget::Modal {
            id: 80,
            visible: s.modal_open,
            on_dismiss: Some(Msg::CloseModal),
            style: Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::percent(1.0),
                },
                ..Default::default()
            },
            child: Box::new(Widget::Column {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    align_items: Some(AlignItems::Stretch),
                    padding: Rect::length(24.0_f32),
                    gap: Size {
                        width: LengthPercentage::length(0.0),
                        height: LengthPercentage::length(16.0),
                    },
                    size: Size {
                        width: Dimension::length(320.0),
                        height: Dimension::auto(),
                    },
                    ..Default::default()
                },
                children: vec![
                    Widget::Text {
                        content: "Confirm action".into(),
                        color: None,
                        size: 16.0,
                        style: Style::default(),
                    },
                    Widget::Text {
                        content: "Are you sure you want to proceed? This cannot be undone.".into(),
                        color: None,
                        size: 13.0,
                        style: Style::default(),
                    },
                    Widget::Row {
                        style: Style {
                            flex_direction: FlexDirection::Row,
                            gap: Size {
                                width: LengthPercentage::length(8.0),
                                height: LengthPercentage::length(0.0),
                            },
                            ..Default::default()
                        },
                        children: vec![
                            Widget::Button {
                                text: "Cancel",
                                on_press: Msg::CloseModal,
                                style: Style {
                                    size: Size {
                                        width: Dimension::percent(1.0),
                                        height: Dimension::length(36.0),
                                    },
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                                color: None,
                                variant: ButtonVariant::Ghost,
                            },
                            Widget::Button {
                                text: "Confirm",
                                on_press: Msg::ConfirmModal,
                                style: Style {
                                    size: Size {
                                        width: Dimension::percent(1.0),
                                        height: Dimension::length(36.0),
                                    },
                                    flex_grow: 1.0,
                                    ..Default::default()
                                },
                                color: None,
                                variant: ButtonVariant::Primary,
                            },
                        ],
                    },
                ],
            }),
        };

        // ── Toast ─────────────────────────────────────────────
        let toast = Widget::Toast {
            id: 90,
            visible: s.toast_visible,
            message: if s.toast_msg.is_empty() {
                "Action completed!"
            } else {
                s.toast_msg.as_str()
            },
            kind: if s.username_state == InputState::Error {
                ToastKind::Error
            } else {
                ToastKind::Success
            },
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast),
            position: ToastPosition::BottomRight,
        };

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                tabbar,
                Widget::Column {
                    style: content_col,
                    children: vec![tab_content],
                },
                modal,
                toast,
            ],
        }
    }

    fn update(s: &mut AppState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::UsernameChanged(v) => {
                s.username = v;
                s.username_state = InputState::Idle;
            }
            Msg::PasswordChanged(v) => s.password = v,
            Msg::RememberToggled(v) => s.remember = v,
            Msg::DarkModeToggled(v) => {
                s.theme = if v {
                    ExampleTheme::Dark
                } else {
                    ExampleTheme::Light
                };
            }
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::VolumeChanged(v) => s.volume = v,
            Msg::LanguageChanged(i) => s.language = i,
            Msg::TabChanged(i) => s.active_tab = i,
            Msg::ListSelected(i) => s.list_selected = Some(i),
            Msg::LoginPressed => {
                if s.username.is_empty() {
                    s.username_state = InputState::Error;
                } else {
                    s.username_state = InputState::Success;
                    s.toast_msg = format!("Welcome, {}!", s.username);
                    s.toast_visible = true;
                }
            }
            Msg::OpenModal => s.modal_open = true,
            Msg::ConfirmModal => {
                s.modal_open = false;
                s.modal_confirmed = true;
                s.toast_msg = "Confirmed!".into();
                s.toast_visible = true;
            }
            Msg::CloseModal => s.modal_open = false,
            Msg::DismissToast => s.toast_visible = false,
            Msg::SimulateProgress => {
                s.upload_progress = (s.upload_progress + 0.1).min(1.0);
                s.is_loading = s.upload_progress < 1.0;
                s.toast_msg = format!("Upload: {:.0}%", s.upload_progress * 100.0);
                s.toast_visible = true;
            }
            Msg::ClearPressed => {
                s.username = String::new();
                s.password = String::new();
                s.username_state = InputState::Idle;
                s.upload_progress = 0.0;
                s.is_loading = false;
                s.modal_confirmed = false;
            }
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<MyApp>::run();
}
