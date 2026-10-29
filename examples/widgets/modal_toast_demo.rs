// ============================================================
// Rutter Framework — demos/modal_toast_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::widget::{ToastKind, ToastPosition};
use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

#[derive(Default)]
pub struct ModalToastDemoState {
    pub theme: ExampleTheme,
    pub modal_open: bool,
    pub confirmed: bool,
    pub toast_info: bool,
    pub toast_success: bool,
    pub toast_warning: bool,
    pub toast_error: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    OpenModal,
    CloseModal,
    ConfirmModal,
    ShowInfo,
    ShowSuccess,
    ShowWarning,
    ShowError,
    DismissToast(u64),
}

pub struct ModalToastDemo;

impl AppLogic for ModalToastDemo {
    type State = ModalToastDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ModalToastDemoState::default()
    }

    fn view<'a>(s: &'a mut ModalToastDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        };
        let col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            gap: Size::length(spacing::MD),
            flex_shrink: 0.0,
            ..responsive_width(560.0, Dimension::auto())
        };
        let btn_s = |w: f32| responsive_width(w, Dimension::length(38.0));
        let row_s = Style {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            gap: Size::length(spacing::MD),
            ..Default::default()
        };

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
                    padding: Rect::length(spacing::XL),
                    gap: Size::length(spacing::LG),
                    size: Size {
                        width: Dimension::percent(1.0),
                        height: Dimension::auto(),
                    },
                    max_size: Size {
                        width: Dimension::length(320.0),
                        height: Dimension::auto(),
                    },
                    ..Default::default()
                },
                children: vec![
                    Widget::Text {
                        content: "Confirmar ação".into(),
                        color: None,
                        size: 16.0,
                        style: Style::default(),
                    },
                    Widget::Text {
                        content: "Esta ação não pode ser desfeita. Deseja continuar?".into(),
                        color: None,
                        size: 13.0,
                        style: Style::default(),
                    },
                    Widget::Row {
                        style: row_s.clone(),
                        children: vec![
                            Widget::Button {
                                text: "Cancelar",
                                on_press: Msg::CloseModal,
                                style: btn_s(140.0),
                                color: None,
                                variant: ButtonVariant::Ghost,
                            },
                            Widget::Button {
                                text: "Confirmar",
                                on_press: Msg::ConfirmModal,
                                style: btn_s(140.0),
                                color: None,
                                variant: ButtonVariant::Primary,
                            },
                        ],
                    },
                ],
            }),
        };

        let toast_info = Widget::Toast {
            id: 90,
            visible: s.toast_info,
            message: "Informação: operação iniciada.",
            kind: ToastKind::Info,
            position: ToastPosition::BottomLeft,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(90)),
        };
        let toast_success = Widget::Toast {
            id: 91,
            visible: s.toast_success,
            message: "Sucesso! Dados salvos corretamente.",
            kind: ToastKind::Success,
            position: ToastPosition::BottomRight,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(91)),
        };
        let toast_warning = Widget::Toast {
            id: 92,
            visible: s.toast_warning,
            message: "Atenção: disco com pouco espaço.",
            kind: ToastKind::Warning,
            position: ToastPosition::TopRight,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(92)),
        };
        let toast_error = Widget::Toast {
            id: 93,
            visible: s.toast_error,
            message: "Erro: falha na conexão.",
            kind: ToastKind::Error,
            position: ToastPosition::TopLeft,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(93)),
        };

        let mut modal_actions = vec![Widget::heading(
            rutter::HeadingLevel::H2,
            "Modal confirmation",
            Style::default(),
        )];
        if s.confirmed {
            modal_actions.push(Widget::Text {
                content: "✓ Ação confirmada!".into(),
                color: None,
                size: 13.0,
                style: Style::default(),
            });
        }
        modal_actions.push(Widget::Button {
            text: "Abrir Modal",
            on_press: Msg::OpenModal,
            style: btn_s(200.0),
            color: None,
            variant: ButtonVariant::Primary,
        });

        super::layout::page(
            "Modal & Toast",
            Widget::Column {
                style: root,
                children: vec![
                    Widget::Column {
                        style: col,
                        children: modal_actions,
                    },
                    super::layout::section(vec![
                        Widget::heading(
                            rutter::HeadingLevel::H2,
                            "Tipos de Toast (Posições diferentes):",
                            Style::default(),
                        ),
                        Widget::Row {
                            style: row_s.clone(),
                            children: vec![
                                Widget::Button {
                                    text: "Info (BL)",
                                    on_press: Msg::ShowInfo,
                                    style: btn_s(100.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                                Widget::Button {
                                    text: "Sucesso (BR)",
                                    on_press: Msg::ShowSuccess,
                                    style: btn_s(120.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                            ],
                        },
                        Widget::Row {
                            style: row_s,
                            children: vec![
                                Widget::Button {
                                    text: "Aviso (TR)",
                                    on_press: Msg::ShowWarning,
                                    style: btn_s(100.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                                Widget::Button {
                                    text: "Erro (TL)",
                                    on_press: Msg::ShowError,
                                    style: btn_s(100.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                            ],
                        },
                    ]),
                    example_theme_selector(s.theme, Msg::ThemeChanged),
                    modal,
                    toast_info,
                    toast_success,
                    toast_warning,
                    toast_error,
                ],
            },
        )
    }

    fn update(s: &mut ModalToastDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::OpenModal => s.modal_open = true,
            Msg::CloseModal => s.modal_open = false,
            Msg::ConfirmModal => {
                s.modal_open = false;
                s.confirmed = true;
            }
            Msg::ShowInfo => s.toast_info = true,
            Msg::ShowSuccess => s.toast_success = true,
            Msg::ShowWarning => s.toast_warning = true,
            Msg::ShowError => s.toast_error = true,
            Msg::DismissToast(id) => match id {
                90 => s.toast_info = false,
                91 => s.toast_success = false,
                92 => s.toast_warning = false,
                93 => s.toast_error = false,
                _ => {}
            },
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<ModalToastDemo>::run();
}

#[cfg(test)]
mod page_tests {
    use super::ModalToastDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<ModalToastDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<ModalToastDemo>();
    }

    use super::super::layout::test_support::assert_overlay;
    use rutter::AppLogic;

    #[test]
    fn open_modal_paints_above_and_blocks_the_scroll_document() {
        use super::{ModalToastDemo, ModalToastDemoState, Msg};
        let mut state = ModalToastDemoState {
            modal_open: true,
            ..Default::default()
        };
        assert_overlay(&ModalToastDemo::view(&mut state), |message| {
            matches!(message, Msg::CloseModal | Msg::ConfirmModal)
        });
    }
}
