use rutter::{HeadingLevel, Widget, spacing};
use taffy::prelude::*;

// A reserved manual ID, stable across reconstruction and independent of demo IDs.
pub(super) const PAGE_SCROLL_ID: u64 = 0x61_0000;
pub(super) const THEME_BUTTON_SIZE: f32 = 44.0;
pub(super) const PAGE_TOP: f32 = spacing::LG + THEME_BUTTON_SIZE + spacing::MD;

/// Normalizes a demo document, keeping viewport overlays and the theme action at
/// the root. Documents retain intrinsic height; short windows scroll the page
/// instead of compressing field bodies. The header action has its own safe row.
pub(super) fn page<'a, Message>(title: &str, view: Widget<'a, Message>) -> Widget<'a, Message> {
    let Widget::Column {
        children,
        mut style,
    } = view
    else {
        panic!("example page must be a column");
    };
    let mut root_children = Vec::new();
    let mut overlays = Vec::new();
    let mut document = Vec::new();
    for child in children {
        if matches!(&child, Widget::ButtonContent { label, .. }
            if label.starts_with("Switch to ") && label.ends_with(" theme"))
        {
            root_children.push(child);
        } else if matches!(
            &child,
            Widget::Modal { .. } | Widget::Dialog { .. } | Widget::Toast { .. }
        ) {
            overlays.push(child);
        } else {
            document.push(child);
        }
    }
    // Promote existing titles without changing their copy; demos that previously
    // started with a control or helper paragraph receive a real page heading.
    match document.first() {
        Some(Widget::Heading {
            level: HeadingLevel::H1,
            ..
        }) => {}
        Some(Widget::Text { size, .. }) if *size >= 18.0 => {
            let Widget::Text { content, .. } = document.remove(0) else {
                unreachable!()
            };
            document.insert(
                0,
                Widget::heading(HeadingLevel::H1, content, Style::default()),
            );
        }
        _ => document.insert(
            0,
            Widget::heading(HeadingLevel::H1, title, Style::default()),
        ),
    }
    if matches!(document.get(1), Some(Widget::Text { size, .. }) if *size <= 16.0) {
        let header = document.drain(..2).collect();
        document.insert(
            0,
            Widget::Column {
                children: header,
                style: Style {
                    gap: Size::length(spacing::SM),
                    flex_shrink: 0.0,
                    ..responsive_width(760.0, Dimension::auto())
                },
            },
        );
    }
    // A demo that already owns a document viewport keeps that stable ID. Its
    // document joins the page flow so the title also remains reachable when short.
    let scroll_id = if document.len() == 2 && matches!(document[1], Widget::ScrollView { .. }) {
        let Widget::ScrollView { id, child, .. } = document.pop().unwrap() else {
            unreachable!()
        };
        document.push(*child);
        id
    } else {
        PAGE_SCROLL_ID
    };
    style.size = Size {
        width: Dimension::percent(1.0),
        height: Dimension::auto(),
    };
    style.padding = Rect::zero();
    style.flex_grow = 0.0;
    style.flex_shrink = 0.0;
    style.justify_content = Some(JustifyContent::FlexStart);
    style.align_items = Some(AlignItems::Stretch);
    style.gap = Size::length(spacing::XL);
    root_children.push(scroll_document(
        scroll_id,
        Widget::Column {
            children: document,
            style,
        },
    ));
    // Root siblings paint forwards and hit-test backwards. Portaled page
    // overlays must follow the document to cover and intercept its controls.
    root_children.extend(overlays);
    Widget::Column {
        children: root_children,
        style: Style {
            size: Size::percent(1.0_f32),
            padding: Rect {
                top: LengthPercentage::length(PAGE_TOP),
                ..Rect::length(spacing::XL)
            },
            ..Style::default()
        },
    }
}

/// A group owns only sibling spacing; its parent owns section separation.
pub(super) fn section<'a, Message>(children: Vec<Widget<'a, Message>>) -> Widget<'a, Message> {
    Widget::Column {
        children,
        style: Style {
            gap: Size::length(spacing::MD),
            flex_shrink: 0.0,
            ..responsive_width(760.0, Dimension::auto())
        },
    }
}

/// Flow rows must leave space between wrapped lines as well as columns.
pub(super) fn flow() -> Style {
    Style {
        flex_wrap: FlexWrap::Wrap,
        align_items: Some(AlignItems::Center),
        gap: Size::length(spacing::MD),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

pub(super) fn scroll_document<'a, Message>(
    id: u64,
    child: Widget<'a, Message>,
) -> Widget<'a, Message> {
    Widget::ScrollView {
        id,
        child: Box::new(child),
        style: Style {
            flex_grow: 1.0,
            align_items: Some(AlignItems::FlexStart),
            min_size: Size::from_lengths(0.0, 0.0),
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(0.0),
            },
            ..Style::default()
        },
    }
}

