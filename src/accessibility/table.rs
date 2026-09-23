// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use accesskit::{Action, Node, NodeId, Role, SortDirection};
use skia_safe::Rect as SkiaRect;

use super::{AccessibilityBuilder, LayoutFrame, access_node_id, access_rect};
use crate::widget::{
    Widget, resolve_table_cell_id, resolve_table_empty_id, resolve_table_empty_row_id,
    resolve_table_header_id, resolve_table_header_row_id, resolve_table_row_id,
};
use crate::widgets::table::geometry::{TableRect, TableViewport};
use crate::widgets::table::{
    TableCellTarget, TableLayoutDirection, TableModel, TableOptions, TableSelection,
    TableSortDirection, allocate_column_widths, calculate_viewport, logical_cell_rect,
    minimum_content_width,
};

pub(super) fn collect<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    widget: &Widget<Msg>,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    frame: LayoutFrame,
    path: &[usize],
) -> Vec<NodeId> {
    let table_id = widget.resolved_id(path).unwrap();
    let geometry = TableAccessibilityGeometry::new(builder, table_id, model, options, frame);
    let header_row = push_header_row(builder, table_id, model, options, geometry);
    let mut children = vec![header_row];
    if model.rows().is_empty() {
        children.push(push_empty_row(builder, table_id, model, options, geometry));
    } else {
        children.extend(push_body_rows(builder, table_id, model, options, geometry));
    }
    push_table_root(builder, table_id, model, options, frame, geometry, children);
    vec![access_node_id(table_id)]
}

#[derive(Clone, Copy)]
struct TableAccessibilityGeometry {
    origin: skia_safe::Point,
    viewport: TableViewport,
    scroll: (f32, f32),
    direction: TableLayoutDirection,
}

impl TableAccessibilityGeometry {
    fn new<Msg>(
        builder: &AccessibilityBuilder<'_, '_>,
        table_id: u64,
        model: &TableModel<'_>,
        options: &TableOptions<'_, Msg>,
        frame: LayoutFrame,
    ) -> Self {
        let size = (
            (frame.rect.x1 - frame.rect.x0) as f32,
            (frame.rect.y1 - frame.rect.y0) as f32,
        );
        let viewport = calculate_viewport(
            size,
            options.metrics(),
            model.rows().len(),
            minimum_content_width(model.columns()),
        );
        let scroll = builder
            .inputs
            .widget_states
            .get(&table_id)
            .and_then(crate::engine::widget_state::WidgetState::as_table)
            .map(|state| (state.scroll_x, state.scroll_y))
            .unwrap_or_default();
        Self {
            origin: frame.origin,
            viewport,
            scroll,
            direction: table_direction(builder.inputs.direction),
        }
    }

    fn widths(self, model: &TableModel<'_>) -> Vec<f32> {
        allocate_column_widths(model.columns(), self.viewport.body.width)
    }

    fn rect(self, rect: TableRect) -> accesskit::Rect {
        access_rect(SkiaRect::from_xywh(
            self.origin.x + rect.x,
            self.origin.y + rect.y,
            rect.width,
            rect.height,
        ))
    }
}

fn push_header_row<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    geometry: TableAccessibilityGeometry,
) -> NodeId {
    let widths = geometry.widths(model);
    let children = model
        .columns()
        .iter()
        .enumerate()
        .filter_map(|(index, column)| {
            let rect = logical_cell_rect(
                &widths,
                index,
                None,
                geometry.viewport,
                geometry.scroll,
                geometry.direction,
                options.metrics(),
            )?;
            Some(push_header_cell(
                builder,
                table_id,
                column,
                index,
                geometry.rect(rect),
                options,
            ))
        })
        .collect::<Vec<_>>();
    let id = access_node_id(resolve_table_header_row_id(table_id));
    let mut node = Node::new(Role::Row);
    node.set_bounds(geometry.rect(geometry.viewport.header));
    node.set_row_index(0);
    node.set_children(children);
    builder.nodes.push((id, node));
    id
}

