use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cosmic_text::FontSystem;
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

use super::{
    HeadingLevel, TableOfContentsOptions, collect_entries, entry_offset_y, entry_rects,
    layout_nodes,
};
use crate::engine::widget_state::{ScrollState, WidgetState};
use crate::layout::{build_taffy_tree, compute_layout};
use crate::widget::Widget;

fn fixed_style(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(height),
        },
        ..Style::default()
    }
}

fn document() -> Widget<'static, ()> {
    Widget::Column {
        style: Style::default(),
        children: vec![
            Widget::heading(HeadingLevel::H1, "Overview", fixed_style(320.0, 36.0)),
            Widget::Spacer {
                style: fixed_style(320.0, 296.0),
            },
            Widget::heading(HeadingLevel::H2, "Installation", fixed_style(320.0, 32.0)),
        ],
    }
}

fn heading_document(count: usize) -> Widget<'static, ()> {
    Widget::Column {
        style: Style::default(),
        children: (0..count)
            .map(|index| {
                Widget::heading(
                    HeadingLevel::H2,
                    format!("Section {index}"),
                    fixed_style(320.0, 28.0),
                )
            })
            .collect(),
    }
}

fn table_layout(
    table: &Widget<'_, ()>,
    size: PhysicalSize<u32>,
) -> (TaffyTree<crate::layout::RutterContext>, taffy::NodeId) {
    widget_layout(table, size)
}

fn widget_layout(
    widget: &Widget<'_, ()>,
    size: PhysicalSize<u32>,
) -> (TaffyTree<crate::layout::RutterContext>, taffy::NodeId) {
    let states = HashMap::from([(42, WidgetState::Scroll(ScrollState::default()))]);
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        size,
        fonts,
        &crate::render::RichTextRenderer::default(),
    );
    (taffy, root)
}

#[test]
fn discovers_headings_and_resolves_their_document_offsets() {
    let table =
        Widget::table_of_contents("Contents", document(), fixed_style(320.0, 200.0)).with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(320, 200));

    let Widget::TableOfContents { child, .. } = &table else {
        panic!("expected a TableOfContents widget");
    };
    let entries = collect_entries(child);
    let nodes = layout_nodes(&taffy, root).unwrap();
    let first_offset = entry_offset_y(child, &taffy, nodes.content, 0).unwrap();
    let second_offset = entry_offset_y(child, &taffy, nodes.content, 1).unwrap();
    let rects = entry_rects(&taffy, root);
    let table_height = taffy.layout(root).unwrap().size.height;
    let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;
    let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;
    let content_height = taffy.layout(nodes.content).unwrap().size.height;

    assert_eq!(entries[0].title, "Overview");
    assert_eq!(entries[1].level, HeadingLevel::H2);
    assert!(second_offset > first_offset);
    assert_eq!(viewport_height, 200.0);
    assert!((navigation_height + viewport_height - table_height).abs() < 0.01);
    assert!(content_height > viewport_height);
    assert!(rects[1].top > rects[0].top);
}

#[test]
fn heading_outline_normalizes_skipped_heading_levels() {
    let document: Widget<'_, ()> = Widget::Column {
        style: Style::default(),
        children: vec![
            Widget::heading(HeadingLevel::H3, "First", Style::default()),
            Widget::heading(HeadingLevel::H5, "Nested", Style::default()),
            Widget::heading(HeadingLevel::H4, "Sibling", Style::default()),
            Widget::heading(HeadingLevel::H2, "Top level", Style::default()),
            Widget::heading(HeadingLevel::H3, "Child", Style::default()),
        ],
    };
    let depths = collect_entries(&document)
        .into_iter()
        .map(|entry| entry.depth)
        .collect::<Vec<_>>();

    assert_eq!(depths, vec![0, 1, 1, 0, 1]);
}

