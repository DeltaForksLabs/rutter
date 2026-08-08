use std::{cell::Cell, cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::WidgetState;
use rutter::layout::{build_taffy_tree_with_direction, compute_layout};
use rutter::render::draw_widgets;
use rutter::render::hit_test::{HitResult, collect_stateful_ids, hit_test};
use rutter::render::text::TextBufferCache;
use rutter::{CarouselConfig, LayoutDirection, Theme, Widget, WidgetIdSnapshot};
use skia_safe::{Color, Font, Point, surfaces};
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Select(usize),
}

#[test]
fn carousel_constructor_retains_builder_callback_and_config() {
    let cards = |index| Some(text_card(index));
    let config = CarouselConfig::weighted([1, 6, 1])
        .unwrap()
        .with_item_snapping(true);
    let widget = Widget::carousel_view(50, cards, Msg::Select, config.clone(), carousel_style());

    let Widget::CarouselView {
        item_count,
        on_select,
        config: actual,
        ..
    } = widget
    else {
        panic!("expected CarouselView");
    };
    assert_eq!(item_count, 50);
    assert_eq!(on_select(7), Msg::Select(7));
    assert_eq!(actual, config);
}

#[test]
fn carousel_builder_can_capture_state_backed_items() {
    let titles = vec!["Inbox".to_owned(), "Archive".to_owned()];
    let widget = carousel_from_titles(&titles);
    let Widget::CarouselView { items, .. } = widget else {
        panic!("expected CarouselView");
    };
    let Some(Widget::Text { content, .. }) = items(1) else {
        panic!("expected captured title item");
    };
    assert_eq!(content, "Archive");
}

#[test]
fn carousel_layout_changes_preserve_widget_identity() {
    let fixed_cards = |index| Some(text_card(index));
    let weighted_cards = |index| Some(text_card(index));
    let fixed = Widget::carousel_view(
        20,
        fixed_cards,
        Msg::Select,
        CarouselConfig::uncontained(220.0).unwrap(),
        carousel_style(),
    )
    .with_id(440);
    let weighted = Widget::carousel_view(
        20,
        weighted_cards,
        Msg::Select,
        CarouselConfig::weighted([1, 5, 2]).unwrap(),
        carousel_style(),
    )
    .with_id(440);

    let previous = WidgetIdSnapshot::capture(&fixed).unwrap();
    let next = WidgetIdSnapshot::capture(&weighted).unwrap();
    previous.validate_transition_to(&next).unwrap();
}

#[test]
fn weighted_carousel_hit_maps_dynamic_item_bounds() {
    let cards = |index| Some(text_card(index));
    let widget = Widget::carousel_view(
        20,
        cards,
        Msg::Select,
        CarouselConfig::weighted([1, 6, 1]).unwrap(),
        carousel_style(),
    )
    .with_id(441);
    let (mut taffy, root) = layout_carousel(&widget);
    let hit = hit_test(
        &widget,
        &taffy,
        root,
        Point::new(400.0, 100.0),
        Point::new(0.0, 0.0),
        &HashMap::new(),
    );

    assert!(matches!(
        hit,
        Some(HitResult::CarouselSelect { id: 441, index: 0 })
    ));
    taffy.clear();
}

#[test]
fn rtl_carousel_hit_uses_mirrored_item_bounds() {
    let widget = Widget::carousel_view(
        20,
        |index| Some(text_card(index)),
        Msg::Select,
        CarouselConfig::weighted([1, 6, 1]).unwrap(),
        carousel_style(),
    )
    .with_id(443);
    let (taffy, root) = layout_carousel_with_direction(&widget, LayoutDirection::Rtl);
    let hit = hit_test(
        &widget,
        &taffy,
        root,
        Point::new(50.0, 100.0),
        Point::new(0.0, 0.0),
        &HashMap::new(),
    );

    assert!(matches!(
        hit,
        Some(HitResult::CarouselSelect { id: 443, index: 1 })
    ));
}

#[test]
fn weighted_render_materializes_only_transition_window() {
    let builds = Cell::new(0_usize);
    let cards = |index| {
        builds.set(builds.get() + 1);
        Some(text_card(index))
    };
    let widget = Widget::carousel_view(
        10_000,
        cards,
        Msg::Select,
        CarouselConfig::weighted([1, 6, 1]).unwrap(),
        carousel_style(),
    );
    render_carousel(&widget);

    assert!(
        (1..=4).contains(&builds.get()),
        "built {} items",
        builds.get()
    );
}

#[test]
fn carousel_registers_one_stateful_focus_owner() {
    let cards = |index| Some(text_card(index));
    let widget = Widget::carousel_view(
        20,
        cards,
        Msg::Select,
        CarouselConfig::uncontained(200.0).unwrap(),
        carousel_style(),
    )
    .with_id(442);
    let mut stateful = Vec::new();
    collect_stateful_ids(&widget, &mut stateful);

    assert_eq!(stateful, vec![(442, "carousel")]);
}

fn text_card<'a>(index: usize) -> Widget<'a, Msg> {
    Widget::Text {
        content: format!("Card {}", index + 1),
        style: Style::default(),
        color: None,
        size: 16.0,
    }
}

fn carousel_from_titles<'a>(titles: &'a [String]) -> Widget<'a, Msg> {
    Widget::carousel_view(
        titles.len(),
        move |index| titles.get(index).map(|title| text_label(title)),
        Msg::Select,
        CarouselConfig::uncontained(200.0).unwrap(),
        carousel_style(),
    )
}

fn text_label<'a>(content: &str) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.to_owned(),
        style: Style::default(),
        color: None,
        size: 16.0,
    }
}

fn carousel_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(800.0),
            height: Dimension::length(200.0),
        },
        ..Style::default()
    }
}

fn layout_carousel(
    widget: &Widget<'_, Msg>,
) -> (TaffyTree<rutter::layout::RutterContext>, taffy::NodeId) {
    layout_carousel_with_direction(widget, LayoutDirection::Ltr)
}

fn layout_carousel_with_direction(
    widget: &Widget<'_, Msg>,
    direction: LayoutDirection,
) -> (TaffyTree<rutter::layout::RutterContext>, taffy::NodeId) {
    let mut taffy = TaffyTree::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let root = build_taffy_tree_with_direction(
        &mut taffy,
        widget,
        fonts.clone(),
        &HashMap::new(),
        direction,
    );
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(800, 200),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    (taffy, root)
}

fn render_carousel(widget: &Widget<'_, Msg>) {
    let (taffy, root) = layout_carousel(widget);
    let mut surface = surfaces::raster_n32_premul((800, 200)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    draw_widgets(
        surface.canvas(),
        &taffy,
        root,
        widget,
        &mut FontSystem::new(),
        &mut SwashCache::new(),
        Point::new(-1.0, -1.0),
        None,
        &HashMap::new(),
        &HashMap::<u64, WidgetState>::new(),
        &mut HashMap::<(String, u32), Font>::new(),
        &mut TextBufferCache::default(),
        true,
        &Theme::default(),
        1.0,
    );
}
