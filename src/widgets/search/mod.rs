// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Search suggestion support for the framework search bar: matching engine,
//! validated configuration, and popup labels.

pub(crate) const SEARCH_BAR_LEADING_TEXT_INSET: f32 = 4.0;

mod labels;
pub use labels::SearchLabels;

mod matcher;
pub use matcher::{SearchMatch, SearchMatcher, SearchScoreFn, filter_ranked};

mod widgets;
pub use widgets::{SearchConfigError, SearchSuggestions};

#[cfg(test)]
mod widget_tests;