fn push_header_cell<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    column: &crate::widgets::table::TableColumn<'_>,
    index: usize,
    bounds: accesskit::Rect,
    options: &TableOptions<'_, Msg>,
) -> NodeId {
    let id = access_node_id(resolve_table_header_id(table_id, column.key()));
    let mut node = Node::new(Role::ColumnHeader);
    node.set_bounds(bounds);
    node.set_label(column.label());
    node.set_row_index(0);
    node.set_column_index(index);
    apply_header_sort(&mut node, column, options);
    node.add_action(Action::ScrollIntoView);
    let sortable = options.sorting().is_some() && column.is_sortable();
    if sortable || !matches!(options.selection(), TableSelection::None) {
        node.add_action(Action::Focus);
    }
    if sortable {
        node.add_action(Action::Click);
    }
    builder.nodes.push((id, node));
    id
}

fn apply_header_sort<Msg>(
    node: &mut Node,
    column: &crate::widgets::table::TableColumn<'_>,
    options: &TableOptions<'_, Msg>,
) {
    let Some(sort) = options.sorting().and_then(|sorting| sorting.current()) else {
        return;
    };
    if sort.column() != column.key() {
        return;
    }
    node.set_sort_direction(match sort.direction() {
        TableSortDirection::Ascending => SortDirection::Ascending,
        TableSortDirection::Descending => SortDirection::Descending,
    });
}

fn push_body_rows<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    geometry: TableAccessibilityGeometry,
) -> Vec<NodeId> {
    let widths = geometry.widths(model);
    model
        .rows()
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            push_body_row(
                builder, table_id, model, options, geometry, &widths, row_index, row,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn push_body_row<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    geometry: TableAccessibilityGeometry,
    widths: &[f32],
    row_index: usize,
    row: &crate::widgets::table::TableRow<'_>,
) -> NodeId {
    let cells = row
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(column_index, cell)| {
            let rect = logical_cell_rect(
                widths,
                column_index,
                Some(row_index),
                geometry.viewport,
                geometry.scroll,
                geometry.direction,
                options.metrics(),
            )?;
            Some(push_body_cell(
                builder,
                table_id,
                model,
                options,
                row,
                cell,
                row_index,
                column_index,
                geometry.rect(rect),
            ))
        })
        .collect::<Vec<_>>();
    push_row_node(builder, table_id, row, row_index, geometry, options, cells)
}

#[allow(clippy::too_many_arguments)]
fn push_body_cell<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    row: &crate::widgets::table::TableRow<'_>,
    cell: &crate::widgets::table::TableCell<'_>,
    row_index: usize,
    column_index: usize,
    bounds: accesskit::Rect,
) -> NodeId {
    let column = &model.columns()[column_index];
    let role = cell_role(options, column.is_row_header());
    let id = access_node_id(resolve_table_cell_id(table_id, row.key(), column.key()));
    let mut node = Node::new(role);
    node.set_bounds(bounds);
    node.set_label(cell.accessibility_label().unwrap_or(cell.text()));
    node.set_row_index(row_index + 1);
    node.set_column_index(column_index);
    node.set_selected(row_is_selected(options.selection(), row.key()));
    node.add_action(Action::ScrollIntoView);
    if !matches!(options.selection(), TableSelection::None) {
        node.add_action(Action::Focus);
        node.add_action(Action::Click);
    }
    builder.nodes.push((id, node));
    id
}

fn push_row_node<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    row: &crate::widgets::table::TableRow<'_>,
    row_index: usize,
    geometry: TableAccessibilityGeometry,
    options: &TableOptions<'_, Msg>,
    children: Vec<NodeId>,
) -> NodeId {
    let y = geometry.viewport.body.y + row_index as f32 * options.metrics().row_height()
        - geometry.scroll.1;
    let rect = TableRect::new(
        0.0,
        y,
        geometry.viewport.body.width,
        options.metrics().row_height(),
    );
    let id = access_node_id(resolve_table_row_id(table_id, row.key()));
    let mut node = Node::new(Role::Row);
    node.set_bounds(geometry.rect(rect));
    node.set_row_index(row_index + 1);
    node.set_selected(row_is_selected(options.selection(), row.key()));
    node.set_children(children);
    if !matches!(options.selection(), TableSelection::None) {
        node.add_action(Action::Focus);
        node.add_action(Action::Click);
        node.add_action(Action::ScrollIntoView);
    }
    builder.nodes.push((id, node));
    id
}

