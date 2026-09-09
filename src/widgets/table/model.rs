// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::borrow::Cow;
use std::collections::HashMap;

use super::{TableColumnKey, TableColumnWidth, TableConfigError, TableRowKey};

/// Logical inline alignment that follows the table's LTR or RTL direction.
///
/// ```rust
/// use rutter::table::TableAlignment;
/// let alignment = TableAlignment::End;
/// assert_ne!(alignment, TableAlignment::Start);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableAlignment {
    /// Aligns content with the logical start edge.
    #[default]
    Start,
    /// Centers content within the cell.
    Center,
    /// Aligns content with the logical end edge.
    End,
}

/// Declarative metadata for one stable table column.
///
/// ```rust
/// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
/// let column = TableColumn::new(
///     TableColumnKey::new(1),
///     "Name",
///     TableColumnWidth::flex(120.0, 1).unwrap(),
/// );
/// assert_eq!(column.label(), "Name");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn<'a> {
    key: TableColumnKey,
    label: Cow<'a, str>,
    width: TableColumnWidth,
    alignment: TableAlignment,
    row_header: bool,
    sortable: bool,
}

impl<'a> TableColumn<'a> {
    /// Creates a column with start alignment and no optional semantics.
    ///
    /// ```rust
    /// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(
    ///     TableColumnKey::new(1), "Name", TableColumnWidth::fixed(160.0).unwrap(),
    /// );
    /// assert!(!column.is_sortable());
    /// ```
    pub fn new(
        key: TableColumnKey,
        label: impl Into<Cow<'a, str>>,
        width: TableColumnWidth,
    ) -> Self {
        Self {
            key,
            label: label.into(),
            width,
            alignment: TableAlignment::Start,
            row_header: false,
            sortable: false,
        }
    }

    /// Sets logical text alignment without depending on physical direction.
    ///
    /// ```rust
    /// use rutter::table::{TableAlignment, TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Total", TableColumnWidth::fixed(80.0).unwrap())
    ///     .with_alignment(TableAlignment::End);
    /// assert_eq!(column.alignment(), TableAlignment::End);
    /// ```
    #[must_use]
    pub fn with_alignment(mut self, alignment: TableAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Marks whether cells in this column identify their rows to accessibility clients.
    ///
    /// ```rust
    /// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap())
    ///     .with_row_header(true);
    /// assert!(column.is_row_header());
    /// ```
    #[must_use]
    pub fn with_row_header(mut self, enabled: bool) -> Self {
        self.row_header = enabled;
        self
    }

    /// Marks whether activation may request sorting by this column.
    ///
    /// ```rust
    /// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap())
    ///     .with_sortable(true);
    /// assert!(column.is_sortable());
    /// ```
    #[must_use]
    pub fn with_sortable(mut self, enabled: bool) -> Self {
        self.sortable = enabled;
        self
    }

    /// Returns the stable identity used across reordering and model updates.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(8), "Name", TableColumnWidth::fixed(90.0).unwrap());
    /// assert_eq!(column.key().get(), 8);
    /// ```
    pub const fn key(&self) -> TableColumnKey {
        self.key
    }

    /// Returns the visible column header label.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Status", TableColumnWidth::fixed(90.0).unwrap());
    /// assert_eq!(column.label(), "Status");
    /// ```
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the validated sizing policy used by horizontal layout.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap());
    /// assert_eq!(column.width().minimum_width(), 90.0);
    /// ```
    pub const fn width(&self) -> TableColumnWidth {
        self.width
    }

    /// Returns the column's logical content alignment.
    ///
    /// ```rust
    /// # use rutter::table::{TableAlignment, TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap());
    /// assert_eq!(column.alignment(), TableAlignment::Start);
    /// ```
    pub const fn alignment(&self) -> TableAlignment {
        self.alignment
    }

    /// Reports whether cells in this column provide row-header semantics.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap());
    /// assert!(!column.is_row_header());
    /// ```
    pub const fn is_row_header(&self) -> bool {
        self.row_header
    }

    /// Reports whether this column accepts controlled sort activation.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap());
    /// assert!(!column.is_sortable());
    /// ```
    pub const fn is_sortable(&self) -> bool {
        self.sortable
    }
}

/// Borrowed or owned text and optional accessible name for one body cell.
///
/// ```rust
/// use rutter::table::TableCell;
/// let cell = TableCell::new("Ada").with_accessibility_label("Name: Ada");
/// assert_eq!(cell.accessibility_label(), Some("Name: Ada"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell<'a> {
    text: Cow<'a, str>,
    accessibility_label: Option<Cow<'a, str>>,
}

