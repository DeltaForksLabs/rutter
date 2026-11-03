// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::borrow::Cow;
use std::fmt;

use super::interaction::{TableSelection, TableSorting};
use super::{TableColumnKey, TableRowKey};
use super::{TableSelectionWith, TableSortingWith};

const DEFAULT_CELL_HEIGHT: f32 = 44.0;

/// Reports a value that violates table model or geometry invariants.
/// ```rust
/// use rutter::table::{TableColumnWidth, TableConfigError};
/// let error = TableColumnWidth::fixed(0.0).unwrap_err();
/// assert!(matches!(error, TableConfigError::InvalidFixedWidth { .. }));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TableConfigError {
    /// A fixed width was not finite and strictly positive.
    InvalidFixedWidth { value: f32 },
    /// A flex minimum width was not finite and strictly positive.
    InvalidFlexMinimumWidth { value: f32 },
    /// A flex allocation weight was zero.
    ZeroFlexWeight { value: u16 },
    /// A header height was not finite and strictly positive.
    InvalidHeaderHeight { value: f32 },
    /// A row height was not finite and strictly positive.
    InvalidRowHeight { value: f32 },
    /// The table's accessible name had no non-whitespace characters.
    EmptyAccessibilityLabel { value: String },
    /// The table schema contained no columns.
    MissingColumns { count: usize },
    /// A stable column key appeared more than once.
    DuplicateColumnKey {
        key: TableColumnKey,
        first_index: usize,
        duplicate_index: usize,
    },
    /// A stable row key appeared more than once.
    DuplicateRowKey {
        key: TableRowKey,
        first_index: usize,
        duplicate_index: usize,
    },
    /// More than one column requested row-header semantics.
    MultipleRowHeaderColumns {
        first_key: TableColumnKey,
        duplicate_key: TableColumnKey,
    },
    /// A row did not contain exactly one cell per column.
    CellCountMismatch {
        row_key: TableRowKey,
        row_index: usize,
        actual: usize,
        expected: usize,
    },
}

impl fmt::Display for TableConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFixedWidth { value } => write!(
                formatter,
                "invalid table fixed width {value:?}, expected a finite value > 0 logical pixels"
            ),
            Self::InvalidFlexMinimumWidth { value } => write!(
                formatter,
                "invalid table flex minimum width {value:?}, expected a finite value > 0 logical pixels"
            ),
            Self::ZeroFlexWeight { value } => write!(
                formatter,
                "invalid table flex weight {value}, expected an integer in 1..={}",
                u16::MAX
            ),
            Self::InvalidHeaderHeight { value } => write!(
                formatter,
                "invalid table header height {value:?}, expected a finite value > 0 logical pixels"
            ),
            Self::InvalidRowHeight { value } => write!(
                formatter,
                "invalid table row height {value:?}, expected a finite value > 0 logical pixels"
            ),
            Self::EmptyAccessibilityLabel { value } => write!(
                formatter,
                "invalid table accessibility label {value:?}, expected at least one non-whitespace character"
            ),
            Self::MissingColumns { count } => write!(
                formatter,
                "invalid table column count {count}, expected at least 1 column"
            ),
            Self::DuplicateColumnKey {
                key,
                first_index,
                duplicate_index,
            } => write!(
                formatter,
                "duplicate table column key {} at index {duplicate_index}, expected a unique key; first occurrence is at index {first_index}",
                key.get()
            ),
            Self::DuplicateRowKey {
                key,
                first_index,
                duplicate_index,
            } => write!(
                formatter,
                "duplicate table row key {} at index {duplicate_index}, expected a unique key; first occurrence is at index {first_index}",
                key.get()
            ),
            Self::MultipleRowHeaderColumns {
                first_key,
                duplicate_key,
            } => write!(
                formatter,
                "invalid row-header column key {}, expected at most one row-header column; column key {} was already marked",
                duplicate_key.get(),
                first_key.get()
            ),
            Self::CellCountMismatch {
                row_key,
                row_index,
                actual,
                expected,
            } => write!(
                formatter,
                "invalid cell count {actual} for table row key {} at index {row_index}, expected exactly {expected} cells",
                row_key.get()
            ),
        }
    }
}

impl std::error::Error for TableConfigError {}

/// Validated fixed or weighted sizing for one table column.
/// ```rust
/// use rutter::table::TableColumnWidth;
/// let width = TableColumnWidth::flex(100.0, 2).unwrap();
/// assert_eq!(width.flex_weight(), Some(2));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableColumnWidth(TableColumnWidthKind);

#[derive(Debug, Clone, Copy, PartialEq)]
enum TableColumnWidthKind {
    Fixed { width: f32 },
    Flex { minimum: f32, weight: u16 },
}

