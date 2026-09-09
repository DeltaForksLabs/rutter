// ============================================================
// Rutter Framework — demos/table_demo.rs
// Stable keys keep controlled selection and sorting independent of row order.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    AppLogic, RutterRunner, TableAlignment, TableCell, TableColumn, TableColumnKey,
    TableColumnWidth, TableModel, TableOptions, TableRow, TableRowKey, TableSelection, TableSort,
    TableSortDirection, TableSorting, Theme, Widget,
};
use taffy::prelude::*;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const NAME_COLUMN: TableColumnKey = TableColumnKey::new(1);
const ROLE_COLUMN: TableColumnKey = TableColumnKey::new(2);
const STATUS_COLUMN: TableColumnKey = TableColumnKey::new(3);

struct PersonRow {
    key: TableRowKey,
    name: &'static str,
    role: &'static str,
    status: &'static str,
}

pub struct TableDemoState {
    pub theme: ExampleTheme,
    rows: Vec<PersonRow>,
    selected_rows: Vec<TableRowKey>,
    sort: Option<TableSort>,
}

impl Default for TableDemoState {
    fn default() -> Self {
        Self {
            theme: ExampleTheme::default(),
            rows: example_rows(),
            selected_rows: Vec::new(),
            sort: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeSelected(ExampleTheme),
    RowsSelected(Vec<TableRowKey>),
    SortRequested(TableSort),
}

pub struct TableDemo;

impl AppLogic for TableDemo {
    type State = TableDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TableDemoState::default()
    }

    fn view<'a>(state: &'a mut TableDemoState) -> Widget<'a, Msg> {
        let options = TableOptions::default()
            .with_selection(TableSelection::multiple(
                &state.selected_rows,
                Msg::RowsSelected,
            ))
            .with_sorting(TableSorting::new(state.sort, Msg::SortRequested))
            .with_empty_label("No people match the current view");
        Widget::Column {
            style: page_style(),
            children: vec![
                example_theme_selector(state.theme, Msg::ThemeSelected),
                description(),
                Widget::table_with_options(table_model(&state.rows), options, table_style())
                    .with_id(330),
            ],
        }
    }

    fn update(state: &mut TableDemoState, message: Msg, _: &mut Clipboard) {
        match message {
            Msg::ThemeSelected(theme) => state.theme = theme,
            Msg::RowsSelected(rows) => state.selected_rows = rows,
            Msg::SortRequested(sort) => {
                state.sort = Some(sort);
                sort_people(&mut state.rows, sort);
            }
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn table_model(rows: &[PersonRow]) -> TableModel<'_> {
    let body = rows
        .iter()
        .map(|person| {
            TableRow::new(
                person.key,
                [
                    TableCell::new(person.name),
                    TableCell::new(person.role),
                    TableCell::new(person.status),
                ],
            )
        })
        .collect::<Vec<_>>();
    TableModel::new("Team directory", table_columns(), body)
        .expect("table demo uses unique keys, one row header, and three cells per row")
}

fn table_columns() -> Vec<TableColumn<'static>> {
    vec![
        TableColumn::new(NAME_COLUMN, "Name", TableColumnWidth::fixed(190.0).unwrap())
            .with_row_header(true)
            .with_sortable(true),
        TableColumn::new(
            ROLE_COLUMN,
            "Role",
            TableColumnWidth::flex(220.0, 1).unwrap(),
        )
        .with_sortable(true),
        TableColumn::new(
            STATUS_COLUMN,
            "Status",
            TableColumnWidth::fixed(120.0).unwrap(),
        )
        .with_alignment(TableAlignment::Center)
        .with_sortable(true),
    ]
}

fn description<'a>() -> Widget<'a, Msg> {
    Widget::Text {
        content: "Table — sort columns, select rows with Ctrl/Command, extend ranges with Shift, and scroll in both axes.".into(),
        style: Style::default(),
        color: None,
        size: 14.0,
    }
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        size: Size::percent(1.0_f32),
        padding: Rect::length(24.0_f32),
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(16.0),
        },
        ..Style::default()
    }
}

fn table_style() -> Style {
    responsive_width(760.0, Dimension::length(420.0))
}

fn sort_people(rows: &mut [PersonRow], sort: TableSort) {
    rows.sort_by(|left, right| {
        person_value(left, sort.column()).cmp(person_value(right, sort.column()))
    });
    if sort.direction() == TableSortDirection::Descending {
        rows.reverse();
    }
}

fn person_value(person: &PersonRow, column: TableColumnKey) -> &'static str {
    match column {
        NAME_COLUMN => person.name,
        ROLE_COLUMN => person.role,
        STATUS_COLUMN => person.status,
        _ => "",
    }
}

fn example_rows() -> Vec<PersonRow> {
    [
        (10, "Ada Lovelace", "Architecture", "Active"),
        (11, "Alan Turing", "Research", "Active"),
        (12, "Grace Hopper", "Compilers", "Away"),
        (13, "Katherine Johnson", "Navigation", "Active"),
        (14, "Edsger Dijkstra", "Algorithms", "Focus"),
        (15, "Margaret Hamilton", "Reliability", "Active"),
        (16, "Barbara Liskov", "Languages", "Away"),
        (17, "Donald Knuth", "Analysis", "Focus"),
        (18, "Radia Perlman", "Networks", "Active"),
        (19, "Frances Allen", "Optimization", "Away"),
        (20, "John Backus", "Languages", "Focus"),
        (21, "Mary Jackson", "Engineering", "Active"),
    ]
    .into_iter()
    .map(|(key, name, role, status)| PersonRow {
        key: TableRowKey::new(key),
        name,
        role,
        status,
    })
    .collect()
}

pub fn run() {
    RutterRunner::<TableDemo>::run();
}

#[cfg(test)]
mod tests {
    use rutter::AppLogic;

    use super::{TableDemo, TableDemoState, Widget};

    #[test]
    fn demo_exposes_a_controlled_multi_select_table() {
        let mut state = TableDemoState::default();
        let Widget::Column { children, .. } = TableDemo::view(&mut state) else {
            panic!("expected the demo root to be a column");
        };
        let Some(Widget::Table { model, options, .. }) = children.get(2) else {
            panic!("expected the third demo child to be a Table widget");
        };

        assert_eq!(model.rows().len(), 12);
        assert!(options.sorting().is_some());
        assert!(matches!(
            options.selection(),
            rutter::TableSelection::Multiple { .. }
        ));
    }
}
