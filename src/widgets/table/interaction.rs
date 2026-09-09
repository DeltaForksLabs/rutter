// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::model::{TableColumn, TableRow};
use super::{TableColumnKey, TableRowKey};

/// Controlled direction for one currently sorted column.
/// ```rust
/// use rutter::table::TableSortDirection;
/// assert_ne!(TableSortDirection::Ascending, TableSortDirection::Descending);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSortDirection {
    /// Values increase from the first displayed row.
    Ascending,
    /// Values decrease from the first displayed row.
    Descending,
}

/// Controlled sort descriptor keyed independently of the column's position.
/// ```rust
/// use rutter::table::{TableColumnKey, TableSort, TableSortDirection};
/// let sort = TableSort::new(TableColumnKey::new(2), TableSortDirection::Ascending);
/// assert_eq!(sort.column().get(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSort {
    column: TableColumnKey,
    direction: TableSortDirection,
}

impl TableSort {
    /// Creates a controlled sort descriptor for a stable column key.
    /// ```rust
    /// use rutter::table::{TableColumnKey, TableSort, TableSortDirection};
    /// let sort = TableSort::new(TableColumnKey::new(1), TableSortDirection::Descending);
    /// assert_eq!(sort.direction(), TableSortDirection::Descending);
    /// ```
    pub const fn new(column: TableColumnKey, direction: TableSortDirection) -> Self {
        Self { column, direction }
    }

    /// Returns the stable key of the sorted column.
    /// ```rust
    /// # use rutter::table::{TableColumnKey, TableSort, TableSortDirection};
    /// let sort = TableSort::new(TableColumnKey::new(4), TableSortDirection::Ascending);
    /// assert_eq!(sort.column().get(), 4);
    /// ```
    pub const fn column(self) -> TableColumnKey {
        self.column
    }

    /// Returns the controlled direction displayed for this sort.
    /// ```rust
    /// # use rutter::table::{TableColumnKey, TableSort, TableSortDirection};
    /// let sort = TableSort::new(TableColumnKey::new(4), TableSortDirection::Ascending);
    /// assert_eq!(sort.direction(), TableSortDirection::Ascending);
    /// ```
    pub const fn direction(self) -> TableSortDirection {
        self.direction
    }
}

/// Controlled sorting state and the message callback for sort requests.
/// ```rust
/// use rutter::table::TableSorting;
/// let sorting = TableSorting::new(None, std::convert::identity);
/// assert_eq!(sorting.current(), None);
/// ```
#[derive(Clone, Copy)]
pub struct TableSorting<Msg> {
    current: Option<TableSort>,
    on_sort: fn(TableSort) -> Msg,
}

impl<Msg> TableSorting<Msg> {
    /// Binds the current controlled value to a sort-request callback.
    /// ```rust
    /// use rutter::table::TableSorting;
    /// let sorting = TableSorting::new(None, std::convert::identity);
    /// assert!(sorting.current().is_none());
    /// ```
    pub const fn new(current: Option<TableSort>, on_sort: fn(TableSort) -> Msg) -> Self {
        Self { current, on_sort }
    }

    /// Returns the sort value currently supplied by the application.
    /// ```rust
    /// # use rutter::table::TableSorting;
    /// let sorting = TableSorting::new(None, std::convert::identity);
    /// assert_eq!(sorting.current(), None);
    /// ```
    pub const fn current(&self) -> Option<TableSort> {
        self.current
    }

    /// Computes the next request, toggling the active column or starting ascending.
    /// ```rust
    /// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableSortDirection, TableSorting};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(80.0).unwrap()).with_sortable(true);
    /// let next = TableSorting::new(None, std::convert::identity).next_sort(&column).unwrap();
    /// assert_eq!(next.direction(), TableSortDirection::Ascending);
    /// ```
    pub fn next_sort(&self, column: &TableColumn<'_>) -> Option<TableSort> {
        if !column.is_sortable() {
            return None;
        }
        let direction = match self.current {
            Some(sort) if sort.column == column.key() => opposite(sort.direction),
            _ => TableSortDirection::Ascending,
        };
        Some(TableSort::new(column.key(), direction))
    }

