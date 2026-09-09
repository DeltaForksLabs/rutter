// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use crate::widgets::table::{
    TableCellTarget, TableColumnKey, TableModel, TableOptions, TableRowKey, TableSelection,
    TableSort, TableState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableInteractionModifiers {
    pub(crate) toggle: bool,
    pub(crate) range: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableNavigation {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableAccessibilityTarget {
    pub(crate) parent_id: u64,
    pub(crate) target: TableCellTarget,
}

#[derive(Debug, Clone)]
enum TableSelectionRuntime<Msg> {
    None,
    Single {
        selected: Option<TableRowKey>,
        on_selection: fn(Option<TableRowKey>) -> Msg,
    },
    Multiple {
        selected: Vec<TableRowKey>,
        on_selection: fn(Vec<TableRowKey>) -> Msg,
    },
}

#[derive(Debug, Clone, Copy)]
struct TableSortingRuntime<Msg> {
    current: Option<TableSort>,
    on_sort: fn(TableSort) -> Msg,
}

#[derive(Debug, Clone)]
pub(crate) struct TableRuntime<Msg> {
    pub(crate) columns: Vec<TableColumnKey>,
    pub(crate) rows: Vec<TableRowKey>,
    pub(crate) widths: Vec<f32>,
    pub(crate) row_height: f32,
    pub(crate) body_height: f32,
    sortable_columns: Vec<TableColumnKey>,
    selection: TableSelectionRuntime<Msg>,
    sorting: Option<TableSortingRuntime<Msg>>,
}

impl<Msg> TableRuntime<Msg> {
    pub(crate) fn new(
        model: &TableModel<'_>,
        options: &TableOptions<'_, Msg>,
        widths: Vec<f32>,
        body_height: f32,
    ) -> Self {
        Self {
            columns: model.columns().iter().map(|column| column.key()).collect(),
            rows: model.rows().iter().map(|row| row.key()).collect(),
            widths,
            row_height: options.metrics().row_height(),
            body_height,
            sortable_columns: model
                .columns()
                .iter()
                .filter(|column| column.is_sortable())
                .map(|column| column.key())
                .collect(),
            selection: selection_runtime(options.selection(), model),
            sorting: options.sorting().map(|sorting| TableSortingRuntime {
                current: sorting.current(),
                on_sort: sorting.on_sort_callback(),
            }),
        }
    }

    pub(crate) fn initial_target(&self) -> Option<TableCellTarget> {
        self.columns.first().copied().map(TableCellTarget::header)
    }

    pub(crate) fn message_for_target(
        &self,
        target: TableCellTarget,
        anchor: Option<TableRowKey>,
        modifiers: TableInteractionModifiers,
    ) -> Option<Msg> {
        match target.row {
            None => self.sort_message(target.column),
            Some(row) => self.selection_message(row, anchor, modifiers),
        }
    }

    pub(crate) fn move_target(
        &self,
        active: Option<TableCellTarget>,
        navigation: TableNavigation,
        rtl: bool,
        primary: bool,
    ) -> Option<TableCellTarget> {
        let current = active.or_else(|| self.initial_target())?;
        let (row, column) = self.target_indices(current)?;
        let next = self.next_indices(row, column, navigation, rtl, primary);
        self.target_at(next.0, next.1)
    }

    pub(crate) fn reveal_target(&self, state: &mut TableState, target: TableCellTarget) -> bool {
        let Some((row, column)) = self.target_indices(target) else {
            return false;
        };
        state.reveal_position(column, row, &self.widths, self.row_height);
        true
    }

    fn sort_message(&self, column: TableColumnKey) -> Option<Msg> {
        if !self.sortable_columns.contains(&column) {
            return None;
        }
        let sorting = self.sorting.as_ref()?;
        let direction = match sorting.current {
            Some(current) if current.column() == column => match current.direction() {
                crate::widgets::table::TableSortDirection::Ascending => {
                    crate::widgets::table::TableSortDirection::Descending
                }
                crate::widgets::table::TableSortDirection::Descending => {
                    crate::widgets::table::TableSortDirection::Ascending
                }
            },
            _ => crate::widgets::table::TableSortDirection::Ascending,
        };
        Some((sorting.on_sort)(TableSort::new(column, direction)))
    }

    fn selection_message(
        &self,
        row: TableRowKey,
        anchor: Option<TableRowKey>,
        modifiers: TableInteractionModifiers,
    ) -> Option<Msg> {
        if !self.rows.contains(&row) {
            return None;
        }
        match &self.selection {
            TableSelectionRuntime::None => None,
            TableSelectionRuntime::Single {
                selected,
                on_selection,
            } => {
                let next = if modifiers.toggle && *selected == Some(row) {
                    None
                } else {
                    Some(row)
                };
                Some(on_selection(next))
            }
            TableSelectionRuntime::Multiple {
                selected,
                on_selection,
            } => Some(on_selection(
                self.multiple_selection(selected, row, anchor, modifiers),
            )),
        }
    }

    fn multiple_selection(
        &self,
        selected: &[TableRowKey],
        row: TableRowKey,
        anchor: Option<TableRowKey>,
        modifiers: TableInteractionModifiers,
    ) -> Vec<TableRowKey> {
        if modifiers.range {
            return self.range_selection(anchor.unwrap_or(row), row);
        }
        if modifiers.toggle {
            return self.toggled_selection(selected, row);
        }
        vec![row]
    }

    fn toggled_selection(&self, selected: &[TableRowKey], row: TableRowKey) -> Vec<TableRowKey> {
        let mut next = selected
            .iter()
            .copied()
            .filter(|key| self.rows.contains(key) && *key != row)
            .collect::<Vec<_>>();
        if !selected.contains(&row) {
            next.push(row);
        }
        self.ordered_keys(&next)
    }

    fn range_selection(&self, anchor: TableRowKey, row: TableRowKey) -> Vec<TableRowKey> {
        let anchor_index = self.rows.iter().position(|key| *key == anchor);
        let row_index = self.rows.iter().position(|key| *key == row).unwrap_or(0);
        let start = anchor_index.unwrap_or(row_index).min(row_index);
        let end = anchor_index.unwrap_or(row_index).max(row_index);
        self.rows[start..=end].to_vec()
    }

    fn ordered_keys(&self, keys: &[TableRowKey]) -> Vec<TableRowKey> {
        self.rows
            .iter()
            .copied()
            .filter(|key| keys.contains(key))
            .collect()
    }

    fn target_indices(&self, target: TableCellTarget) -> Option<(Option<usize>, usize)> {
        let column = self.columns.iter().position(|key| *key == target.column)?;
        let row = target
            .row
            .and_then(|key| self.rows.iter().position(|candidate| *candidate == key));
        Some((row, column))
    }

    fn target_at(&self, row: Option<usize>, column: usize) -> Option<TableCellTarget> {
        let column = *self.columns.get(column)?;
        match row {
            Some(row) => Some(TableCellTarget::body(*self.rows.get(row)?, column)),
            None => Some(TableCellTarget::header(column)),
        }
    }

    fn next_indices(
        &self,
        row: Option<usize>,
        column: usize,
        navigation: TableNavigation,
        rtl: bool,
        primary: bool,
    ) -> (Option<usize>, usize) {
        if primary && navigation == TableNavigation::Home {
            return (None, 0);
        }
        if primary && navigation == TableNavigation::End {
            return (
                self.rows.len().checked_sub(1),
                self.columns.len().saturating_sub(1),
            );
        }
        self.next_local_indices(row, column, navigation, rtl)
    }

    fn next_local_indices(
        &self,
        row: Option<usize>,
        column: usize,
        navigation: TableNavigation,
        rtl: bool,
    ) -> (Option<usize>, usize) {
        match navigation {
            TableNavigation::Left => (row, move_column(column, self.columns.len(), rtl)),
            TableNavigation::Right => (row, move_column(column, self.columns.len(), !rtl)),
            TableNavigation::Up => (move_row_up(row), column),
            TableNavigation::Down => (move_row_down(row, self.rows.len()), column),
            TableNavigation::Home => (row, 0),
            TableNavigation::End => (row, self.columns.len().saturating_sub(1)),
            TableNavigation::PageUp => (self.page_row(row, false), column),
            TableNavigation::PageDown => (self.page_row(row, true), column),
        }
    }

    fn page_row(&self, row: Option<usize>, forward: bool) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        let current = row.unwrap_or(0);
        let page = (self.body_height / self.row_height).floor().max(1.0) as usize;
        if forward {
            Some((current + page).min(self.rows.len() - 1))
        } else {
            Some(current.saturating_sub(page))
        }
    }
}

fn selection_runtime<Msg>(
    selection: &TableSelection<'_, Msg>,
    model: &TableModel<'_>,
) -> TableSelectionRuntime<Msg> {
    match selection {
        TableSelection::None => TableSelectionRuntime::None,
        TableSelection::Single {
            selected,
            on_selection,
        } => TableSelectionRuntime::Single {
            selected: selected.filter(|key| model.rows().iter().any(|row| row.key() == *key)),
            on_selection: *on_selection,
        },
        TableSelection::Multiple {
            selected,
            on_selection,
        } => TableSelectionRuntime::Multiple {
            selected: model
                .rows()
                .iter()
                .map(|row| row.key())
                .filter(|key| selected.contains(key))
                .collect(),
            on_selection: *on_selection,
        },
    }
}

fn move_column(current: usize, count: usize, forward: bool) -> usize {
    if forward {
        (current + 1).min(count.saturating_sub(1))
    } else {
        current.saturating_sub(1)
    }
}

fn move_row_up(current: Option<usize>) -> Option<usize> {
    match current {
        Some(0) | None => None,
        Some(row) => Some(row - 1),
    }
}

fn move_row_down(current: Option<usize>, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(current.map_or(0, |row| (row + 1).min(count - 1)))
}

#[cfg(test)]
#[path = "table_runtime_tests.rs"]
mod tests;
