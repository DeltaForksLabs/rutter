// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Canvas, Font, Paint, Point, RRect, Rect as SkiaRect, paint};

use super::{draw_focus_outline, draw_single_line_text, get_cached_font, measure_single_line_text};
use crate::i18n::LayoutDirection;
use crate::text_controls::{TextControlPolicy, normalize_text_controls};
use crate::theme::Theme;
use crate::widgets::table::geometry::{TableRect, TableViewport};
use crate::widgets::table::{
    TableAlignment, TableCellTarget, TableLayoutDirection, TableModel, TableOptions,
    TableSelection, TableSortDirection, TableState, allocate_column_widths, calculate_viewport,
    logical_cell_rect, minimum_content_width,
};

const TABLE_CELL_PADDING_X: f32 = 12.0;

pub(super) struct TableRenderInput<'a, 'model, Msg> {
    pub(super) canvas: &'a Canvas,
    pub(super) model: &'a TableModel<'model>,
    pub(super) options: &'a TableOptions<'model, Msg>,
    pub(super) state: Option<&'a TableState>,
    pub(super) size: (f32, f32),
    pub(super) mouse: Point,
    pub(super) focused: bool,
    pub(super) shows_interaction_effects: bool,
    pub(super) direction: LayoutDirection,
    pub(super) font_cache: &'a mut HashMap<(String, u32), Font>,
    pub(super) theme: &'a Theme,
}

pub(super) fn draw_table<Msg>(mut input: TableRenderInput<'_, '_, Msg>) {
    let direction = table_direction(input.direction);
    let viewport = calculate_viewport(
        input.size,
        input.options.metrics(),
        input.model.rows().len(),
        minimum_content_width(input.model.columns()),
    );
    let widths = allocate_column_widths(input.model.columns(), viewport.body.width);
    let scroll = table_scroll(input.state, viewport);
    draw_table_surface(input.canvas, input.size, input.theme);
    draw_body(&mut input, viewport, &widths, scroll, direction);
    draw_header(&mut input, viewport, &widths, scroll, direction);
    draw_scrollbars(&input, viewport, scroll, direction);
    draw_table_border(input.canvas, input.size, input.focused, input.theme);
}

fn table_scroll(state: Option<&TableState>, viewport: TableViewport) -> (f32, f32) {
    let scroll = state
        .map(|state| (state.scroll_x, state.scroll_y))
        .unwrap_or_default();
    (
        scroll.0.clamp(0.0, viewport.max_scroll_x()),
        scroll.1.clamp(0.0, viewport.max_scroll_y()),
    )
}

fn draw_table_surface(canvas: &Canvas, size: (f32, f32), theme: &Theme) {
    let mut paint = Paint::default();
    paint.set_color(theme.surface);
    paint.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(table_rect(size), theme.radius_sm, theme.radius_sm),
        &paint,
    );
}

fn draw_body<Msg>(
    input: &mut TableRenderInput<'_, '_, Msg>,
    viewport: TableViewport,
    widths: &[f32],
    scroll: (f32, f32),
    direction: TableLayoutDirection,
) {
    input.canvas.save();
    input.canvas.clip_rect(to_skia(viewport.body), None, true);
    if input.model.rows().is_empty() {
        draw_empty_state(input, viewport.body);
    } else {
        draw_visible_rows(input, viewport, widths, scroll, direction);
    }
    input.canvas.restore();
}

fn draw_visible_rows<Msg>(
    input: &mut TableRenderInput<'_, '_, Msg>,
    viewport: TableViewport,
    widths: &[f32],
    scroll: (f32, f32),
    direction: TableLayoutDirection,
) {
    let rows = input
        .state
        .map(|state| {
            state
                .visible_rows(input.options.metrics(), input.model.rows().len())
                .0
        })
        .unwrap_or_else(|| {
            crate::widgets::table::geometry::visible_row_range(
                scroll.1,
                viewport.body.height,
                input.options.metrics().row_height(),
                input.model.rows().len(),
            )
            .0
        });
    for row in rows {
        draw_row(input, viewport, widths, scroll, direction, row);
    }
}

fn draw_row<Msg>(
    input: &mut TableRenderInput<'_, '_, Msg>,
    viewport: TableViewport,
    widths: &[f32],
    scroll: (f32, f32),
    direction: TableLayoutDirection,
    row_index: usize,
) {
    let row = &input.model.rows()[row_index];
    let row_rect = TableRect::new(
        0.0,
        viewport.body.y + row_index as f32 * input.options.metrics().row_height() - scroll.1,
        viewport.body.width,
        input.options.metrics().row_height(),
    );
    draw_row_background(input, row.key(), row_rect);
    for (column_index, cell) in row.cells().iter().enumerate() {
        let Some(rect) = logical_cell_rect(
            widths,
            column_index,
            Some(row_index),
            viewport,
            scroll,
            direction,
            input.options.metrics(),
        ) else {
            continue;
        };
        draw_cell(
            input,
            cell.text(),
            column_index,
            Some(row.key()),
            rect,
            false,
        );
    }
    draw_horizontal_rule(input.canvas, row_rect, input.theme);
}

