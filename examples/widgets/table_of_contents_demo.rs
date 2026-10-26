// ============================================================
// Rutter Framework — demos/table_of_contents_demo.rs
// Semantic headings are discovered automatically by TableOfContents.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use rutter::{AppLogic, RutterRunner, TableOfContentsOptions, Theme, Widget};
use taffy::prelude::*;

#[path = "table_of_contents_article.rs"]
mod article;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

pub struct TableOfContentsDemoState {
    pub theme: ExampleTheme,
    pub contents_expanded: bool,
}

impl Default for TableOfContentsDemoState {
    fn default() -> Self {
        Self {
            theme: ExampleTheme::default(),
            contents_expanded: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    ToggleContents,
}

pub struct TableOfContentsDemo;

impl AppLogic for TableOfContentsDemo {
    type State = TableOfContentsDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TableOfContentsDemoState::default()
    }

    fn view<'a>(state: &'a mut TableOfContentsDemoState) -> Widget<'a, Msg> {
        let mut page = super::layout::page(
            "Table of contents",
            Widget::Column {
                style: page_style(),
                children: vec![
                example_theme_selector(state.theme, Msg::ThemeChanged),
                Widget::Text {
                    content:
                        "Table of Contents — clique em um título para rolar suavemente até a seção."
                            .into(),
                    style: Style::default(),
                    color: None,
                    size: 14.0,
                },
                Widget::table_of_contents_with_options(
                    "Nesta página",
                    documentation(),
                    document_viewport_style(),
                    TableOfContentsOptions::new(2)
                        .expect("table of contents demo uses 2 columns, expected columns > 0")
                        .with_accordion_state(state.contents_expanded, Msg::ToggleContents),
                )
                .with_id(320),
            ],
            },
        );
        // Opt this reading page into available-height layout without changing
        // the intrinsic-document policy used by the other examples. Min-height
        // still allows the navigation and a usable article to overflow together.
        if let Widget::Column { children, .. } = &mut page {
            for child in children {
                if let Widget::ScrollView { child, .. } = child
                    && let Widget::Column { style, .. } = child.as_mut()
                {
                    style.min_size.height = Dimension::percent(1.0);
                }
            }
        }
        page
    }

    fn update(state: &mut TableOfContentsDemoState, message: Msg, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
            Msg::ToggleContents => state.contents_expanded = !state.contents_expanded,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::percent(1.0),
        },
        padding: Rect::length(spacing::XL),
        gap: Size::length(spacing::LG),
        ..Style::default()
    }
}

fn document_viewport_style() -> Style {
    Style {
        // The shared page owns the XL gap above navigation; this section owns
        // navigation/document separation, independently of the theme's density.
        gap: Size::length(spacing::TWO_XL),
        flex_grow: 1.0,
        ..responsive_width(680.0, Dimension::auto())
    }
}

fn documentation<'a>() -> Widget<'a, Msg> {
    Widget::Column {
        style: documentation_style(),
        children: documentation_sections(),
    }
}

fn documentation_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        gap: Size::length(spacing::TWO_XL),
        flex_shrink: 0.0,
        // ScrollView paints its 8px track 2px inside the physical right edge,
        // in both text directions. Keep an SM gap before it; padding participates
        // in both text measurement and painting, including heading destinations.
        padding: Rect {
            right: LengthPercentage::length(rutter::layout::SCROLLBAR_W + 2.0 + spacing::SM),
            ..Rect::zero()
        },
        ..Style::default()
    }
}

fn documentation_sections<'a>() -> Vec<Widget<'a, Msg>> {
    let mut sections: Vec<_> = article::SECTIONS
        .iter()
        .map(|section| {
            let mut children = vec![Widget::heading(
                section.level,
                section.title,
                Style::default(),
            )];
            children.extend(section.paragraphs.iter().map(|paragraph| body(paragraph)));
            super::layout::section(children)
        })
        .collect();
    // Attribution is supporting text, not an extra document navigation entry.
    sections.push(super::layout::section(vec![body(article::ATTRIBUTION)]));
    sections
}

fn body<'a>(content: &str) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: Style::default(),
        color: None,
        size: 15.0,
    }
}

pub fn run() {
    RutterRunner::<TableOfContentsDemo>::run();
}

#[cfg(test)]
mod tests {
    use rutter::{AppLogic, HeadingLevel};

    use super::{TableOfContentsDemo, TableOfContentsDemoState, Widget, documentation};

    #[test]
    fn documentation_contains_semantic_headings() {
        let Widget::Column { children, .. } = documentation() else {
            panic!("expected the documentation to be a column");
        };

        assert!(matches!(
            match children.first().unwrap() {
                Widget::Column { children, .. } => children.first(),
                _ => panic!("documentation sections group headings with their paragraphs"),
            },
            Some(Widget::Heading {
                level: HeadingLevel::H1,
                ..
            })
        ));
    }

    #[test]
    fn demo_starts_with_an_expanded_two_column_navigation() {
        let mut state = TableOfContentsDemoState::default();
        let view = TableOfContentsDemo::view(&mut state);
        let children = super::super::layout::document_children(&view);
        let Some(Widget::TableOfContents { options, .. }) = children
            .iter()
            .find(|child| matches!(child, Widget::TableOfContents { id: 320, .. }))
        else {
            panic!("expected the table of contents in the scrollable page document");
        };

        assert_eq!(options.columns(), 2);
        assert!(options.is_accordion());
        assert!(options.is_expanded());
    }
}

#[cfg(test)]
#[path = "../../tests/unit/table_of_contents_demo_unit_tests.rs"]
mod article_tests;