impl<'a> TableCell<'a> {
    /// Creates a cell whose visible text is also its accessible name.
    ///
    /// ```rust
    /// use rutter::table::TableCell;
    /// assert_eq!(TableCell::new("Ready").text(), "Ready");
    /// ```
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            text: text.into(),
            accessibility_label: None,
        }
    }

    /// Overrides the name announced for this cell while preserving visible text.
    ///
    /// ```rust
    /// use rutter::table::TableCell;
    /// let cell = TableCell::new("42").with_accessibility_label("42 open tasks");
    /// assert_eq!(cell.accessibility_label(), Some("42 open tasks"));
    /// ```
    #[must_use]
    pub fn with_accessibility_label(mut self, label: impl Into<Cow<'a, str>>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Returns visible cell text.
    ///
    /// ```rust
    /// use rutter::table::TableCell;
    /// assert_eq!(TableCell::new("Active").text(), "Active");
    /// ```
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns an explicit accessible name when one was supplied.
    ///
    /// ```rust
    /// use rutter::table::TableCell;
    /// assert_eq!(TableCell::new("Active").accessibility_label(), None);
    /// ```
    pub fn accessibility_label(&self) -> Option<&str> {
        self.accessibility_label.as_deref()
    }
}

/// One stable row whose cells follow the model's column order.
///
/// ```rust
/// use rutter::table::{TableCell, TableRow, TableRowKey};
/// let row = TableRow::new(TableRowKey::new(1), vec![TableCell::new("Ada")]);
/// assert_eq!(row.cells().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow<'a> {
    key: TableRowKey,
    cells: Vec<TableCell<'a>>,
}

impl<'a> TableRow<'a> {
    /// Creates a row from cells in declared column order.
    ///
    /// ```rust
    /// use rutter::table::{TableCell, TableRow, TableRowKey};
    /// let row = TableRow::new(TableRowKey::new(4), [TableCell::new("Ready")]);
    /// assert_eq!(row.key().get(), 4);
    /// ```
    pub fn new(key: TableRowKey, cells: impl Into<Vec<TableCell<'a>>>) -> Self {
        Self {
            key,
            cells: cells.into(),
        }
    }

    /// Returns stable row identity independent of display position.
    ///
    /// ```rust
    /// # use rutter::table::{TableRow, TableRowKey};
    /// let row = TableRow::new(TableRowKey::new(6), Vec::new());
    /// assert_eq!(row.key().get(), 6);
    /// ```
    pub const fn key(&self) -> TableRowKey {
        self.key
    }

    /// Returns cells in the same order as model columns.
    ///
    /// ```rust
    /// # use rutter::table::{TableCell, TableRow, TableRowKey};
    /// let row = TableRow::new(TableRowKey::new(1), [TableCell::new("A")]);
    /// assert_eq!(row.cells()[0].text(), "A");
    /// ```
    pub fn cells(&self) -> &[TableCell<'a>] {
        &self.cells
    }
}

/// Validated accessibility label, column schema, and displayed row collection.
///
/// ```rust
/// use rutter::table::{TableCell, TableColumn, TableColumnKey, TableColumnWidth, TableModel, TableRow, TableRowKey};
/// let columns = vec![TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(120.0).unwrap())];
/// let rows = vec![TableRow::new(TableRowKey::new(1), [TableCell::new("Ada")])];
/// let model = TableModel::new("People", columns, rows).unwrap();
/// assert_eq!(model.rows().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TableModel<'a> {
    accessibility_label: Cow<'a, str>,
    columns: Vec<TableColumn<'a>>,
    rows: Vec<TableRow<'a>>,
}

impl<'a> TableModel<'a> {
    /// Validates and creates a complete table model in current display order.
    ///
    /// ```rust
    /// use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableModel};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(90.0).unwrap());
    /// let model = TableModel::new("People", vec![column], Vec::new()).unwrap();
    /// assert_eq!(model.accessibility_label(), "People");
    /// ```
    pub fn new(
        accessibility_label: impl Into<Cow<'a, str>>,
        columns: impl Into<Vec<TableColumn<'a>>>,
        rows: impl Into<Vec<TableRow<'a>>>,
    ) -> Result<Self, TableConfigError> {
        let model = Self {
            accessibility_label: accessibility_label.into(),
            columns: columns.into(),
            rows: rows.into(),
        };
        model.validate()?;
        Ok(model)
    }

