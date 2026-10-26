use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::{PopoverState, WidgetState};
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::draw_widgets;
use rutter::render::hit_test::{HitResult, PopoverOverlayHit, hit_test, hit_test_popover_overlay};
use rutter::render::text::TextBufferCache;
use rutter::{
    ButtonVariant, CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, Theme, WeekStart,
    Widget, WidgetIdSnapshot,
};
use skia_safe::textlayout::{
    FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextStyle,
};
use skia_safe::{Color, Font, Point, surfaces};
use skia_safe::{FontMgr, FontStyle, Rect};
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Toggle,
    Close,
    Select(CalendarDate),
    Navigate(CalendarMonth),
}

#[test]
fn calendar_month_updates_preserve_widget_structure_and_ids() {
    let july = standalone_calendar(CalendarMonth::new(2026, 7).unwrap());
    let august = standalone_calendar(CalendarMonth::new(2026, 8).unwrap());
    let july_snapshot = WidgetIdSnapshot::capture(&july).unwrap();
    let august_snapshot = WidgetIdSnapshot::capture(&august).unwrap();

    july_snapshot
        .validate_transition_to(&august_snapshot)
        .unwrap();
    july_snapshot
        .validate_reconstruction(&august_snapshot)
        .unwrap();
}

#[test]
fn open_date_picker_routes_popup_day_clicks_to_typed_messages() {
    let picker = open_date_picker().with_id(920);
    let mut popover = PopoverState::default();
    popover.set_open(true);
    popover.set_anchor_rect(0.0, 0.0, 230.0, 44.0);
    let states = HashMap::from([(920, WidgetState::Popover(popover))]);
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &picker, font_system(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(800, 600),
        font_system(),
        &rutter::render::RichTextRenderer::default(),
    );

    let hit = hit_test_popover_overlay(
        &picker,
        &taffy,
        root,
        Point::new(25.0, 140.0),
        (800.0, 600.0),
        &states,
    );

    assert!(matches!(
        hit,
        Some(PopoverOverlayHit::Content(HitResult::Message {
            msg: Msg::Select(_),
            ..
        }))
    ));
}

