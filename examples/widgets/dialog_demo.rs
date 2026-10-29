// ============================================================
// Rutter Framework — demos/dialog_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, DialogPosition, RutterRunner, Theme, Widget};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

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
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(spacing::XL),
            gap: Size::length(spacing::LG),
            ..Default::default()
        };

        let row = Style {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            gap: Size::length(spacing::MD),
            ..responsive_width(480.0, Dimension::auto())
        };
        let btn = |w: f32| responsive_width(w, Dimension::length(40.0));

        super::layout::page(
            "Dialog",
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
            },
        )
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

#[cfg(test)]
mod page_tests {
    use super::DialogDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<DialogDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<DialogDemo>();
    }

    use super::super::layout::test_support::assert_overlay;
    use rutter::AppLogic;

    #[test]
    fn open_dialogs_paint_above_and_block_the_scroll_document() {
        use super::{DialogDemo, DialogDemoState, Msg};
        for index in 0..3 {
            let mut state = DialogDemoState {
                confirm_open: index == 0,
                delete_open: index == 1,
                info_open: index == 2,
                ..Default::default()
            };
            assert_overlay(&DialogDemo::view(&mut state), |message| {
                matches!(
                    message,
                    Msg::CloseAll | Msg::ConfirmSave | Msg::ConfirmDelete
                )
            });
        }
    }
}
