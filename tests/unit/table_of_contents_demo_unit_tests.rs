use super::*;
use cosmic_text::SwashCache;
use rutter::engine::widget_state::{ScrollState, WidgetState};
use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
use rutter::render::hit_test::{HitResult, hit_test};
use rutter::render::{RichTextRenderer, draw_widgets, text::TextBufferCache};
use rutter::{HeadingLevel, WidgetIdSnapshot};
use skia_safe::{Point, Rect as Bounds, surfaces};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use winit::dpi::PhysicalSize;

type Fonts = Rc<RefCell<FontSystem>>;

fn layout(
    view: &Widget<'_, Msg>,
    fonts: &Fonts,
    width: u32,
    height: u32,
) -> (TaffyTree<RutterContext>, NodeId) {
    let mut tree = TaffyTree::new();
    let root = build_taffy_tree(&mut tree, view, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut tree,
        root,
        PhysicalSize::new(width, height),
        fonts.clone(),
        &RichTextRenderer::default(),
    );
    (tree, root)
}

#[test]
fn article_has_a_real_outline_and_medium_length_without_annotation_entries() {
    let words: usize = article::SECTIONS
        .iter()
        .flat_map(|section| section.paragraphs)
        .map(|paragraph| paragraph.split_whitespace().count())
        .sum();
    assert!((700..=1100).contains(&words), "article has {words} words");
    let levels: Vec<_> = article::SECTIONS
        .iter()
        .map(|section| section.level)
        .collect();
    assert_eq!(
        levels
            .iter()
            .filter(|&&level| level == HeadingLevel::H1)
            .count(),
        1
    );
    assert_eq!(
        levels
            .iter()
            .filter(|&&level| level == HeadingLevel::H2)
            .count(),
        5
    );
    assert_eq!(
        levels
            .iter()
            .filter(|&&level| level == HeadingLevel::H3)
            .count(),
        5
    );
    assert_eq!(levels[0], HeadingLevel::H1);
    for pair in levels[1..].chunks_exact(2) {
        assert_eq!(pair, [HeadingLevel::H2, HeadingLevel::H3]);
    }
    let Widget::Column { children, .. } = documentation() else {
        unreachable!()
    };
    assert_eq!(children.len(), levels.len() + 1);
    let Widget::Column {
        children: footer, ..
    } = children.last().unwrap()
    else {
        unreachable!()
    };
    assert!(matches!(&footer[0], Widget::Text { content, .. }
        if content.contains("https://en.wikipedia.org/wiki/Solar_System")
        && content.contains("CC BY-SA 4.0") && content.contains("no article prose copied")));
}

