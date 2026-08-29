// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! User-facing strings for the search suggestions popup.
//!
//! Follows the family convention of static label structs with an English
//! default (see `widgets::calendar::labels` and `widgets::time::labels`);
//! applications override them for localization while accessibility labels
//! stay caller-provided on the widget itself.

/// Labels shown by the integrated suggestions popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLabels<'a> {
    /// Single row rendered when the query matches nothing.
    pub no_results: &'a str,
}

impl Default for SearchLabels<'static> {
    fn default() -> Self {
        Self::ENGLISH
    }
}

impl SearchLabels<'static> {
    /// Built-in English labels used when no override is supplied.
    pub const ENGLISH: Self = Self {
        no_results: "No matches",
    };

    /// Built-in Portuguese labels for localized applications.
    pub const PORTUGUESE: Self = Self {
        no_results: "Nenhum resultado",
    };
}
