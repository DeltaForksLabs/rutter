// ============================================================
// Rutter Framework - demos/popover_demo.rs
// Demo isolada de Widget::Popover.
// Exercita overlay generico ancorado, clique externo e conteudo interativo.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use taffy::prelude::*;

use rutter::{AppLogic, ButtonVariant, RutterRunner, Theme, Widget};

pub struct PopoverDemoState {
    pub open: bool,
    pub selected: &'static str,
    pub last_action: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Toggle,
    Close,
    Pick(&'static str),
    Apply,
}

pub struct PopoverDemo;

impl AppLogic for PopoverDemo {
    type State = PopoverDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        PopoverDemoState {
            open: false,
            selected: "Amber",
            last_action: "Open the popover and pick an option.".into(),
        }
    }

    fn view<'a>(s: &'a mut PopoverDemoState) -> Widget<'a, Msg> {
        let root = Style {
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::FlexStart),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(32.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(16.0),
            },
            ..Default::default()
        };

        let anchor_style = Style {
            size: Size {
                width: Dimension::length(190.0),
                height: Dimension::length(40.0),
            },
            ..Default::default()
        };

        let popup_style = Style {
            size: Size {
                width: Dimension::length(320.0),
                height: Dimension::length(190.0),
            },
            ..Default::default()
        };

        let popup_content_style = Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            padding: Rect::length(16.0),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(10.0),
            },
            ..Default::default()
        };

        let row_style = Style {
            flex_direction: FlexDirection::Row,
            gap: Size {
                width: LengthPercentage::length(10.0),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        };

        let option_button = |label: &'static str| Widget::Button {
            text: label,
            on_press: Msg::Pick(label),
            style: Style {
                size: Size {
                    width: Dimension::length(88.0),
                    height: Dimension::length(36.0),
                },
                ..Default::default()
            },
            color: None,
            variant: ButtonVariant::Ghost,
        };

        let anchor = Widget::Button {
            text: if s.open {
                "Close popover"
            } else {
                "Open popover"
            },
            on_press: Msg::Toggle,
            style: anchor_style.clone(),
            color: None,
            variant: ButtonVariant::Primary,
        };

        let content = Widget::Column {
            style: popup_content_style,
            children: vec![
                Widget::Text {
                    content: "Generic dropdown content".into(),
                    color: None,
                    size: 16.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: format!("Selected palette: {}", s.selected),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::Row {
                    style: row_style,
                    children: vec![
                        option_button("Amber"),
                        option_button("Cyan"),
                        option_button("Graphite"),
                    ],
                },
                Widget::Button {
                    text: "Apply selection",
                    on_press: Msg::Apply,
                    style: Style {
                        size: Size {
                            width: Dimension::length(160.0),
                            height: Dimension::length(36.0),
                        },
                        ..Default::default()
                    },
                    color: None,
                    variant: ButtonVariant::Primary,
                },
            ],
        };

        Widget::Column {
            style: root,
            children: vec![
                Widget::Text {
                    content: "Popover Demo".into(),
                    color: None,
                    size: 22.0,
                    style: Style::default(),
                },
                Widget::Text {
                    content: "The floating panel is measured by Taffy and clamped to the viewport."
                        .into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::popover(
                    s.open,
                    anchor,
                    content,
                    Some(Msg::Close),
                    anchor_style,
                    popup_style,
                )
                .with_id(910),
                Widget::Text {
                    content: s.last_action.clone(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
            ],
        }
    }

    fn update(s: &mut PopoverDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::Toggle => s.open = !s.open,
            Msg::Close => s.open = false,
            Msg::Pick(value) => {
                s.selected = value;
                s.last_action = format!("Picked {value} from the popover.");
            }
            Msg::Apply => {
                s.open = false;
                s.last_action = format!("Applied {}.", s.selected);
            }
        }
    }

    fn theme() -> Theme {
        Theme::dark()
    }
}

pub fn run() {
    RutterRunner::<PopoverDemo>::run();
}
