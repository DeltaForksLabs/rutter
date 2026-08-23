use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::{PopoverState, WidgetState};
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::draw_widgets;
use rutter::render::hit_test::{HitResult, PopoverOverlayHit, hit_test_popover_overlay};
use rutter::render::text::TextBufferCache;
use rutter::{
    ClockConfig, ClockFormat, Theme, TimeOfDay, TimePickerConfig, TimeZone, Widget,
    WidgetIdSnapshot,
};
use skia_safe::{Color, Font, Point, surfaces};
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Toggle,
    Close,
    Hour(i64),
    Minute(i64),
    Second(i64),
    Zone(usize),
}

const TIME_ZONES: &[&str] = &["UTC", "America/Sao_Paulo"];

#[test]
fn clock_renders_as_a_timezone_aware_text_leaf() {
    let widget = Widget::clock_with_config(
        TimeZone::UTC,
        ClockConfig::new(ClockFormat::twenty_four_hour(), 28.0).unwrap(),
        clock_style(),
        "UTC clock",
    )
    .with_id(71);

    assert!(rendered_pixel_count(&widget) > 100);
}

#[test]
fn time_picker_changes_preserve_its_composed_widget_ids() {
    let before = time_picker(TimeOfDay::new(9, 30, 0).unwrap(), 0);
    let after = time_picker(TimeOfDay::new(17, 45, 30).unwrap(), 1);
    let before_snapshot = WidgetIdSnapshot::capture(&before).unwrap();
    let after_snapshot = WidgetIdSnapshot::capture(&after).unwrap();

    before_snapshot
        .validate_transition_to(&after_snapshot)
        .unwrap();
    before_snapshot
        .validate_reconstruction(&after_snapshot)
        .unwrap();
}

#[test]
fn open_time_picker_routes_counter_clicks_from_its_popover_content() {
    let hit = open_picker_hit(Point::new(20.0, 108.0));

    assert!(matches!(
        hit,
        Some(PopoverOverlayHit::Content(HitResult::CounterAdjust {
            increment: false,
            ..
        }))
    ));
}

#[test]
fn open_time_picker_routes_timezone_clicks_to_its_select_trigger() {
    let hit = open_picker_hit(Point::new(20.0, 180.0));

    assert!(matches!(
        hit,
        Some(PopoverOverlayHit::Content(HitResult::SelectToggle(_)))
    ));
}

fn time_picker(time: TimeOfDay, selected_time_zone: usize) -> Widget<'static, Msg> {
    let config = TimePickerConfig::new(
        TIME_ZONES,
        selected_time_zone,
        Msg::Toggle,
        Msg::Close,
        Msg::Hour,
        Msg::Minute,
        Msg::Second,
        Msg::Zone,
        "Event time",
    )
    .unwrap();
    Widget::time_picker(true, time, config, picker_style(), popup_style()).with_id(72)
}

fn open_picker_states() -> HashMap<u64, WidgetState> {
    let mut popover = PopoverState::default();
    popover.set_open(true);
    popover.set_anchor_rect(0.0, 0.0, 260.0, 44.0);
    HashMap::from([(72, WidgetState::Popover(popover))])
}

fn open_picker_hit(mouse: Point) -> Option<PopoverOverlayHit<Msg>> {
    let picker = time_picker(TimeOfDay::new(9, 30, 0).unwrap(), 0);
    let states = open_picker_states();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &picker, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(800, 600),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    hit_test_popover_overlay(&picker, &taffy, root, mouse, (800.0, 600.0), &states)
}

fn clock_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(320.0),
            height: Dimension::length(56.0),
        },
        ..Style::default()
    }
}

fn picker_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(260.0),
            height: Dimension::length(44.0),
        },
        ..Style::default()
    }
}

fn popup_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(360.0),
            height: Dimension::length(210.0),
        },
        ..Style::default()
    }
}

fn rendered_pixel_count(widget: &Widget<'_, Msg>) -> usize {
    let states = HashMap::<u64, WidgetState>::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(320, 56),
        fonts,
        &rutter::render::RichTextRenderer::default(),
    );
    let mut surface = surfaces::raster_n32_premul((320, 56)).unwrap();
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
        &states,
        &mut HashMap::<(String, u32), Font>::new(),
        &mut TextBufferCache::default(),
        true,
        &Theme::default(),
        1.0,
    );
    nontransparent_pixel_count(&mut surface, 320, 56)
}

fn nontransparent_pixel_count(surface: &mut skia_safe::Surface, width: i32, height: i32) -> usize {
    let pixels = surface.peek_pixels().unwrap();
    let mut count = 0;
    for y in 0..height {
        for x in 0..width {
            count += usize::from(pixels.get_color((x, y)).a() > 0);
        }
    }
    count
}
