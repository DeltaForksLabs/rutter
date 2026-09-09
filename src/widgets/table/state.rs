// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::geometry::{
    TableLayoutDirection, TableScrollbarThumbs, TableViewport, TableVisibleRows, reveal_offset,
    scrollbar_thumbs, visible_row_range,
};
use super::{TableColumnKey, TableMetrics, TableModel, TableRowKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCellTarget {
    pub(crate) row: Option<TableRowKey>,
    pub(crate) column: TableColumnKey,
}

impl TableCellTarget {
    pub(crate) const fn header(column: TableColumnKey) -> Self {
        Self { row: None, column }
    }

    pub(crate) const fn body(row: TableRowKey, column: TableColumnKey) -> Self {
        Self {
            row: Some(row),
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableState {
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
    pub(crate) viewport: TableViewport,
    pub(crate) active: Option<TableCellTarget>,
    pub(crate) hovered: Option<TableCellTarget>,
    pub(crate) selection_anchor: Option<TableRowKey>,
}

impl Default for TableState {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            viewport: TableViewport {
                width: 0.0,
                height: 0.0,
                content_width: 0.0,
                content_height: 0.0,
                header: super::geometry::TableRect::new(0.0, 0.0, 0.0, 0.0),
                body: super::geometry::TableRect::new(0.0, 0.0, 0.0, 0.0),
                horizontal_track: None,
                vertical_track: None,
            },
            active: None,
            hovered: None,
            selection_anchor: None,
        }
    }
}

impl TableState {
    pub(crate) fn sync_viewport(&mut self, viewport: TableViewport) {
        self.viewport = viewport;
        self.clamp_scroll();
    }

    pub(crate) fn reconcile(&mut self, model: &TableModel<'_>) {
        self.active = self.active.filter(|target| target_is_live(*target, model));
        self.hovered = self.hovered.filter(|target| target_is_live(*target, model));
        self.selection_anchor = self
            .selection_anchor
            .filter(|key| model.rows().iter().any(|row| row.key() == *key));
    }

    pub(crate) fn scroll_by(&mut self, delta_x: f32, delta_y: f32) -> bool {
        let previous = (self.scroll_x, self.scroll_y);
        self.scroll_x += delta_x;
        self.scroll_y += delta_y;
        self.clamp_scroll();
        previous != (self.scroll_x, self.scroll_y)
    }

    pub(crate) fn set_scroll_x(&mut self, offset: f32) {
        self.scroll_x = offset.clamp(0.0, self.viewport.max_scroll_x());
    }

    pub(crate) fn set_scroll_y(&mut self, offset: f32) {
        self.scroll_y = offset.clamp(0.0, self.viewport.max_scroll_y());
    }

    pub(crate) fn visible_rows(&self, metrics: TableMetrics, row_count: usize) -> TableVisibleRows {
        visible_row_range(
            self.scroll_y,
            self.viewport.body.height,
            metrics.row_height(),
            row_count,
        )
    }

    pub(crate) fn thumbs(&self, direction: TableLayoutDirection) -> TableScrollbarThumbs {
        scrollbar_thumbs(self.viewport, (self.scroll_x, self.scroll_y), direction)
    }

    pub(crate) fn reveal_position(
        &mut self,
        column: usize,
        row: Option<usize>,
        widths: &[f32],
        row_height: f32,
    ) {
        self.reveal_column(column, widths);
        self.reveal_row_index(row, row_height);
    }

    fn reveal_column(&mut self, column: usize, widths: &[f32]) {
        let start: f32 = widths[..column].iter().sum();
        let end = start + widths.get(column).copied().unwrap_or(0.0);
        self.scroll_x = reveal_offset(
            self.scroll_x,
            self.viewport.body.width,
            start,
            end,
            self.viewport.content_width,
        );
    }

    fn reveal_row_index(&mut self, row: Option<usize>, row_height: f32) {
        let Some(index) = row else {
            return;
        };
        let start = index as f32 * row_height;
        self.scroll_y = reveal_offset(
            self.scroll_y,
            self.viewport.body.height,
            start,
            start + row_height,
            self.viewport.content_height,
        );
    }

    fn clamp_scroll(&mut self) {
        self.scroll_x = self.scroll_x.clamp(0.0, self.viewport.max_scroll_x());
        self.scroll_y = self.scroll_y.clamp(0.0, self.viewport.max_scroll_y());
    }
}

fn target_is_live(target: TableCellTarget, model: &TableModel<'_>) -> bool {
    if !model
        .columns()
        .iter()
        .any(|column| column.key() == target.column)
    {
        return false;
    }
    target
        .row
        .is_none_or(|key| model.rows().iter().any(|row| row.key() == key))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
