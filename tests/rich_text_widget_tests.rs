use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::WidgetState;
use rutter::layout::{
    RutterContext, SyncedLayoutTree, compute_layout, sync_taffy_tree_with_direction,
};
use rutter::render::draw_widgets;
use rutter::render::text::TextBufferCache;
use rutter::{
    LayoutDirection, RichText, RichTextColor, RichTextSize, RichTextSpan, RichTextStyle, Theme,
    Widget, WidgetIdSnapshot,
};
use skia_safe::{Color, Font, Point, surfaces};
use taffy::prelude::{Dimension, Rect, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[test]
fn rich_text_widget_retains_mixed_spans_and_layout_style() {
    let layout_style = width_style(320.0);
    let content = sample_content();
    let widget: Widget<'_, ()> = Widget::rich_text(content, layout_style.clone());

    let Widget::RichText { content, style } = widget else {
        panic!("expected RichText widget");
    };
    assert_eq!(content.plain_text(), "Rutter rich text");
    assert_eq!(content.spans().len(), 3);
    assert_eq!(style.size.width, layout_style.size.width);
}

#[test]
fn rich_text_wraps_and_increases_layout_height() {
    let wide = rich_widget(500.0);
    let narrow = rich_widget(80.0);
    let (_, wide_layout) = layout_widget(&wide, 500);
    let (_, narrow_layout) = layout_widget(&narrow, 80);

    assert!(wide_layout.height > 0.0);
    assert!(narrow_layout.height > wide_layout.height);
    assert_eq!(narrow_layout.width, 80.0);
}

#[test]
fn span_style_changes_reuse_one_taffy_leaf() {
    let mut taffy = TaffyTree::<RutterContext>::new();
    let root = taffy.new_leaf(Style::default()).unwrap();
    let mut tree = SyncedLayoutTree::placeholder(root);
    let first = rich_widget(240.0);
    let second: Widget<'_, ()> = Widget::rich_text(
        RichText::from_span(RichTextSpan::new("Changed").italic().underline()),
        width_style(240.0),
    );

    sync_widget(&mut taffy, &mut tree, &first);
    sync_widget(&mut taffy, &mut tree, &second);

    assert_eq!(tree.node_id(), root);
    assert_eq!(taffy.total_node_count(), 1);
}

#[test]
fn plain_and_rich_text_keep_the_text_structure_family() {
    let plain: Widget<'_, ()> = Widget::Text {
        content: "plain".into(),
        style: Style::default(),
        color: None,
        size: 16.0,
    };
    let rich: Widget<'_, ()> = Widget::rich_text(RichText::plain("rich"), Style::default());
    let plain_snapshot = WidgetIdSnapshot::capture(&plain).unwrap();
    let rich_snapshot = WidgetIdSnapshot::capture(&rich).unwrap();

    plain_snapshot
        .validate_transition_to(&rich_snapshot)
        .unwrap();
    plain_snapshot
        .validate_reconstruction(&rich_snapshot)
        .unwrap();
}

#[test]
fn rich_text_raster_contains_multiple_painted_colors() {
    let widget = rich_widget(360.0);
    let (taffy, root) = layout_tree(&widget, 360);
    let mut surface = surfaces::raster_n32_premul((360, 80)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    draw_rich_widget(surface.canvas(), &taffy, root, &widget);

    let pixels = surface.peek_pixels().unwrap();
    let colors: HashSet<[u8; 3]> = pixels
        .bytes()
        .unwrap()
        .chunks_exact(4)
        .filter(|pixel| pixel[3] > 32)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    assert!(colors.len() > 2);
}

#[test]
fn padding_offsets_paint_and_fixed_height_clips_overflow() {
    let widget: Widget<'_, ()> = Widget::rich_text(
        RichText::plain("padded rich text wraps across several lines"),
        Style {
            size: Size {
                width: Dimension::length(120.0),
                height: Dimension::length(32.0),
            },
            padding: Rect::length(12.0_f32),
            ..Style::default()
        },
    );
    let (taffy, root) = layout_tree(&widget, 120);
    let mut surface = surfaces::raster_n32_premul((120, 70)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    draw_rich_widget(surface.canvas(), &taffy, root, &widget);
    let pixels = surface.peek_pixels().unwrap();
    let bytes = pixels.bytes().unwrap();

    assert!(minimum_painted_x(bytes, 120) >= 12);
    assert!(rows_are_transparent(bytes, 120, 32));
}

fn sample_content() -> RichText<'static> {
    let defaults = RichTextStyle::default()
        .with_size(RichTextSize::new(18.0).unwrap())
        .with_color(RichTextColor::rgb(30, 80, 180));
    RichText::from_spans([
        RichTextSpan::new("Rutter ").bold(),
        RichTextSpan::new("rich ")
            .italic()
            .with_color(RichTextColor::rgb(190, 40, 70)),
        RichTextSpan::new("text").underline(),
    ])
    .with_default_style(defaults)
}

fn rich_widget(width: f32) -> Widget<'static, ()> {
    Widget::rich_text(sample_content(), width_style(width))
}

fn width_style(width: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::auto(),
        },
        ..Style::default()
    }
}

fn sync_widget(
    taffy: &mut TaffyTree<RutterContext>,
    tree: &mut SyncedLayoutTree,
    widget: &Widget<'_, ()>,
) {
    sync_taffy_tree_with_direction(taffy, tree, widget, &HashMap::new(), LayoutDirection::Ltr);
}

fn layout_widget(widget: &Widget<'_, ()>, viewport_width: u32) -> (f32, taffy::Size<f32>) {
    let (taffy, root) = layout_tree(widget, viewport_width);
    let layout = taffy.layout(root).unwrap();
    (layout.location.x, layout.size)
}

fn layout_tree(
    widget: &Widget<'_, ()>,
    viewport_width: u32,
) -> (TaffyTree<RutterContext>, taffy::NodeId) {
    let mut taffy = TaffyTree::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let root = rutter::layout::build_taffy_tree_with_direction(
        &mut taffy,
        widget,
        fonts.clone(),
        &HashMap::new(),
        LayoutDirection::Ltr,
    );
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(viewport_width, 200),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    (taffy, root)
}

fn draw_rich_widget(
    canvas: &skia_safe::Canvas,
    taffy: &TaffyTree<RutterContext>,
    root: taffy::NodeId,
    widget: &Widget<'_, ()>,
) {
    draw_widgets(
        canvas,
        taffy,
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

fn minimum_painted_x(bytes: &[u8], width: usize) -> usize {
    bytes
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, pixel)| (pixel[3] > 0).then_some(index % width))
        .min()
        .unwrap()
}

fn rows_are_transparent(bytes: &[u8], width: usize, first_row: usize) -> bool {
    bytes
        .chunks_exact(4)
        .enumerate()
        .filter(|(index, _)| index / width >= first_row)
        .all(|(_, pixel)| pixel[3] == 0)
}
