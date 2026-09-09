use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cosmic_text::FontSystem;
use skia_safe::Point;
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

use super::*;
use crate::engine::widget_state::WidgetState;
use crate::layout::{build_taffy_tree_with_direction, compute_layout};
use crate::widgets::table::{
    TableCell, TableCellTarget, TableColumn, TableColumnKey, TableColumnWidth, TableModel,
    TableOptions, TableRow, TableRowKey, TableSelection, TableSort, TableSorting, TableState,
    calculate_viewport, minimum_content_width,
};

#[derive(Clone)]
struct Message;

fn selection_message(_row: Option<TableRowKey>) -> Message {
    Message
}

fn sort_message(_sort: TableSort) -> Message {
    Message
}

fn model(rows: usize) -> TableModel<'static> {
    let columns = vec![
        TableColumn::new(
            TableColumnKey::new(1),
            "Name",
            TableColumnWidth::fixed(120.0).unwrap(),
        )
        .with_sortable(true),
        TableColumn::new(
            TableColumnKey::new(2),
            "State",
            TableColumnWidth::fixed(120.0).unwrap(),
        ),
    ];
    let rows = (0..rows)
        .map(|index| {
            TableRow::new(
                TableRowKey::new(index as u64 + 10),
                [TableCell::new("Ada"), TableCell::new("Ready")],
            )
        })
        .collect::<Vec<_>>();
    TableModel::new("People", columns, rows).unwrap()
}

fn style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(160.0),
            height: Dimension::length(132.0),
        },
        ..Style::default()
    }
}

fn interactive_widget() -> Widget<'static, Message> {
    let options = TableOptions::default()
        .with_selection(TableSelection::single(None, selection_message))
        .with_sorting(TableSorting::new(None, sort_message));
    Widget::table_with_options(model(4), options, style()).with_id(77)
}

fn layout_table(
    widget: &Widget<'_, Message>,
    direction: LayoutDirection,
) -> (TaffyTree<RutterContext>, NodeId, HashMap<u64, WidgetState>) {
    let model = match widget {
        Widget::Table { model, .. } => model,
        _ => unreachable!(),
    };
    let viewport = calculate_viewport(
        (160.0, 132.0),
        crate::widgets::table::TableMetrics::default(),
        model.rows().len(),
        minimum_content_width(model.columns()),
    );
    let mut table_state = TableState::default();
    table_state.sync_viewport(viewport);
    let states = HashMap::from([(77, WidgetState::Table(table_state))]);
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root =
        build_taffy_tree_with_direction(&mut taffy, widget, fonts.clone(), &states, direction);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(160, 132),
        fonts,
        &crate::render::RichTextRenderer::default(),
    );
    (taffy, root, states)
}

#[test]
fn table_hit_routes_sortable_headers_and_selectable_cells_by_key() {
    let widget = interactive_widget();
    let (taffy, root, states) = layout_table(&widget, LayoutDirection::Ltr);
    let header = hit_test(
        &widget,
        &taffy,
        root,
        Point::new(20.0, 20.0),
        Point::new(0.0, 0.0),
        &states,
    );
    let cell = hit_test(
        &widget,
        &taffy,
        root,
        Point::new(20.0, 60.0),
        Point::new(0.0, 0.0),
        &states,
    );

    assert!(matches!(
        header,
        Some(HitResult::TableActivate {
            id: 77,
            target
        }) if target == TableCellTarget::header(TableColumnKey::new(1))
    ));
    assert!(matches!(
        cell,
        Some(HitResult::TableActivate {
            id: 77,
            target
        }) if target == TableCellTarget::body(TableRowKey::new(10), TableColumnKey::new(1))
    ));
}

#[test]
fn table_exposes_horizontal_and_vertical_scrollbar_drag_axes() {
    let widget = interactive_widget();
    let (taffy, root, states) = layout_table(&widget, LayoutDirection::Ltr);
    let horizontal = find_scrollbar_drag_hit(
        &widget,
        &taffy,
        root,
        Point::new(20.0, 126.0),
        Point::new(0.0, 0.0),
        &states,
    )
    .unwrap();
    let vertical = find_scrollbar_drag_hit(
        &widget,
        &taffy,
        root,
        Point::new(154.0, 60.0),
        Point::new(0.0, 0.0),
        &states,
    )
    .unwrap();

    assert_eq!(horizontal.axis, ScrollbarAxis::Horizontal);
    assert!(!horizontal.reversed);
    assert_eq!(vertical.axis, ScrollbarAxis::Vertical);
}

#[test]
fn rtl_horizontal_scrollbar_drag_uses_reversed_logical_offset() {
    let widget = interactive_widget();
    let (taffy, root, states) = layout_table(&widget, LayoutDirection::Rtl);
    let hit = find_scrollbar_drag_hit(
        &widget,
        &taffy,
        root,
        Point::new(130.0, 126.0),
        Point::new(0.0, 0.0),
        &states,
    )
    .unwrap();

    assert_eq!(hit.axis, ScrollbarAxis::Horizontal);
    assert!(hit.reversed);
}

#[test]
fn passive_table_cells_only_claim_scroll_focus() {
    let widget: Widget<'_, Message> = Widget::table(model(1), style()).with_id(77);
    let (taffy, root, states) = layout_table(&widget, LayoutDirection::Ltr);
    let hit = hit_test(
        &widget,
        &taffy,
        root,
        Point::new(20.0, 60.0),
        Point::new(0.0, 0.0),
        &states,
    );

    assert!(matches!(hit, Some(HitResult::ScrollFocus(77))));
}
