// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;
use std::num::NonZeroUsize;

/// Reports an invalid table-of-contents presentation configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableOfContentsConfigError {
    InvalidColumns { columns: usize },
}

impl fmt::Display for TableOfContentsConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidColumns { columns } => write!(
                formatter,
                "invalid table of contents column count {columns}, expected columns > 0"
            ),
        }
    }
}

impl std::error::Error for TableOfContentsConfigError {}

/// Configures a table-of-contents navigation list.
///
/// ```
/// use rutter::TableOfContentsOptions;
///
/// let options = TableOfContentsOptions::<()>::new(2).unwrap();
/// assert_eq!(options.columns(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct TableOfContentsOptions<Msg> {
    columns: NonZeroUsize,
    accordion: Option<TableOfContentsAccordion<Msg>>,
}

#[derive(Debug, Clone)]
pub(crate) struct TableOfContentsAccordion<Msg> {
    expanded: bool,
    on_toggle: Msg,
}

impl<Msg> TableOfContentsOptions<Msg> {
    /// Creates an inline navigation list with the requested number of columns.
    pub fn new(columns: usize) -> Result<Self, TableOfContentsConfigError> {
        let Some(columns) = NonZeroUsize::new(columns) else {
            return Err(TableOfContentsConfigError::InvalidColumns { columns });
        };
        Ok(Self {
            columns,
            accordion: None,
        })
    }

    /// Adds an accordion header that starts expanded.
    pub fn with_accordion(self, on_toggle: Msg) -> Self {
        self.with_accordion_state(true, on_toggle)
    }

    /// Adds an accordion header with a controlled expanded state.
    pub fn with_accordion_state(mut self, expanded: bool, on_toggle: Msg) -> Self {
        self.accordion = Some(TableOfContentsAccordion {
            expanded,
            on_toggle,
        });
        self
    }

    /// Returns the number of vertical navigation columns.
    pub const fn columns(&self) -> usize {
        self.columns.get()
    }

    /// Returns whether navigation links are wrapped in an accordion header.
    pub const fn is_accordion(&self) -> bool {
        self.accordion.is_some()
    }

    /// Returns whether navigation links are currently visible.
    pub const fn is_expanded(&self) -> bool {
        match &self.accordion {
            Some(accordion) => accordion.expanded,
            None => true,
        }
    }

    pub(crate) fn accordion(&self) -> Option<&TableOfContentsAccordion<Msg>> {
        self.accordion.as_ref()
    }
}

impl<Msg> Default for TableOfContentsOptions<Msg> {
    fn default() -> Self {
        Self {
            columns: NonZeroUsize::MIN,
            accordion: None,
        }
    }
}

impl<Msg> TableOfContentsAccordion<Msg> {
    pub(crate) const fn expanded(&self) -> bool {
        self.expanded
    }

    pub(crate) fn on_toggle(&self) -> &Msg {
        &self.on_toggle
    }
}

#[cfg(test)]
mod tests {
    use super::{TableOfContentsConfigError, TableOfContentsOptions};

    #[test]
    fn options_reject_zero_columns() {
        let result = TableOfContentsOptions::<()>::new(0);

        assert!(matches!(
            result,
            Err(TableOfContentsConfigError::InvalidColumns { columns: 0 })
        ));
    }

    #[test]
    fn accordion_options_start_expanded_by_default() {
        let options = TableOfContentsOptions::new(2).unwrap().with_accordion(());

        assert_eq!(options.columns(), 2);
        assert!(options.is_accordion());
        assert!(options.is_expanded());
    }

    #[test]
    fn accordion_options_accept_a_controlled_collapsed_state() {
        let options = TableOfContentsOptions::default().with_accordion_state(false, ());

        assert!(options.is_accordion());
        assert!(!options.is_expanded());
    }
}
