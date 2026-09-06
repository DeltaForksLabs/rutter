// ============================================================
// Rutter Framework — demos/table_of_contents_demo.rs
// Semantic headings are discovered automatically by TableOfContents.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{AppLogic, HeadingLevel, RutterRunner, Theme, Widget};
use taffy::prelude::*;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

#[derive(Default)]
pub struct TableOfContentsDemoState {
    pub theme: ExampleTheme,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
}

pub struct TableOfContentsDemo;

impl AppLogic for TableOfContentsDemo {
    type State = TableOfContentsDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TableOfContentsDemoState::default()
    }

    fn view<'a>(state: &'a mut TableOfContentsDemoState) -> Widget<'a, Msg> {
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
                Widget::table_of_contents("Conteúdo", documentation(), document_viewport_style())
                    .with_id(320),
            ],
        }
    }

    fn update(state: &mut TableOfContentsDemoState, message: Msg, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
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
        padding: Rect::length(24.0_f32),
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(16.0),
        },
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
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(14.0),
        },
        ..Style::default()
    }
}

fn documentation_sections<'a>() -> Vec<Widget<'a, Msg>> {
    [
        (HeadingLevel::H1, "Visão geral", "O sumário é gerado a partir dos headings semânticos do documento."),
        (HeadingLevel::H2, "Instalação", "Cada item navega para a posição calculada pelo layout, sem IDs manuais por seção."),
        (HeadingLevel::H2, "Navegação", "Os links podem receber foco pelo teclado e expõem papéis de heading e link para tecnologias assistivas."),
        (HeadingLevel::H3, "Rolagem suave", "A animação é cancelada ao usar a roda do mouse ou arrastar a barra de rolagem."),
    ]
    .into_iter()
    .flat_map(|(level, title, text)| section(level, title, text))
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
    use super::{HeadingLevel, Widget, documentation};

    #[test]
    fn documentation_contains_semantic_headings() {
        let Widget::Column { children, .. } = documentation() else {
            panic!("expected the documentation to be a column");
        };

        assert!(matches!(
            children.first(),
            Some(Widget::Heading {
                level: HeadingLevel::H1,
                ..
            })
        ));
    }
}
