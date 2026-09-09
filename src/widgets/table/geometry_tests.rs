use super::*;
use crate::widgets::table::{TableColumnKey, TableColumnWidth};

fn fixed_column(key: u64, width: f32) -> TableColumn<'static> {
    TableColumn::new(
        TableColumnKey::new(key),
        format!("Column {key}"),
        TableColumnWidth::fixed(width).unwrap(),
    )
}

fn flex_column(key: u64, minimum: f32, weight: u16) -> TableColumn<'static> {
    TableColumn::new(
        TableColumnKey::new(key),
        format!("Column {key}"),
        TableColumnWidth::flex(minimum, weight).unwrap(),
    )
}

#[test]
fn flex_columns_share_only_remaining_width_by_weight() {
    let columns = [
        fixed_column(1, 100.0),
        flex_column(2, 50.0, 1),
        flex_column(3, 50.0, 3),
    ];

    assert_eq!(
        allocate_column_widths(&columns, 400.0),
        [100.0, 100.0, 200.0]
    );
}

#[test]
fn scrollbar_interdependency_reduces_both_viewport_axes() {
    let viewport = calculate_viewport((200.0, 144.0), TableMetrics::default(), 3, 200.0);

    assert!(viewport.horizontal_track.is_some());
    assert!(viewport.vertical_track.is_some());
    assert_eq!(viewport.body.width, 188.0);
    assert_eq!(viewport.body.height, 88.0);
}

#[test]
fn visible_rows_include_one_overscan_row() {
    let rows = visible_row_range(44.0, 88.0, 44.0, 10);

    assert_eq!(rows.0, 1..4);
}

#[test]
fn logical_columns_reverse_physical_placement_in_rtl() {
    let viewport = calculate_viewport((200.0, 132.0), TableMetrics::default(), 1, 200.0);
    let widths = [80.0, 120.0];
    let ltr = logical_cell_rect(
        &widths,
        0,
        None,
        viewport,
        (0.0, 0.0),
        TableLayoutDirection::Ltr,
        TableMetrics::default(),
    )
    .unwrap();
    let rtl = logical_cell_rect(
        &widths,
        0,
        None,
        viewport,
        (0.0, 0.0),
        TableLayoutDirection::Rtl,
        TableMetrics::default(),
    )
    .unwrap();

    assert_eq!(ltr.x, 0.0);
    assert_eq!(rtl.x, 120.0);
}

#[test]
fn hit_testing_distinguishes_headers_cells_and_empty_state() {
    let metrics = TableMetrics::default();
    let viewport = calculate_viewport((200.0, 132.0), metrics, 1, 200.0);
    let widths = [100.0, 100.0];

    assert_eq!(
        hit_test(
            (150.0, 20.0),
            &widths,
            1,
            viewport,
            (0.0, 0.0),
            TableLayoutDirection::Ltr,
            metrics,
        ),
        Some(TableHit::Header { column: 1 })
    );
    assert_eq!(
        hit_test(
            (20.0, 60.0),
            &widths,
            1,
            viewport,
            (0.0, 0.0),
            TableLayoutDirection::Ltr,
            metrics,
        ),
        Some(TableHit::Cell { row: 0, column: 0 })
    );

    let empty = calculate_viewport((200.0, 132.0), metrics, 0, 200.0);
    assert_eq!(
        hit_test(
            (20.0, 60.0),
            &widths,
            0,
            empty,
            (0.0, 0.0),
            TableLayoutDirection::Ltr,
            metrics,
        ),
        Some(TableHit::EmptyState)
    );
}

#[test]
fn thumbs_respect_minimum_size_and_rtl_origin() {
    let viewport = calculate_viewport((100.0, 100.0), TableMetrics::default(), 20, 1000.0);
    let ltr = scrollbar_thumbs(viewport, (0.0, 0.0), TableLayoutDirection::Ltr);
    let rtl = scrollbar_thumbs(viewport, (0.0, 0.0), TableLayoutDirection::Rtl);

    assert!(ltr.horizontal.unwrap().width >= 20.0);
    assert!(ltr.vertical.unwrap().height >= 20.0);
    assert!(rtl.horizontal.unwrap().x > ltr.horizontal.unwrap().x);
}

#[test]
fn reveal_offset_moves_only_when_item_is_outside_viewport() {
    assert_eq!(reveal_offset(20.0, 100.0, 30.0, 50.0, 400.0), 20.0);
    assert_eq!(reveal_offset(20.0, 100.0, 150.0, 180.0, 400.0), 80.0);
    assert_eq!(reveal_offset(80.0, 100.0, 10.0, 30.0, 400.0), 10.0);
}
