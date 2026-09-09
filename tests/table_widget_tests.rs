use rutter::{
    TableAlignment, TableCell, TableColumn, TableColumnKey, TableColumnWidth, TableModel,
    TableOptions, TableRow, TableRowKey, TableSelection, TableSort, TableSortDirection,
    TableSorting, Widget, WidgetIdSnapshot,
};
use taffy::prelude::Style;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Message {
    Selection(Vec<TableRowKey>),
    Sort(TableSort),
}

fn table_model() -> TableModel<'static> {
    TableModel::new(
        "People",
        vec![
            TableColumn::new(
                TableColumnKey::new(1),
                "Name",
                TableColumnWidth::fixed(160.0).unwrap(),
            )
            .with_row_header(true)
            .with_sortable(true),
            TableColumn::new(
                TableColumnKey::new(2),
                "Tasks",
                TableColumnWidth::flex(80.0, 1).unwrap(),
            )
            .with_alignment(TableAlignment::End),
        ],
        vec![TableRow::new(
            TableRowKey::new(10),
            [TableCell::new("Ada"), TableCell::new("4")],
        )],
    )
    .unwrap()
}

#[test]
fn table_constructor_is_available_through_the_public_api() {
    let widget: Widget<'_, ()> = Widget::table(table_model(), Style::default()).with_id(91);

    assert!(WidgetIdSnapshot::capture(&widget).is_ok());
    let Widget::Table {
        id, model, options, ..
    } = widget
    else {
        panic!("expected a Table widget");
    };
    assert_eq!(id, 91);
    assert_eq!(model.accessibility_label(), "People");
    assert_eq!(model.columns()[1].alignment(), TableAlignment::End);
    assert!(matches!(options.selection(), TableSelection::None));
}

#[test]
fn configured_table_exposes_controlled_selection_and_sort_messages() {
    let selected = [TableRowKey::new(10)];
    let options = TableOptions::default()
        .with_selection(TableSelection::multiple(&selected, Message::Selection))
        .with_sorting(TableSorting::new(None, Message::Sort));
    let widget = Widget::table_with_options(table_model(), options, Style::default());
    let Widget::Table { model, options, .. } = widget else {
        panic!("expected a configured Table widget");
    };

    assert_eq!(
        options
            .selection()
            .message_for_toggle(model.rows(), TableRowKey::new(10),),
        Some(Message::Selection(Vec::new()))
    );
    assert_eq!(
        options
            .sorting()
            .unwrap()
            .message_for_activation(&model.columns()[0]),
        Some(Message::Sort(TableSort::new(
            TableColumnKey::new(1),
            TableSortDirection::Ascending,
        )))
    );
}
