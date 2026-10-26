//! Full launcher inventory: real AppLogic views, no window or clipboard required.
//! Geometry and software raster evidence are deliberately not visual acceptance.
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::{ScrollState, WidgetState};
use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
use rutter::render::hit_test::{HitResult, hit_test};
use rutter::render::{RichTextRenderer, draw_widgets, text::TextBufferCache};
use rutter::{
    AppLogic, HeadingLevel, MultiWindowAppLogic, SurfaceId, Theme, Widget, WidgetIdSnapshot,
};
use skia_safe::{Point, Rect as Bounds, surfaces};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

type Fonts = Rc<RefCell<FontSystem>>;

#[test]
fn all_launcher_demos_reflow_and_retain_reachable_documents() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    macro_rules! check {
        ($($module:ident::$application:ident),+ $(,)?) => {
            $(assert_application::<super::$module::$application>(&fonts);)+
        };
    }
    check!(
        accordion_demo::AccordionDemo,
        advanced_widgets_demo::AdvancedWidgetsDemo,
        button_content_demo::ButtonContentDemo,
        calendar_demo::CalendarDemo,
        carousel_demo::CarouselDemo,
        color_picker_demo::ColorPickerDemo,
        combo_box_demo::ComboBoxDemo,
        context_menu_demo::ContextMenuDemo,
        controls_demo::ControlsDemo,
        counter_demo::CounterDemo,
        dialog_demo::DialogDemo,
        drag_drop_demo::DragDropDemo,
        dropdown_menu_demo::DropdownMenuDemo,
        form_demo::MyApp,
        image_viewer_demo::ImageViewerDemo,
        modal_toast_demo::ModalToastDemo,
        popover_demo::PopoverDemo,
        progress_demo::ProgressDemo,
        rich_text_demo::RichTextDemo,
        scroll_demo::ScrollDemo,
        search_bar_demo::SearchBarDemo,
        slider_demo::SliderDemo,
        tab_demo::TabDemo,
        table_demo::TableDemo,
        table_of_contents_demo::TableOfContentsDemo,
        text_area_demo::TextAreaDemo,
        text_input_demo::TextInputDemo,
        time_demo::TimeDemo,
        vgrid_demo::VGridDemo,
        vlist_demo::VListDemo,
    );
    use super::multi_window_demo::MultiWindowDemo;
    let mut state = MultiWindowDemo::new(&mut fonts.borrow_mut());
    for surface in [SurfaceId::PRIMARY, SurfaceId::new(1)] {
        let view = MultiWindowDemo::view(&mut state, surface);
        assert_page(&view, &fonts, "multi_window");
    }
}

fn assert_application<Application: AppLogic>(fonts: &Fonts) {
    let mut state = Application::new(&mut fonts.borrow_mut());
    let view = Application::view(&mut state);
    assert_page(&view, fonts, std::any::type_name::<Application>());
}

fn assert_page<Message>(view: &Widget<'_, Message>, fonts: &Fonts, name: &str) {
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
        assert_document_hierarchy(
            &tree,
            document_node,
            super::layout::document_children(view),
            name,
        );
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

fn bounds(tree: &TaffyTree<RutterContext>, node: NodeId, origin: Point) -> Bounds {
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
                        child_rect.left >= rect.left - 1.0 && child_rect.right <= rect.right + 1.0,
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
        Widget::Popover { anchor, .. } => inspect(anchor, tree, nodes[0], origin, headings, name),
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
            assert_document_hierarchy(tree, tree.children(nodes[1]).unwrap()[0], children, name);
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
    for y in 61..super::layout::PAGE_TOP as i32 {
        for x in 0..size.0 as i32 {
            assert_eq!(
                pixels.get_color((x, y)),
                theme.surface,
                "{name}: ink in action/title safe band at {x},{y}"
            );
        }
    }
}

#[test]
fn form_tabs_and_long_validation_previews_keep_their_layout_contract() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for active_tab in 0..3 {
        let mut state = super::form_demo::AppState {
            active_tab,
            username_state: rutter::InputState::Error,
            theme: super::theme_selector::ExampleTheme::Light,
            ..Default::default()
        };
        assert_page(
            &super::form_demo::MyApp::view(&mut state),
            &fonts,
            "form tab/error",
        );
    }
    let mut state = super::text_input_demo::TextInputDemoState {
        text_normal: "A long localized name — nome completo — 名前 ".repeat(12),
        theme: super::theme_selector::ExampleTheme::Light,
        ..Default::default()
    };
    assert_page(
        &super::text_input_demo::TextInputDemo::view(&mut state),
        &fonts,
        "long input preview",
    );
}