    /// Converts a valid sortable-column activation into an application message.
    /// ```rust
    /// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableSorting};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(80.0).unwrap()).with_sortable(true);
    /// let message = TableSorting::new(None, std::convert::identity).message_for_activation(&column);
    /// assert!(message.is_some());
    /// ```
    pub fn message_for_activation(&self, column: &TableColumn<'_>) -> Option<Msg> {
        self.next_sort(column).map(self.on_sort)
    }

    pub(crate) const fn on_sort_callback(&self) -> fn(TableSort) -> Msg {
        self.on_sort
    }
}

fn opposite(direction: TableSortDirection) -> TableSortDirection {
    match direction {
        TableSortDirection::Ascending => TableSortDirection::Descending,
        TableSortDirection::Descending => TableSortDirection::Ascending,
    }
}

/// Controlled row-selection policy and application callback.
/// ```rust
/// use rutter::table::TableSelection;
/// assert!(matches!(TableSelection::<()>::none(), TableSelection::None));
/// ```
#[derive(Clone, Copy)]
pub enum TableSelection<'a, Msg> {
    /// Rows are not selectable.
    None,
    /// At most one row is selected.
    Single {
        selected: Option<TableRowKey>,
        on_selection: fn(Option<TableRowKey>) -> Msg,
    },
    /// Any number of rows may be selected.
    Multiple {
        selected: &'a [TableRowKey],
        on_selection: fn(Vec<TableRowKey>) -> Msg,
    },
}

impl<'a, Msg> TableSelection<'a, Msg> {
    /// Creates a policy that ignores all row-selection gestures.
    /// ```rust
    /// use rutter::table::TableSelection;
    /// assert!(matches!(TableSelection::<()>::none(), TableSelection::None));
    /// ```
    pub const fn none() -> Self {
        Self::None
    }

    /// Creates controlled single selection with an optional current row key.
    /// ```rust
    /// use rutter::table::{TableRowKey, TableSelection};
    /// let selection = TableSelection::single(Some(TableRowKey::new(1)), std::convert::identity);
    /// assert!(matches!(selection, TableSelection::Single { .. }));
    /// ```
    pub const fn single(
        selected: Option<TableRowKey>,
        on_selection: fn(Option<TableRowKey>) -> Msg,
    ) -> Self {
        Self::Single {
            selected,
            on_selection,
        }
    }

    /// Creates controlled multiple selection from borrowed stable row keys.
    /// ```rust
    /// use rutter::table::{TableRowKey, TableSelection};
    /// let keys = [TableRowKey::new(1)];
    /// let selection = TableSelection::multiple(&keys, std::convert::identity);
    /// assert!(matches!(selection, TableSelection::Multiple { .. }));
    /// ```
    pub const fn multiple(
        selected: &'a [TableRowKey],
        on_selection: fn(Vec<TableRowKey>) -> Msg,
    ) -> Self {
        Self::Multiple {
            selected,
            on_selection,
        }
    }