#[test]
fn page_and_article_keep_section_owned_gaps_and_intrinsic_height() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for width in [320, 800, 1920] {
        for height in [260, 700, 981] {
            for expanded in [false, true] {
                let mut state = TableOfContentsDemoState {
                    contents_expanded: expanded,
                    ..Default::default()
                };
                let view = TableOfContentsDemo::view(&mut state);
                WidgetIdSnapshot::capture(&view).unwrap();
                let (tree, root) = layout(&view, &fonts, width, height);
                // The theme action remains a root sibling before the page viewport.
                let page_viewport = tree.children(root).unwrap()[1];
                let page_document = tree.children(page_viewport).unwrap()[0];
                let page_nodes = tree.children(page_document).unwrap();
                let header = tree.layout(page_nodes[0]).unwrap();
                let table_node = page_nodes[1];
                let table = tree.layout(table_node).unwrap();
                assert!(
                    (table.location.y - header.location.y - header.size.height - spacing::XL).abs()
                        <= 1.0
                );
                let nodes = tree.children(table_node).unwrap();
                let navigation = tree.layout(nodes[0]).unwrap();
                let viewport = tree.layout(nodes[1]).unwrap();
                assert!(
                    (viewport.location.y
                        - navigation.location.y
                        - navigation.size.height
                        - spacing::TWO_XL)
                        .abs()
                        <= 1.0
                );
                let outer = tree.layout(page_viewport).unwrap();
                let document = tree.layout(page_document).unwrap();
                let minimum_article_height = 44.0;
                let available = outer.size.height - table.location.y - viewport.location.y;
                assert!(
                    (viewport.size.height - available.max(minimum_article_height)).abs() <= 1.0,
                    "{width}x{height}, expanded={expanded}: {viewport:?}, available={available}"
                );
                assert!(
                    (document.size.height
                        - outer
                            .size
                            .height
                            .max(table.location.y + viewport.location.y + minimum_article_height))
                    .abs()
                        <= 1.0,
                    "document={document:?}, outer={outer:?}, table={table:?}, viewport={viewport:?}"
                );
                if available >= minimum_article_height {
                    let bottom = outer.location.y
                        + table.location.y
                        + viewport.location.y
                        + viewport.size.height;
                    assert!((bottom - height as f32 + spacing::XL).abs() <= 1.0);
                    assert!(
                        document.size.height <= outer.size.height + 1.0,
                        "no outer double-scroll"
                    );
                } else {
                    let mut outer_scroll = ScrollState {
                        offset_y: 0.0,
                        content_height: document.size.height,
                        viewport_h: outer.size.height,
                    };
                    outer_scroll.scroll_by(f32::MAX);
                    assert!(
                        (table.location.y + viewport.location.y + viewport.size.height
                            - outer_scroll.offset_y
                            - outer.size.height)
                            .abs()
                            <= 1.0
                    );
                }
                let content_node = tree.children(nodes[1]).unwrap()[0];
                let content = tree.layout(content_node).unwrap();
                assert!(
                    content.size.height > viewport.size.height,
                    "several screens of natural text"
                );
                assert!(tree.style(content_node).unwrap().size.height.is_auto());
                let sections = tree.children(content_node).unwrap();
                for pair in sections.windows(2) {
                    let previous = tree.layout(pair[0]).unwrap();
                    let next = tree.layout(pair[1]).unwrap();
                    assert!(
                        (next.location.y
                            - previous.location.y
                            - previous.size.height
                            - spacing::TWO_XL)
                            .abs()
                            <= 1.0
                    );
                    assert!((next.location.x - previous.location.x).abs() <= 1.0);
                }
                for section in sections {
                    let group = tree.layout(section).unwrap();
                    assert!(group.size.width <= viewport.size.width + 1.0);
                    assert!(
                        group.location.x + group.size.width
                            <= viewport.size.width
                                - rutter::layout::SCROLLBAR_W
                                - 2.0
                                - spacing::SM
                                + 1.0
                    );
                    let children = tree.children(section).unwrap();
                    for pair in children.windows(2) {
                        let previous = tree.layout(pair[0]).unwrap();
                        let next = tree.layout(pair[1]).unwrap();
                        assert!(
                            (next.location.y
                                - previous.location.y
                                - previous.size.height
                                - spacing::MD)
                                .abs()
                                <= 1.0
                        );
                        assert!(next.location.y + next.size.height <= group.size.height + 1.0);
                        assert!(next.size.width > 0.0 && next.size.height > 0.0);
                    }
                }
            }
        }
    }
}

