// ============================================================
// Rutter Framework — demos/controls_demo.rs
// Demo isolada de Checkbox, Switch, Radio e Select.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::{AppLogic, RutterRunner, Theme, Widget};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const LANGUAGES: &[&str] = &["Rust", "Python", "TypeScript", "Go", "Zig", "C++"];

#[derive(Default)]
pub struct ControlsDemoState {
    pub remember: bool,
    pub theme: ExampleTheme,
    pub newsletter: bool,
    pub selected_lang: usize,
    pub radio_sel: u8,
}

#[derive(Debug, Clone)]
pub enum Msg {
    RememberToggled(bool),
    DarkModeToggled(bool),
    ThemeChanged(ExampleTheme),
    NewsletterToggled(bool),
    LanguageChanged(usize),
    RadioA,
    RadioB,
    RadioC,
}

pub struct ControlsDemo;

fn controls_section<'a>(children: Vec<Widget<'a, Msg>>) -> Widget<'a, Msg> {
    let mut section = super::layout::section(children);
    if let Widget::Column { style, .. } = &mut section {
        // Compact controls intentionally paint focus outside their bodies.
        // Own that safe area here, inside the page's scrolling viewport.
        style.padding = Rect::length(spacing::SM);
    }
    section
}

impl AppLogic for ControlsDemo {
    type State = ControlsDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ControlsDemoState::default()
    }

    fn view<'a>(s: &'a mut ControlsDemoState) -> Widget<'a, Msg> {
        let col = Style {
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
        let item_row = Style {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::FlexStart),
            gap: Size::length(spacing::MD),
            ..responsive_width(320.0, Dimension::length(32.0))
        };
        let checkbox_row = Style {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: Some(AlignItems::Center),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::auto(),
            },
            max_size: Size {
                width: Dimension::length(384.0),
                height: Dimension::auto(),
            },
            gap: Size {
                width: LengthPercentage::length(spacing::XL),
                height: LengthPercentage::length(spacing::MD),
            },
            ..Default::default()
        };
        let check_s = responsive_width(180.0, Dimension::length(28.0));
        let switch_s = Style {
            size: Size {
                width: Dimension::length(50.0),
                height: Dimension::length(28.0),
            },
            ..Default::default()
        };
        let select_s = responsive_width(320.0, Dimension::length(44.0));
        let radio_s = responsive_width(160.0, Dimension::length(28.0));

        super::layout::page(
            "Controls",
            Widget::Column {
                style: col,
                children: vec![
                    example_theme_selector(s.theme, Msg::ThemeChanged),
                    Widget::Text {
                        content: "Controls".into(),
                        color: None,
                        size: 24.0,
                        style: Style::default(),
                    },
                    controls_section(vec![
                        // ── Checkboxes ───────────────────────────────────────
                        Widget::heading(rutter::HeadingLevel::H2, "Checkboxes", Style::default()),
                        Widget::Row {
                            style: checkbox_row,
                            children: vec![
                                Widget::Checkbox {
                                    checked: s.remember,
                                    on_change: Msg::RememberToggled,
                                    label: "Lembrar de mim",
                                    style: check_s.clone(),
                                },
                                Widget::Checkbox {
                                    checked: s.newsletter,
                                    on_change: Msg::NewsletterToggled,
                                    label: "Receber newsletter",
                                    style: check_s.clone(),
                                },
                            ],
                        },
                        // ── Switch ───────────────────────────────────────────
                    ]),
                    controls_section(vec![
                        Widget::heading(rutter::HeadingLevel::H2, "Switch", Style::default()),
                        Widget::Row {
                            style: item_row,
                            children: vec![
                                Widget::Text {
                                    content: "Modo escuro".into(),
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
                        // ── Radio ────────────────────────────────────────────
                    ]),
                    controls_section(vec![
                        Widget::heading(rutter::HeadingLevel::H2, "Radio", Style::default()),
                        Widget::Radio {
                            selected: s.radio_sel == 0,
                            on_select: || Msg::RadioA,
                            label: "Opção A",
                            style: radio_s.clone(),
                        },
                        Widget::Radio {
                            selected: s.radio_sel == 1,
                            on_select: || Msg::RadioB,
                            label: "Opção B",
                            style: radio_s.clone(),
                        },
                        Widget::Radio {
                            selected: s.radio_sel == 2,
                            on_select: || Msg::RadioC,
                            label: "Opção C",
                            style: radio_s.clone(),
                        },
                        // ── Select ───────────────────────────────────────────
                    ]),
                    controls_section(vec![
                        Widget::heading(
                            rutter::HeadingLevel::H2,
                            "Select (dropdown)",
                            Style::default(),
                        ),
                        Widget::Select {
                            id: 10,
                            options: LANGUAGES,
                            selected_index: s.selected_lang,
                            on_change: Msg::LanguageChanged,
                            style: select_s,
                            label: "Linguagem favorita",
                            placeholder: "Escolha...",
                        },
                        // ── Resumo ───────────────────────────────────────────
                    ]),
                    Widget::Text {
                        content: format!(
                            "Lembrar={} | Newsletter={} | DarkMode={} | Radio={} | Lang={}",
                            s.remember,
                            s.newsletter,
                            s.theme == ExampleTheme::Dark,
                            s.radio_sel,
                            LANGUAGES.get(s.selected_lang).copied().unwrap_or("-")
                        ),
                        color: None,
                        size: 11.0,
                        style: Style::default(),
                    },
                ],
            },
        )
    }

    fn update(s: &mut ControlsDemoState, msg: Msg, _: &mut Clipboard) {
        match msg {
            Msg::RememberToggled(v) => s.remember = v,
            Msg::DarkModeToggled(v) => {
                s.theme = if v {
                    ExampleTheme::Dark
                } else {
                    ExampleTheme::Light
                };
            }
            Msg::ThemeChanged(theme) => s.theme = theme,
            Msg::NewsletterToggled(v) => s.newsletter = v,
            Msg::LanguageChanged(i) => s.selected_lang = i,
            Msg::RadioA => s.radio_sel = 0,
            Msg::RadioB => s.radio_sel = 1,
            Msg::RadioC => s.radio_sel = 2,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

pub fn run() {
    RutterRunner::<ControlsDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rutter::engine::widget_state::{ScrollState, WidgetState};
    use rutter::layout::{build_taffy_tree, compute_layout};
    use rutter::render::{
        RichTextRenderer,
        hit_test::{HitResult, hit_test},
    };
    use skia_safe::{Point, Rect as Bounds};
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    use winit::dpi::PhysicalSize;

    #[test]
    fn sections_reserve_focus_space_inside_the_viewport_and_keep_scrolled_hits() {
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut state = ControlsDemoState::default();
        let view = ControlsDemo::view(&mut state);
        for width in [320, 469, 800] {
            for height in [260, 700] {
                let mut tree = TaffyTree::new();
                let root = build_taffy_tree(&mut tree, &view, fonts.clone(), &HashMap::new());
                compute_layout(
                    &mut tree,
                    root,
                    PhysicalSize::new(width, height),
                    fonts.clone(),
                    &RichTextRenderer::default(),
                );
                let Widget::Column { children, .. } = &view else {
                    unreachable!()
                };
                let index = children
                    .iter()
                    .position(|child| matches!(child, Widget::ScrollView { .. }))
                    .unwrap();
                let viewport_node = tree.children(root).unwrap()[index];
                let viewport = tree.layout(viewport_node).unwrap();
                let document_node = tree.children(viewport_node).unwrap()[0];
                let document = tree.layout(document_node).unwrap();
                let document_children = super::super::layout::document_children(&view);
                let nodes = tree.children(document_node).unwrap();
                let mut controls = Vec::new();
                for (widget, node) in document_children.iter().zip(nodes) {
                    collect_control_bounds(
                        widget,
                        &tree,
                        node,
                        Point::new(viewport.location.x, viewport.location.y),
                        &mut controls,
                    );
                }
                assert_eq!(controls.len(), 7);
                for (bounds, select) in controls {
                    assert!(bounds.left >= viewport.location.x + spacing::SM);
                    assert!(
                        bounds.right
                            <= viewport.location.x + viewport.size.width - spacing::SM + 1.0
                    );
                    assert!(bounds.width() > 0.0 && bounds.height() >= 28.0);
                    let mut scroll = ScrollState {
                        offset_y: 0.0,
                        content_height: document.size.height,
                        viewport_h: viewport.size.height,
                    };
                    scroll.scroll_by(
                        (bounds.center_y() - viewport.location.y - viewport.size.height / 2.0)
                            .max(0.0),
                    );
                    let states = HashMap::from([(
                        super::super::layout::PAGE_SCROLL_ID,
                        WidgetState::Scroll(scroll.clone()),
                    )]);
                    let mouse = Point::new(bounds.left + 9.0, bounds.center_y() - scroll.offset_y);
                    let hit = hit_test(&view, &tree, root, mouse, Point::default(), &states);
                    if select {
                        assert!(matches!(hit, Some(HitResult::SelectToggle(10))));
                    } else {
                        assert!(matches!(hit, Some(HitResult::Message { .. })));
                    }
                }
            }
        }
    }

    fn collect_control_bounds(
        widget: &Widget<'_, Msg>,
        tree: &TaffyTree<rutter::layout::RutterContext>,
        node: NodeId,
        origin: Point,
        controls: &mut Vec<(Bounds, bool)>,
    ) {
        let layout = tree.layout(node).unwrap();
        let origin = Point::new(origin.x + layout.location.x, origin.y + layout.location.y);
        match widget {
            Widget::Checkbox { .. }
            | Widget::Radio { .. }
            | Widget::Switch { .. }
            | Widget::Select { .. } => {
                controls.push((
                    Bounds::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height),
                    matches!(widget, Widget::Select { .. }),
                ));
            }
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                for (child, node) in children.iter().zip(tree.children(node).unwrap()) {
                    collect_control_bounds(child, tree, node, origin, controls);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn checkbox_row_wraps_and_uses_bounded_fluid_widths() {
        let mut state = ControlsDemoState::default();
        let view = ControlsDemo::view(&mut state);
        let children = super::super::layout::document_children(&view);
        let (checkboxes, style) = checkbox_row(children)
            .expect("controls demo must contain one Row with only Checkbox children");

        assert_eq!(style.flex_wrap, FlexWrap::Wrap);
        assert_eq!(style.gap.width, LengthPercentage::length(24.0));
        assert_eq!(style.gap.height, LengthPercentage::length(12.0));
        assert_eq!(style.size.width, Dimension::percent(1.0));
        assert_eq!(style.max_size.width, Dimension::length(384.0));
        assert_eq!(checkboxes.len(), 2);
        for checkbox in checkboxes {
            let Widget::Checkbox { style, .. } = checkbox else {
                unreachable!()
            };
            assert_eq!(style.size.width, Dimension::percent(1.0));
            assert_eq!(style.max_size.width, Dimension::length(180.0));
        }
    }

    fn checkbox_row<'view, 'widget>(
        children: &'view [Widget<'widget, Msg>],
    ) -> Option<(&'view [Widget<'widget, Msg>], &'view Style)> {
        children.iter().find_map(|child| match child {
            Widget::Row { children, style }
                if children
                    .iter()
                    .all(|child| matches!(child, Widget::Checkbox { .. })) =>
            {
                Some((children.as_slice(), style))
            }
            Widget::Column { children, .. } => checkbox_row(children),
            _ => None,
        })
    }
}

#[cfg(test)]
mod page_tests {
    use super::ControlsDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<ControlsDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<ControlsDemo>();
    }
}