#[test]
fn open_date_picker_matches_validated_calendar_raster_and_exact_day_hits() {
    for (viewport_width, calendar_width) in [(320, 272), (800, 340)] {
        let calendar = regression_calendar(calendar_width, CalendarLabels::PORTUGUESE, 5);
        let (calendar_tree, calendar_root) = layout_calendar(&calendar, calendar_width);
        let picker = Widget::date_picker_with_config(
            true,
            CalendarMonth::new(2028, 5).unwrap(),
            Some(CalendarDate::new(2028, 5, 28).unwrap()),
            Msg::Toggle,
            Msg::Close,
            Msg::Select,
            Msg::Navigate,
            "Data",
            "AAAA-MM-DD",
            CalendarConfig::new(CalendarLabels::PORTUGUESE, WeekStart::Monday),
            picker_style(),
            Style {
                size: Size::from_lengths(calendar_width as f32, 330.0),
                ..Style::default()
            },
        )
        .with_id(920);
        let mut popover = PopoverState::default();
        popover.set_open(true);
        popover.set_anchor_rect(0.0, 0.0, 230.0, 44.0);
        let states = HashMap::from([(920, WidgetState::Popover(popover))]);
        let fonts = font_system();
        let mut tree = TaffyTree::new();
        let root = build_taffy_tree(&mut tree, &picker, fonts.clone(), &states);
        // A short viewport still accommodates the popup below the anchor.
        compute_layout(
            &mut tree,
            root,
            PhysicalSize::new(viewport_width, 420),
            fonts,
            &rutter::render::RichTextRenderer::default(),
        );
        let content = calendar_tree.children(calendar_root).unwrap()[0];
        let weeks = calendar_tree.children(content).unwrap();
        for day in [9, 10, 28, 31] {
            // May 2028 starts on Monday; the first cell is May 1.
            let index = day as usize - 1;
            let button = calendar_tree.children(weeks[2 + index / 7]).unwrap()[index % 7];
            let center = absolute_rect(&calendar_tree, button).center();
            let hit = hit_test_popover_overlay(
                &picker,
                &tree,
                root,
                Point::new(center.x + 8.0, center.y + 52.0),
                (viewport_width as f32, 420.0),
                &states,
            );
            assert!(
                matches!(hit, Some(PopoverOverlayHit::Content(HitResult::Message { msg: Msg::Select(date), .. })) if date == CalendarDate::new(2028, 5, day).unwrap())
            );
        }
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                let dimensions = (
                    (viewport_width as f32 * scale) as i32,
                    (420.0 * scale) as i32,
                );
                let mut actual = surfaces::raster_n32_premul(dimensions).unwrap();
                let mut expected = surfaces::raster_n32_premul(dimensions).unwrap();
                for surface in [&mut actual, &mut expected] {
                    surface.canvas().clear(theme.surface);
                    surface.canvas().scale((scale, scale));
                }
                draw_test_tree(
                    actual.canvas(),
                    &tree,
                    root,
                    &picker,
                    &states,
                    &theme,
                    scale,
                );
                expected.canvas().translate((8.0, 52.0));
                draw_test_tree(
                    expected.canvas(),
                    &calendar_tree,
                    calendar_root,
                    &calendar,
                    &HashMap::new(),
                    &theme,
                    scale,
                );
                let actual_pixels = actual.peek_pixels().unwrap();
                let expected_pixels = expected.peek_pixels().unwrap();
                // Compare the entire content, excluding the popup's intentional
                // outer surface/shadow. Standalone labels have independent references above.
                for y in (60.0 * scale) as i32..(374.0 * scale) as i32 {
                    for x in (16.0 * scale) as i32..(calendar_width as f32 * scale) as i32 {
                        assert_eq!(
                            actual_pixels.get_color((x, y)),
                            expected_pixels.get_color((x, y)),
                            "popup content at ({x},{y}), viewport={viewport_width}, scale={scale}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn standalone_calendar_renders_selected_day_and_controls() {
    let month = CalendarMonth::new(2026, 7).unwrap();
    let widget = Widget::calendar(
        month,
        Some(CalendarDate::new(2026, 7, 31).unwrap()),
        Msg::Select,
        Msg::Navigate,
        calendar_style(),
    );

    assert!(rendered_pixel_count(&widget) > 500);
}

#[test]
fn calendar_month_and_year_share_rich_text_alignment() {
    let widget = standalone_calendar(CalendarMonth::new(2026, 7).unwrap());
    let states = HashMap::new();
    let fonts = font_system();
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(340, 330),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    let [month_node, year_node] = calendar_heading_text_nodes(&taffy, root);
    let month = taffy.layout(month_node).unwrap();
    let year = taffy.layout(year_node).unwrap();

    assert!(matches!(
        taffy.get_node_context(month_node),
        Some(rutter::layout::RutterContext::RichText(_))
    ));
    assert_eq!(month.location.y, year.location.y);
    assert_eq!(month.size.height, year.size.height);
    assert!(month.size.width > 0.0 && year.size.width > 0.0);
    assert!(
        month.location.x + month.size.width <= year.location.x,
        "month box {:?} overlaps year box {:?}",
        month,
        year
    );
}

#[test]
fn localized_calendar_labels_match_complete_unbounded_reference_rasters() {
    // The demo owns a 24px inset on either side and caps calendars at 340px.
    // Include full-width embeddings as well as these actual demo allocations.
    for width in [272, 320, 340, 800] {
        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5, 2.0] {
                for (labels, month) in [
                    (CalendarLabels::PORTUGUESE, 5), // Screenshot: Maio 2028.
                    (CalendarLabels::PORTUGUESE, 9), // Setembro wraps at 272px.
                    (CalendarLabels::ENGLISH, 9),    // September is the longest built-in month.
                ] {
                    assert_calendar_labels(width, &theme, scale, labels, month);
                }
            }
        }
    }
}

#[test]
fn compact_calendar_days_preserve_labels_variants_focus_and_hit_messages() {
    for width in [272, 320, 340, 800] {
        let widget = regression_calendar(width, CalendarLabels::PORTUGUESE, 5);
        let (tree, root) = layout_calendar(&widget, width);
        let content = tree.children(root).unwrap()[0];
        let sections = tree.children(content).unwrap();
        let Widget::Container { child, .. } = &widget else {
            unreachable!()
        };
        let Widget::Column { children, .. } = child.as_ref() else {
            unreachable!()
        };
        let mut focus_ids = std::collections::HashSet::new();
        for (week_index, week) in children[2..].iter().enumerate() {
            let Widget::Row { children: days, .. } = week else {
                unreachable!()
            };
            for (column, day) in days.iter().enumerate() {
                let Widget::ButtonContent {
                    label,
                    on_press,
                    variant,
                    ..
                } = day
                else {
                    panic!("day must retain accessible ButtonContent semantics");
                };
                let Msg::Select(date) = on_press else {
                    panic!("expected typed selection")
                };
                assert_eq!(*label, date.day().to_string());
                assert_eq!(
                    *variant,
                    if date.day() == 28 && date.month() == 5 {
                        ButtonVariant::Primary
                    } else if date.month() == 5 {
                        ButtonVariant::Ghost
                    } else {
                        ButtonVariant::Text
                    }
                );
                let week_node = sections[week_index + 2];
                let node = tree.children(week_node).unwrap()[column];
                let rect = absolute_rect(&tree, node);
                let hit = hit_test(
                    &widget,
                    &tree,
                    root,
                    rect.center(),
                    Point::new(0.0, 0.0),
                    &HashMap::new(),
                );
                let Some(HitResult::Message { msg, focus_id }) = hit else {
                    panic!("day center must hit button")
                };
                assert_eq!(msg, *on_press);
                assert!(
                    focus_ids.insert(focus_id.unwrap()),
                    "each date remains separately focusable"
                );
            }
        }
        assert_eq!(focus_ids.len(), 42);
    }
}

fn regression_calendar(
    width: u32,
    labels: CalendarLabels<'static>,
    month: u8,
) -> Widget<'static, Msg> {
    Widget::calendar_with_config(
        CalendarMonth::new(2028, month).unwrap(),
        Some(CalendarDate::new(2028, month, 28).unwrap()),
        Msg::Select,
        Msg::Navigate,
        CalendarConfig::new(labels, WeekStart::Monday),
        Style {
            size: Size::from_lengths(width as f32, 330.0),
            ..Style::default()
        },
    )
}

fn layout_calendar(
    widget: &Widget<'_, Msg>,
    width: u32,
) -> (
    TaffyTree<rutter::layout::RutterContext>,
    taffy::prelude::NodeId,
) {
    let fonts = font_system();
    let mut tree = TaffyTree::new();
    let root = build_taffy_tree(&mut tree, widget, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut tree,
        root,
        PhysicalSize::new(width, 330),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    (tree, root)
}

fn absolute_rect(
    tree: &TaffyTree<rutter::layout::RutterContext>,
    node: taffy::prelude::NodeId,
) -> Rect {
    let layout = tree.layout(node).unwrap();
    let mut origin = Point::new(layout.location.x, layout.location.y);
    let mut parent = tree.parent(node);
    while let Some(node) = parent {
        let layout = tree.layout(node).unwrap();
        origin.x += layout.location.x;
        origin.y += layout.location.y;
        parent = tree.parent(node);
    }
    Rect::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height)
}

fn reference_paragraph(text: &str, size: f32, bold: bool, plain: bool, color: Color) -> Paragraph {
    let mut fonts = FontCollection::new();
    fonts.set_default_font_manager(FontMgr::new(), Some("sans-serif"));
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["sans-serif"]);
    text_style.set_font_size(size);
    text_style.set_color(color);
    text_style.set_font_style(if bold {
        FontStyle::bold()
    } else {
        FontStyle::normal()
    });
    if plain {
        text_style.set_height(1.2);
        text_style.set_height_override(true);
    }
    let mut style = ParagraphStyle::new();
    style.set_text_style(&text_style);
    let mut builder = ParagraphBuilder::new(&style, fonts);
    builder.add_text(text);
    let mut paragraph = builder.build();
    // Independent, unconstrained reference: never reuse the calendar's text bounds.
    paragraph.layout(10_000.0);
    paragraph
}