#[test]
fn every_article_link_targets_its_heading_and_last_section_and_footer_are_reachable() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for width in [272, 680] {
        let table = Widget::table_of_contents_with_options(
            "Nesta página",
            documentation(),
            Style {
                size: Size {
                    height: Dimension::percent(1.0),
                    ..document_viewport_style().size
                },
                ..document_viewport_style()
            },
            TableOfContentsOptions::new(2)
                .unwrap()
                .with_accordion_state(true, Msg::ToggleContents),
        )
        .with_id(320);
        let (tree, root) = layout(&table, &fonts, width, 900);
        let nodes = tree.children(root).unwrap();
        let navigation = nodes[0];
        let viewport = tree.layout(nodes[1]).unwrap();
        let content_node = tree.children(nodes[1]).unwrap()[0];
        let sections = tree.children(content_node).unwrap();
        let content = tree.layout(content_node).unwrap();
        let mut index = 0;
        let entries = tree.children(navigation).unwrap()[1];
        let nav = tree.layout(navigation).unwrap();
        let entries_layout = tree.layout(entries).unwrap();
        for column in tree.children(entries).unwrap() {
            let column_layout = tree.layout(column).unwrap();
            for entry in tree.children(column).unwrap() {
                let entry = tree.layout(entry).unwrap();
                let point = Point::new(
                    nav.location.x
                        + entries_layout.location.x
                        + column_layout.location.x
                        + entry.location.x
                        + entry.size.width / 2.0,
                    nav.location.y
                        + entries_layout.location.y
                        + column_layout.location.y
                        + entry.location.y
                        + entry.size.height / 2.0,
                );
                let Some(HitResult::TableOfContentsActivate {
                    id,
                    index: hit_index,
                    target_y,
                    ..
                }) = hit_test(
                    &table,
                    &tree,
                    root,
                    point,
                    Point::default(),
                    &HashMap::new(),
                )
                else {
                    panic!("missing article link {index}")
                };
                assert_eq!((id, hit_index), (320, index));
                let section = tree.layout(sections[index]).unwrap();
                let heading_node = tree.children(sections[index]).unwrap()[0];
                let heading = tree.layout(heading_node).unwrap();
                assert!((target_y - section.location.y - heading.location.y).abs() <= 1.0);
                let mut scroll = ScrollState {
                    offset_y: 0.0,
                    content_height: content.size.height,
                    viewport_h: viewport.size.height,
                };
                scroll.scroll_by(target_y);
                assert!(target_y - scroll.offset_y >= -1.0);
                assert!(
                    target_y - scroll.offset_y + heading.size.height <= viewport.size.height + 1.0
                );
                if index == 0 || index == article::SECTIONS.len() - 1 {
                    let heading_bounds = Bounds::from_xywh(
                        section.location.x + heading.location.x,
                        target_y,
                        heading.size.width,
                        heading.size.height,
                    );
                    for theme in [Theme::dark(), Theme::light()] {
                        assert_document_raster(
                            &table,
                            (&tree, root),
                            &fonts,
                            &theme,
                            scroll.clone(),
                            heading_bounds,
                        );
                    }
                }
                index += 1;
            }
        }
        assert_eq!(index, 11, "only article headings become links");
        let mut scroll = ScrollState {
            offset_y: 0.0,
            content_height: content.size.height,
            viewport_h: viewport.size.height,
        };
        scroll.scroll_by(f32::MAX);
        let footer = tree.layout(*sections.last().unwrap()).unwrap();
        assert!(
            (footer.location.y + footer.size.height - scroll.offset_y - viewport.size.height).abs()
                <= 1.0
        );
        for theme in [Theme::dark(), Theme::light()] {
            assert_document_raster(
                &table,
                (&tree, root),
                &fonts,
                &theme,
                scroll.clone(),
                Bounds::from_xywh(
                    footer.location.x,
                    footer.location.y,
                    footer.size.width,
                    footer.size.height,
                ),
            );
        }
    }
}

fn assert_document_raster(
    table: &Widget<'_, Msg>,
    (tree, root): (&TaffyTree<RutterContext>, NodeId),
    fonts: &Fonts,
    theme: &Theme,
    scroll: ScrollState,
    text_bounds: Bounds,
) {
    let width = tree.layout(root).unwrap().size.width as i32;
    let nodes = tree.children(root).unwrap();
    let navigation = tree.layout(nodes[0]).unwrap();
    let viewport = tree.layout(nodes[1]).unwrap();
    let states = HashMap::from([(320, WidgetState::Scroll(scroll.clone()))]);
    let mut surface = surfaces::raster_n32_premul((width, 900)).unwrap();
    surface.canvas().clear(theme.surface);
    draw_widgets(
        surface.canvas(),
        tree,
        root,
        table,
        &mut fonts.borrow_mut(),
        &mut SwashCache::new(),
        Point::new(-100.0, -100.0),
        None,
        &HashMap::new(),
        &states,
        &mut HashMap::new(),
        &mut TextBufferCache::default(),
        false,
        theme,
        1.0,
    );
    let pixels = surface.peek_pixels().unwrap();
    let painted_text = Bounds::from_xywh(
        viewport.location.x + text_bounds.left,
        viewport.location.y + text_bounds.top - scroll.offset_y,
        text_bounds.width(),
        text_bounds.height(),
    );
    // Sample text bands, not font-specific glyph shapes. This covers wrapped
    // attribution and the first/last destination heading after navigation.
    let band_count = (painted_text.height() / 24.0).floor().max(1.0) as i32;
    for band in 0..band_count {
        let y = (painted_text.top + painted_text.height() * band as f32 / band_count as f32).ceil()
            as i32;
        let end = (painted_text.top + painted_text.height() * (band + 1) as f32 / band_count as f32)
            .floor() as i32;
        assert!(
            (y..end).any(|row| (painted_text.left as i32..painted_text.right as i32)
                .any(|x| pixels.get_color((x, row)) != theme.surface)),
            "missing document text band at {y}, bounds={painted_text:?}, viewport={viewport:?}, scroll={scroll:?}"
        );
    }
    // Neither scrolled article text nor navigation paint may invade the
    // section gap or escape below the deliberately bounded document viewport.
    let gap_start = (navigation.location.y + navigation.size.height).ceil() as i32;
    let gap_end = viewport.location.y.floor() as i32;
    let viewport_end = (viewport.location.y + viewport.size.height).ceil() as i32;
    for y in (gap_start..gap_end).chain(viewport_end..900) {
        for x in 0..width {
            assert_eq!(
                pixels.get_color((x, y)),
                theme.surface,
                "document escaped viewport"
            );
        }
    }
}