    /// Returns the non-empty table name announced by accessibility clients.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableModel};
    /// let model = TableModel::new("Jobs", vec![TableColumn::new(TableColumnKey::new(1), "State", TableColumnWidth::fixed(80.0).unwrap())], Vec::new()).unwrap();
    /// assert_eq!(model.accessibility_label(), "Jobs");
    /// ```
    pub fn accessibility_label(&self) -> &str {
        &self.accessibility_label
    }

    /// Returns validated columns in logical display order.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableModel};
    /// let model = TableModel::new("Jobs", vec![TableColumn::new(TableColumnKey::new(1), "State", TableColumnWidth::fixed(80.0).unwrap())], Vec::new()).unwrap();
    /// assert_eq!(model.columns()[0].label(), "State");
    /// ```
    pub fn columns(&self) -> &[TableColumn<'a>] {
        &self.columns
    }

    /// Returns rows in the application's current sorted or filtered order.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableModel};
    /// let model = TableModel::new("Jobs", vec![TableColumn::new(TableColumnKey::new(1), "State", TableColumnWidth::fixed(80.0).unwrap())], Vec::new()).unwrap();
    /// assert!(model.rows().is_empty());
    /// ```
    pub fn rows(&self) -> &[TableRow<'a>] {
        &self.rows
    }

    /// Returns the sole row-header column when one was configured.
    ///
    /// ```rust
    /// # use rutter::table::{TableColumn, TableColumnKey, TableColumnWidth, TableModel};
    /// let column = TableColumn::new(TableColumnKey::new(1), "Name", TableColumnWidth::fixed(80.0).unwrap()).with_row_header(true);
    /// let model = TableModel::new("People", vec![column], Vec::new()).unwrap();
    /// assert_eq!(model.row_header_column().unwrap().key().get(), 1);
    /// ```
    pub fn row_header_column(&self) -> Option<&TableColumn<'a>> {
        self.columns.iter().find(|column| column.is_row_header())
    }

    fn validate(&self) -> Result<(), TableConfigError> {
        self.validate_label()?;
        if self.columns.is_empty() {
            return Err(TableConfigError::MissingColumns { count: 0 });
        }
        self.validate_column_keys()?;
        self.validate_row_header_columns()?;
        self.validate_rows()
    }

    fn validate_label(&self) -> Result<(), TableConfigError> {
        if self.accessibility_label.trim().is_empty() {
            return Err(TableConfigError::EmptyAccessibilityLabel {
                value: self.accessibility_label.to_string(),
            });
        }
        Ok(())
    }

    fn validate_column_keys(&self) -> Result<(), TableConfigError> {
        let mut first_indices = HashMap::with_capacity(self.columns.len());
        for (index, column) in self.columns.iter().enumerate() {
            if let Some(first_index) = first_indices.insert(column.key(), index) {
                return Err(TableConfigError::DuplicateColumnKey {
                    key: column.key(),
                    first_index,
                    duplicate_index: index,
                });
            }
        }
        Ok(())
    }

    fn validate_row_header_columns(&self) -> Result<(), TableConfigError> {
        let mut row_header = None;
        for column in self.columns.iter().filter(|column| column.is_row_header()) {
            if let Some(first_key) = row_header {
                return Err(TableConfigError::MultipleRowHeaderColumns {
                    first_key,
                    duplicate_key: column.key(),
                });
            }
            row_header = Some(column.key());
        }
        Ok(())
    }

    fn validate_rows(&self) -> Result<(), TableConfigError> {
        let mut first_indices = HashMap::with_capacity(self.rows.len());
        for (index, row) in self.rows.iter().enumerate() {
            self.validate_cell_count(row, index)?;
            if let Some(first_index) = first_indices.insert(row.key(), index) {
                return Err(TableConfigError::DuplicateRowKey {
                    key: row.key(),
                    first_index,
                    duplicate_index: index,
                });
            }
        }
        Ok(())
    }

    fn validate_cell_count(
        &self,
        row: &TableRow<'_>,
        row_index: usize,
    ) -> Result<(), TableConfigError> {
        let actual = row.cells().len();
        let expected = self.columns.len();
        if actual != expected {
            return Err(TableConfigError::CellCountMismatch {
                row_key: row.key(),
                row_index,
                actual,
                expected,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
