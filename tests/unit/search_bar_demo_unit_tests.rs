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