#[test]
fn article_reserves_the_physical_scrollbar_gutter_in_both_directions_and_scales() {
    use rutter::{LayoutDirection, layout::build_taffy_tree_with_direction};
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        for width in [272, 680] {
            for expanded in [false, true] {
                let style = Style {
                    size: Size {
                        height: Dimension::percent(1.0),
                        ..document_viewport_style().size
                    },
                    ..document_viewport_style()
                };
                let options = TableOfContentsOptions::new(2)
                    .unwrap()
                    .with_accordion_state(expanded, Msg::ToggleContents);
                let table = Widget::table_of_contents_with_options(
                    "Nesta página",
                    documentation(),
                    style.clone(),
                    options.clone(),
                )
                .with_id(320);
                // Same layout and scrollbar, but no article ink: a reference
                // detects even antialiased text painted underneath the track.
                let reference = Widget::table_of_contents_with_options(
                    "Nesta página",
                    Widget::Column {
                        style: documentation_style(),
                        children: vec![],
                    },
                    style,
                    options,
                )
                .with_id(320);
                let mut tree = TaffyTree::new();
                let root = build_taffy_tree_with_direction(
                    &mut tree,
                    &table,
                    fonts.clone(),
                    &HashMap::new(),
                    direction,
                );
                compute_layout(
                    &mut tree,
                    root,
                    PhysicalSize::new(width, 900),
                    fonts.clone(),
                    &RichTextRenderer::default(),
                );
                let viewport_node = tree.children(root).unwrap()[1];
                let viewport = tree.layout(viewport_node).unwrap();
                let content_node = tree.children(viewport_node).unwrap()[0];
                let content = tree.layout(content_node).unwrap();
                let safe_right =
                    viewport.size.width - rutter::layout::SCROLLBAR_W - 2.0 - spacing::SM;
                for section in tree.children(content_node).unwrap() {
                    let section = tree.layout(section).unwrap();
                    assert!(
                        content.location.x + section.location.x + section.size.width
                            <= safe_right + 1.0,
                        "{direction:?}"
                    );
                }
                for offset in [0.0, content.size.height - viewport.size.height] {
                    let states = HashMap::from([(
                        320,
                        WidgetState::Scroll(ScrollState {
                            offset_y: offset,
                            content_height: content.size.height,
                            viewport_h: viewport.size.height,
                        }),
                    )]);
                    for theme in [Theme::dark(), Theme::light()] {
                        for scale in [1.0_f32, 1.5, 2.0] {
                            let render = |widget: &Widget<'_, Msg>| {
                                let mut surface = surfaces::raster_n32_premul((
                                    (width as f32 * scale) as i32,
                                    (900.0 * scale) as i32,
                                ))
                                .unwrap();
                                surface.canvas().clear(theme.surface);
                                surface.canvas().scale((scale, scale));
                                draw_widgets(
                                    surface.canvas(),
                                    &tree,
                                    root,
                                    widget,
                                    &mut fonts.borrow_mut(),
                                    &mut SwashCache::new(),
                                    Point::new(-100.0, -100.0),
                                    None,
                                    &HashMap::new(),
                                    &states,
                                    &mut HashMap::new(),
                                    &mut TextBufferCache::default(),
                                    false,
                                    &theme,
                                    scale,
                                );
                                surface
                            };
                            let mut actual = render(&table);
                            let mut expected = render(&reference);
                            let actual = actual.peek_pixels().unwrap();
                            let expected = expected.peek_pixels().unwrap();
                            for y in ((viewport.location.y * scale).ceil() as i32)
                                ..(900.0 * scale) as i32
                            {
                                for x in ((safe_right * scale).ceil() as i32)
                                    ..(width as f32 * scale) as i32
                                {
                                    assert_eq!(
                                        actual.get_color((x, y)),
                                        expected.get_color((x, y)),
                                        "text in gutter: {direction:?}, width={width}, expanded={expanded}, scale={scale}, offset={offset}, pixel={x},{y}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