impl TableColumnWidth {
    /// Creates a column that always requests one exact logical width.
    /// ```rust
    /// use rutter::table::TableColumnWidth;
    /// assert_eq!(TableColumnWidth::fixed(128.0).unwrap().minimum_width(), 128.0);
    /// ```
    pub fn fixed(width: f32) -> Result<Self, TableConfigError> {
        if !width.is_finite() || width <= 0.0 {
            return Err(TableConfigError::InvalidFixedWidth { value: width });
        }
        Ok(Self(TableColumnWidthKind::Fixed { width }))
    }

    /// Creates a column that keeps a minimum width and shares remaining space by weight.
    /// ```rust
    /// use rutter::table::TableColumnWidth;
    /// assert_eq!(TableColumnWidth::flex(96.0, 3).unwrap().flex_weight(), Some(3));
    /// ```
    pub fn flex(minimum_width: f32, weight: u16) -> Result<Self, TableConfigError> {
        if !minimum_width.is_finite() || minimum_width <= 0.0 {
            return Err(TableConfigError::InvalidFlexMinimumWidth {
                value: minimum_width,
            });
        }
        if weight == 0 {
            return Err(TableConfigError::ZeroFlexWeight { value: weight });
        }
        Ok(Self(TableColumnWidthKind::Flex {
            minimum: minimum_width,
            weight,
        }))
    }

    /// Returns the fixed width or flex lower bound used before allocation.
    /// ```rust
    /// use rutter::table::TableColumnWidth;
    /// assert_eq!(TableColumnWidth::flex(72.0, 1).unwrap().minimum_width(), 72.0);
    /// ```
    pub const fn minimum_width(self) -> f32 {
        match self.0 {
            TableColumnWidthKind::Fixed { width } => width,
            TableColumnWidthKind::Flex { minimum, .. } => minimum,
        }
    }

    /// Returns a flex weight, or `None` when the width is fixed.
    /// ```rust
    /// use rutter::table::TableColumnWidth;
    /// assert_eq!(TableColumnWidth::fixed(72.0).unwrap().flex_weight(), None);
    /// ```
    pub const fn flex_weight(self) -> Option<u16> {
        match self.0 {
            TableColumnWidthKind::Fixed { .. } => None,
            TableColumnWidthKind::Flex { weight, .. } => Some(weight),
        }
    }
}

/// Validated vertical metrics shared by layout, hit testing, and virtualization.
/// ```rust
/// use rutter::table::TableMetrics;
/// let metrics = TableMetrics::new(48.0, 40.0).unwrap();
/// assert_eq!(metrics.row_height(), 40.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableMetrics {
    header_height: f32,
    row_height: f32,
}

impl TableMetrics {
    /// Validates finite, positive header and row heights.
    /// ```rust
    /// use rutter::table::TableMetrics;
    /// assert!(TableMetrics::new(44.0, 36.0).is_ok());
    /// ```
    pub fn new(header_height: f32, row_height: f32) -> Result<Self, TableConfigError> {
        if !header_height.is_finite() || header_height <= 0.0 {
            return Err(TableConfigError::InvalidHeaderHeight {
                value: header_height,
            });
        }
        if !row_height.is_finite() || row_height <= 0.0 {
            return Err(TableConfigError::InvalidRowHeight { value: row_height });
        }
        Ok(Self {
            header_height,
            row_height,
        })
    }

    /// Returns the logical height reserved for column headers.
    /// ```rust
    /// use rutter::table::TableMetrics;
    /// assert_eq!(TableMetrics::default().header_height(), 44.0);
    /// ```
    pub const fn header_height(self) -> f32 {
        self.header_height
    }

    /// Returns the uniform logical height used to virtualize body rows.
    /// ```rust
    /// use rutter::table::TableMetrics;
    /// assert_eq!(TableMetrics::default().row_height(), 44.0);
    /// ```
    pub const fn row_height(self) -> f32 {
        self.row_height
    }
}

impl Default for TableMetrics {
    fn default() -> Self {
        Self {
            header_height: DEFAULT_CELL_HEIGHT,
            row_height: DEFAULT_CELL_HEIGHT,
        }
    }
}

/// Declarative interaction and empty-state options for one table instance.
/// ```rust
/// use rutter::table::TableOptions;
/// assert_eq!(TableOptions::<()>::default().empty_label(), "No rows");
/// ```
pub struct TableOptions<'a, Msg> {
    selection: TableSelection<'a, Msg>,
    sorting: Option<TableSorting<Msg>>,
    selection_with: Option<TableSelectionWith<'a, Msg>>,
    sorting_with: Option<TableSortingWith<Msg>>,
    empty_label: Cow<'a, str>,
    metrics: TableMetrics,
}

impl<'a, Msg> TableOptions<'a, Msg> {
    /// Replaces the controlled row-selection policy, dropping any captured policy.
    /// ```rust
    /// use rutter::table::{TableOptions, TableSelection};
    /// let options = TableOptions::<()>::default().with_selection(TableSelection::none());
    /// assert!(matches!(options.selection(), TableSelection::None));
    /// ```
    #[must_use]
    pub fn with_selection(mut self, selection: TableSelection<'a, Msg>) -> Self {
        self.selection = selection;
        self.selection_with = None;
        self
    }

