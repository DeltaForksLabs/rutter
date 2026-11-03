use super::*;
use crate::table::{TableColumnKey, TableColumnWidth, TableOptions, TableSortDirection};

#[test]
fn captured_table_helpers_match_legacy_source_order_and_unknown_key_semantics() {
    let rows: Vec<_> = [30, 10, 20]
        .into_iter()
        .map(|key| TableRow::new(TableRowKey::new(key), Vec::new()))
        .collect();
    let selected = [
        TableRowKey::new(20),
        TableRowKey::new(99),
        TableRowKey::new(20),
    ];
    let legacy = TableSelection::multiple(&selected, std::convert::identity);
    let captured = TableSelectionWith::multiple(&selected, std::convert::identity);
    assert_eq!(
        captured
            .selected_rows(&rows)
            .iter()
            .map(|row| row.key())
            .collect::<Vec<_>>(),
        vec![TableRowKey::new(20)]
    );
    for key in [10, 20, 99].map(TableRowKey::new) {
        assert_eq!(
            captured.message_for_plain_click(&rows, key),
            legacy.message_for_plain_click(&rows, key)
        );
        assert_eq!(
            captured.message_for_toggle(&rows, key),
            legacy.message_for_toggle(&rows, key)
        );
        for anchor in [None, Some(TableRowKey::new(30)), Some(TableRowKey::new(99))] {
            assert_eq!(
                captured.message_for_range(&rows, anchor, key),
                legacy.message_for_range(&rows, anchor, key)
            );
        }
    }
    for selected in [None, Some(TableRowKey::new(10)), Some(TableRowKey::new(99))] {
        let legacy = TableSelection::single(selected, std::convert::identity);
        let captured = TableSelectionWith::single(selected, std::convert::identity);
        for key in [10, 99].map(TableRowKey::new) {
            assert_eq!(
                captured.message_for_toggle(&rows, key),
                legacy.message_for_toggle(&rows, key)
            );
            assert_eq!(
                captured.message_for_range(&rows, selected, key),
                legacy.message_for_range(&rows, selected, key)
            );
        }
    }
    assert_eq!(
        TableSelectionWith::<()>::none().message_for_plain_click(&rows, TableRowKey::new(10)),
        None
    );
}

#[test]
fn captured_table_replacement_order_clears_inactive_callbacks_and_read_views() {
    let owner = std::sync::Arc::new(());
    let weak = std::sync::Arc::downgrade(&owner);
    let selection = TableSelectionWith::single(None, move |_| {
        let _keep_alive = &owner;
    });
    let options = TableOptions::default().selection_with(selection);
    assert!(matches!(options.selection(), TableSelection::None));
    assert!(matches!(
        options.effective_selection(),
        TableSelectionWith::Single { .. }
    ));
    let options = options.with_selection(TableSelection::none());
    assert!(weak.upgrade().is_none());
    assert!(matches!(
        options.effective_selection(),
        TableSelectionWith::None
    ));
    let options =
        options.with_selection(TableSelection::single(Some(TableRowKey::new(10)), |_| ()));
    let options = options.selection_with(TableSelectionWith::multiple(&[], |_| ()));
    assert!(matches!(options.selection(), TableSelection::None));
    assert!(matches!(
        options.effective_selection(),
        TableSelectionWith::Multiple { .. }
    ));

    let column = TableColumn::new(
        TableColumnKey::new(1),
        "Name",
        TableColumnWidth::fixed(80.0).unwrap(),
    )
    .with_sortable(true);
    let ascending = TableSort::new(column.key(), TableSortDirection::Ascending);
    let owner = std::sync::Arc::new(());
    let weak = std::sync::Arc::downgrade(&owner);
    let options = options
        .with_sorting(TableSorting::new(None, |_| ()))
        .sorting_with(TableSortingWith::new(Some(ascending), move |_| {
            let _keep_alive = &owner;
        }));
    assert!(options.sorting().is_none());
    assert_eq!(
        options
            .effective_sorting()
            .unwrap()
            .next_sort(&column)
            .unwrap()
            .direction(),
        TableSortDirection::Descending
    );
    let options = options.with_sorting(TableSorting::new(None, |_| ()));
    assert!(weak.upgrade().is_none());
    assert_eq!(options.effective_sorting().unwrap().current(), None);
    assert!(options.sorting().is_some());
}

#[test]
fn captured_table_legacy_copy_const_and_message_independent_clone() {
    const NONE: TableSelection<'static, ()> = TableSelection::none();
    const SINGLE: TableSelection<'static, ()> = TableSelection::single(None, |_| ());
    const MULTIPLE: TableSelection<'static, ()> = TableSelection::multiple(&[], |_| ());
    const SORTING: TableSorting<()> = TableSorting::new(None, |_| ());
    const CURRENT: Option<TableSort> = SORTING.current();
    fn copy<T: Copy>(value: T) -> (T, T) {
        (value, value)
    }
    let _ = (copy(NONE), copy(SINGLE), copy(MULTIPLE), copy(SORTING));
    assert_eq!(CURRENT, None);
    struct NonClone;
    let capture = NonClone;
    let policy = TableSelectionWith::single(None, move |_| {
        let _keep_alive = &capture;
        NonClone
    });
    let _ = policy.clone();
    let capture = NonClone;
    let sorting = TableSortingWith::new(None, move |_| {
        let _keep_alive = &capture;
        NonClone
    });
    let _ = sorting.clone();
}
