// ============================================================
// Rutter Framework — demos/search_bar_demo.rs
// Demonstrates the integrated SearchBar suggestions popup in
// both matching modes: the built-in fuzzy ranker and an
// application-supplied custom scorer.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use taffy::prelude::*;

use rutter::search::SearchScoreFn;
use rutter::{
    AppLogic, ButtonVariant, RutterRunner, SearchLabels, SearchMatcher, SearchSuggestions, Theme,
    Widget,
};

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const MOVIES: &[&str] = &[
    "Matrix",
    "Interestelar",
    "O Senhor dos Anéis",
    "Vingadores",
    "Clube da Luta",
    "A Origem",
    "Coringa",
    "Pulp Fiction",
    "Forrest Gump",
    "Star Wars",
    "De Volta para o Futuro",
];

const FRAMEWORKS: &[&str] = &[
    "Rutter Framework",
    "Rocket Rails",
    "Sable Stack",
    "Nimbus Kit",
    "Orbit Tools",
];

/// Custom-mode example: matches the INITIAL LETTERS of each word, so "rf"
/// finds "Rutter Framework" and "ot" finds "Orbit Tools".
fn acronym_score(query: &str, item: &str) -> Option<u32> {
    let wanted: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    if wanted.is_empty() {
        return None;
    }
    let initials: Vec<char> = item
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let mut cursor = 0;
    for &letter in &wanted {
        let found = initials[cursor..].iter().position(|&c| c == letter)?;
        cursor += found + 1;
    }
    // Exact full-acronym hits outrank partial ones.
    let covers_full_acronym = wanted.len() == initials.len() && cursor == initials.len();
    Some(if covers_full_acronym { 100 } else { 40 })
}

const ACRONYM_MATCHER: SearchMatcher = SearchMatcher::Custom(acronym_score as SearchScoreFn);

#[derive(Default)]
pub struct SearchBarDemoState {
    pub theme: ExampleTheme,
    pub fuzzy_query: String,
    pub custom_query: String,
    pub last_picked: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    FuzzyQueryChanged(String),
    CustomQueryChanged(String),
    PickMovie(usize),
    PickFramework(usize),
    ClearSearches,
}

pub struct SearchBarDemo;

impl AppLogic for SearchBarDemo {
    type State = SearchBarDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        SearchBarDemoState {
            status: "Digite em um dos campos; Enter seleciona o item destacado.".into(),
            ..Default::default()
        }
    }

    fn view<'a>(s: &'a mut SearchBarDemoState) -> Widget<'a, Msg> {
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

        let field = |width: f32| responsive_width(width, Dimension::length(44.0));

        let movie_suggestions =
            SearchSuggestions::new(MOVIES, SearchMatcher::Fuzzy, 6, Some(Msg::PickMovie))
                .expect("demo suggestions use a valid configuration");
        let framework_suggestions =
            SearchSuggestions::new(FRAMEWORKS, ACRONYM_MATCHER, 5, Some(Msg::PickFramework))
                .expect("demo suggestions use a valid configuration")
                .with_labels(SearchLabels::PORTUGUESE);

        super::layout::page("Search", Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: "SearchBar com sugestões integradas".into(),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
                super::layout::section(vec![
                Widget::Text {
                    content: "Modo fuzzy (padrão): ignora acentos e maiúsculas, ranqueia por relevância.".into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::search_bar_with_suggestions(
                    Msg::FuzzyQueryChanged,
                    None,
                    None,
                    Some(Msg::ClearSearches),
                    "Buscar por filmes...",
                    movie_suggestions,
                    field(560.0),
                ),
                ]),
                super::layout::section(vec![
                Widget::Text {
                    content: "Modo personalizado: casa as iniciais de cada palavra (ex.: \"rf\" → Rutter Framework).".into(),
                    color: None,
                    size: 13.0,
                    style: Style::default(),
                },
                Widget::search_bar_with_suggestions(
                    Msg::CustomQueryChanged,
                    None,
                    None,
                    Some(Msg::ClearSearches),
                    "Buscar frameworks pelas iniciais...",
                    framework_suggestions,
                    field(560.0),
                ),
                ]),
                Widget::Button {
                    text: "Limpar buscas",
                    on_press: Msg::ClearSearches,
                    style: responsive_width(140.0, Dimension::length(38.0)),
                    color: None,
                    variant: ButtonVariant::Ghost,
                },
                Widget::Container {
                    style: Style {
                        padding: Rect::length(spacing::LG),
                        ..responsive_width(560.0, Dimension::auto())
                    },
                    color: Some(skia_safe::Color::from_argb(32, 255, 255, 255)),
                    radius: 10.0,
                    child: Box::new(Widget::Text {
                        content: format!(
                            "Consulta fuzzy: \"{}\"\nConsulta custom: \"{}\"\nÚltima seleção: {}\nStatus: {}",
                            s.fuzzy_query,
                            s.custom_query,
                            s.last_picked.as_deref().unwrap_or("—"),
                            s.status
                        ),
                        color: None,
                        size: 13.0,
                        style: Style::default(),
                    }),
                },
            ],
        })
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        apply_search_demo_message(state, message);
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn apply_search_demo_message(s: &mut SearchBarDemoState, msg: Msg) {
    match msg {
        Msg::ThemeChanged(theme) => s.theme = theme,
        Msg::FuzzyQueryChanged(value) => {
            s.fuzzy_query = value;
            s.status = "Editando a busca fuzzy...".into();
        }
        Msg::CustomQueryChanged(value) => {
            s.custom_query = value;
            s.status = "Editando a busca custom...".into();
        }
        Msg::PickMovie(index) => match MOVIES.get(index) {
            Some(movie) => {
                s.last_picked = Some((*movie).to_string());
                s.status = format!("Filme selecionado (índice original {index}).");
            }
            None => s.status = format!("Índice inválido {index}."),
        },
        Msg::PickFramework(index) => match FRAMEWORKS.get(index) {
            Some(framework) => {
                s.last_picked = Some((*framework).to_string());
                s.status = format!("Framework selecionado (índice original {index}).");
            }
            None => s.status = format!("Índice inválido {index}."),
        },
        Msg::ClearSearches => {
            s.fuzzy_query.clear();
            s.custom_query.clear();
            s.last_picked = None;
            s.status = "Buscas limpas.".into();
        }
    }
}