fn assert_calendar_labels(
    width: u32,
    theme: &Theme,
    scale: f32,
    labels: CalendarLabels<'static>,
    month: u8,
) {
    let widget = regression_calendar(width, labels, month);
    let (tree, root) = layout_calendar(&widget, width);
    assert_calendar_geometry(&tree, root);
    let dimensions = ((width as f32 * scale) as i32, (330.0 * scale) as i32);
    let mut actual = surfaces::raster_n32_premul(dimensions).unwrap();
    actual.canvas().clear(theme.surface);
    actual.canvas().scale((scale, scale));
    draw_widgets(
        actual.canvas(),
        &tree,
        root,
        &widget,
        &mut FontSystem::new(),
        &mut SwashCache::new(),
        Point::new(-1.0, -1.0),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &mut HashMap::new(),
        &mut TextBufferCache::default(),
        true,
        theme,
        scale,
    );

    let content = tree.children(root).unwrap()[0];
    let sections = tree.children(content).unwrap();
    let [month_node, year_node] = calendar_heading_text_nodes(&tree, root);
    let mut targets = vec![
        (
            month_node,
            labels.months[month as usize - 1].to_owned(),
            16.0,
            false,
            false,
            false,
        ),
        (year_node, "2028".to_owned(), 16.0, true, false, false),
    ];
    let sunday = tree.children(sections[1]).unwrap()[6];
    targets.push((
        tree.children(sunday).unwrap()[0],
        labels.weekdays_sunday_first[0].to_owned(),
        11.0,
        false,
        true,
        false,
    ));
    // Find visible-month dates through callbacks, not a hardcoded grid position.
    let Widget::Container { child, .. } = &widget else {
        unreachable!()
    };
    let Widget::Column { children, .. } = child.as_ref() else {
        unreachable!()
    };
    for (week_index, week) in children[2..].iter().enumerate() {
        let Widget::Row { children: days, .. } = week else {
            unreachable!()
        };
        for (column, day) in days.iter().enumerate() {
            let Widget::ButtonContent {
                label,
                on_press: Msg::Select(date),
                ..
            } = day
            else {
                unreachable!()
            };
            if date.month() == month && [9, 10, 28, 31].contains(&date.day()) {
                let button = tree.children(sections[week_index + 2]).unwrap()[column];
                let container = tree.children(button).unwrap()[0];
                targets.push((
                    tree.children(container).unwrap()[0],
                    (*label).to_owned(),
                    16.0,
                    false,
                    true,
                    date.day() == 28,
                ));
            }
        }
    }
    assert_eq!(targets.len(), if month == 5 { 7 } else { 6 });
    for (node, text, size, bold, plain, selected) in targets {
        let rect = absolute_rect(&tree, node);
        let background = if selected {
            theme.primary
        } else {
            theme.surface
        };
        let foreground = if selected {
            theme.on_primary
        } else {
            theme.on_surface
        };
        let paragraph = reference_paragraph(&text, size, bold, plain, foreground);
        assert!(
            rect.width() + 0.01 >= paragraph.max_intrinsic_width(),
            "{text} width: {rect:?}"
        );
        assert!(
            rect.height() + 0.01 >= paragraph.height(),
            "{text} height: {rect:?}"
        );
        let mut expected = surfaces::raster_n32_premul(dimensions).unwrap();
        expected.canvas().clear(background);
        expected.canvas().scale((scale, scale));
        paragraph.paint(
            expected.canvas(),
            (
                rect.left,
                rect.top + (rect.height() - paragraph.height()) / 2.0,
            ),
        );
        let expected_pixels = expected.peek_pixels().unwrap();
        let actual_pixels = actual.peek_pixels().unwrap();
        let mut ink = 0;
        for y in
            ((rect.top - 1.0) * scale).floor() as i32..((rect.bottom + 1.0) * scale).ceil() as i32
        {
            for x in ((rect.left - 1.0) * scale).floor() as i32
                ..((rect.right + 1.0) * scale).ceil() as i32
            {
                let expected_color = expected_pixels.get_color((x, y));
                let actual_color = actual_pixels.get_color((x, y));
                // Unbounded and bounded Skia paragraphs can quantize edge coverage
                // differently at fractional scale; missing glyphs far exceed this.
                assert!(
                    actual_color.a().abs_diff(expected_color.a()) <= 1
                        && actual_color.r().abs_diff(expected_color.r()) <= 12
                        && actual_color.g().abs_diff(expected_color.g()) <= 12
                        && actual_color.b().abs_diff(expected_color.b()) <= 12,
                    "complete {text:?} at ({x},{y}), bounds={rect:?}, actual={actual_color:?}, expected={expected_color:?}, width={width}, scale={scale}, theme={theme:?}"
                );
                ink += usize::from(expected_color != background);
            }
        }
        assert!(ink > 0, "reference must contain glyphs for {text}");
    }
}

