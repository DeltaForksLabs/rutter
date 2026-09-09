use super::*;

fn column(key: u64) -> TableColumn<'static> {
    TableColumn::new(
        TableColumnKey::new(key),
        format!("Column {key}"),
        TableColumnWidth::fixed(80.0).unwrap(),
    )
}

fn row(key: u64, count: usize) -> TableRow<'static> {
    let cells = (0..count)
        .map(|index| TableCell::new(format!("Cell {index}")))
        .collect::<Vec<_>>();
    TableRow::new(TableRowKey::new(key), cells)
}

#[test]
fn model_accepts_stable_rectangular_content() {
    let model = TableModel::new("People", vec![column(1), column(2)], vec![row(7, 2)]).unwrap();

    assert_eq!(model.columns().len(), 2);
    assert_eq!(model.rows()[0].key(), TableRowKey::new(7));
}

#[test]
fn model_rejects_blank_accessibility_label() {
    let error = TableModel::new("  ", vec![column(1)], Vec::new()).unwrap_err();

    assert!(matches!(
        error,
        TableConfigError::EmptyAccessibilityLabel { value } if value == "  "
    ));
}

#[test]
fn model_rejects_duplicate_column_and_row_keys() {
    let duplicate_column =
        TableModel::new("Items", vec![column(1), column(1)], Vec::new()).unwrap_err();
    let duplicate_row =
        TableModel::new("Items", vec![column(1)], vec![row(2, 1), row(2, 1)]).unwrap_err();

    assert!(matches!(
        duplicate_column,
        TableConfigError::DuplicateColumnKey { key, .. } if key == TableColumnKey::new(1)
    ));
    assert!(matches!(
        duplicate_row,
        TableConfigError::DuplicateRowKey { key, .. } if key == TableRowKey::new(2)
    ));
}

#[test]
fn model_rejects_more_than_one_row_header_column() {
    let columns = vec![
        column(1).with_row_header(true),
        column(2).with_row_header(true),
    ];

    assert!(matches!(
        TableModel::new("People", columns, Vec::new()),
        Err(TableConfigError::MultipleRowHeaderColumns { .. })
    ));
}

#[test]
fn model_rejects_non_rectangular_rows_with_precise_context() {
    let error = TableModel::new("People", vec![column(1), column(2)], vec![row(9, 1)]).unwrap_err();

    assert!(matches!(
        error,
        TableConfigError::CellCountMismatch {
            row_key,
            actual: 1,
            expected: 2,
            ..
        } if row_key == TableRowKey::new(9)
    ));
}