/// Fills the available width on narrow surfaces without exceeding the desktop limit.
pub(super) fn responsive_width(max_width: f32, height: Dimension) -> Style {
    Style {
        flex_shrink: 0.0,
        size: Size {
            width: Dimension::percent(1.0),
            height,
        },
        max_size: Size {
            width: Dimension::length(max_width),
            height: Dimension::auto(),
        },
        ..Style::default()
    }
}

/// Semantic test entry point: page wrappers are not part of demo behavior.
#[cfg(test)]
pub(super) fn document_children<'w, 'a, Message>(
    view: &'w Widget<'a, Message>,
) -> &'w [Widget<'a, Message>] {
    let Widget::Column { children, .. } = view else {
        panic!("expected page root")
    };
    let document = children
        .iter()
        .find_map(|child| match child {
            Widget::ScrollView { child, .. } => Some(child.as_ref()),
            _ => None,
        })
        .expect("expected page scroll viewport");
    let Widget::Column { children, .. } = document else {
        panic!("expected page document")
    };
    children
}

#[cfg(test)]
pub(super) mod test_support {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use cosmic_text::{FontSystem, SwashCache};
    use rutter::engine::widget_state::ScrollState;
    use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
    use rutter::render::{RichTextRenderer, draw_widgets, text::TextBufferCache};
    use rutter::{AppLogic, HeadingLevel, Theme, Widget, WidgetIdSnapshot};
    use skia_safe::{Point, Rect as Bounds, surfaces};
    use taffy::prelude::*;
    use winit::dpi::PhysicalSize;

    pub(crate) type Fonts = Rc<RefCell<FontSystem>>;

    pub(crate) fn assert_application<Application: AppLogic>(fonts: &Fonts) {
        let mut state = Application::new(&mut fonts.borrow_mut());
        let view = Application::view(&mut state);
        assert_page(&view, fonts, std::any::type_name::<Application>());
    }

    pub(crate) fn assert_page<Message>(view: &Widget<'_, Message>, fonts: &Fonts, name: &str) {
        WidgetIdSnapshot::capture(view).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let Widget::Column { children, .. } = view else {
            panic!("{name}: page root")
        };
        let selector_index = children
            .iter()
            .position(|child| {
                matches!(child,
            Widget::ButtonContent { label, .. } if label.starts_with("Switch to "))
            })
            .expect("theme selector stays a direct root child");
        let viewport_index = children
            .iter()
            .position(|child| matches!(child, Widget::ScrollView { .. }))
            .expect("page must expose a scroll viewport");
        for (width, height) in [(800, 700), (320, 700), (800, 260), (320, 260)] {
            let mut tree = TaffyTree::new();
            let root = build_taffy_tree(&mut tree, view, fonts.clone(), &HashMap::new());
            compute_layout(
                &mut tree,
                root,
                PhysicalSize::new(width, height),
                fonts.clone(),
                &RichTextRenderer::default(),
            );
            let nodes = tree.children(root).unwrap();
            let selector = bounds(&tree, nodes[selector_index], Point::default());
            let viewport_node = nodes[viewport_index];
            let viewport = bounds(&tree, viewport_node, Point::default());
            assert_eq!(selector.width(), 44.0, "{name}");
            assert_eq!(selector.height(), 44.0, "{name}");
            assert!(
                selector.right <= width as f32 && selector.top >= 0.0,
                "{name}"
            );
            assert!(
                viewport.top - selector.bottom >= rutter::spacing::MD - 1.0,
                "{name}: header safe row"
            );
            assert!(
                viewport.height() > 0.0 && viewport.bottom <= height as f32,
                "{name}: viewport"
            );
            assert_eq!(
                viewport.left,
                rutter::spacing::XL,
                "{name}: shared start edge"
            );
            let document_node = tree.children(viewport_node).unwrap()[0];
            let document = bounds(
                &tree,
                document_node,
                Point::new(viewport.left, viewport.top),
            );
            assert!(
                document.height() > 0.0 && document.width() <= viewport.width() + 1.0,
                "{name}: intrinsic document"
            );
            assert_document_hierarchy(&tree, document_node, super::document_children(view), name);
            let mut headings = Vec::new();
            inspect(view, &tree, root, Point::default(), &mut headings, name);
            let title = headings.first().expect("every page needs a semantic H1");
            assert!(
                (title.left - viewport.left).abs() <= 1.0,
                "{name}: title start edge"
            );
            assert!(
                title.top >= viewport.top && title.bottom > title.top,
                "{name}: title allocation"
            );
            let mut scroll = ScrollState {
                offset_y: 0.0,
                content_height: document.height(),
                viewport_h: viewport.height(),
            };
            scroll.scroll_by(f32::MAX);
            assert!(
                (document.bottom - scroll.offset_y - viewport.bottom).abs() <= 1.0
                    || document.height() <= viewport.height(),
                "{name}: document bottom unreachable"
            );
            assert!(
                tree.style(document_node).unwrap().size.height.is_auto(),
                "{name}: document height must be intrinsic"
            );
            assert_eq!(
                tree.style(document_node).unwrap().flex_shrink,
                0.0,
                "{name}: short surfaces must not compress the document"
            );
            // Theme-independent geometry is painted against both actual palettes.
            // This checks title ink and the reserved action/title band, not font shapes.
            for theme in [Theme::dark(), Theme::light()] {
                assert_header_raster(
                    view,
                    (&tree, root),
                    fonts,
                    &theme,
                    (width, height),
                    *title,
                    name,
                );
            }
        }
    }