fn draw_row_background<Msg>(
    input: &TableRenderInput<'_, '_, Msg>,
    row_key: crate::widgets::table::TableRowKey,
    rect: TableRect,
) {
    let hovered = input.shows_interaction_effects && hovered_row(input, row_key, rect);
    let selected = row_is_selected(input.options.selection(), row_key);
    if !hovered && !selected {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(if selected {
        Theme::alpha(input.theme.primary, 34)
    } else {
        Theme::alpha(input.theme.on_surface, 12)
    });
    input.canvas.draw_rect(to_skia(rect), &paint);
}

fn hovered_row<Msg>(
    input: &TableRenderInput<'_, '_, Msg>,
    row_key: crate::widgets::table::TableRowKey,
    rect: TableRect,
) -> bool {
    input
        .state
        .and_then(|state| state.hovered)
        .is_some_and(|target| target.row == Some(row_key))
        || rect.contains(input.mouse.x, input.mouse.y)
}

fn row_is_selected<Msg>(
    selection: &TableSelection<'_, Msg>,
    key: crate::widgets::table::TableRowKey,
) -> bool {
    match selection {
        TableSelection::None => false,
        TableSelection::Single { selected, .. } => *selected == Some(key),
        TableSelection::Multiple { selected, .. } => selected.contains(&key),
    }
}

fn draw_header<Msg>(
    input: &mut TableRenderInput<'_, '_, Msg>,
    viewport: TableViewport,
    widths: &[f32],
    scroll: (f32, f32),
    direction: TableLayoutDirection,
) {
    input.canvas.save();
    input.canvas.clip_rect(to_skia(viewport.header), None, true);
    draw_header_background(input.canvas, viewport.header, input.theme);
    for (index, column) in input.model.columns().iter().enumerate() {
        let Some(rect) = logical_cell_rect(
            widths,
            index,
            None,
            viewport,
            scroll,
            direction,
            input.options.metrics(),
        ) else {
            continue;
        };
        let label = header_label(input.options, column.key(), column.label());
        draw_cell(input, &label, index, None, rect, true);
    }
    draw_horizontal_rule(input.canvas, viewport.header, input.theme);
    input.canvas.restore();
}

fn draw_header_background(canvas: &Canvas, rect: TableRect, theme: &Theme) {
    let mut paint = Paint::default();
    paint.set_color(Theme::alpha(theme.on_surface, 10));
    canvas.draw_rect(to_skia(rect), &paint);
}

fn header_label<Msg>(
    options: &TableOptions<'_, Msg>,
    column: crate::widgets::table::TableColumnKey,
    label: &str,
) -> String {
    let Some(sort) = options.sorting().and_then(|sorting| sorting.current()) else {
        return label.to_owned();
    };
    if sort.column() != column {
        return label.to_owned();
    }
    let indicator = match sort.direction() {
        TableSortDirection::Ascending => "↑",
        TableSortDirection::Descending => "↓",
    };
    format!("{label} {indicator}")
}

fn draw_cell<Msg>(
    input: &mut TableRenderInput<'_, '_, Msg>,
    text: &str,
    column_index: usize,
    row: Option<crate::widgets::table::TableRowKey>,
    rect: TableRect,
    header: bool,
) {
    draw_vertical_rule(input.canvas, rect, input.theme);
    let text_rect = inset_cell(rect);
    draw_aligned_text(
        input,
        text,
        text_rect,
        input.model.columns()[column_index].alignment(),
        header,
    );
    draw_active_cell_outline(input, column_index, row, rect);
}

fn draw_aligned_text<Msg>(
    input: &mut TableRenderInput<'_, '_, Msg>,
    text: &str,
    rect: TableRect,
    alignment: TableAlignment,
    header: bool,
) {
    let font_size = if header {
        input.theme.font_label.max(13.0)
    } else {
        input.theme.font_body
    };
    let normalized = normalize_text_controls(text, TextControlPolicy::FlattenLineBreaks);
    let font = get_cached_font(input.font_cache, "sans-serif", font_size);
    let mut paint = Paint::default();
    paint.set_color(input.theme.on_surface);
    paint.set_anti_alias(true);
    let measured = measure_single_line_text(&font, &normalized, &paint).min(rect.width);
    let physical = physical_alignment(alignment, input.direction);
    let x = aligned_text_x(rect, measured, physical);
    let y = rect.y + rect.height * 0.5 + font_size * 0.34;
    input.canvas.save();
    input.canvas.clip_rect(to_skia(rect), None, true);
    draw_single_line_text(input.canvas, &normalized, (x, y), &font, &paint);
    input.canvas.restore();
}

fn physical_alignment(alignment: TableAlignment, direction: LayoutDirection) -> TableAlignment {
    match (alignment, direction) {
        (TableAlignment::Start, LayoutDirection::Rtl) => TableAlignment::End,
        (TableAlignment::End, LayoutDirection::Rtl) => TableAlignment::Start,
        _ => alignment,
    }
}

fn aligned_text_x(rect: TableRect, measured: f32, alignment: TableAlignment) -> f32 {
    match alignment {
        TableAlignment::Start => rect.x,
        TableAlignment::Center => rect.x + (rect.width - measured) * 0.5,
        TableAlignment::End => rect.x + rect.width - measured,
    }
}

fn draw_active_cell_outline<Msg>(
    input: &TableRenderInput<'_, '_, Msg>,
    column_index: usize,
    row: Option<crate::widgets::table::TableRowKey>,
    rect: TableRect,
) {
    if !input.focused || !input.shows_interaction_effects {
        return;
    }
    let Some(target) = input.state.and_then(|state| state.active) else {
        return;
    };
    let column = input.model.columns()[column_index].key();
    if target == (TableCellTarget { row, column }) {
        draw_focus_outline(input.canvas, to_skia(rect), 0.0, input.theme);
    }
}

fn draw_empty_state<Msg>(input: &mut TableRenderInput<'_, '_, Msg>, rect: TableRect) {
    draw_aligned_text(
        input,
        input.options.empty_label(),
        inset_cell(rect),
        TableAlignment::Center,
        false,
    );
}

fn draw_scrollbars<Msg>(
    input: &TableRenderInput<'_, '_, Msg>,
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
) {
    let fallback = input.state.map(|state| state.thumbs(direction));
    let thumbs = fallback.unwrap_or_else(|| {
        crate::widgets::table::geometry::scrollbar_thumbs(viewport, scroll, direction)
    });
    if let (Some(track), Some(thumb)) = (viewport.horizontal_track, thumbs.horizontal) {
        draw_scrollbar(input.canvas, track, thumb, input.theme);
    }
    if let (Some(track), Some(thumb)) = (viewport.vertical_track, thumbs.vertical) {
        draw_scrollbar(input.canvas, track, thumb, input.theme);
    }
}

fn draw_scrollbar(canvas: &Canvas, track: TableRect, thumb: TableRect, theme: &Theme) {
    let mut paint = Paint::default();
    paint.set_color(Theme::alpha(theme.on_surface, 18));
    canvas.draw_rect(to_skia(track), &paint);
    paint.set_color(Theme::alpha(theme.on_surface, 96));
    canvas.draw_rrect(RRect::new_rect_xy(to_skia(thumb), 5.0, 5.0), &paint);
}

fn draw_horizontal_rule(canvas: &Canvas, rect: TableRect, theme: &Theme) {
    let mut paint = Paint::default();
    paint.set_color(Theme::alpha(theme.on_surface, 28));
    paint.set_stroke_width(1.0);
    canvas.draw_line(
        (rect.x, rect.y + rect.height),
        (rect.x + rect.width, rect.y + rect.height),
        &paint,
    );
}

fn draw_vertical_rule(canvas: &Canvas, rect: TableRect, theme: &Theme) {
    let mut paint = Paint::default();
    paint.set_color(Theme::alpha(theme.on_surface, 20));
    paint.set_stroke_width(1.0);
    canvas.draw_line(
        (rect.x + rect.width, rect.y),
        (rect.x + rect.width, rect.y + rect.height),
        &paint,
    );
}

fn draw_table_border(canvas: &Canvas, size: (f32, f32), focused: bool, theme: &Theme) {
    if focused {
        draw_focus_outline(canvas, table_rect(size), theme.radius_sm, theme);
        return;
    }
    let mut paint = Paint::default();
    paint.set_style(paint::Style::Stroke);
    paint.set_stroke_width(1.0);
    paint.set_color(Theme::alpha(theme.on_surface, 44));
    canvas.draw_rrect(
        RRect::new_rect_xy(table_rect(size), theme.radius_sm, theme.radius_sm),
        &paint,
    );
}

fn inset_cell(rect: TableRect) -> TableRect {
    TableRect::new(
        rect.x + TABLE_CELL_PADDING_X,
        rect.y,
        (rect.width - TABLE_CELL_PADDING_X * 2.0).max(0.0),
        rect.height,
    )
}

fn table_rect(size: (f32, f32)) -> SkiaRect {
    SkiaRect::from_xywh(0.0, 0.0, size.0.max(0.0), size.1.max(0.0))
}

fn to_skia(rect: TableRect) -> SkiaRect {
    SkiaRect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

fn table_direction(direction: LayoutDirection) -> TableLayoutDirection {
    match direction {
        LayoutDirection::Ltr => TableLayoutDirection::Ltr,
        LayoutDirection::Rtl => TableLayoutDirection::Rtl,
    }
}
