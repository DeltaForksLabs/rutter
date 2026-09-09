// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Validated domain models, interaction configuration, and retained state for tables.

mod config;
pub(crate) mod geometry;
mod interaction;
mod keys;
mod model;
mod state;

pub use config::{TableColumnWidth, TableConfigError, TableMetrics, TableOptions};
pub use interaction::{TableSelection, TableSort, TableSortDirection, TableSorting};
pub use keys::{TableColumnKey, TableRowKey};
pub use model::{TableAlignment, TableCell, TableColumn, TableModel, TableRow};
pub(crate) use state::{TableCellTarget, TableState};

pub(crate) use geometry::{
    TableHit, TableLayoutDirection, allocate_column_widths, calculate_viewport, hit_test,
    logical_cell_rect, minimum_content_width,
};
