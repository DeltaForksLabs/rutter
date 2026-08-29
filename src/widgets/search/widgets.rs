// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Validated configuration for integrated [`Widget::SearchBar`] suggestions.
//!
//! ```
//! use rutter::search::{SearchMatcher, SearchSuggestions};
//!
//! enum Msg {
//!     Picked(usize),
//! }
//!
//! let zones = ["UTC", "America/Sao_Paulo"];
//! let suggestions = SearchSuggestions::new(
//!     &zones,
//!     SearchMatcher::Fuzzy,
//!     6,
//!     Some(Msg::Picked),
//! )
//! .unwrap();
//! assert_eq!(suggestions.max_results(), 6);
//! ```

use std::fmt;

use super::labels::SearchLabels;
use super::matcher::SearchMatcher;

/// Error returned when search suggestion configuration is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchConfigError {
    /// `max_results` must be at least one row so the popup can show either a
    /// match or the explicit no-results label.
    MaxResults {
        /// The offending value.
        value: usize,
    },
}

impl fmt::Display for SearchConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxResults { value } => write!(
                formatter,
                "invalid search suggestions max_results {value}; expected at least 1"
            ),
        }
    }
}

impl std::error::Error for SearchConfigError {}

/// Suggestion source attached to a [`Widget::SearchBar`](crate::Widget::SearchBar).
///
/// Items are borrowed from application state each frame and matched against
/// the field's current text. Source positions are callback indices and
/// accessibility identities, so keep the order stable while an exposed popup
/// may still have queued assistive-technology actions.
#[derive(Debug, Clone, Copy)]
pub struct SearchSuggestions<'a, Msg> {
    pub(crate) items: &'a [&'a str],
    pub(crate) matcher: SearchMatcher,
    pub(crate) max_results: usize,
    pub(crate) labels: SearchLabels<'a>,
    pub(crate) on_select: Option<fn(usize) -> Msg>,
}

impl<'a, Msg> SearchSuggestions<'a, Msg> {
    /// Validates and builds the suggestion configuration.
    ///
    /// # Errors
    /// Returns [`SearchConfigError::MaxResults`] when `max_results` is zero.
    ///
    /// ```
    /// use rutter::search::{SearchConfigError, SearchMatcher, SearchSuggestions};
    ///
    /// fn picked(index: usize) -> &'static str {
    ///     if index == 0 { "first" } else { "other" }
    /// }
    ///
    /// let items = ["Alpha", "Beta"];
    /// assert!(matches!(
    ///     SearchSuggestions::new(&items, SearchMatcher::Fuzzy, 0, Some(picked)),
    ///     Err(SearchConfigError::MaxResults { value: 0 }),
    /// ));
    /// ```
    pub fn new(
        items: &'a [&'a str],
        matcher: SearchMatcher,
        max_results: usize,
        on_select: Option<fn(usize) -> Msg>,
    ) -> Result<Self, SearchConfigError> {
        if max_results == 0 {
            return Err(SearchConfigError::MaxResults { value: max_results });
        }
        Ok(Self {
            items,
            matcher,
            max_results,
            labels: SearchLabels::default(),
            on_select,
        })
    }

    /// Overrides the popup strings; see [`SearchLabels::PORTUGUESE`].
    pub fn with_labels(mut self, labels: SearchLabels<'a>) -> Self {
        self.labels = labels;
        self
    }

    /// Maximum number of rows rendered by the popup.
    pub fn max_results(&self) -> usize {
        self.max_results
    }

    /// Matching strategy used to rank `items`.
    pub fn matcher(&self) -> SearchMatcher {
        self.matcher
    }
}
