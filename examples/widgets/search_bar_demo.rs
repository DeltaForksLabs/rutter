// ============================================================
// Rutter Framework — demos/search_bar_demo.rs
// Demonstrates the integrated SearchBar suggestions popup in
// both matching modes: the built-in fuzzy ranker and an
// application-supplied custom scorer.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
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
            padding: Rect::length(32.0_f32),
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(16.0),
            },
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

        Widget::Column {
            style: root,
            children: vec![
                example_theme_selector(s.theme, Msg::ThemeChanged),
                Widget::Text {
                    content: "SearchBar com sugestões integradas".into(),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
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
                Widget::Button {
                    text: "Limpar buscas",
                    on_press: Msg::ClearSearches,
                    style: responsive_width(140.0, Dimension::length(38.0)),
                    color: None,
                    variant: ButtonVariant::Ghost,
                },
                Widget::Container {
                    style: Style {
                        padding: Rect::length(16.0_f32),
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
        }
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
#[path = "../../tests/unit/search_bar_demo_unit_tests.rs"]
mod tests;