#[test]
fn focused_form_error_keeps_painted_annotations_and_body_hits_after_page_scroll() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for selected in [
        super::theme_selector::ExampleTheme::Dark,
        super::theme_selector::ExampleTheme::Light,
    ] {
        for (width, height) in [(800, 700), (320, 700), (800, 260), (320, 260)] {
            let mut state = super::form_demo::AppState {
                username_state: rutter::InputState::Error,
                theme: selected,
                ..Default::default()
            };
            let view = super::form_demo::MyApp::view(&mut state);
            let mut tree = TaffyTree::new();
            let root = build_taffy_tree(&mut tree, &view, fonts.clone(), &HashMap::new());
            compute_layout(
                &mut tree,
                root,
                PhysicalSize::new(width, height),
                fonts.clone(),
                &RichTextRenderer::default(),
            );
            let Widget::Column { children, .. } = &view else {
                panic!("expected form page root")
            };
            let viewport_index = children
                .iter()
                .position(|child| matches!(child, Widget::ScrollView { .. }))
                .expect("expected form document viewport");
            let viewport_node = tree.children(root).unwrap()[viewport_index];
            let viewport = bounds(&tree, viewport_node, Point::default());
            let document_node = tree.children(viewport_node).unwrap()[0];
            let document = bounds(
                &tree,
                document_node,
                Point::new(viewport.left, viewport.top),
            );
            let field = find_input(&view, &tree, root, Point::default(), 1)
                .expect("login username field must remain in the full example view");
            assert_eq!(field.height(), 80.0, "40px body plus label and error rows");
            let mut scroll = ScrollState {
                offset_y: 0.0,
                content_height: document.height(),
                viewport_h: viewport.height(),
            };
            scroll.scroll_by((field.top - document.top - rutter::spacing::SM).max(0.0));
            let field = Bounds::from_xywh(
                field.left,
                field.top - scroll.offset_y,
                field.width(),
                field.height(),
            );
            assert!(field.top >= viewport.top && field.bottom <= viewport.bottom);
            let states =
                HashMap::from([(super::layout::PAGE_SCROLL_ID, WidgetState::Scroll(scroll))]);
            let point = Point::new(field.center_x(), field.top + 40.0);
            assert!(
                matches!(hit_test(&view, &tree, root, point, Point::default(), &states),
                Some(HitResult::InputFocus { id: 1, height: 40.0, local_y, .. })
                    if (local_y - 20.0).abs() <= 1.0)
            );

            let theme = selected.resolve();
            let mut surface = surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();
            surface.canvas().clear(theme.surface);
            draw_widgets(
                surface.canvas(),
                &tree,
                root,
                &view,
                &mut fonts.borrow_mut(),
                &mut SwashCache::new(),
                Point::new(-100.0, -100.0),
                Some(1),
                &HashMap::new(),
                &states,
                &mut HashMap::new(),
                &mut TextBufferCache::default(),
                false,
                &theme,
                1.0,
            );
            let pixels = surface.peek_pixels().unwrap();
            // Avoid the body border/focus stroke: only annotation ink can
            // satisfy these inset row checks.
            for (top, bottom) in [
                (field.top + 2.0, field.top + 18.0),
                (field.bottom - 18.0, field.bottom - 2.0),
            ] {
                assert!(
                    (top.ceil() as i32..bottom.floor() as i32)
                        .any(|y| (field.left.ceil() as i32..field.right.floor() as i32)
                            .any(|x| pixels.get_color((x, y)) != theme.surface)),
                    "username label and Required error must both paint inside their rows"
                );
            }
        }
    }
}

