// ============================================================
// Rutter Framework — demos/modal_toast_demo.rs
// Demo isolada de Widget::Modal e Widget::Toast.
// FIX-3e: Toast ao copiar — widget id 91 reservado para copy toast.
//         Para ativar descomente show_copy_toast() em runner.rs.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::widget::ToastKind;
use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

#[derive(Default)]
pub struct ModalToastDemoState {
    pub modal_open: bool,
    pub confirmed: bool,
    pub toast_info: bool,
    pub toast_success: bool,
    pub toast_warning: bool,
    pub toast_error: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
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
            align_items: Some(AlignItems::Center),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        };
        let col = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Center),
            padding: Rect::length(32.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(16.0),
            },
            flex_grow: 1.0,
            ..Default::default()
        };
        let btn_s = |w: f32| Style {
            size: Size {
                width: Dimension::length(w),
                height: Dimension::length(38.0),
            },
            ..Default::default()
        };
        let row_s = Style {
            flex_direction: FlexDirection::Row,
            gap: Size {
                width: LengthPercentage::length(10.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };

        // Modal
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
                    align_items: Some(AlignItems::FlexStart),
                    padding: Rect::length(24.0),
                    gap: Size {
                        width: LengthPercentage::length(0.0),
                        height: LengthPercentage::length(16.0),
                    },
                    size: Size {
                        width: Dimension::auto(),
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
                        color: Some(skia_safe::Color::from_rgb(0x9d, 0x9d, 0x9d)),
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

        // Toasts
        let toast_info = Widget::Toast {
            id: 90,
            message: "Informação: operação iniciada.",
            kind: ToastKind::Info,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(90)),
        };
        let toast_success = Widget::Toast {
            id: 91,
            message: "Sucesso! Dados salvos corretamente.",
            kind: ToastKind::Success,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(91)),
        };
        let toast_warning = Widget::Toast {
            id: 92,
            message: "Atenção: disco com pouco espaço.",
            kind: ToastKind::Warning,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(92)),
        };
        let toast_error = Widget::Toast {
            id: 93,
            message: "Erro: falha na conexão.",
            kind: ToastKind::Error,
            duration_ms: 3000,
            on_dismiss: Some(Msg::DismissToast(93)),
        };

        Widget::Column {
            style: root,
            children: vec![
                Widget::Column {
                    style: col,
                    children: vec![
                        Widget::Text {
                            content: "Modal & Toast".into(),
                            color: None,
                            size: 18.0,
                            style: Style::default(),
                        },
                        if s.confirmed {
                            Widget::Text {
                                content: "✓ Ação confirmada!".into(),
                                color: Some(skia_safe::Color::from_rgb(0x4e, 0xc9, 0xb0)),
                                size: 13.0,
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
                        Widget::Button {
                            text: "Abrir Modal",
                            on_press: Msg::OpenModal,
                            style: btn_s(200.0),
                            color: None,
                            variant: ButtonVariant::Primary,
                        },
                        Widget::Text {
                            content: "Tipos de Toast:".into(),
                            color: None,
                            size: 13.0,
                            style: Style::default(),
                        },
                        Widget::Row {
                            style: row_s.clone(),
                            children: vec![
                                Widget::Button {
                                    text: "Info",
                                    on_press: Msg::ShowInfo,
                                    style: btn_s(80.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                                Widget::Button {
                                    text: "Sucesso",
                                    on_press: Msg::ShowSuccess,
                                    style: btn_s(90.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                            ],
                        },
                        Widget::Row {
                            style: row_s,
                            children: vec![
                                Widget::Button {
                                    text: "Aviso",
                                    on_press: Msg::ShowWarning,
                                    style: btn_s(80.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                                Widget::Button {
                                    text: "Erro",
                                    on_press: Msg::ShowError,
                                    style: btn_s(80.0),
                                    color: None,
                                    variant: ButtonVariant::Ghost,
                                },
                            ],
                        },
                    ],
                },
                modal,
                toast_info,
                toast_success,
                toast_warning,
                toast_error,
            ],
        }
    }

    fn update(s: &mut ModalToastDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
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

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<ModalToastDemo>::run();
}
