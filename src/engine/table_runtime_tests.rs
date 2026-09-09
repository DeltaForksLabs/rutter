use super::*;
use crate::widgets::table::{
    TableCell, TableColumn, TableColumnWidth, TableOptions, TableRow, TableSelection, TableSort,
    TableSortDirection, TableSorting,
};

#[derive(Debug, PartialEq)]
enum Message {
    Selection(Vec<TableRowKey>),
    Sort(TableSort),
}

fn model() -> TableModel<'static> {
    let columns = vec![
        TableColumn::new(
            TableColumnKey::new(1),
            "Name",
            TableColumnWidth::fixed(100.0).unwrap(),
        )
        .with_sortable(true),
        TableColumn::new(
            TableColumnKey::new(2),
            "State",
            TableColumnWidth::fixed(100.0).unwrap(),
        ),
    ];
    let rows = (1..=5)
        .map(|key| {
            TableRow::new(
                TableRowKey::new(key),
                [TableCell::new(key.to_string()), TableCell::new("Ready")],
            )
        })
        .collect::<Vec<_>>();
    TableModel::new("People", columns, rows).unwrap()
}

fn selection_message(keys: Vec<TableRowKey>) -> Message {
    Message::Selection(keys)
}

fn sort_message(sort: TableSort) -> Message {
    Message::Sort(sort)
}

#[test]
fn runtime_toggles_the_controlled_sort_direction() {
    let model = model();
    let current = TableSort::new(TableColumnKey::new(1), TableSortDirection::Ascending);
    let options =
        TableOptions::default().with_sorting(TableSorting::new(Some(current), sort_message));
    let runtime = TableRuntime::new(&model, &options, vec![100.0, 100.0], 88.0);

    let message = runtime
        .message_for_target(
            TableCellTarget::header(TableColumnKey::new(1)),
            None,
            TableInteractionModifiers {
                toggle: false,
                range: false,
            },
        )
        .unwrap();

    assert_eq!(
        message,
        Message::Sort(TableSort::new(
            TableColumnKey::new(1),
            TableSortDirection::Descending
        ))
    );
}

#[test]
fn runtime_applies_toggle_and_range_selection_in_display_order() {
    let model = model();
    let selected = [TableRowKey::new(2), TableRowKey::new(5)];
    let options = TableOptions::default()
        .with_selection(TableSelection::multiple(&selected, selection_message));
    let runtime = TableRuntime::new(&model, &options, vec![100.0, 100.0], 88.0);

    let toggle = runtime
        .message_for_target(
            TableCellTarget::body(TableRowKey::new(3), TableColumnKey::new(1)),
            Some(TableRowKey::new(2)),
            TableInteractionModifiers {
                toggle: true,
                range: false,
            },
        )
        .unwrap();
    let range = runtime
        .message_for_target(
            TableCellTarget::body(TableRowKey::new(4), TableColumnKey::new(1)),
            Some(TableRowKey::new(2)),
            TableInteractionModifiers {
                toggle: false,
                range: true,
            },
        )
        .unwrap();

    assert_eq!(
        toggle,
        Message::Selection(vec![
            TableRowKey::new(2),
            TableRowKey::new(3),
            TableRowKey::new(5)
        ])
    );
    assert_eq!(
        range,
        Message::Selection(vec![
            TableRowKey::new(2),
            TableRowKey::new(3),
            TableRowKey::new(4)
        ])
    );
}

#[test]
fn runtime_navigation_uses_one_composite_focus_target() {
    let model = model();
    let options = TableOptions::<Message>::default();
    let runtime = TableRuntime::new(&model, &options, vec![100.0, 100.0], 88.0);
    let header = TableCellTarget::header(TableColumnKey::new(1));

    let first_row = runtime
        .move_target(Some(header), TableNavigation::Down, false, false)
        .unwrap();
    let rtl_left = runtime
        .move_target(Some(first_row), TableNavigation::Left, true, false)
        .unwrap();
    let last = runtime
        .move_target(Some(first_row), TableNavigation::End, false, true)
        .unwrap();

    assert_eq!(first_row.row, Some(TableRowKey::new(1)));
    assert_eq!(rtl_left.column, TableColumnKey::new(2));
    assert_eq!(last.row, Some(TableRowKey::new(5)));
    assert_eq!(last.column, TableColumnKey::new(2));
}
