// ============================================================
// Rutter Framework — demos/table_of_contents_demo.rs
// Semantic headings are discovered automatically by TableOfContents.
// ============================================================

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use rutter::{AppLogic, RutterRunner, TableOfContentsOptions, Theme, Widget};
use taffy::prelude::*;

mod article {
    //! Original English factual summary informed by Wikipedia's “Solar System”.
    //! No Wikipedia prose or media is reproduced. See README's example attribution.

    use rutter::HeadingLevel;

    pub(super) const ATTRIBUTION: &str = "Source: Solar System — Wikipedia contributors. Original English factual summary, shortened and reorganized for this demo; no article prose copied. https://en.wikipedia.org/wiki/Solar_System (source text: CC BY-SA 4.0).";

    pub(super) struct Section {
        pub level: HeadingLevel,
        pub title: &'static str,
        pub paragraphs: &'static [&'static str],
    }

    pub(super) const SECTIONS: &[Section] = &[
        Section {
            level: HeadingLevel::H1,
            title: "The Solar System",
            paragraphs: &[
                "Earth belongs to a family of worlds held together by the Sun's gravity. Eight planets travel around this star, accompanied by moons, dwarf planets, asteroids, comets and dust. These objects differ enormously in size and composition. Together they preserve clues to the conditions in which our planetary system began, about 4.6 billion years ago.",
                "A journey outward crosses several distinct regions rather than an evenly spaced collection of planets. Small rocky worlds occupy the inner region. Giant planets lie farther away, followed by populations of icy bodies. Most of the space between these objects is almost empty, so familiar diagrams usually enlarge the worlds or compress their distances to make them visible.",
            ],
        },
        Section {
            level: HeadingLevel::H2,
            title: "Origins and motion",
            paragraphs: &[
                "The system developed when part of a cloud of gas and dust collapsed under gravity. Material gathered in the centre to form the young Sun, while a rotating disc surrounded it. Within that disc, solid particles joined into larger bodies. Repeated encounters and collisions helped build planets, but also left smaller remnants that never became part of a large world.",
            ],
        },
        Section {
            level: HeadingLevel::H3,
            title: "Orbits and distances",
            paragraphs: &[
                "Most planets orbit in roughly the same plane and in the same direction, a legacy of the rotating disc. Their paths are ellipses rather than perfect circles. Farther planets take longer to complete a circuit. Astronomers often describe distances in astronomical units: one unit is approximately the average distance between Earth and the Sun, about 150 million kilometres.",
            ],
        },
        Section {
            level: HeadingLevel::H2,
            title: "The Sun",
            paragraphs: &[
                "The Sun contains almost all the mass in the Solar System, making it the dominant influence on planetary motion. Deep inside it, nuclear fusion converts hydrogen into helium and releases energy. That energy eventually escapes as radiation. Sunlight warms planetary surfaces and atmospheres, although distance, clouds and atmospheric composition give each world a very different response to that incoming energy.",
            ],
        },
        Section {
            level: HeadingLevel::H3,
            title: "The solar wind",
            paragraphs: &[
                "The Sun also sends a stream of charged particles into space. This solar wind creates a vast surrounding region called the heliosphere. Its outer boundary marks a change in the particle environment, not the end of the Sun's gravitational influence. The distinction matters when describing spacecraft that have entered interstellar space but remain much closer than the most distant bodies associated with the Sun.",
            ],
        },
        Section {
            level: HeadingLevel::H2,
            title: "The inner worlds",
            paragraphs: &[
                "Mercury, Venus, Earth and Mars are the terrestrial planets. Rock and metal make up much of their bulk, and all have solid surfaces. Mercury is the smallest planet and the closest to the Sun. Venus has a dense atmosphere and an extremely hot surface. Earth has extensive liquid-water oceans, while Mars displays a cold landscape with evidence of a wetter past.",
            ],
        },
        Section {
            level: HeadingLevel::H3,
            title: "The asteroid belt",
            paragraphs: &[
                "Between Mars and Jupiter, many small rocky bodies occupy the asteroid belt. It is a broad region, not a tightly packed obstacle course: typical separations are large compared with the objects themselves. Ceres, the largest member, is classified as a dwarf planet. The belt's varied objects help researchers investigate materials and events from the early history of the planetary system.",
            ],
        },
        Section {
            level: HeadingLevel::H2,
            title: "The giant planets",
            paragraphs: &[
                "Jupiter and Saturn are gas giants dominated by hydrogen and helium. Jupiter is the largest planet, while Saturn is especially familiar for its bright rings. Uranus and Neptune are called ice giants because their interiors contain a larger proportion of heavier substances. These names describe broad differences in composition; they do not mean that the outer planets are simple balls of gas or exposed frozen water.",
            ],
        },
        Section {
            level: HeadingLevel::H3,
            title: "Moons and rings",
            paragraphs: &[
                "All four giant planets have rings and many natural satellites. Their moons are worlds in their own right, with diverse surfaces and histories. Some show strong geological activity; others preserve heavily cratered terrain. Rings consist of numerous separate particles orbiting the planet rather than a single solid disc. Studying these smaller systems reveals how gravity shapes material on scales far below that of a planetary orbit.",
            ],
        },
        Section {
            level: HeadingLevel::H2,
            title: "Beyond Neptune",
            paragraphs: &[
                "The planetary region does not contain everything that circles the Sun. Beyond Neptune lies the Kuiper belt, home to Pluto and many other icy objects. Pluto is a dwarf planet, not one of the eight planets. More distant populations follow a wide range of elongated and tilted orbits, reminding us that the outer system is more complex than a set of neat concentric rings.",
            ],
        },
        Section {
            level: HeadingLevel::H3,
            title: "Comets and distant frontiers",
            paragraphs: &[
                "When an icy comet approaches the Sun, warming can release gas and dust, producing a surrounding coma and sometimes long tails. Some comets return on relatively short schedules; others follow paths that extend far beyond the planets. A much more distant reservoir, the proposed Oort cloud, helps explain the arrival of long-period comets, although its individual members are too remote to observe directly as a population.",
                "Exploration combines telescope observations with spacecraft measurements. Flybys reveal brief close views, orbiters follow changes over time, and landers examine particular surfaces. Each method answers different questions. Even after visits to every major planet, the distant Solar System remains difficult to survey, so a map of our celestial neighbourhood is also a record of what we have yet to learn.",
            ],
        },
    ];
}

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