    /// Resolves controlled keys to unique rows in current display order.
    /// ```rust
    /// use rutter::table::{TableRow, TableRowKey, TableSelection};
    /// let rows = [TableRow::new(TableRowKey::new(1), Vec::new())];
    /// let selection = TableSelection::single(Some(TableRowKey::new(1)), std::convert::identity);
    /// assert_eq!(selection.selected_rows(&rows).len(), 1);
    /// ```
    pub fn selected_rows<'rows, 'content>(
        &self,
        rows: &'rows [TableRow<'content>],
    ) -> Vec<&'rows TableRow<'content>> {
        let selected = self.ordered_selected_keys(rows);
        rows.iter()
            .filter(|row| selected.contains(&row.key()))
            .collect()
    }

    /// Emits the replacement selection for a plain click on a displayed row.
    /// ```rust
    /// use rutter::table::{TableRow, TableRowKey, TableSelection};
    /// let rows = [TableRow::new(TableRowKey::new(1), Vec::new())];
    /// let message = TableSelection::single(None, std::convert::identity).message_for_plain_click(&rows, TableRowKey::new(1));
    /// assert_eq!(message, Some(Some(TableRowKey::new(1))));
    /// ```
    pub fn message_for_plain_click(
        &self,
        rows: &[TableRow<'_>],
        clicked: TableRowKey,
    ) -> Option<Msg> {
        if !contains_row(rows, clicked) {
            return None;
        }
        match self {
            Self::None => None,
            Self::Single { on_selection, .. } => Some(on_selection(Some(clicked))),
            Self::Multiple { on_selection, .. } => Some(on_selection(vec![clicked])),
        }
    }

    /// Emits a Ctrl/Command-style toggle while preserving display order.
    /// ```rust
    /// use rutter::table::{TableRow, TableRowKey, TableSelection};
    /// let rows = [TableRow::new(TableRowKey::new(1), Vec::new())];
    /// let selected = [TableRowKey::new(1)];
    /// let message = TableSelection::multiple(&selected, std::convert::identity).message_for_toggle(&rows, TableRowKey::new(1));
    /// assert_eq!(message, Some(Vec::new()));
    /// ```
    pub fn message_for_toggle(&self, rows: &[TableRow<'_>], clicked: TableRowKey) -> Option<Msg> {
        if !contains_row(rows, clicked) {
            return None;
        }
        match self {
            Self::None => None,
            Self::Single {
                selected,
                on_selection,
            } => {
                let current = (*selected).filter(|key| contains_row(rows, *key));
                Some(on_selection((current != Some(clicked)).then_some(clicked)))
            }
            Self::Multiple { on_selection, .. } => {
                Some(on_selection(self.toggled_keys(rows, clicked)))
            }
        }
    }

    /// Emits a contiguous Shift-style range from an anchor to a displayed row.
    /// ```rust
    /// use rutter::table::{TableRow, TableRowKey, TableSelection};
    /// let rows = [TableRow::new(TableRowKey::new(1), Vec::new()), TableRow::new(TableRowKey::new(2), Vec::new())];
    /// let message = TableSelection::multiple(&[], std::convert::identity).message_for_range(&rows, Some(TableRowKey::new(1)), TableRowKey::new(2));
    /// assert_eq!(message.unwrap().len(), 2);
    /// ```
    pub fn message_for_range(
        &self,
        rows: &[TableRow<'_>],
        anchor: Option<TableRowKey>,
        clicked: TableRowKey,
    ) -> Option<Msg> {
        let clicked_index = row_index(rows, clicked)?;
        match self {
            Self::None => None,
            Self::Single { on_selection, .. } => Some(on_selection(Some(clicked))),
            Self::Multiple { on_selection, .. } => {
                let anchor_index = anchor.and_then(|key| row_index(rows, key));
                let keys =
                    contiguous_keys(rows, anchor_index.unwrap_or(clicked_index), clicked_index);
                Some(on_selection(keys))
            }
        }
    }

    fn ordered_selected_keys(&self, rows: &[TableRow<'_>]) -> Vec<TableRowKey> {
        match self {
            Self::None => Vec::new(),
            Self::Single { selected, .. } => selected
                .filter(|key| contains_row(rows, *key))
                .into_iter()
                .collect(),
            Self::Multiple { selected, .. } => ordered_known_keys(rows, selected),
        }
    }

    fn toggled_keys(&self, rows: &[TableRow<'_>], clicked: TableRowKey) -> Vec<TableRowKey> {
        let mut selected = self.ordered_selected_keys(rows);
        if selected.contains(&clicked) {
            selected.retain(|key| *key != clicked);
        } else {
            selected.push(clicked);
        }
        ordered_known_keys(rows, &selected)
    }
}

fn contains_row(rows: &[TableRow<'_>], key: TableRowKey) -> bool {
    rows.iter().any(|row| row.key() == key)
}

fn row_index(rows: &[TableRow<'_>], key: TableRowKey) -> Option<usize> {
    rows.iter().position(|row| row.key() == key)
}

fn ordered_known_keys(rows: &[TableRow<'_>], keys: &[TableRowKey]) -> Vec<TableRowKey> {
    let mut ordered = Vec::with_capacity(keys.len().min(rows.len()));
    for row in rows {
        if keys.contains(&row.key()) && !ordered.contains(&row.key()) {
            ordered.push(row.key());
        }
    }
    ordered
}

fn contiguous_keys(
    rows: &[TableRow<'_>],
    anchor_index: usize,
    clicked_index: usize,
) -> Vec<TableRowKey> {
    let start = anchor_index.min(clicked_index);
    let end = anchor_index.max(clicked_index);
    rows[start..=end].iter().map(TableRow::key).collect()
}

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;
