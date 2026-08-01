use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::{PopoverState, WidgetState};
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::draw_widgets;
use rutter::render::hit_test::{HitResult, PopoverOverlayHit, hit_test_popover_overlay};
use rutter::render::text::TextBufferCache;
use rutter::{
    CalendarConfig, CalendarDate, CalendarLabels, CalendarMonth, Theme, WeekStart, Widget,
    WidgetIdSnapshot,
};
use skia_safe::{Color, Font, Point, surfaces};
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
    compute_layout(&mut taffy, root, PhysicalSize::new(800, 600), font_system());

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
fn calendar_month_and_year_layout_boxes_do_not_overlap() {
    let widget = standalone_calendar(CalendarMonth::new(2026, 7).unwrap());
    let states = HashMap::new();
    let fonts = font_system();
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &widget, fonts.clone(), &states);
    compute_layout(&mut taffy, root, PhysicalSize::new(340, 330), fonts);
    let [month_node, year_node] = calendar_heading_text_nodes(&taffy, root);
    let month = taffy.layout(month_node).unwrap();
    let year = taffy.layout(year_node).unwrap();

    assert!(month.size.width > 0.0 && year.size.width > 0.0);
    assert!(
        month.location.x + month.size.width <= year.location.x,
        "month box {:?} overlaps year box {:?}",
        month,
        year
    );
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
    compute_layout(&mut taffy, root, PhysicalSize::new(340, 330), fonts);
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
        &Theme::default(),
        1.0,
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
