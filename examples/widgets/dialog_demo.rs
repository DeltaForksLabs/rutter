// ============================================================
// Rutter Framework — demos/dialog_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, DialogPosition, RutterRunner, Theme, Widget};

use super::theme_selector::{ExampleTheme, example_theme_selector};

#[derive(Default)]
pub struct DialogDemoState {
    pub theme: ExampleTheme,
    pub confirm_open: bool,
    pub delete_open: bool,
    pub info_open: bool,
    pub last_action: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    OpenConfirm,
    OpenDelete,
    OpenInfo,
    CloseAll,
    ConfirmSave,
    ConfirmDelete,
}

pub struct DialogDemo;

impl AppLogic for DialogDemo {
    type State = DialogDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        DialogDemoState {
            last_action: "Nenhuma ação executada ainda.".into(),
            ..Default::default()
        }
    }

    fn view<'a>(s: &'a mut DialogDemoState) -> Widget<'a, Msg> {
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
            gap: Size {
                width: LengthPercentage::length(12.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };
        let btn = |w: f32| Style {
            size: Size {
                width: Dimension::length(w),
                height: Dimension::length(40.0),
            },
            ..Default::default()
        };

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: "Dialog Demo".into(),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: s.last_action.clone(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::Row {
                    style: row,
                    children: vec![
                        Widget::Button {
                            text: "Abrir confirmação",
                            on_press: Msg::OpenConfirm,
                            style: btn(170.0),
                            color: None,
                            variant: ButtonVariant::Primary,
                        },
                        Widget::Button {
                            text: "Abrir exclusão",
                            on_press: Msg::OpenDelete,
                            style: btn(150.0),
                            color: None,
                            variant: ButtonVariant::Ghost,
                        },
                        Widget::Button {
                            text: "Abrir info",
                            on_press: Msg::OpenInfo,
                            style: btn(120.0),
                            color: None,
                            variant: ButtonVariant::Ghost,
                        },
                    ],
                },
                Widget::Dialog {
                    id: 401,
                    visible: s.confirm_open,
                    title: "Salvar alterações",
                    message: "Deseja persistir as alterações realizadas neste formulário?",
                    confirm_label: "Salvar",
                    cancel_label: "Cancelar",
                    on_confirm: Msg::ConfirmSave,
                    on_cancel: Msg::CloseAll,
                    position: DialogPosition::Center,
                    style: Style {
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::percent(1.0),
                        },
                        ..Default::default()
                    },
                    on_dismiss: None,
                    child: Box::new(Widget::Spacer {
                        style: Style::default(),
                    }),
                },
                Widget::Dialog {
                    id: 402,
                    visible: s.delete_open,
                    title: "Excluir item",
                    message: "Esta ação é destrutiva e não poderá ser desfeita.",
                    confirm_label: "Excluir",
                    cancel_label: "Voltar",
                    on_confirm: Msg::ConfirmDelete,
                    on_cancel: Msg::CloseAll,
                    position: DialogPosition::Bottom,
                    style: Style {
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::percent(1.0),
                        },
                        ..Default::default()
                    },
                    on_dismiss: None,
                    child: Box::new(Widget::Spacer {
                        style: Style::default(),
                    }),
                },
                Widget::Dialog {
                    id: 403,
                    visible: s.info_open,
                    title: "Informação",
                    message: "Este diálogo é somente informativo e pode ser fechado sem efeito colateral.",
                    confirm_label: "Ok",
                    cancel_label: "Fechar",
                    on_confirm: Msg::CloseAll,
                    on_cancel: Msg::CloseAll,
                    position: DialogPosition::Top,
                    style: Style {
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::percent(1.0),
                        },
                        ..Default::default()
                    },
                    on_dismiss: None,
                    child: Box::new(Widget::Spacer {
                        style: Style::default(),
                    }),
                },
            ],
        }
    }

    fn update(s: &mut DialogDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::OpenConfirm => {
                s.confirm_open = true;
                s.delete_open = false;
                s.info_open = false;
            }
            Msg::OpenDelete => {
                s.confirm_open = false;
                s.delete_open = true;
                s.info_open = false;
            }
            Msg::OpenInfo => {
                s.confirm_open = false;
                s.delete_open = false;
                s.info_open = true;
            }
            Msg::CloseAll => {
                s.confirm_open = false;
                s.delete_open = false;
                s.info_open = false;
                s.last_action = "Diálogo fechado.".into();
            }
            Msg::ConfirmSave => {
                s.confirm_open = false;
                s.last_action = "Alterações salvas com sucesso.".into();
            }
            Msg::ConfirmDelete => {
                s.delete_open = false;
                s.last_action = "Item removido com sucesso.".into();
            }
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<DialogDemo>::run();
}
