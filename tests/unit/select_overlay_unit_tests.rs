use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::FontSystem;
use skia_safe::{Color, surfaces};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use super::*;
use crate::engine::widget_state::{PopoverState, ScrollState, SelectState};
use crate::layout::{build_taffy_tree, compute_layout};
use crate::render::RichTextRenderer;
use crate::widget::{AUTO_ID, DialogPosition};

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestMessage {
    Select(usize),
    Action,
}

#[test]
fn popup_uses_space_below_the_anchor_when_available() {
    let anchor = SkiaRect::from_xywh(12.0, 20.0, 120.0, 40.0);

    let popup = select_popup_rect(anchor, 2, (320.0, 240.0));

    assert_eq!(popup, SkiaRect::from_xywh(12.0, 60.0, 120.0, 64.0));
}

#[test]
fn popup_moves_above_the_anchor_near_the_viewport_bottom() {
    let anchor = SkiaRect::from_xywh(12.0, 180.0, 120.0, 40.0);

    let popup = select_popup_rect(anchor, 2, (320.0, 240.0));

    assert_eq!(popup, SkiaRect::from_xywh(12.0, 116.0, 120.0, 64.0));
}

#[test]
fn constrained_popup_uses_the_larger_side_without_covering_trigger() {
    let anchor = SkiaRect::from_xywh(12.0, 100.0, 120.0, 40.0);

    let popup = select_popup_rect(anchor, 5, (320.0, 240.0));

    assert_eq!(popup.top, anchor.bottom);
    assert!(popup.bottom <= 240.0);
}

#[test]
fn constrained_popup_window_follows_the_focused_option() {
    let anchor = SkiaRect::from_xywh(12.0, 100.0, 120.0, 40.0);

    let popup = popup_layout_for_focus(anchor, 20, 19, (320.0, 240.0));

    assert_eq!(popup.visible_options, 3);
    assert_eq!(popup.first_option, 17);
    assert!(popup.rect.bottom <= 240.0);
}

#[test]
fn overlay_hit_selects_an_option_outside_the_trigger_bounds() {
    let (widget, states, taffy, root) = laid_out_open_select();
    let hit = hit_test_select_overlay(
        &widget,
        &taffy,
        root,
        &states,
        Point::new(20.0, 88.0),
        (320.0, 240.0),
    );

    assert_eq!(hit, Some(SelectOptionOverlayHit { id: 55, index: 1 }));
}

#[test]
fn overlay_draws_popup_pixels_below_the_trigger() {
    let (widget, states, taffy, root) = laid_out_open_select();
    let mut surface = surfaces::raster_n32_premul((320, 240)).unwrap();
    surface.canvas().clear(Color::RED);
    let mut fonts = HashMap::new();

    draw_select_overlays(
        surface.canvas(),
        &taffy,
        root,
        &widget,
        &states,
        Point::new(0.0, 0.0),
        &mut fonts,
        &Theme::light(),
        1.0,
    );

    let pixel = surface.peek_pixels().unwrap().get_color((10, 88));
    assert_ne!(pixel, Color::RED);
}

#[test]
fn scrolled_out_trigger_does_not_leave_clickable_popup() {
    let widget = scrolled_select_widget();
    let states = scrolled_select_states();
    let (taffy, root) = layout_widget(&widget, &states);

    let hit = hit_test_select_overlay(
        &widget,
        &taffy,
        root,
        &states,
        Point::new(20.0, 10.0),
        (320.0, 240.0),
    );

    assert_eq!(hit, None);
}

#[test]
fn visible_modal_suppresses_underlying_select_popup() {
    let widget = modal_over_open_select();
    let states = open_select_states();
    let (taffy, root) = layout_widget(&widget, &states);

    let hit = hit_test_select_overlay(
        &widget,
        &taffy,
        root,
        &states,
        Point::new(20.0, 88.0),
        (320.0, 240.0),
    );

    assert_eq!(hit, None);
}

#[test]
fn later_dialog_suppresses_select_from_earlier_modal() {
    let widget = modal_select_under_dialog();
    let states = open_select_states();
    let (taffy, root) = layout_widget(&widget, &states);

    let overlays = collect_open_select_overlays(&widget, &taffy, root, &states, (320.0, 240.0));

    assert!(overlays.is_empty());
}

#[test]
fn root_trigger_outside_viewport_does_not_create_popup() {
    let widget = root_overflowing_select();
    let states = open_select_states();
    let (taffy, root) = layout_widget(&widget, &states);

    let overlays = collect_open_select_overlays(&widget, &taffy, root, &states, (320.0, 240.0));

    assert!(overlays.is_empty());
}

#[test]
fn automatic_select_id_inside_popover_uses_content_path() {
    let widget = popover_with_automatic_select();
    let mut states = HashMap::new();
    let select_id = popover_select_id(&widget);
    states.insert(
        select_id,
        WidgetState::Select(SelectState {
            is_open: true,
            hovered_option: None,
        }),
    );
    states.insert(70, WidgetState::Popover(open_popover_state()));
    let (taffy, root) = layout_widget(&widget, &states);

    let overlays = collect_open_select_overlays(&widget, &taffy, root, &states, (320.0, 240.0));

    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].id, select_id);
}

