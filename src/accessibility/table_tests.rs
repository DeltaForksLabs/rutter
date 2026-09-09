use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use accesskit::{Action, Node, NodeId, Role, SortDirection, TreeUpdate};
use cosmic_text::FontSystem;
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

use super::*;
use crate::engine::widget_state::WidgetState;
use crate::layout::{build_taffy_tree, compute_layout};
use crate::widget::{resolve_table_cell_id, resolve_table_header_id};
use crate::widgets::table::{
    TableCell, TableCellTarget, TableColumn, TableColumnKey, TableColumnWidth, TableModel,
    TableOptions, TableRow, TableRowKey, TableSelection, TableSort, TableSortDirection,
    TableSorting, calculate_viewport, minimum_content_width,
};

fn table_model(rows: usize) -> TableModel<'static> {
    let columns = vec![
        TableColumn::new(
            TableColumnKey::new(1),
            "Name",
            TableColumnWidth::fixed(120.0).unwrap(),
        )
        .with_row_header(true)
        .with_sortable(true),
        TableColumn::new(
            TableColumnKey::new(2),
            "State",
            TableColumnWidth::flex(100.0, 1).unwrap(),
        ),
    ];
    let rows = (0..rows)
        .map(|index| {
            TableRow::new(
                TableRowKey::new(index as u64 + 10),
                [
                    TableCell::new(format!("Person {index}")),
                    TableCell::new("Ready").with_accessibility_label("State: ready"),
                ],
            )
        })
        .collect::<Vec<_>>();
    TableModel::new("People", columns, rows).unwrap()
}

fn table_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(200.0),
            height: Dimension::length(132.0),
        },
        ..Style::default()
    }
}

fn table_update<Msg>(widget: &Widget<'_, Msg>, states: &HashMap<u64, WidgetState>) -> TreeUpdate {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(200, 132),
        fonts,
        &crate::render::RichTextRenderer::default(),
    );
    build_accessibility_update(
        &taffy,
        widget,
        root,
        AccessibilityInputs {
            input_states: &HashMap::new(),
            widget_states: states,
            focused_widget_id: Some(77),
            viewport: (200.0, 132.0),
            direction: LayoutDirection::Ltr,
        },
    )
}

fn node_for(update: &TreeUpdate, role: Role) -> &Node {
    update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == role).then_some(node))
        .unwrap()
}

fn node_by_id(update: &TreeUpdate, id: u64) -> &Node {
    update
        .nodes
        .iter()
        .find_map(|(node_id, node)| (*node_id == NodeId(id)).then_some(node))
        .unwrap()
}

#[test]
fn passive_table_exposes_headers_rows_and_row_header_semantics() {
    let widget: Widget<'_, ()> = Widget::table(table_model(2), table_style()).with_id(77);
    let update = table_update(&widget, &HashMap::new());
    let table = node_for(&update, Role::Table);
    let row_header = node_for(&update, Role::RowHeader);
    let state_cell = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.label() == Some("State: ready")).then_some(node))
        .unwrap();

    assert_eq!(table.label(), Some("People"));
    assert_eq!(table.row_count(), Some(3));
    assert_eq!(table.column_count(), Some(2));
    assert_eq!(row_header.row_index(), Some(1));
    assert_eq!(state_cell.role(), Role::Cell);
    assert_eq!(state_cell.column_index(), Some(1));
}

#[test]
fn selectable_table_exposes_grid_selection_and_active_descendant() {
    let model = table_model(2);
    let selected = [TableRowKey::new(11)];
    let options = TableOptions::default()
        .with_selection(TableSelection::multiple(&selected, std::convert::identity));
    let mut state = crate::widgets::table::TableState::default();
    let viewport = calculate_viewport(
        (200.0, 132.0),
        options.metrics(),
        model.rows().len(),
        minimum_content_width(model.columns()),
    );
    state.sync_viewport(viewport);
    state.active = Some(TableCellTarget::body(
        TableRowKey::new(11),
        TableColumnKey::new(2),
    ));
    let states = HashMap::from([(77, WidgetState::Table(state))]);
    let widget = Widget::table_with_options(model, options, table_style()).with_id(77);
    let update = table_update(&widget, &states);
    let grid = node_for(&update, Role::Grid);
    let active_id = resolve_table_cell_id(77, TableRowKey::new(11), TableColumnKey::new(2));
    let active = node_by_id(&update, active_id);

    assert!(grid.is_multiselectable());
    assert_eq!(grid.active_descendant(), Some(NodeId(active_id)));
    assert_eq!(active.is_selected(), Some(true));
    assert!(active.supports_action(Action::Focus));
    assert!(active.supports_action(Action::Click));
    assert!(active.supports_action(Action::ScrollIntoView));
}

#[test]
fn sorted_header_exposes_direction_and_activation() {
    let model = table_model(1);
    let sort = TableSort::new(TableColumnKey::new(1), TableSortDirection::Descending);
    let options =
        TableOptions::default().with_sorting(TableSorting::new(Some(sort), std::convert::identity));
    let widget = Widget::table_with_options(model, options, table_style()).with_id(77);
    let update = table_update(&widget, &HashMap::new());
    let header = node_by_id(&update, resolve_table_header_id(77, TableColumnKey::new(1)));

    assert_eq!(header.sort_direction(), Some(SortDirection::Descending));
    assert!(header.supports_action(Action::Focus));
    assert!(header.supports_action(Action::Click));
}

#[test]
fn empty_table_exposes_a_spanning_placeholder_row() {
    let options = TableOptions::<()>::default().with_empty_label("No people");
    let widget = Widget::table_with_options(table_model(0), options, table_style()).with_id(77);
    let update = table_update(&widget, &HashMap::new());
    let empty = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.label() == Some("No people")).then_some(node))
        .unwrap();

    assert_eq!(node_for(&update, Role::Table).row_count(), Some(2));
    assert_eq!(empty.column_span(), Some(2));
    assert_eq!(empty.row_index(), Some(1));
}