pub fn run() {
    RutterRunner::<SearchBarDemo>::run();
}

#[cfg(test)]
mod tests {
    // Copyright (c) DeltaForks Labs
    // Licensed under the MIT License OR Apache 2.0.

    use super::{ACRONYM_MATCHER, MOVIES, Msg, SearchBarDemoState, apply_search_demo_message};

    #[test]
    fn fuzzy_query_updates_are_reflected_in_state() {
        let mut state = SearchBarDemoState::default();

        apply_search_demo_message(&mut state, Msg::FuzzyQueryChanged("cor".into()));

        assert_eq!(state.fuzzy_query, "cor");
        assert!(state.status.contains("fuzzy"));
    }

    #[test]
    fn picking_uses_the_original_slice_index() {
        let mut state = SearchBarDemoState::default();

        // "O Senhor dos Anéis" lives at index 2 regardless of ranking position.
        apply_search_demo_message(&mut state, Msg::PickMovie(2));

        assert_eq!(state.last_picked.as_deref(), Some("O Senhor dos Anéis"));
        assert!(state.status.contains("índice original 2"));

        apply_search_demo_message(&mut state, Msg::PickMovie(999));

        assert_eq!(state.last_picked.as_deref(), Some("O Senhor dos Anéis"));
        assert!(state.status.contains("Índice inválido 999"));
    }

    #[test]
    fn custom_mode_picks_frameworks_by_acronym_index() {
        let mut state = SearchBarDemoState::default();

        apply_search_demo_message(&mut state, Msg::PickFramework(0));

        assert_eq!(state.last_picked.as_deref(), Some("Rutter Framework"));
    }

    #[test]
    fn clearing_resets_both_queries_and_selection() {
        let mut state = SearchBarDemoState {
            fuzzy_query: "matrix".into(),
            custom_query: "rf".into(),
            last_picked: Some(MOVIES[0].to_string()),
            ..Default::default()
        };

        apply_search_demo_message(&mut state, Msg::ClearSearches);

        assert!(state.fuzzy_query.is_empty());
        assert!(state.custom_query.is_empty());
        assert_eq!(state.last_picked, None);
        assert_eq!(state.status, "Buscas limpas.");
    }

    #[test]
    fn acronym_matcher_ranks_full_acronyms_above_partial() {
        assert_eq!(ACRONYM_MATCHER.score("sw", "Star Wars"), Some(100));
        // "rf" walks R→F inside "Rutter Framework" and consumes every initial.
        assert_eq!(ACRONYM_MATCHER.score("rf", "Rutter Framework"), Some(100));
        // A single letter only covers part of the acronym.
        assert_eq!(ACRONYM_MATCHER.score("s", "Star Wars"), Some(40));
        assert_eq!(ACRONYM_MATCHER.score("w", "Star Wars"), Some(40));
        assert_eq!(ACRONYM_MATCHER.score("zz", "Star Wars"), None);
    }
}

#[cfg(test)]
mod page_tests {
    use super::SearchBarDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<SearchBarDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<SearchBarDemo>();
    }
}
