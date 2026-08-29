// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::labels::SearchLabels;
use super::matcher::{SearchMatch, SearchMatcher, filter_ranked};
use super::widgets::{SearchConfigError, SearchSuggestions};

const MOVIES: &[&str] = &[
    "Matrix",
    "Interestelar",
    "O Senhor dos Anéis",
    "Star Wars",
    "Coringa",
];

#[test]
fn fuzzy_matches_accents_case_and_spacing() {
    let matcher = SearchMatcher::Fuzzy;
    assert!(matcher.score("matrix", "MATRIX").is_some());
    assert!(matcher.score("  matrix  ", "Matrix").is_some());
    assert!(matcher.score("coracao", "Coração").is_some());
    assert!(matcher.score("interestelar", "Ïntéréstélar").is_some());
    assert_eq!(matcher.score("xyz", MOVIES[0]), None);
}

#[test]
fn empty_query_never_matches_any_item() {
    assert_eq!(SearchMatcher::Fuzzy.score("", MOVIES[0]), None);
    assert_eq!(SearchMatcher::Fuzzy.score("   ", MOVIES[0]), None);
}

#[test]
fn substring_and_start_bonuses_rank_prefix_above_inner_match() {
    let matcher = SearchMatcher::Fuzzy;
    // "mat" is a start-of-item substring: +50 substring, +15 at start,
    // +20 for two consecutive-run links.
    assert_eq!(matcher.score("mat", "Matrix"), Some(85));
    // Inner substring misses only the start bonus.
    assert_eq!(matcher.score("tri", "Matrix"), Some(70));
}

#[test]
fn consecutive_links_accumulate_per_character() {
    let matcher = SearchMatcher::Fuzzy;
    // "mtrx": start bonus 15 plus one link between t→r.
    assert_eq!(matcher.score("mtrx", "Matrix"), Some(25));
}

#[test]
fn word_boundary_bonus_applies_after_separators() {
    let matcher = SearchMatcher::Fuzzy;
    // "wars" starts after a space: +50 substring, +12 word boundary and
    // +30 for the three consecutive-run links.
    let score = matcher.score("wars", "Star Wars").unwrap();
    assert_eq!(score, 92);
}

#[test]
fn substring_scoring_uses_the_contiguous_occurrence() {
    let score = SearchMatcher::Fuzzy.score("abc", "x a--abc").unwrap();

    assert_eq!(score, 82);
}

#[test]
fn filter_orders_by_score_then_original_index() {
    // Only "O Senhor dos Anéis" contains the full subsequence; its score is
    // +50 substring, +12 word boundary and +50 for five run links.
    let ranked = filter_ranked(MOVIES, "senhor", SearchMatcher::Fuzzy, 10);

    assert_eq!(
        ranked,
        vec![SearchMatch {
            index: 2,
            score: 112
        },]
    );
}

#[test]
fn filter_truncates_to_max_results() {
    let ranked = filter_ranked(MOVIES, "a", SearchMatcher::Fuzzy, 2);

    assert_eq!(ranked.len(), 2);
}

#[test]
fn empty_query_returns_browse_mode_in_slice_order() {
    let ranked = filter_ranked(MOVIES, "   ", SearchMatcher::Fuzzy, 3);

    assert_eq!(
        ranked,
        vec![
            SearchMatch { index: 0, score: 0 },
            SearchMatch { index: 1, score: 0 },
            SearchMatch { index: 2, score: 0 },
        ]
    );
}

#[test]
fn custom_matcher_controls_matching_entirely() {
    fn only_short_items(query: &str, item: &str) -> Option<u32> {
        (item.len() == query.len()).then_some(7)
    }

    let ranked = filter_ranked(MOVIES, "matrix", SearchMatcher::Custom(only_short_items), 5);

    assert_eq!(ranked, vec![SearchMatch { index: 0, score: 7 }]);
}

#[test]
fn suggestions_reject_zero_max_results() {
    fn picked(index: usize) -> &'static str {
        if index == 0 { "first" } else { "other" }
    }

    assert!(matches!(
        SearchSuggestions::new(MOVIES, SearchMatcher::Fuzzy, 0, Some(picked)),
        Err(SearchConfigError::MaxResults { value: 0 })
    ));

    let valid = SearchSuggestions::new(MOVIES, SearchMatcher::Fuzzy, 6, Some(picked)).unwrap();
    assert_eq!(valid.max_results(), 6);
    assert!(matches!(valid.matcher(), SearchMatcher::Fuzzy));
    assert_eq!(
        valid
            .with_labels(SearchLabels::PORTUGUESE)
            .labels
            .no_results,
        "Nenhum resultado"
    );
}