fn assert_calendar_geometry(
    tree: &TaffyTree<rutter::layout::RutterContext>,
    node: taffy::prelude::NodeId,
) {
    let parent = absolute_rect(tree, node);
    let children = tree.children(node).unwrap();
    for (index, child) in children.iter().enumerate() {
        let rect = absolute_rect(tree, *child);
        assert!(rect.width() > 0.0 && rect.height() > 0.0);
        assert!(
            rect.left >= parent.left - 1.0
                && rect.right <= parent.right + 1.0
                && rect.top >= parent.top - 1.0
                && rect.bottom <= parent.bottom + 1.0,
            "child {rect:?} outside {parent:?}"
        );
        for sibling in &children[..index] {
            let other = absolute_rect(tree, *sibling);
            assert!(
                rect.right <= other.left + 1.0
                    || other.right <= rect.left + 1.0
                    || rect.bottom <= other.top + 1.0
                    || other.bottom <= rect.top + 1.0,
                "siblings {rect:?} and {other:?} overlap"
            );
        }
        assert_calendar_geometry(tree, *child);
    }
}

fn standalone_calendar(month: CalendarMonth) -> Widget<'static, Msg> {
    Widget::calendar_with_config(
        month,
        None,
        Msg::Select,
        Msg::Navigate,
        CalendarConfig::new(CalendarLabels::ENGLISH, WeekStart::Monday),
        calendar_style(),
    )
}

