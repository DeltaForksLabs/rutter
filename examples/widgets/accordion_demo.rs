// ============================================================
// Rutter Framework — demos/accordion_demo.rs
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

#[derive(Default)]
pub struct AccordionDemoState {
    pub item_a_open: bool,
    pub item_b_open: bool,
    pub item_c_open: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ToggleA,
    ToggleB,
    ToggleC,
    ExpandAll,
    CollapseAll,
}

pub struct AccordionDemo;

impl AppLogic for AccordionDemo {
    type State = AccordionDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        AccordionDemoState {
            item_a_open: true,
            item_b_open: false,
            item_c_open: false,
        }
    }

    fn view<'a>(s: &'a mut AccordionDemoState) -> Widget<'a, Msg> {
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
                height: LengthPercentage::length(14.0),
            },
            ..Default::default()
        };

        let row = Style {
            flex_direction: FlexDirection::Row,
            gap: Size {
                width: LengthPercentage::length(10.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };
        let btn = |w: f32| Style {
            size: Size {
                width: Dimension::length(w),
                height: Dimension::length(38.0),
            },
            ..Default::default()
        };
        let item_style = Style {
            size: Size {
                width: Dimension::length(560.0),
                height: Dimension::auto(),
            },
            ..Default::default()
        };

        let content_style = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            padding: Rect {
                top: LengthPercentage::length(16.0),
                bottom: LengthPercentage::length(16.0),
                left: LengthPercentage::length(24.0),
                right: LengthPercentage::length(24.0),
            },
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(8.0),
            },
            ..Default::default()
        };

        Widget::Column {
            style: root,
            children: vec![
                Widget::Text {
                    content: "Accordion Demo".into(),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
                Widget::Row {
                    style: row,
                    children: vec![
                        Widget::Button {
                            text: "Expandir tudo",
                            on_press: Msg::ExpandAll,
                            style: btn(150.0),
                            color: None,
                            variant: ButtonVariant::Primary,
                        },
                        Widget::Button {
                            text: "Recolher tudo",
                            on_press: Msg::CollapseAll,
                            style: btn(150.0),
                            color: None,
                            variant: ButtonVariant::Ghost,
                        },
                    ],
                },
                Widget::Accordion {
                    id: 501,
                    title: "Configurações gerais",
                    expanded: s.item_a_open,
                    on_toggle: Msg::ToggleA,
                    style: item_style.clone(),
                    child: Box::new(Widget::Container {
                        style: content_style.clone(),
                        color: Some(skia_safe::Color::from_argb(15, 255, 255, 255)),
                        radius: 8.0,
                        child: Box::new(Widget::Text {
                            content: "• Tema: Escuro\n• Idioma: pt-BR\n• Autosave: habilitado"
                                .into(),
                            color: Some(skia_safe::Color::from_rgb(200, 200, 200)),
                            size: 14.0,
                            style: Style::default(),
                        }),
                    }),
                },
                Widget::Accordion {
                    id: 502,
                    title: "Rede e sincronização",
                    expanded: s.item_b_open,
                    on_toggle: Msg::ToggleB,
                    style: item_style.clone(),
                    child: Box::new(Widget::Container {
                        style: content_style.clone(),
                        color: Some(skia_safe::Color::from_argb(15, 255, 255, 255)),
                        radius: 8.0,
                        child: Box::new(Widget::Text {
                            content: "• Endpoint: https://api.local\n• Timeout: 5s\n• Retry: 3"
                                .into(),
                            color: Some(skia_safe::Color::from_rgb(200, 200, 200)),
                            size: 14.0,
                            style: Style::default(),
                        }),
                    }),
                },
                Widget::Accordion {
                    id: 503,
                    title: "Diagnóstico",
                    expanded: s.item_c_open,
                    on_toggle: Msg::ToggleC,
                    style: item_style,
                    child: Box::new(Widget::Container {
                        style: content_style,
                        color: Some(skia_safe::Color::from_argb(15, 255, 255, 255)),
                        radius: 8.0,
                        child: Box::new(Widget::Text {
                            content: format!(
                                "• Painel A: {}\n• Painel B: {}\n• Painel C: {}",
                                if s.item_a_open { "aberto" } else { "fechado" },
                                if s.item_b_open { "aberto" } else { "fechado" },
                                if s.item_c_open { "aberto" } else { "fechado" }
                            ),
                            color: Some(skia_safe::Color::from_rgb(200, 200, 200)),
                            size: 14.0,
                            style: Style::default(),
                        }),
                    }),
                },
            ],
        }
    }

    fn update(s: &mut AccordionDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::ToggleA => s.item_a_open = !s.item_a_open,
            Msg::ToggleB => s.item_b_open = !s.item_b_open,
            Msg::ToggleC => s.item_c_open = !s.item_c_open,
            Msg::ExpandAll => {
                s.item_a_open = true;
                s.item_b_open = true;
                s.item_c_open = true;
            }
            Msg::CollapseAll => {
                s.item_a_open = false;
                s.item_b_open = false;
                s.item_c_open = false;
            }
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<AccordionDemo>::run();
}