fn find_input<Message>(
    widget: &Widget<'_, Message>,
    tree: &TaffyTree<RutterContext>,
    node: NodeId,
    origin: Point,
    wanted: u64,
) -> Option<Bounds> {
    let rect = bounds(tree, node, origin);
    let origin = Point::new(rect.left, rect.top);
    let nodes = tree.children(node).unwrap();
    match widget {
        Widget::TextInput { id, .. } if *id == wanted => Some(rect),
        Widget::Column { children, .. } | Widget::Row { children, .. } => children
            .iter()
            .zip(nodes)
            .find_map(|(child, node)| find_input(child, tree, node, origin, wanted)),
        Widget::ScrollView { child, .. } | Widget::Container { child, .. } => {
            find_input(child, tree, nodes[0], origin, wanted)
        }
        _ => None,
    }
}

#[test]
fn all_example_sources_use_the_shared_spacing_vocabulary() {
    macro_rules! sources {
        ($($file:literal),+ $(,)?) => { [$(($file, include_str!(concat!("../../examples/", $file)))),+] };
    }
    let sources = sources!(
        "widgets/accordion_demo.rs",
        "widgets/advanced_widgets_demo.rs",
        "widgets/button_content_demo.rs",
        "widgets/calendar_demo.rs",
        "widgets/carousel_demo.rs",
        "widgets/color_picker_demo.rs",
        "widgets/combo_box_demo.rs",
        "widgets/context_menu_demo.rs",
        "widgets/controls_demo.rs",
        "widgets/counter_demo.rs",
        "widgets/dialog_demo.rs",
        "widgets/drag_drop_demo.rs",
        "widgets/dropdown_menu_demo.rs",
        "widgets/form_demo.rs",
        "widgets/image_viewer_demo.rs",
        "widgets/modal_toast_demo.rs",
        "widgets/multi_window_demo.rs",
        "widgets/popover_demo.rs",
        "widgets/progress_demo.rs",
        "widgets/rich_text_demo.rs",
        "widgets/scroll_demo.rs",
        "widgets/search_bar_demo.rs",
        "widgets/slider_demo.rs",
        "widgets/tab_demo.rs",
        "widgets/table_demo.rs",
        "widgets/table_of_contents_demo.rs",
        "widgets/text_area_demo.rs",
        "widgets/text_input_demo.rs",
        "widgets/time_demo.rs",
        "widgets/vgrid_demo.rs",
        "widgets/vlist_demo.rs",
        "native_popup_clock.rs",
    );
    assert_eq!(sources.len(), 32);
    for (name, source) in sources {
        assert!(
            source.contains("rutter::spacing"),
            "{name}: import public spacing tokens"
        );
        // Only authored spacing declarations are inspected: calendar/time-picker
        // generated internals, typography, hit targets and borders are not spacing.
        for marker in ["padding:", "gap:", "margin:"] {
            for declaration in source.split(marker).skip(1) {
                let end = declaration.find("},").unwrap_or(declaration.len());
                let declaration = &declaration[..end];
                let declaration = if declaration.trim_start().starts_with("Size {")
                    || declaration.trim_start().starts_with("Rect {")
                {
                    declaration
                } else {
                    declaration.split('\n').next().unwrap()
                };
                assert!(
                    !declaration.contains("::length(1")
                        && !declaration.contains("::length(2")
                        && !declaration.contains("::length(3")
                        && !declaration.contains("::length(4")
                        && !declaration.contains("::length(5")
                        && !declaration.contains("::length(6")
                        && !declaration.contains("::length(7")
                        && !declaration.contains("::length(8")
                        && !declaration.contains("::length(9"),
                    "{name}: off-scale authored {marker} {declaration}"
                );
                if declaration.contains("gap_size(") {
                    assert!(
                        declaration.contains("spacing::"),
                        "{name}: numeric gap alias"
                    );
                }
            }
        }
    }
}
