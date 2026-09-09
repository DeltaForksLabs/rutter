// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::ops::Range;

use super::{TableColumn, TableMetrics};

pub(crate) const TABLE_SCROLLBAR_THICKNESS: f32 = 12.0;
const TABLE_SCROLLBAR_MIN_THUMB: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableRect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl TableRect {
    pub(crate) const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableLayoutDirection {
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableViewport {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) content_width: f32,
    pub(crate) content_height: f32,
    pub(crate) header: TableRect,
    pub(crate) body: TableRect,
    pub(crate) horizontal_track: Option<TableRect>,
    pub(crate) vertical_track: Option<TableRect>,
}

impl TableViewport {
    pub(crate) fn max_scroll_x(self) -> f32 {
        (self.content_width - self.body.width).max(0.0)
    }

    pub(crate) fn max_scroll_y(self) -> f32 {
        (self.content_height - self.body.height).max(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableVisibleRows(pub(crate) Range<usize>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TableScrollbarThumbs {
    pub(crate) horizontal: Option<TableRect>,
    pub(crate) vertical: Option<TableRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableHit {
    Header { column: usize },
    Cell { row: usize, column: usize },
    HorizontalScrollbar,
    VerticalScrollbar,
    EmptyState,
}

pub(crate) fn minimum_content_width(columns: &[TableColumn<'_>]) -> f32 {
    columns
        .iter()
        .map(|column| column.width().minimum_width())
        .sum()
}

pub(crate) fn allocate_column_widths(
    columns: &[TableColumn<'_>],
    available_width: f32,
) -> Vec<f32> {
    let minimum = minimum_content_width(columns);
    let extra = (available_width - minimum).max(0.0);
    let total_weight: u32 = columns
        .iter()
        .filter_map(|column| column.width().flex_weight().map(u32::from))
        .sum();
    columns
        .iter()
        .map(|column| allocated_column_width(column, extra, total_weight))
        .collect()
}

fn allocated_column_width(column: &TableColumn<'_>, extra: f32, total_weight: u32) -> f32 {
    let minimum = column.width().minimum_width();
    let Some(weight) = column.width().flex_weight() else {
        return minimum;
    };
    if total_weight == 0 {
        return minimum;
    }
    minimum + extra * f32::from(weight) / total_weight as f32
}

pub(crate) fn calculate_viewport(
    size: (f32, f32),
    metrics: TableMetrics,
    row_count: usize,
    minimum_width: f32,
) -> TableViewport {
    let width = size.0.max(0.0);
    let height = size.1.max(0.0);
    let header_height = metrics.header_height().min(height);
    let content_height = metrics.row_height() * row_count as f32;
    let (body_width, body_height, horizontal, vertical) = resolve_scrollbars(
        width,
        (height - header_height).max(0.0),
        minimum_width,
        content_height,
    );
    build_viewport(
        (width, height),
        header_height,
        (body_width, body_height),
        (minimum_width.max(body_width), content_height),
        (horizontal, vertical),
    )
}

fn resolve_scrollbars(
    width: f32,
    height: f32,
    content_width: f32,
    content_height: f32,
) -> (f32, f32, bool, bool) {
    let mut horizontal = content_width > width;
    let mut vertical = content_height > height - horizontal_space(horizontal);
    horizontal = horizontal || content_width > width - vertical_space(vertical);
    vertical = vertical || content_height > height - horizontal_space(horizontal);
    let body_width = (width - vertical_space(vertical)).max(0.0);
    let body_height = (height - horizontal_space(horizontal)).max(0.0);
    (body_width, body_height, horizontal, vertical)
}

fn horizontal_space(visible: bool) -> f32 {
    if visible {
        TABLE_SCROLLBAR_THICKNESS
    } else {
        0.0
    }
}

fn vertical_space(visible: bool) -> f32 {
    horizontal_space(visible)
}

fn build_viewport(
    size: (f32, f32),
    header_height: f32,
    body_size: (f32, f32),
    content_size: (f32, f32),
    scrollbars: (bool, bool),
) -> TableViewport {
    let header = TableRect::new(0.0, 0.0, body_size.0, header_height);
    let body = TableRect::new(0.0, header_height, body_size.0, body_size.1);
    TableViewport {
        width: size.0,
        height: size.1,
        content_width: content_size.0,
        content_height: content_size.1,
        header,
        body,
        horizontal_track: horizontal_track(size, header_height, body_size, scrollbars.0),
        vertical_track: vertical_track(header_height, body_size, scrollbars.1),
    }
}

fn horizontal_track(
    size: (f32, f32),
    header_height: f32,
    body_size: (f32, f32),
    visible: bool,
) -> Option<TableRect> {
    visible.then(|| {
        TableRect::new(
            0.0,
            header_height + body_size.1,
            body_size.0,
            TABLE_SCROLLBAR_THICKNESS.min(size.1),
        )
    })
}

fn vertical_track(header_height: f32, body_size: (f32, f32), visible: bool) -> Option<TableRect> {
    visible.then(|| {
        TableRect::new(
            body_size.0,
            header_height,
            TABLE_SCROLLBAR_THICKNESS,
            body_size.1,
        )
    })
}

pub(crate) fn visible_row_range(
    scroll_y: f32,
    viewport_height: f32,
    row_height: f32,
    row_count: usize,
) -> TableVisibleRows {
    if row_height <= 0.0 || row_count == 0 {
        return TableVisibleRows(0..0);
    }
    let first = (scroll_y / row_height).floor().max(0.0) as usize;
    let count = (viewport_height / row_height).ceil() as usize + 1;
    TableVisibleRows(first.min(row_count)..(first + count).min(row_count))
}

pub(crate) fn logical_cell_rect(
    widths: &[f32],
    column: usize,
    row: Option<usize>,
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
    metrics: TableMetrics,
) -> Option<TableRect> {
    let width = *widths.get(column)?;
    let logical_start: f32 = widths[..column].iter().sum();
    let x = cell_x(logical_start, width, viewport, scroll.0, direction);
    let (y, height) = cell_vertical_rect(row, viewport, scroll.1, metrics);
    Some(TableRect::new(x, y, width, height))
}

fn cell_x(
    logical_start: f32,
    width: f32,
    viewport: TableViewport,
    scroll_x: f32,
    direction: TableLayoutDirection,
) -> f32 {
    match direction {
        TableLayoutDirection::Ltr => logical_start - scroll_x,
        TableLayoutDirection::Rtl => viewport.body.width - logical_start - width + scroll_x,
    }
}

fn cell_vertical_rect(
    row: Option<usize>,
    viewport: TableViewport,
    scroll_y: f32,
    metrics: TableMetrics,
) -> (f32, f32) {
    match row {
        None => (viewport.header.y, viewport.header.height),
        Some(row) => (
            viewport.body.y + row as f32 * metrics.row_height() - scroll_y,
            metrics.row_height(),
        ),
    }
}

pub(crate) fn hit_test(
    point: (f32, f32),
    widths: &[f32],
    row_count: usize,
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
    metrics: TableMetrics,
) -> Option<TableHit> {
    if viewport
        .horizontal_track
        .is_some_and(|rect| rect.contains(point.0, point.1))
    {
        return Some(TableHit::HorizontalScrollbar);
    }
    if viewport
        .vertical_track
        .is_some_and(|rect| rect.contains(point.0, point.1))
    {
        return Some(TableHit::VerticalScrollbar);
    }
    if viewport.header.contains(point.0, point.1) {
        return hit_column(point, widths, None, viewport, scroll, direction, metrics);
    }
    hit_body(
        point, widths, row_count, viewport, scroll, direction, metrics,
    )
}

fn hit_body(
    point: (f32, f32),
    widths: &[f32],
    row_count: usize,
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
    metrics: TableMetrics,
) -> Option<TableHit> {
    if !viewport.body.contains(point.0, point.1) {
        return None;
    }
    if row_count == 0 {
        return Some(TableHit::EmptyState);
    }
    let row = ((point.1 - viewport.body.y + scroll.1) / metrics.row_height()).floor() as usize;
    (row < row_count)
        .then(|| {
            hit_column(
                point,
                widths,
                Some(row),
                viewport,
                scroll,
                direction,
                metrics,
            )
        })
        .flatten()
}

fn hit_column(
    point: (f32, f32),
    widths: &[f32],
    row: Option<usize>,
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
    metrics: TableMetrics,
) -> Option<TableHit> {
    widths.iter().enumerate().find_map(|(column, _)| {
        let rect = logical_cell_rect(widths, column, row, viewport, scroll, direction, metrics)?;
        rect.contains(point.0, point.1).then_some(match row {
            Some(row) => TableHit::Cell { row, column },
            None => TableHit::Header { column },
        })
    })
}

pub(crate) fn scrollbar_thumbs(
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
) -> TableScrollbarThumbs {
    TableScrollbarThumbs {
        horizontal: viewport
            .horizontal_track
            .map(|track| horizontal_thumb(track, viewport.content_width, scroll.0, direction)),
        vertical: viewport
            .vertical_track
            .map(|track| axis_thumb(track, viewport.content_height, scroll.1, false)),
    }
}

fn horizontal_thumb(
    track: TableRect,
    content_width: f32,
    scroll_x: f32,
    direction: TableLayoutDirection,
) -> TableRect {
    let max_scroll = (content_width - track.width).max(0.0);
    let visual_offset = match direction {
        TableLayoutDirection::Ltr => scroll_x,
        TableLayoutDirection::Rtl => max_scroll - scroll_x,
    };
    axis_thumb(track, content_width, visual_offset, true)
}

fn axis_thumb(track: TableRect, content_extent: f32, offset: f32, horizontal: bool) -> TableRect {
    let track_extent = if horizontal {
        track.width
    } else {
        track.height
    };
    let thumb_extent = (track_extent * track_extent / content_extent.max(track_extent))
        .max(TABLE_SCROLLBAR_MIN_THUMB)
        .min(track_extent);
    let travel = (track_extent - thumb_extent).max(0.0);
    let max_offset = (content_extent - track_extent).max(0.0);
    let position = if max_offset > 0.0 {
        offset.clamp(0.0, max_offset) / max_offset * travel
    } else {
        0.0
    };
    if horizontal {
        TableRect::new(track.x + position, track.y, thumb_extent, track.height)
    } else {
        TableRect::new(track.x, track.y + position, track.width, thumb_extent)
    }
}

pub(crate) fn reveal_offset(
    current: f32,
    viewport_extent: f32,
    item_start: f32,
    item_end: f32,
    content_extent: f32,
) -> f32 {
    let requested = if item_start < current {
        item_start
    } else if item_end > current + viewport_extent {
        item_end - viewport_extent
    } else {
        current
    };
    requested.clamp(0.0, (content_extent - viewport_extent).max(0.0))
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
