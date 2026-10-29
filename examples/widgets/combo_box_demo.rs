// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Run `cargo run -- combo_box` to browse, filter and commit an editable draft.

use super::{
    layout::{flow, page, responsive_width, section},
    theme_selector::{ExampleTheme, example_theme_selector},
};
use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::combo_box::ComboBox;
use rutter::spacing;
use rutter::{AppLogic, ButtonVariant, HeadingLevel, InputState, RutterRunner, Theme, Widget};
use taffy::prelude::*;

const PLACES: &[&str] = &[
    "Lisboa",
    "São Paulo",
    "Montréal",
    "München",
    "東京 — Tokyo",
    "القاهرة — Cairo",
    "Wellington",
    "Reykjavík",
    "Buenos Aires",
    "Cape Town",
    "San Francisco — International departures and connecting services",
];

#[derive(Default)]
pub struct ComboBoxDemoState {
    pub theme: ExampleTheme,
    pub query: String,
    pub selected: Option<usize>,
}
#[derive(Debug, Clone)]
pub enum Msg {
    Theme(ExampleTheme),
    Draft(String),
    Pick(usize),
    Clear,
    Reset,
}
pub struct ComboBoxDemo;

fn text<'a>(content: impl Into<String>) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        color: None,
        size: 14.0,
        style: responsive_width(640.0, Dimension::auto()),
    }
}
fn action<'a>(label: &'static str, message: Msg) -> Widget<'a, Msg> {
    Widget::Button {
        text: label,
        on_press: message,
        color: None,
        variant: ButtonVariant::Ghost,
        style: responsive_width(140.0, Dimension::length(40.0)),
    }
}
fn reduce(state: &mut ComboBoxDemoState, message: Msg) {
    match message {
        Msg::Theme(theme) => state.theme = theme,
        Msg::Draft(query) => {
            state.query = query;
            state.selected = None;
        }
        Msg::Pick(index) => {
            if let Some(place) = PLACES.get(index) {
                state.query = (*place).into();
                state.selected = Some(index);
            }
        }
        Msg::Clear => {
            state.query.clear();
            state.selected = None;
        }
        Msg::Reset => {
            state.query = PLACES[1].into();
            state.selected = Some(1);
        }
    }
}
impl AppLogic for ComboBoxDemo {
    type State = ComboBoxDemoState;
    type Message = Msg;
    fn new(_: &mut FontSystem) -> Self::State {
        ComboBoxDemoState::default()
    }
    fn view(state: &mut Self::State) -> Widget<'_, Msg> {
        let field = || responsive_width(640.0, Dimension::length(44.0));
        let mut options = ComboBox::new(PLACES, Msg::Pick)
            .with_label("Destination / Destino / 行き先")
            .with_placeholder("Type to filter, or use the dropdown arrow")
            .with_selected_index(state.selected);
        if !state.query.is_empty() && state.selected.is_none() {
            options = options
                .with_state(InputState::Error)
                .with_error("Draft only — choose an option to confirm a destination.");
        }
        page(
            "ComboBox",
            Widget::Column {
                style: Style {
                    gap: Size::length(spacing::XL),
                    ..Style::default()
                },
                children: vec![
                    example_theme_selector(state.theme, Msg::Theme),
                    Widget::heading(HeadingLevel::H1, "Editable ComboBox", Style::default()),
                    text(
                        "Browse the full source with an empty draft, or type to filter. Selection copies the canonical label into the draft.",
                    ),
                    section(vec![
                        Widget::heading(HeadingLevel::H2, "Choose a destination", Style::default()),
                        Widget::combo_box(100, &state.query, Msg::Draft, options, field()),
                        text(
                            "Keyboard: Up/Down opens and highlights; Enter confirms; Escape closes. Tab and Shift+Tab move focus without committing. Home/End move the caret.",
                        ),
                        text(match state.selected {
                            Some(index) => {
                                format!("Committed source index {index}: {}", PLACES[index])
                            }
                            None => "No committed selection; unmatched drafts are allowed.".into(),
                        }),
                        Widget::Row {
                            style: flow(),
                            children: vec![
                                action("Clear draft", Msg::Clear),
                                action("Reset to São Paulo", Msg::Reset),
                            ],
                        },
                    ]),
                    section(vec![
                        Widget::heading(HeadingLevel::H2, "Unavailable field", Style::default()),
                        Widget::combo_box(
                            101,
                            PLACES[10],
                            Msg::Draft,
                            ComboBox::new(PLACES, Msg::Pick)
                                .with_label("Connection (disabled)")
                                .with_selected_index(Some(10)),
                            field(),
                        )
                        .enabled(false),
                        text(
                            "Long and localized labels retain their full accessible names. Disabled fields retain their value but expose no editing or selection actions.",
                        ),
                    ]),
                ],
            },
        )
    }
    fn update(state: &mut Self::State, message: Msg, _: &mut Clipboard) {
        reduce(state, message);
    }
    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}
pub fn run() {
    RutterRunner::<ComboBoxDemo>::run();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_selection_clear_reset_and_unmatched_drafts() {
        let mut state = ComboBoxDemoState::default();
        reduce(&mut state, Msg::Pick(10));
        assert_eq!(state.query, PLACES[10]);
        assert_eq!(state.selected, Some(10));
        reduce(&mut state, Msg::Draft("unmatched".into()));
        assert_eq!(state.selected, None);
        assert_eq!(state.query, "unmatched");
        reduce(&mut state, Msg::Clear);
        assert!(state.query.is_empty());
        reduce(&mut state, Msg::Reset);
        assert_eq!(state.query, PLACES[1]);
        assert_eq!(state.selected, Some(1));
    }
}

#[cfg(test)]
mod page_tests {
    use super::ComboBoxDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<ComboBoxDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<ComboBoxDemo>();
    }
}