pub struct TableOfContentsDemoState {
    pub theme: ExampleTheme,
    pub contents_expanded: bool,
}

impl Default for TableOfContentsDemoState {
    fn default() -> Self {
        Self {
            theme: ExampleTheme::default(),
            contents_expanded: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    ToggleContents,
}

pub struct TableOfContentsDemo;

impl AppLogic for TableOfContentsDemo {
    type State = TableOfContentsDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        TableOfContentsDemoState::default()
    }

    fn view<'a>(state: &'a mut TableOfContentsDemoState) -> Widget<'a, Msg> {
        let mut page = super::layout::page(
            "Table of contents",
            Widget::Column {
                style: page_style(),
                children: vec![
                example_theme_selector(state.theme, Msg::ThemeChanged),
                Widget::Text {
                    content:
                        "Table of Contents — clique em um título para rolar suavemente até a seção."
                            .into(),
                    style: Style::default(),
                    color: None,
                    size: 14.0,
                },
                Widget::table_of_contents_with_options(
                    "Nesta página",
                    documentation(),
                    document_viewport_style(),
                    TableOfContentsOptions::new(2)
                        .expect("table of contents demo uses 2 columns, expected columns > 0")
                        .with_accordion_state(state.contents_expanded, Msg::ToggleContents),
                )
                .with_id(320),
            ],
            },
        );
        // Opt this reading page into available-height layout without changing
        // the intrinsic-document policy used by the other examples. Min-height
        // still allows the navigation and a usable article to overflow together.
        if let Widget::Column { children, .. } = &mut page {
            for child in children {
                if let Widget::ScrollView { child, .. } = child
                    && let Widget::Column { style, .. } = child.as_mut()
                {
                    style.min_size.height = Dimension::percent(1.0);
                }
            }
        }
        page
    }

    fn update(state: &mut TableOfContentsDemoState, message: Msg, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
            Msg::ToggleContents => state.contents_expanded = !state.contents_expanded,
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn page_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::percent(1.0),
        },
        padding: Rect::length(spacing::XL),
        gap: Size::length(spacing::LG),
        ..Style::default()
    }
}