fn open_date_picker() -> Widget<'static, Msg> {
    Widget::date_picker(
        true,
        CalendarMonth::new(2026, 7).unwrap(),
        None,
        Msg::Toggle,
        Msg::Close,
        Msg::Select,
        Msg::Navigate,
        "Date",
        "YYYY-MM-DD",
        picker_style(),
        calendar_style(),
    )
}

fn calendar_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(340.0),
            height: Dimension::length(330.0),
        },
        ..Style::default()
    }
}

fn picker_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(230.0),
            height: Dimension::length(44.0),
        },
        ..Style::default()
    }
}

fn font_system() -> Rc<RefCell<FontSystem>> {
    Rc::new(RefCell::new(FontSystem::new()))
}

fn calendar_heading_text_nodes(
    taffy: &TaffyTree<rutter::layout::RutterContext>,
    root: taffy::prelude::NodeId,
) -> [taffy::prelude::NodeId; 2] {
    let content = taffy.children(root).unwrap()[0];
    let header = taffy.children(content).unwrap()[0];
    let heading = taffy.children(header).unwrap()[2];
    let heading_row = taffy.children(heading).unwrap()[0];
    taffy.children(heading_row).unwrap().try_into().unwrap()
}

fn rendered_pixel_count(widget: &Widget<'_, Msg>) -> usize {
    let states = HashMap::new();
    let fonts = font_system();
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(340, 330),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    let mut surface = surfaces::raster_n32_premul((340, 330)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    draw_calendar_tree(surface.canvas(), &taffy, root, widget, &states);
    nontransparent_pixels(&mut surface, 340, 330)
}

fn draw_calendar_tree(
    canvas: &skia_safe::Canvas,
    taffy: &TaffyTree<rutter::layout::RutterContext>,
    root: taffy::prelude::NodeId,
    widget: &Widget<'_, Msg>,
    states: &HashMap<u64, WidgetState>,
) {
    draw_test_tree(canvas, taffy, root, widget, states, &Theme::default(), 1.0);
}

fn draw_test_tree(
    canvas: &skia_safe::Canvas,
    taffy: &TaffyTree<rutter::layout::RutterContext>,
    root: taffy::prelude::NodeId,
    widget: &Widget<'_, Msg>,
    states: &HashMap<u64, WidgetState>,
    theme: &Theme,
    scale: f32,
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
        states,
        &mut HashMap::<(String, u32), Font>::new(),
        &mut TextBufferCache::default(),
        true,
        theme,
        scale,
    );
}

fn nontransparent_pixels(surface: &mut skia_safe::Surface, width: i32, height: i32) -> usize {
    let pixels = surface.peek_pixels().unwrap();
    let mut count = 0;
    for y in 0..height {
        for x in 0..width {
            count += usize::from(pixels.get_color((x, y)).a() > 0);
        }
    }
    count
}
