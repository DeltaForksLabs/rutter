// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{TableColumn, TableRow, TableRowKey, TableSelection, TableSort, TableSorting};
use crate::ChangeCallback;

/// Owned sorting callback with controlled state. Clones share captured context.
/// Captures follow [`ChangeCallback`]'s `Send + Sync + 'static` contract;
/// retained clones keep their original context after a view rebuild.
///
/// ```rust
/// use rutter::table::{TableOptions, TableSortingWith};
/// let identity = String::from("inventory");
/// let options = TableOptions::default().sorting_with(
///     TableSortingWith::new(None, move |sort| (identity.clone(), sort)));
/// assert!(options.effective_sorting().is_some());
/// ```
pub struct TableSortingWith<Msg> {
    current: Option<TableSort>,
    pub(crate) on_sort: ChangeCallback<TableSort, Msg>,
}

impl<Msg> Clone for TableSortingWith<Msg> {
    fn clone(&self) -> Self {
        Self {
            current: self.current,
            on_sort: self.on_sort.clone(),
        }
    }
}

impl<Msg> TableSortingWith<Msg> {
    /// Owns a callback whose captures need not implement `Clone`.
    pub fn new(
        current: Option<TableSort>,
        on_sort: impl Fn(TableSort) -> Msg + Send + Sync + 'static,
    ) -> Self {
        Self {
            current,
            on_sort: ChangeCallback::new(on_sort),
        }
    }

    /// Returns the application's controlled sort descriptor.
    pub const fn current(&self) -> Option<TableSort> {
        self.current
    }

    /// Toggles the active sortable column, or starts another column ascending.
    pub fn next_sort(&self, column: &TableColumn<'_>) -> Option<TableSort> {
        TableSorting::new(self.current, std::convert::identity).next_sort(column)
    }

    /// Converts a sortable-column activation using the owned context.
    pub fn message_for_activation(&self, column: &TableColumn<'_>) -> Option<Msg> {
        self.next_sort(column).map(|sort| self.on_sort.call(sort))
    }
}

impl<Msg> From<TableSorting<Msg>> for TableSortingWith<Msg> {
    fn from(value: TableSorting<Msg>) -> Self {
        Self {
            current: value.current(),
            on_sort: ChangeCallback::from_function(value.on_sort_callback()),
        }
    }
}

/// Controlled selection with owned, shared callbacks and borrowed selected keys.
/// Captures follow [`ChangeCallback`]'s ownership contract. Cloning does not
/// refresh an old identity or generation snapshot. Requests preserve the supplied
/// row order; sorting and the resulting row order remain application controlled.
///
/// ```rust
/// use rutter::table::{TableOptions, TableSelectionWith};
/// let context = String::from("inventory");
/// let options = TableOptions::default().selection_with(
///     TableSelectionWith::single(None, move |key| (context.clone(), key)));
/// assert!(matches!(options.effective_selection(), TableSelectionWith::Single { .. }));
/// ```
pub enum TableSelectionWith<'a, Msg> {
    /// Selection is disabled.
    None,
    /// At most one controlled stable row key.
    Single {
        selected: Option<TableRowKey>,
        on_selection: ChangeCallback<Option<TableRowKey>, Msg>,
    },
    /// Any number of controlled stable row keys.
    Multiple {
        selected: &'a [TableRowKey],
        on_selection: ChangeCallback<Vec<TableRowKey>, Msg>,
    },
}

impl<Msg> Clone for TableSelectionWith<'_, Msg> {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Single {
                selected,
                on_selection,
            } => Self::Single {
                selected: *selected,
                on_selection: on_selection.clone(),
            },
            Self::Multiple {
                selected,
                on_selection,
            } => Self::Multiple {
                selected,
                on_selection: on_selection.clone(),
            },
        }
    }
}

enum SelectionRequest {
    Single(Option<TableRowKey>),
    Multiple(Vec<TableRowKey>),
}

impl<'a, Msg> TableSelectionWith<'a, Msg> {
    /// Creates a policy that ignores row-selection gestures.
    pub const fn none() -> Self {
        Self::None
    }

    /// Owns a single-selection callback with its current controlled value.
    pub fn single(
        selected: Option<TableRowKey>,
        on_selection: impl Fn(Option<TableRowKey>) -> Msg + Send + Sync + 'static,
    ) -> Self {
        Self::Single {
            selected,
            on_selection: ChangeCallback::new(on_selection),
        }
    }

    /// Owns a multiple-selection callback with borrowed controlled keys.
    pub fn multiple(
        selected: &'a [TableRowKey],
        on_selection: impl Fn(Vec<TableRowKey>) -> Msg + Send + Sync + 'static,
    ) -> Self {
        Self::Multiple {
            selected,
            on_selection: ChangeCallback::new(on_selection),
        }
    }

    // Reuse the legacy algorithms, including unknown-key filtering and source order.
    fn requests(&self) -> TableSelection<'_, SelectionRequest> {
        match self {
            Self::None => TableSelection::None,
            Self::Single { selected, .. } => {
                TableSelection::single(*selected, SelectionRequest::Single)
            }
            Self::Multiple { selected, .. } => {
                TableSelection::multiple(selected, SelectionRequest::Multiple)
            }
        }
    }

    fn message(&self, request: SelectionRequest) -> Msg {
        match (self, request) {
            (Self::Single { on_selection, .. }, SelectionRequest::Single(key)) => {
                on_selection.call(key)
            }
            (Self::Multiple { on_selection, .. }, SelectionRequest::Multiple(keys)) => {
                on_selection.call(keys)
            }
            _ => unreachable!("selection request matches its policy"),
        }
    }

    /// Resolves selected known keys in the supplied row order.
    pub fn selected_rows<'rows, 'content>(
        &self,
        rows: &'rows [TableRow<'content>],
    ) -> Vec<&'rows TableRow<'content>> {
        self.requests().selected_rows(rows)
    }

    /// Requests replacement selection for a known row.
    pub fn message_for_plain_click(
        &self,
        rows: &[TableRow<'_>],
        clicked: TableRowKey,
    ) -> Option<Msg> {
        self.requests()
            .message_for_plain_click(rows, clicked)
            .map(|request| self.message(request))
    }

    /// Requests a Ctrl/Command toggle, preserving supplied row order.
    pub fn message_for_toggle(&self, rows: &[TableRow<'_>], clicked: TableRowKey) -> Option<Msg> {
        self.requests()
            .message_for_toggle(rows, clicked)
            .map(|request| self.message(request))
    }

    /// Requests a contiguous Shift range, falling back to the clicked row.
    pub fn message_for_range(
        &self,
        rows: &[TableRow<'_>],
        anchor: Option<TableRowKey>,
        clicked: TableRowKey,
    ) -> Option<Msg> {
        self.requests()
            .message_for_range(rows, anchor, clicked)
            .map(|request| self.message(request))
    }
}

impl<'a, Msg> From<TableSelection<'a, Msg>> for TableSelectionWith<'a, Msg> {
    fn from(value: TableSelection<'a, Msg>) -> Self {
        match value {
            TableSelection::None => Self::None,
            TableSelection::Single {
                selected,
                on_selection,
            } => Self::Single {
                selected,
                on_selection: ChangeCallback::from_function(on_selection),
            },
            TableSelection::Multiple {
                selected,
                on_selection,
            } => Self::Multiple {
                selected,
                on_selection: ChangeCallback::from_function(on_selection),
            },
        }
    }
}

#[cfg(test)]
#[path = "captured_tests.rs"]
mod tests;