#[test]
fn visual_outline_keeps_deep_descendants_at_one_readable_indent() {
    let document: Widget<'_, ()> = Widget::Column {
        style: Style::default(),
        children: vec![
            Widget::heading(HeadingLevel::H1, "Root", Style::default()),
            Widget::heading(HeadingLevel::H2, "Section", Style::default()),
            Widget::heading(HeadingLevel::H3, "Detail", Style::default()),
            Widget::heading(HeadingLevel::H4, "Example", Style::default()),
        ],
    };
    let visual_depths = collect_entries(&document)
        .into_iter()
        .map(|entry| entry.visual_depth())
        .collect::<Vec<_>>();

    assert_eq!(visual_depths, vec![0, 1, 1, 1]);
}

#[test]
fn columns_flow_top_to_bottom_before_starting_the_next_column() {
    let options = TableOfContentsOptions::new(2).unwrap();
    let table = Widget::table_of_contents_with_options(
        "Contents",
        heading_document(6),
        fixed_style(320.0, 220.0),
        options,
    )
    .with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(320, 220));
    let rects = entry_rects(&taffy, root);
    let first = rects[0];
    let third = rects[2];
    let fourth = rects[3];

    assert_eq!(first.left, 12.0);
    assert_eq!(first.left, third.left);
    assert!(third.top > first.top);
    assert!(fourth.left > first.left);
    assert_eq!(fourth.top, first.top);
}

#[test]
fn configured_columns_are_exact_and_balanced_without_reordering_entries() {
    let options = TableOfContentsOptions::new(4).unwrap();
    let table = Widget::table_of_contents_with_options(
        "Contents",
        heading_document(10),
        fixed_style(400.0, 300.0),
        options,
    )
    .with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(400, 300));
    let entries_node = layout_nodes(&taffy, root).unwrap().entries.unwrap();
    let columns = taffy.children(entries_node).unwrap();
    let column_sizes = columns
        .iter()
        .map(|column| taffy.children(*column).unwrap().len())
        .collect::<Vec<_>>();
    let rects = entry_rects(&taffy, root);

    assert_eq!(column_sizes, vec![3, 3, 2, 2]);
    assert_eq!(rects.len(), 10);
    assert_eq!(rects[2].left, rects[0].left);
    assert!(rects[3].left > rects[2].left);
    assert!(rects[6].left > rects[5].left);
    assert!(rects[8].left > rects[7].left);
}

#[test]
fn excess_column_count_does_not_create_empty_layout_columns() {
    let options = TableOfContentsOptions::new(4).unwrap();
    let table = Widget::table_of_contents_with_options(
        "Contents",
        heading_document(2),
        fixed_style(320.0, 220.0),
        options,
    )
    .with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(320, 220));
    let entries_node = layout_nodes(&taffy, root).unwrap().entries.unwrap();
    let columns = taffy.children(entries_node).unwrap();

    assert_eq!(columns.len(), 2);
    assert!(
        columns
            .iter()
            .all(|column| !taffy.children(*column).unwrap().is_empty())
    );
}

#[test]
fn long_entry_titles_keep_single_line_geometry_inside_their_columns() {
    let document: Widget<'_, ()> = Widget::Column {
        style: Style::default(),
        children: (0..4)
            .map(|index| {
                Widget::heading(
                    HeadingLevel::H2,
                    format!("A very long section title that cannot fit in column {index}"),
                    fixed_style(180.0, 28.0),
                )
            })
            .collect(),
    };
    let options = TableOfContentsOptions::new(2).unwrap();
    let table = Widget::table_of_contents_with_options(
        "Contents",
        document,
        fixed_style(180.0, 220.0),
        options,
    )
    .with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(180, 220));
    let rects = entry_rects(&taffy, root);

    assert_eq!(rects.len(), 4);
    assert!(rects.iter().all(|rect| rect.height() == 28.0));
    assert!(rects[2].left > rects[0].left);
}

#[test]
fn empty_documents_do_not_create_a_navigation_body() {
    let document: Widget<'_, ()> = Widget::Column {
        style: Style::default(),
        children: Vec::new(),
    };
    let table =
        Widget::table_of_contents("Contents", document, fixed_style(320.0, 220.0)).with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(320, 220));
    let nodes = layout_nodes(&taffy, root).unwrap();
    let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;

    assert!(nodes.entries.is_none());
    assert!(entry_rects(&taffy, root).is_empty());
    assert!(navigation_height < 60.0);
}

