// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Matching engine for search suggestions: a built-in fuzzy ranker plus a
//! pluggable custom scorer.
//!
//! Both modes answer the same question — "how well does `item` match
//! `query`?" — through [`SearchMatcher::score`], returning `None` for a
//! non-match and a deterministic score otherwise so results can be ranked
//! uniformly regardless of the active mode.
//!
//! ```
//! use rutter::search::{SearchMatcher, filter_ranked};
//!
//! let matcher = SearchMatcher::Fuzzy;
//! // Start-of-item bonus (15) plus one consecutive run link (10).
//! assert_eq!(matcher.score("mtrx", "Matrix"), Some(25));
//!
//! // The same ranking works over whole slices and keeps original indices.
//! let ranked = filter_ranked(&["Matrix", "Coringa"], "mat", matcher, 5);
//! assert_eq!(ranked.first().map(|m| m.index), Some(0));
//! ```

/// Scorer signature for [`SearchMatcher::Custom`].
///
/// Receives the raw (unfolded) query and item; returns `None` when the item
/// does not match or a deterministic score when it does. Higher scores rank
/// first, exactly like the fuzzy mode.
pub type SearchScoreFn = fn(&str, &str) -> Option<u32>;

/// Matching strategy for search suggestions.
#[derive(Debug, Clone, Copy)]
pub enum SearchMatcher {
    /// Built-in case- and accent-insensitive ranked subsequence match.
    ///
    /// Score bonuses: exact folded substring +50, match at item start +15 or
    /// a word boundary +12, and +10 per character continuing a consecutive
    /// run.
    Fuzzy,
    /// Application-supplied scorer; see [`SearchScoreFn`] for the contract.
    Custom(SearchScoreFn),
}

impl SearchMatcher {
    /// Scores `item` against `query`, or returns `None` on a non-match.
    ///
    /// An empty (or whitespace-only) query never matches so callers can pick
    /// their own browse behavior for that state.
    pub fn score(self, query: &str, item: &str) -> Option<u32> {
        if query.trim().is_empty() {
            return None;
        }
        match self {
            Self::Fuzzy => fuzzy_score(query.trim(), item),
            Self::Custom(matcher) => matcher(query, item),
        }
    }
}

/// A scored suggestion keeping its original position in the source slice.
///
/// `index` is the position in the items slice passed to [`filter_ranked`],
/// NOT the rank position; callbacks receive it so applications can map a
/// selection back to their own data regardless of filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    pub index: usize,
    pub score: u32,
}

/// Ranks `items` against `query` using `matcher`.
///
/// Returns at most `max_results` matches ordered by descending score, then by
/// original index. A whitespace-only query yields the first `max_results`
/// items in slice order with score 0 (browse mode).
///
/// ```
/// use rutter::search::{SearchMatcher, filter_ranked};
///
/// let matches = filter_ranked(
///     &["O Senhor dos Anéis", "Star Wars", "Senhor dos Aneis"],
///     "senhor",
///     SearchMatcher::Fuzzy,
///     10,
/// );
/// assert_eq!(matches.len(), 2);
/// // The title starting with the query wins: start-of-item bonus stacks on
/// // top of the shared substring and consecutive-run bonuses.
/// assert_eq!(matches[0].index, 2);
/// assert!(matches[0].score > matches[1].score);
/// ```
pub fn filter_ranked(
    items: &[&str],
    query: &str,
    matcher: SearchMatcher,
    max_results: usize,
) -> Vec<SearchMatch> {
    if query.trim().is_empty() {
        return items
            .iter()
            .take(max_results)
            .copied()
            .enumerate()
            .map(|(index, _)| SearchMatch { index, score: 0 })
            .collect();
    }
    let mut matches: Vec<SearchMatch> = items
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, item)| {
            Some(SearchMatch {
                index,
                score: matcher.score(query, item)?,
            })
        })
        .collect();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.index.cmp(&right.index))
    });
    matches.truncate(max_results);
    matches
}

/// Case- and accent-insensitive fuzzy score implementing the bonuses
/// documented on [`SearchMatcher::Fuzzy`].
fn fuzzy_score(query: &str, item: &str) -> Option<u32> {
    let query: Vec<char> = query.chars().map(fold_char).collect();
    if query.is_empty() {
        return None;
    }
    let item: Vec<char> = item.chars().map(fold_char).collect();
    let (positions, is_substring) = match_positions(&query, &item)?;
    Some(score_positions(&positions, &item, is_substring))
}

fn match_positions(needle: &[char], haystack: &[char]) -> Option<(Vec<usize>, bool)> {
    if let Some(start) = haystack
        .windows(needle.len())
        .position(|window| window == needle)
    {
        return Some(((start..start + needle.len()).collect(), true));
    }
    Some((subsequence_positions(needle, haystack)?, false))
}

fn score_positions(positions: &[usize], item: &[char], is_substring: bool) -> u32 {
    let mut score = if is_substring { 50 } else { 0 };
    for (step, &position) in positions.iter().enumerate() {
        let at_start = position == 0;
        let at_boundary = position > 0 && !item[position - 1].is_alphanumeric();
        if at_start {
            score += 15;
        } else if at_boundary {
            score += 12;
        }
        if step > 0 && position == positions[step - 1] + 1 {
            score += 10;
        }
    }
    score
}

/// Returns the position of every `needle` char inside `haystack` forming an
/// in-order (not necessarily contiguous) match.
fn subsequence_positions(needle: &[char], haystack: &[char]) -> Option<Vec<usize>> {
    let mut positions = Vec::with_capacity(needle.len());
    let mut cursor = 0;
    for &wanted in needle {
        let found = haystack[cursor..]
            .iter()
            .position(|&candidate| candidate == wanted)
            .map(|offset| cursor + offset)?;
        positions.push(found);
        cursor = found + 1;
    }
    Some(positions)
}

/// Lowercases and maps the common Latin accents used by Portuguese and
/// neighboring orthographies to their ASCII base letter. Unmapped characters
/// pass through unchanged.
fn fold_char(character: char) -> char {
    let lowered = character.to_lowercase().next().unwrap_or(character);
    match lowered {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        'š' => 's',
        'ž' => 'z',
        'œ' => 'o',
        'æ' => 'a',
        'ß' => 's',
        other => other,
    }
}