    /// Enables controlled sorting, dropping any captured sorting policy.
    /// ```rust
    /// use rutter::table::{TableOptions, TableSorting};
    /// let options = TableOptions::default().with_sorting(TableSorting::new(None, std::convert::identity));
    /// assert!(options.sorting().is_some());
    /// ```
    #[must_use]
    pub fn with_sorting(mut self, sorting: TableSorting<Msg>) -> Self {
        self.sorting = Some(sorting);
        self.sorting_with = None;
        self
    }

    /// Replaces either selection policy with an owned callback policy.
    /// The legacy `selection()` accessor returns `TableSelection::None` after this call;
    /// use `effective_selection()` to inspect either callback family.
    #[must_use]
    pub fn selection_with(mut self, selection: impl Into<TableSelectionWith<'a, Msg>>) -> Self {
        self.selection = TableSelection::None;
        self.selection_with = Some(selection.into());
        self
    }

    /// Replaces either sorting policy with an owned callback policy.
    /// The legacy `sorting()` accessor returns `None` after this call;
    /// use `effective_sorting()` to inspect either callback family.
    #[must_use]
    pub fn sorting_with(mut self, sorting: impl Into<TableSortingWith<Msg>>) -> Self {
        self.sorting = None;
        self.sorting_with = Some(sorting.into());
        self
    }

    /// Returns the active policy, sharing owned callback context when captured.
    pub fn effective_selection(&self) -> TableSelectionWith<'a, Msg> {
        if let Some(selection) = &self.selection_with {
            return selection.clone();
        }
        match &self.selection {
            TableSelection::None => TableSelectionWith::None,
            TableSelection::Single {
                selected,
                on_selection,
            } => TableSelection::single(*selected, *on_selection).into(),
            TableSelection::Multiple {
                selected,
                on_selection,
            } => TableSelection::multiple(selected, *on_selection).into(),
        }
    }

    /// Returns the active sorting policy, sharing its callback context.
    pub fn effective_sorting(&self) -> Option<TableSortingWith<Msg>> {
        self.sorting_with.clone().or_else(|| {
            self.sorting.as_ref().map(|sorting| {
                TableSorting::new(sorting.current(), sorting.on_sort_callback()).into()
            })
        })
    }

    /// Replaces the text shown when no rows are displayed.
    /// ```rust
    /// use rutter::table::TableOptions;
    /// let options = TableOptions::<()>::default().with_empty_label("Nothing found");
    /// assert_eq!(options.empty_label(), "Nothing found");
    /// ```
    #[must_use]
    pub fn with_empty_label(mut self, label: impl Into<Cow<'a, str>>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Replaces validated header and row geometry metrics.
    /// ```rust
    /// use rutter::table::{TableMetrics, TableOptions};
    /// let options = TableOptions::<()>::default().with_metrics(TableMetrics::new(50.0, 38.0).unwrap());
    /// assert_eq!(options.metrics().header_height(), 50.0);
    /// ```
    #[must_use]
    pub fn with_metrics(mut self, metrics: TableMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Returns the legacy selection policy, or `TableSelection::None` when captured.
    /// Use [`Self::effective_selection`] to inspect either callback family.
    /// ```rust
    /// use rutter::table::{TableOptions, TableSelection};
    /// assert!(matches!(TableOptions::<()>::default().selection(), TableSelection::None));
    /// ```
    pub const fn selection(&self) -> &TableSelection<'a, Msg> {
        &self.selection
    }

    /// Returns legacy sorting, or `None` when a captured sorting policy is active.
    /// Use [`Self::effective_sorting`] to inspect either callback family.
    /// ```rust
    /// use rutter::table::TableOptions;
    /// assert!(TableOptions::<()>::default().sorting().is_none());
    /// ```
    pub const fn sorting(&self) -> Option<&TableSorting<Msg>> {
        self.sorting.as_ref()
    }

    /// Returns empty-state text for a model with no displayed rows.
    /// ```rust
    /// use rutter::table::TableOptions;
    /// assert_eq!(TableOptions::<()>::default().empty_label(), "No rows");
    /// ```
    pub fn empty_label(&self) -> &str {
        &self.empty_label
    }

    /// Returns validated geometry shared by rendering and interaction.
    /// ```rust
    /// use rutter::table::TableOptions;
    /// assert_eq!(TableOptions::<()>::default().metrics().row_height(), 44.0);
    /// ```
    pub const fn metrics(&self) -> TableMetrics {
        self.metrics
    }
}

impl<'a, Msg> Default for TableOptions<'a, Msg> {
    fn default() -> Self {
        Self {
            selection: TableSelection::None,
            sorting: None,
            selection_with: None,
            sorting_with: None,
            empty_label: Cow::Borrowed("No rows"),
            metrics: TableMetrics::default(),
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
