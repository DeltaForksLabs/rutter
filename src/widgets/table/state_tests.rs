use super::*;
use crate::widgets::table::{
    TableCell, TableColumn, TableColumnWidth, TableRow, allocate_column_widths, calculate_viewport,
    minimum_content_width,
};

fn model(row_keys: &[u64]) -> TableModel<'static> {
    let columns = vec![
        TableColumn::new(
            TableColumnKey::new(1),
            "Name",
            TableColumnWidth::fixed(120.0).unwrap(),
        ),
        TableColumn::new(
            TableColumnKey::new(2),
            "State",
            TableColumnWidth::fixed(120.0).unwrap(),
        ),
    ];
    let rows = row_keys
        .iter()
        .map(|key| {
            TableRow::new(
                TableRowKey::new(*key),
                [TableCell::new("Ada"), TableCell::new("Ready")],
            )
        })
        .collect::<Vec<_>>();
    TableModel::new("People", columns, rows).unwrap()
}

fn synchronized_state(model: &TableModel<'_>) -> TableState {
    let viewport = calculate_viewport(
        (160.0, 132.0),
        TableMetrics::default(),
        model.rows().len(),
        minimum_content_width(model.columns()),
    );
    let mut state = TableState::default();
    state.sync_viewport(viewport);
    state
}

#[test]
fn geometry_sync_clamps_offsets_after_content_shrinks() {
    let model = model(&[1, 2, 3, 4]);
    let mut state = synchronized_state(&model);
    state.scroll_by(500.0, 500.0);
    let smaller = calculate_viewport((400.0, 400.0), TableMetrics::default(), 1, 240.0);

    state.sync_viewport(smaller);

    assert_eq!((state.scroll_x, state.scroll_y), (0.0, 0.0));
}

#[test]
fn reconciliation_removes_targets_for_deleted_keys() {
    let original = model(&[1, 2]);
    let mut state = synchronized_state(&original);
    state.active = Some(TableCellTarget::body(
        TableRowKey::new(2),
        TableColumnKey::new(1),
    ));
    state.selection_anchor = Some(TableRowKey::new(2));

    state.reconcile(&model(&[1]));

    assert!(state.active.is_none());
    assert!(state.selection_anchor.is_none());
}

#[test]
fn reveal_target_scrolls_both_axes_to_a_keyed_cell() {
    let model = model(&[1, 2, 3, 4]);
    let widths = allocate_column_widths(model.columns(), 148.0);
    let mut state = synchronized_state(&model);
    state.reveal_position(1, Some(3), &widths, TableMetrics::default().row_height());

    assert!(state.scroll_x > 0.0);
    assert!(state.scroll_y > 0.0);
}

#[test]
fn visible_rows_and_thumbs_share_synchronized_geometry() {
    let model = model(&[1, 2, 3, 4]);
    let state = synchronized_state(&model);

    assert_eq!(state.visible_rows(TableMetrics::default(), 4).0, 0..3);
    assert!(state.thumbs(TableLayoutDirection::Ltr).horizontal.is_some());
    assert!(state.thumbs(TableLayoutDirection::Ltr).vertical.is_some());
}