    pub(crate) fn bounds(tree: &TaffyTree<RutterContext>, node: NodeId, origin: Point) -> Bounds {
        let layout = tree.layout(node).unwrap();
        Bounds::from_xywh(
            origin.x + layout.location.x,
            origin.y + layout.location.y,
            layout.size.width,
            layout.size.height,
        )
    }

    fn inspect<Message>(
        widget: &Widget<'_, Message>,
        tree: &TaffyTree<RutterContext>,
        node: NodeId,
        origin: Point,
        headings: &mut Vec<Bounds>,
        name: &str,
    ) {
        let rect = bounds(tree, node, origin);
        assert!(
            rect.left.is_finite()
                && rect.top.is_finite()
                && rect.width().is_finite()
                && rect.height().is_finite()
                && rect.width() >= 0.0
                && rect.height() >= 0.0,
            "{name}: {rect:?}"
        );
        let nodes = tree.children(node).unwrap();
        let origin = Point::new(rect.left, rect.top);
        match widget {
            Widget::Heading {
                level: HeadingLevel::H1,
                ..
            } => headings.push(rect),
            Widget::TextInput {
                style,
                label,
                error_msg,
                ..
            }
            | Widget::TextArea {
                style,
                label,
                error_msg,
                ..
            }
            | Widget::ComboBox {
                style,
                label,
                error_msg,
                ..
            } => {
                assert_field(rect, style, label, error_msg.as_deref(), name);
            }
            Widget::Select { style, label, .. } => assert_field(rect, style, label, None, name),
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                assert_eq!(children.len(), nodes.len());
                assert_sibling_separation(tree, &nodes, name);
                if let Widget::Row { style, .. } = widget
                    && style.flex_wrap == FlexWrap::Wrap
                {
                    assert!(
                        style.gap.width != LengthPercentage::length(0.0)
                            && style.gap.height != LengthPercentage::length(0.0),
                        "{name}: wrapped lines need both gaps"
                    );
                }
                for (child, node) in children.iter().zip(nodes) {
                    let child_rect = bounds(tree, node, origin);
                    if tree.style(node).unwrap().position != Position::Absolute {
                        assert!(
                            child_rect.left >= rect.left - 1.0
                                && child_rect.right <= rect.right + 1.0,
                            "{name}: child overflows the containing width: {child_rect:?} in {rect:?}"
                        );
                    }
                    inspect(child, tree, node, origin, headings, name);
                }
            }
            Widget::ScrollView { child, .. }
            | Widget::Container { child, .. }
            | Widget::ButtonContent { child, .. }
            | Widget::PointerRegion { child, .. }
            | Widget::ContextMenu { child, .. }
            | Widget::Tooltip { child, .. } => {
                inspect(child, tree, nodes[0], origin, headings, name);
            }
            Widget::Disabled { child } => inspect(
                child,
                tree,
                node,
                Point::new(
                    rect.left - tree.layout(node).unwrap().location.x,
                    rect.top - tree.layout(node).unwrap().location.y,
                ),
                headings,
                name,
            ),
            Widget::Accordion {
                child,
                expanded: true,
                ..
            } => inspect(child, tree, nodes[0], origin, headings, name),
            Widget::TableOfContents { child, .. } => {
                let viewport = nodes[1];
                let viewport_bounds = bounds(tree, viewport, origin);
                inspect(
                    child,
                    tree,
                    tree.children(viewport).unwrap()[0],
                    Point::new(viewport_bounds.left, viewport_bounds.top),
                    headings,
                    name,
                );
            }
            Widget::Popover { anchor, .. } => {
                inspect(anchor, tree, nodes[0], origin, headings, name)
            }
            // Overlay geometry is separately covered in the popup/color-picker tests.
            _ => {}
        }
    }

    fn assert_document_hierarchy<Message>(
        tree: &TaffyTree<RutterContext>,
        document: NodeId,
        widgets: &[Widget<'_, Message>],
        name: &str,
    ) {
        let sections = tree.children(document).unwrap();
        for pair in sections.windows(2) {
            let previous = bounds(tree, pair[0], Point::default());
            let next = bounds(tree, pair[1], Point::default());
            assert!(
                next.top - previous.bottom >= rutter::spacing::XL - 1.0,
                "{name}: independent page blocks need clear separation"
            );
        }
        for (section, widget) in sections.into_iter().zip(widgets) {
            // A TOC contains independent navigation and document regions, rather
            // than compact peers. Keep checking the article's actual text groups.
            if let Widget::TableOfContents { child, .. } = widget {
                let nodes = tree.children(section).unwrap();
                let navigation = bounds(tree, nodes[0], Point::default());
                let viewport = bounds(tree, nodes[1], Point::default());
                assert!(viewport.top - navigation.bottom >= rutter::spacing::XL - 1.0);
                let Widget::Column { children, .. } = child.as_ref() else {
                    panic!("{name}: expected grouped article sections");
                };
                assert_document_hierarchy(
                    tree,
                    tree.children(nodes[1]).unwrap()[0],
                    children,
                    name,
                );
                continue;
            }
            if tree.style(section).unwrap().flex_direction != FlexDirection::Column {
                continue;
            }
            for pair in tree.children(section).unwrap().windows(2) {
                let previous = bounds(tree, pair[0], Point::default());
                let next = bounds(tree, pair[1], Point::default());
                assert!(
                    next.top - previous.bottom < rutter::spacing::XL - 1.0,
                    "{name}: members of a group must be closer than independent page blocks"
                );
            }
        }
    }

    fn assert_field(rect: Bounds, style: &Style, label: &str, error: Option<&str>, name: &str) {
        let annotation = if label.is_empty() { 0.0 } else { 20.0 }
            + if error.is_some_and(|message| !message.is_empty()) {
                20.0
            } else {
                0.0
            };
        let body = rect.height() - annotation;
        assert!(
            rect.width() >= 40.0 && body > 0.0,
            "{name}: unusable field body {rect:?}"
        );
        if let Some(requested) = style.size.height.into_option() {
            assert!(
                body >= requested - 1.0,
                "{name}: annotations consumed body height {body} < {requested}"
            );
        }
    }

    fn assert_sibling_separation(tree: &TaffyTree<RutterContext>, nodes: &[NodeId], name: &str) {
        for (index, node) in nodes.iter().enumerate() {
            if tree.style(*node).unwrap().position == Position::Absolute {
                continue;
            }
            let first = bounds(tree, *node, Point::default());
            if first.width() <= 0.0 || first.height() <= 0.0 {
                continue;
            }
            for other in &nodes[index + 1..] {
                if tree.style(*other).unwrap().position == Position::Absolute {
                    continue;
                }
                let second = bounds(tree, *other, Point::default());
                if second.width() <= 0.0 || second.height() <= 0.0 {
                    continue;
                }
                assert!(
                    first.right <= second.left + 1.0
                        || second.right <= first.left + 1.0
                        || first.bottom <= second.top + 1.0
                        || second.bottom <= first.top + 1.0,
                    "{name}: intersecting siblings {first:?}, {second:?}"
                );
            }
        }
    }

    fn assert_header_raster<Message>(
        view: &Widget<'_, Message>,
        layout: (&TaffyTree<RutterContext>, NodeId),
        fonts: &Fonts,
        theme: &Theme,
        size: (u32, u32),
        title: Bounds,
        name: &str,
    ) {
        let (tree, root) = layout;
        let mut surface = surfaces::raster_n32_premul((size.0 as i32, size.1 as i32)).unwrap();
        surface.canvas().clear(theme.surface);
        draw_widgets(
            surface.canvas(),
            tree,
            root,
            view,
            &mut fonts.borrow_mut(),
            &mut SwashCache::new(),
            Point::new(-100.0, -100.0),
            None,
            &HashMap::new(),
            &HashMap::new(),
            &mut HashMap::new(),
            &mut TextBufferCache::default(),
            false,
            theme,
            1.0,
        );
        let pixels = surface.peek_pixels().unwrap();
        let bottom = (title.bottom.ceil() as i32).min(size.1 as i32);
        assert!(
            (title.top as i32..bottom).any(|y| (title.left as i32..title.right as i32)
                .any(|x| pixels.get_color((x, y)) != theme.surface)),
            "{name}: missing painted title"
        );
        for y in 61..super::PAGE_TOP as i32 {
            for x in 0..size.0 as i32 {
                assert_eq!(
                    pixels.get_color((x, y)),
                    theme.surface,
                    "{name}: ink in action/title safe band at {x},{y}"
                );
            }
        }
    }

    use rutter::render::hit_test::{HitResult, hit_test};
    use skia_safe::Color;

    pub(crate) fn assert_overlay<Message: Clone>(
        view: &Widget<'_, Message>,
        allowed: fn(&Message) -> bool,
    ) {
        let Widget::Column { children, .. } = view else {
            panic!("expected page column")
        };
        let document_index = children
            .iter()
            .position(|child| matches!(child, Widget::ScrollView { .. }))
            .unwrap();
        let overlay_index = children
            .iter()
            .position(|child| {
                matches!(
                    child,
                    Widget::Modal { visible: true, .. } | Widget::Dialog { visible: true, .. }
                )
            })
            .unwrap();
        assert!(overlay_index > document_index);
        let overlay = &children[overlay_index];
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        for (width, height) in [(800, 700), (320, 260)] {
            let mut tree = TaffyTree::new();
            let root = build_taffy_tree(&mut tree, view, fonts.clone(), &HashMap::new());
            compute_layout(
                &mut tree,
                root,
                PhysicalSize::new(width, height),
                fonts.clone(),
                &RichTextRenderer::default(),
            );
            let overlay_node = tree.children(root).unwrap()[overlay_index];
            let frame = tree.layout(overlay_node).unwrap();
            assert_eq!(frame.location, taffy::geometry::Point::ZERO);
            // Includes empty card space, controls, backdrop and theme action. None is
            // a legitimate inert hit, but no underlying page callback/scroll may leak.
            let mut overlay_actions = 0;
            for y in (4..height).step_by(8) {
                for x in (4..width).step_by(8) {
                    match hit_test(
                        view,
                        &tree,
                        root,
                        Point::new(x as f32, y as f32),
                        Point::default(),
                        &HashMap::new(),
                    ) {
                        Some(HitResult::Message { msg, .. }) => {
                            assert!(allowed(&msg), "underlying page callback leaked");
                            overlay_actions += 1;
                        }
                        Some(HitResult::ModalDismiss(_)) | None => {}
                        _ => panic!("underlying page interaction leaked"),
                    }
                }
            }
            assert!(overlay_actions > 0, "overlay actions must remain clickable");
            for theme in [Theme::dark(), Theme::light()] {
                let mut full = surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();
                let mut isolated =
                    surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();
                for (surface, widget, node, background) in [
                    (&mut full, view, root, theme.surface),
                    (&mut isolated, overlay, overlay_node, Color::TRANSPARENT),
                ] {
                    surface.canvas().clear(background);
                    draw_widgets(
                        surface.canvas(),
                        &tree,
                        node,
                        widget,
                        &mut fonts.borrow_mut(),
                        &mut SwashCache::new(),
                        Point::new(-100.0, -100.0),
                        None,
                        &HashMap::new(),
                        &HashMap::new(),
                        &mut HashMap::new(),
                        &mut TextBufferCache::default(),
                        false,
                        &theme,
                        1.0,
                    );
                }
                let full = full.peek_pixels().unwrap();
                let isolated = isolated.peek_pixels().unwrap();
                // Opaque overlay pixels must be independent of the page underneath.
                // Transparent-backed rendering identifies them without duplicating
                // private card geometry; the translucent backdrop is excluded.
                let mut opaque_pixels = 0;
                for y in (0..height as i32).step_by(3) {
                    for x in (0..width as i32).step_by(3) {
                        let expected = isolated.get_color((x, y));
                        if expected.a() == 255 {
                            assert_eq!(full.get_color((x, y)), expected);
                            opaque_pixels += 1;
                        }
                    }
                }
                assert!(opaque_pixels > 50, "overlay card must actually paint");
            }
        }
    }
}