#[test]
fn long_navigation_expands_the_table_without_collapsing_the_document_viewport() {
    let table =
        Widget::table_of_contents("Contents", heading_document(16), fixed_style(320.0, 220.0))
            .with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(320, 220));
    let nodes = layout_nodes(&taffy, root).unwrap();
    let rects = entry_rects(&taffy, root);
    let table_height = taffy.layout(root).unwrap().size.height;
    let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;
    let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;

    assert!(navigation_height > 220.0);
    assert_eq!(viewport_height, 220.0);
    assert!((navigation_height + viewport_height - table_height).abs() < 0.01);
    assert!(rects.last().unwrap().bottom <= navigation_height);
}

#[test]
fn percentage_height_keeps_a_document_viewport_below_long_navigation() {
    let table_style = Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::percent(0.5),
        },
        min_size: Size::zero(),
        ..Style::default()
    };
    let table =
        Widget::table_of_contents("Contents", heading_document(16), table_style).with_id(42);
    let host: Widget<'_, ()> = Widget::Column {
        style: fixed_style(320.0, 400.0),
        children: vec![table],
    };
    let (taffy, root) = widget_layout(&host, PhysicalSize::new(320, 400));
    let table_node = taffy.children(root).unwrap()[0];
    let nodes = layout_nodes(&taffy, table_node).unwrap();
    let table_height = taffy.layout(table_node).unwrap().size.height;
    let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;
    let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;
    let rects = entry_rects(&taffy, table_node);

    assert!(navigation_height > 200.0);
    assert_eq!(viewport_height, 44.0);
    assert!((navigation_height + viewport_height - table_height).abs() < 0.01);
    assert!(rects.last().unwrap().bottom <= navigation_height);
    assert_eq!(
        taffy.style(table_node).unwrap().size.height,
        Dimension::auto()
    );
    assert_eq!(
        taffy.style(table_node).unwrap().min_size.height,
        Dimension::percent(0.5)
    );
}

#[test]
fn flex_fill_height_leaves_space_for_the_document_viewport() {
    let table_style = Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        min_size: Size::zero(),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Style::default()
    };
    let table = Widget::table_of_contents("Contents", document(), table_style).with_id(42);
    let host: Widget<'_, ()> = Widget::Column {
        style: fixed_style(320.0, 300.0),
        children: vec![
            Widget::Spacer {
                style: Style {
                    flex_shrink: 0.0,
                    ..fixed_style(320.0, 50.0)
                },
            },
            table,
        ],
    };
    let (taffy, root) = widget_layout(&host, PhysicalSize::new(320, 300));
    let table_node = taffy.children(root).unwrap()[1];
    let nodes = layout_nodes(&taffy, table_node).unwrap();
    let table_height = taffy.layout(table_node).unwrap().size.height;
    let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;
    let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;

    assert!((table_height - 250.0).abs() < 0.01);
    assert!((navigation_height + viewport_height - table_height).abs() < 0.01);
    assert!(viewport_height >= 44.0);
    assert_eq!(taffy.style(table_node).unwrap().flex_grow, 1.0);
}

#[test]
fn collapsed_accordion_preserves_the_configured_document_viewport_height() {
    let options = TableOfContentsOptions::new(2)
        .unwrap()
        .with_accordion_state(false, ());
    let table = Widget::table_of_contents_with_options(
        "Contents",
        heading_document(3),
        fixed_style(320.0, 220.0),
        options,
    )
    .with_id(42);
    let (taffy, root) = table_layout(&table, PhysicalSize::new(320, 220));
    let nodes = layout_nodes(&taffy, root).unwrap();
    let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;
    let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;
    let table_height = taffy.layout(root).unwrap().size.height;

    assert!(nodes.entries.is_none());
    assert!(entry_rects(&taffy, root).is_empty());
    assert_eq!(navigation_height, 44.0);
    assert_eq!(viewport_height, 220.0);
    assert_eq!(table_height, 264.0);
}
