use super::*;
use crate::widgets::table::{TableCell, TableColumnWidth, TableSelection};

fn rows() -> Vec<TableRow<'static>> {
    (1..=4)
        .map(|key| TableRow::new(TableRowKey::new(key), [TableCell::new(key.to_string())]))
        .collect()
}

fn sortable_column() -> TableColumn<'static> {
    TableColumn::new(
        TableColumnKey::new(3),
        "Name",
        TableColumnWidth::fixed(120.0).unwrap(),
    )
    .with_sortable(true)
}

#[test]
fn sorting_starts_ascending_and_toggles_current_column() {
    let column = sortable_column();
    let first = TableSorting::new(None, std::convert::identity)
        .next_sort(&column)
        .unwrap();
    let second = TableSorting::new(Some(first), std::convert::identity)
        .next_sort(&column)
        .unwrap();

    assert_eq!(first.direction(), TableSortDirection::Ascending);
    assert_eq!(second.direction(), TableSortDirection::Descending);
}

#[test]
fn sorting_ignores_columns_not_marked_sortable() {
    let column = sortable_column().with_sortable(false);

    assert!(
        TableSorting::<TableSort>::new(None, std::convert::identity)
            .next_sort(&column)
            .is_none()
    );
}

#[test]
fn multiple_selection_normalizes_unknown_and_duplicate_keys() {
    let rows = rows();
    let selected = [
        TableRowKey::new(3),
        TableRowKey::new(3),
        TableRowKey::new(99),
        TableRowKey::new(1),
    ];
    let selection = TableSelection::<Vec<TableRowKey>>::multiple(&selected, std::convert::identity);

    let selected_rows = selection.selected_rows(&rows);
    assert_eq!(selected_rows[0].key(), TableRowKey::new(1));
    assert_eq!(selected_rows[1].key(), TableRowKey::new(3));
}

#[test]
fn multiple_toggle_and_range_follow_current_display_order() {
    let rows = rows();
    let selected = [TableRowKey::new(2), TableRowKey::new(4)];
    let selection = TableSelection::multiple(&selected, std::convert::identity);

    let toggled = selection
        .message_for_toggle(&rows, TableRowKey::new(3))
        .unwrap();
    let ranged = selection
        .message_for_range(&rows, Some(TableRowKey::new(2)), TableRowKey::new(4))
        .unwrap();

    assert_eq!(
        toggled,
        [
            TableRowKey::new(2),
            TableRowKey::new(3),
            TableRowKey::new(4)
        ]
    );
    assert_eq!(
        ranged,
        [
            TableRowKey::new(2),
            TableRowKey::new(3),
            TableRowKey::new(4)
        ]
    );
}

#[test]
fn single_toggle_can_clear_the_selected_row() {
    let rows = rows();
    let selection = TableSelection::single(Some(TableRowKey::new(2)), std::convert::identity);

    assert_eq!(
        selection.message_for_toggle(&rows, TableRowKey::new(2)),
        Some(None)
    );
}