fn laid_out_open_select() -> (
    Widget<'static, TestMessage>,
    HashMap<u64, WidgetState>,
    TaffyTree<RutterContext>,
    NodeId,
) {
    let widget = open_select_widget();
    let states = open_select_states();
    let (taffy, root) = layout_widget(&widget, &states);
    (widget, states, taffy, root)
}

fn layout_widget<Message: Clone>(
    widget: &Widget<'_, Message>,
    states: &HashMap<u64, WidgetState>,
) -> (TaffyTree<RutterContext>, NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(320, 240),
        fonts,
        &RichTextRenderer::default(),
    );
    (taffy, root)
}

fn open_select_widget() -> Widget<'static, TestMessage> {
    Widget::Select {
        id: 55,
        options: &["Alpha", "Beta", "Gamma"],
        selected_index: 0,
        on_change: TestMessage::Select,
        style: Style {
            size: Size::from_lengths(120.0, 40.0),
            ..Style::default()
        },
        label: "",
        placeholder: "Choose",
    }
}

fn open_select_states() -> HashMap<u64, WidgetState> {
    HashMap::from([(
        55,
        WidgetState::Select(SelectState {
            is_open: true,
            hovered_option: None,
        }),
    )])
}

fn scrolled_select_widget() -> Widget<'static, TestMessage> {
    Widget::ScrollView {
        id: 77,
        child: Box::new(Widget::Column {
            children: vec![fixed_spacer(200.0), open_select_widget()],
            style: Style::default(),
        }),
        style: Style {
            size: Size::from_lengths(120.0, 100.0),
            ..Style::default()
        },
    }
}

fn scrolled_select_states() -> HashMap<u64, WidgetState> {
    let mut states = open_select_states();
    states.insert(
        77,
        WidgetState::Scroll(ScrollState {
            offset_y: 250.0,
            content_height: 240.0,
            viewport_h: 100.0,
        }),
    );
    states
}

fn modal_over_open_select() -> Widget<'static, TestMessage> {
    Widget::Column {
        children: vec![
            open_select_widget(),
            Widget::Modal {
                id: 88,
                visible: true,
                child: Box::new(fixed_spacer(80.0)),
                on_dismiss: None,
                style: Style::default(),
            },
        ],
        style: Style {
            size: Size::from_lengths(320.0, 240.0),
            ..Style::default()
        },
    }
}

fn modal_select_under_dialog() -> Widget<'static, TestMessage> {
    Widget::Column {
        children: vec![
            Widget::Modal {
                id: 89,
                visible: true,
                child: Box::new(open_select_widget()),
                on_dismiss: None,
                style: Style::default(),
            },
            visible_dialog(),
        ],
        style: Style {
            size: Size::from_lengths(320.0, 240.0),
            ..Style::default()
        },
    }
}

fn visible_dialog() -> Widget<'static, TestMessage> {
    Widget::Dialog {
        id: 90,
        title: "Blocking dialog",
        message: "Later overlays own pointer routing",
        confirm_label: "OK",
        cancel_label: "Cancel",
        visible: true,
        on_confirm: TestMessage::Action,
        on_cancel: TestMessage::Action,
        on_dismiss: None,
        position: DialogPosition::Center,
        style: Style::default(),
        child: Box::new(fixed_spacer(80.0)),
    }
}

fn root_overflowing_select() -> Widget<'static, TestMessage> {
    Widget::Column {
        children: vec![fixed_spacer(300.0), open_select_widget()],
        style: Style {
            size: Size::from_lengths(320.0, 400.0),
            ..Style::default()
        },
    }
}

fn popover_with_automatic_select() -> Widget<'static, TestMessage> {
    Widget::Popover {
        id: 70,
        open: true,
        anchor: Box::new(fixed_spacer(40.0)),
        content: Box::new(Widget::Select {
            id: AUTO_ID,
            options: &["Alpha", "Beta"],
            selected_index: 0,
            on_change: TestMessage::Select,
            style: Style {
                size: Size::from_lengths(120.0, 40.0),
                ..Style::default()
            },
            label: "",
            placeholder: "Choose",
        }),
        on_dismiss: None,
        style: Style {
            size: Size::from_lengths(120.0, 40.0),
            ..Style::default()
        },
        popup_style: Style {
            size: Size::from_lengths(120.0, 80.0),
            ..Style::default()
        },
    }
}

fn popover_select_id(widget: &Widget<'_, TestMessage>) -> u64 {
    let Widget::Popover { content, .. } = widget else {
        unreachable!()
    };
    content.resolved_id(&[1]).unwrap()
}

fn open_popover_state() -> PopoverState {
    PopoverState {
        is_open: true,
        anchor_x: 0.0,
        anchor_y: 0.0,
        anchor_w: 120.0,
        anchor_h: 40.0,
    }
}

fn fixed_spacer(height: f32) -> Widget<'static, TestMessage> {
    Widget::Spacer {
        style: Style {
            size: Size::from_lengths(120.0, height),
            ..Style::default()
        },
    }
}
