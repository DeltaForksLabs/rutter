use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::FontSystem;
use rutter::engine::widget_state::WidgetState;
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::hit_test::{HitResult, collect_stateful_ids, hit_test};
use rutter::{VirtualSelection, Widget, WidgetIdSnapshot};
use skia_safe::Point;
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Msg {
    Single(usize),
    Multiple(Vec<usize>),
}

#[test]
fn typed_selection_keeps_single_and_multiple_callback_contracts_distinct() {
    let single = VirtualSelection::single(Msg::Single);
    let multiple = VirtualSelection::multiple(&[3, 1], Msg::Multiple);

    assert!(matches!(single, VirtualSelection::Single(callback) if callback(2) == Msg::Single(2)));
    assert!(matches!(
        multiple,
        VirtualSelection::Multiple { selected, on_change }
            if selected == &[3, 1] && on_change(vec![1, 3]) == Msg::Multiple(vec![1, 3])
    ));
}

#[test]
fn configured_virtual_collections_register_existing_state_families() {
    let selected = [1, 3];
    let list = Widget::virtual_list_with_selection(
        32.0,
        5,
        &labels,
        VirtualSelection::multiple(&selected, Msg::Multiple),
        collection_style(240.0, 160.0),
    )
    .with_id(401);
    let grid = Widget::virtual_grid_content_with_selection(
        2,
        48.0,
        5,
        &content_cells,
        VirtualSelection::single(Msg::Single),
        collection_style(320.0, 160.0),
    )
    .with_id(402);
    let tree = Widget::Column {
        children: vec![list, grid],
        style: Style::default(),
    };
    let mut stateful = Vec::new();

    collect_stateful_ids(&tree, &mut stateful);

    assert!(stateful.contains(&(401, "vlist")));
    assert!(stateful.contains(&(402, "vgrid")));
}

#[test]
fn configured_content_list_retains_multiselection_callback() {
    let selected = [1, 2];
    let list = Widget::virtual_list_content_with_selection(
        40.0,
        4,
        &content_cells,
        VirtualSelection::multiple(&selected, Msg::Multiple),
        collection_style(240.0, 160.0),
    );

    let Widget::VirtualListContentWithSelection { selection, .. } = list else {
        panic!("expected a configured virtual content list");
    };
    assert!(matches!(
        selection,
        VirtualSelection::Multiple { selected, .. } if selected == &[1, 2]
    ));
}

#[test]
fn configured_multiselection_hit_tests_keep_virtual_item_geometry() {
    let selected = [0, 2];
    let list = Widget::virtual_list_with_selection(
        32.0,
        5,
        &labels,
        VirtualSelection::multiple(&selected, Msg::Multiple),
        collection_style(240.0, 160.0),
    )
    .with_id(403);
    let grid = Widget::virtual_grid_with_selection(
        2,
        48.0,
        5,
        &labels,
        VirtualSelection::multiple(&selected, Msg::Multiple),
        collection_style(320.0, 160.0),
    )
    .with_id(404);

    assert!(matches!(
        hit_at(&list, Point::new(10.0, 48.0)),
        Some(HitResult::VListSelect { id: 403, index: 1 })
    ));
    assert!(matches!(
        hit_at(&grid, Point::new(24.0, 24.0)),
        Some(HitResult::VGridSelect { id: 404, index: 0 })
    ));
}

#[test]
fn configured_selection_preserves_legacy_virtual_id_ownership() {
    let selected = [2];
    let legacy = Widget::virtual_list(32.0, 5, &labels, Msg::Single, Style::default()).with_id(405);
    let configured = Widget::virtual_list_with_selection(
        32.0,
        5,
        &labels,
        VirtualSelection::multiple(&selected, Msg::Multiple),
        Style::default(),
    )
    .with_id(405);

    WidgetIdSnapshot::capture(&legacy)
        .unwrap()
        .validate_transition_to(&WidgetIdSnapshot::capture(&configured).unwrap())
        .unwrap();
}

fn labels(index: usize) -> Option<String> {
    Some(format!("Item {index}"))
}

fn content_cells<'a>(index: usize) -> Option<Widget<'a, Msg>> {
    Some(Widget::Text {
        content: format!("Cell {index}"),
        style: Style::default(),
        color: None,
        size: 14.0,
    })
}

fn collection_style(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(height),
        },
        ..Style::default()
    }
}

fn hit_at(widget: &Widget<'_, Msg>, point: Point) -> Option<HitResult<Msg>> {
    let mut taffy = TaffyTree::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(320, 160),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    hit_test(
        widget,
        &taffy,
        root,
        point,
        Point::new(0.0, 0.0),
        &HashMap::<u64, WidgetState>::new(),
    )
}
