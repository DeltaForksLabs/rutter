// ============================================================
// Rutter Framework — demos/table_of_contents_demo.rs
// Semantic headings are discovered automatically by TableOfContents.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use rutter::{AppLogic, HeadingLevel, RutterRunner, TableOfContentsOptions, Theme, Widget};
use taffy::prelude::*;

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
        super::layout::page(
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
        )
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
    responsive_width(680.0, Dimension::length(420.0))
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
        gap: Size::length(spacing::XL),
        ..Style::default()
    }
}

fn documentation_sections<'a>() -> Vec<Widget<'a, Msg>> {
    [
        (HeadingLevel::H1, "Visão geral", "O sumário é gerado a partir dos headings semânticos do documento."),
        (HeadingLevel::H2, "Instalação", "Cada item navega para a posição calculada pelo layout, sem IDs manuais por seção."),
        (HeadingLevel::H3, "Navegação", "Os links podem receber foco pelo teclado e expõem papéis de heading e link para tecnologias assistivas."),
        (HeadingLevel::H4, "Rolagem suave", "A animação é cancelada ao usar a roda do mouse ou arrastar a barra de rolagem."),
        (HeadingLevel::H5, "Rolagem suave", "A animação é cancelada ao usar a roda do mouse ou arrastar a barra de rolagem."),
    ]
    .into_iter()
    .map(|(level, title, text)| super::layout::section(section(level, title, text).into()))
    .collect()
}

fn section<'a>(level: HeadingLevel, title: &str, text: &str) -> [Widget<'a, Msg>; 2] {
    [Widget::heading(level, title, Style::default()), body(text)]
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
    use rutter::AppLogic;

    use super::{
        HeadingLevel, TableOfContentsDemo, TableOfContentsDemoState, Widget, documentation,
    };

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