fn push_empty_cell<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    geometry: TableAccessibilityGeometry,
) -> NodeId {
    let id = access_node_id(resolve_table_empty_id(table_id));
    let mut node = Node::new(cell_role(options, false));
    node.set_bounds(geometry.rect(geometry.viewport.body));
    node.set_label(options.empty_label());
    node.set_row_index(1);
    node.set_column_index(0);
    node.set_column_span(model.columns().len());
    builder.nodes.push((id, node));
    id
}

fn push_empty_row<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    geometry: TableAccessibilityGeometry,
) -> NodeId {
    let cell = push_empty_cell(builder, table_id, model, options, geometry);
    let id = access_node_id(resolve_table_empty_row_id(table_id));
    let mut node = Node::new(Role::Row);
    node.set_bounds(geometry.rect(geometry.viewport.body));
    node.set_row_index(1);
    node.set_children(vec![cell]);
    builder.nodes.push((id, node));
    id
}

fn push_table_root<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    frame: LayoutFrame,
    geometry: TableAccessibilityGeometry,
    children: Vec<NodeId>,
) {
    let selectable = !matches!(options.selection(), TableSelection::None);
    let mut node = Node::new(if selectable { Role::Grid } else { Role::Table });
    node.set_bounds(frame.rect);
    node.set_label(model.accessibility_label());
    node.set_row_count(model.rows().len().max(1) + 1);
    node.set_column_count(model.columns().len());
    node.set_children(children);
    apply_root_interaction(&mut node, builder, table_id, model, options);
    node.set_scroll_x(geometry.scroll.0 as f64);
    node.set_scroll_x_min(0.0);
    node.set_scroll_x_max(geometry.viewport.max_scroll_x() as f64);
    node.set_scroll_y(geometry.scroll.1 as f64);
    node.set_scroll_y_min(0.0);
    node.set_scroll_y_max(geometry.viewport.max_scroll_y() as f64);
    builder.nodes.push((access_node_id(table_id), node));
}

fn apply_root_interaction<Msg>(
    node: &mut Node,
    builder: &AccessibilityBuilder<'_, '_>,
    table_id: u64,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
) {
    if matches!(options.selection(), TableSelection::Multiple { .. }) {
        node.set_multiselectable();
    }
    if !table_is_interactive(model, options) {
        return;
    }
    node.add_action(Action::Focus);
    let active = builder
        .inputs
        .widget_states
        .get(&table_id)
        .and_then(crate::engine::widget_state::WidgetState::as_table)
        .and_then(|state| state.active);
    if let Some(target) = active {
        node.set_active_descendant(access_node_id(target_id(table_id, target)));
    }
}

fn target_id(table_id: u64, target: TableCellTarget) -> u64 {
    match target.row {
        Some(row) => resolve_table_cell_id(table_id, row, target.column),
        None => resolve_table_header_id(table_id, target.column),
    }
}

fn table_is_interactive<Msg>(model: &TableModel<'_>, options: &TableOptions<'_, Msg>) -> bool {
    !matches!(options.selection(), TableSelection::None)
        || options.sorting().is_some() && model.columns().iter().any(|column| column.is_sortable())
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

fn cell_role<Msg>(options: &TableOptions<'_, Msg>, row_header: bool) -> Role {
    if row_header {
        return Role::RowHeader;
    }
    if matches!(options.selection(), TableSelection::None) {
        Role::Cell
    } else {
        Role::GridCell
    }
}

fn table_direction(direction: crate::i18n::LayoutDirection) -> TableLayoutDirection {
    match direction {
        crate::i18n::LayoutDirection::Ltr => TableLayoutDirection::Ltr,
        crate::i18n::LayoutDirection::Rtl => TableLayoutDirection::Rtl,
    }
}