fn document_viewport_style() -> Style {
    Style {
        // The shared page owns the XL gap above navigation; this section owns
        // navigation/document separation, independently of the theme's density.
        gap: Size::length(spacing::TWO_XL),
        flex_grow: 1.0,
        ..responsive_width(680.0, Dimension::auto())
    }
}

fn documentation<'a>() -> Widget<'a, Msg> {
    Widget::Column {
        style: documentation_style(),
        children: documentation_sections(),
    }
}

fn documentation_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        gap: Size::length(spacing::TWO_XL),
        flex_shrink: 0.0,
        // ScrollView paints its 8px track 2px inside the physical right edge,
        // in both text directions. Keep an SM gap before it; padding participates
        // in both text measurement and painting, including heading destinations.
        padding: Rect {
            right: LengthPercentage::length(rutter::layout::SCROLLBAR_W + 2.0 + spacing::SM),
            ..Rect::zero()
        },
        ..Style::default()
    }
}

fn documentation_sections<'a>() -> Vec<Widget<'a, Msg>> {
    let mut sections: Vec<_> = article::SECTIONS
        .iter()
        .map(|section| {
            let mut children = vec![Widget::heading(
                section.level,
                section.title,
                Style::default(),
            )];
            children.extend(section.paragraphs.iter().map(|paragraph| body(paragraph)));
            super::layout::section(children)
        })
        .collect();
    // Attribution is supporting text, not an extra document navigation entry.
    sections.push(super::layout::section(vec![body(article::ATTRIBUTION)]));
    sections
}

fn body<'a>(content: &str) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: Style::default(),
        color: None,
        size: 15.0,
    }
}

pub fn run() {
    RutterRunner::<TableOfContentsDemo>::run();
}

#[cfg(test)]
mod tests {
    use rutter::{AppLogic, HeadingLevel};

    use super::{TableOfContentsDemo, TableOfContentsDemoState, Widget, documentation};

    #[test]
    fn documentation_contains_semantic_headings() {
        let Widget::Column { children, .. } = documentation() else {
            panic!("expected the documentation to be a column");
        };

        assert!(matches!(
            match children.first().unwrap() {
                Widget::Column { children, .. } => children.first(),
                _ => panic!("documentation sections group headings with their paragraphs"),
            },
            Some(Widget::Heading {
                level: HeadingLevel::H1,
                ..
            })
        ));
    }

    #[test]
    fn demo_starts_with_an_expanded_two_column_navigation() {
        let mut state = TableOfContentsDemoState::default();
        let view = TableOfContentsDemo::view(&mut state);
        let children = super::super::layout::document_children(&view);
        let Some(Widget::TableOfContents { options, .. }) = children
            .iter()
            .find(|child| matches!(child, Widget::TableOfContents { id: 320, .. }))
        else {
            panic!("expected the table of contents in the scrollable page document");
        };

        assert_eq!(options.columns(), 2);
        assert!(options.is_accordion());
        assert!(options.is_expanded());
    }
}

#[cfg(test)]
mod article_tests {
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
                        (table.location.y - header.location.y - header.size.height - spacing::XL)
                            .abs()
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
                            - outer.size.height.max(
                                table.location.y + viewport.location.y + minimum_article_height
                            ))
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
                        target_y - scroll.offset_y + heading.size.height
                            <= viewport.size.height + 1.0
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
                (footer.location.y + footer.size.height - scroll.offset_y - viewport.size.height)
                    .abs()
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
            let y = (painted_text.top + painted_text.height() * band as f32 / band_count as f32)
                .ceil() as i32;
            let end = (painted_text.top
                + painted_text.height() * (band + 1) as f32 / band_count as f32)
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
}

#[cfg(test)]
mod page_tests {
    use super::TableOfContentsDemo;

    #[test]
    fn page_reflows_and_retains_a_reachable_document() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        super::super::layout::test_support::assert_application::<TableOfContentsDemo>(&fonts);
    }

    #[test]
    fn starts_dark_with_a_theme_selector() {
        super::super::theme_selector::test_support::assert_single_window_theme::<TableOfContentsDemo>(
        );
    }
}
