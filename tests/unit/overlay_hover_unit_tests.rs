use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use skia_safe::{Color, Font, Point, surfaces};
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

use super::*;
use crate::engine::widget_state::{
    ContextMenuState, PopoverState, SliderState, VirtualGridState, VirtualListState, WidgetState,
};
use crate::i18n::LayoutDirection;
use crate::input_limits::{InputKind, InputLimits};
use crate::input_state::{InputWidgetState, TextSelection};
use crate::layout::{build_taffy_tree, compute_layout};
use crate::render::text::TextBufferCache;
use crate::render::{RichTextRenderer, draw_widgets};
use crate::theme::Theme;
use crate::widget::{ButtonVariant, ContextMenuEntry, InputState, Widget};

const CONTEXT_MENU_ID: u64 = 41;
const POPOVER_ID: u64 = 42;
const VIRTUAL_LIST_ID: u64 = 43;
const VIRTUAL_GRID_ID: u64 = 44;
const SLIDER_ID: u64 = 45;
const TEXT_INPUT_ID: u64 = 46;

#[test]
fn context_menu_surface_masks_underlying_button_hover() {
    let pointer = Point::new(25.0, 40.0);
    let idle = button_pixel(Point::new(-10.0, -10.0), false, false);
    let hovered = button_pixel(pointer, false, false);
    let masked = button_pixel(pointer, true, false);

    assert_ne!(hovered, idle);
    assert_eq!(masked, idle);
}

#[test]
fn context_menu_surface_masks_underlying_button_focus() {
    let pointer = Point::new(25.0, 40.0);
    let unfocused = button_pixel(pointer, true, false);
    let focused = button_pixel(pointer, true, true);

    assert_eq!(focused, unfocused);
}

#[test]
fn context_menu_surface_masks_underlying_virtual_row_hover() {
    let pointer = Point::new(25.0, 40.0);
    let idle = virtual_list_pixel(Point::new(-10.0, -10.0), false, None);
    let hovered = virtual_list_pixel(pointer, false, None);
    let masked = virtual_list_pixel(pointer, true, None);

    assert_ne!(hovered, idle);
    assert_eq!(masked, idle);
}

#[test]
fn context_menu_surface_masks_underlying_virtual_row_state() {
    let pointer = Point::new(25.0, 40.0);
    let idle = virtual_list_pixel(pointer, true, None);
    let highlighted = virtual_list_pixel(pointer, true, Some(2));

    assert_eq!(highlighted, idle);
}

#[test]
fn context_menu_surface_masks_underlying_virtual_grid_state() {
    let pointer = Point::new(25.0, 40.0);
    let idle = virtual_grid_pixel(pointer, true, None);
    let highlighted = virtual_grid_pixel(pointer, true, Some(1));

    assert_eq!(highlighted, idle);
}

#[test]
fn context_menu_surface_masks_underlying_slider_drag_state() {
    let idle = slider_pixel(false, false, Point::new(-10.0, -10.0));
    let dragging = slider_pixel(true, false, Point::new(-10.0, -10.0));
    let masked = slider_pixel(true, true, Point::new(165.0, 40.0));

    assert_ne!(dragging, idle);
    assert_eq!(masked, idle);
}

#[test]
fn context_menu_surface_masks_underlying_text_selection() {
    let idle = text_selection_pixel(None, false, Point::new(-10.0, -10.0));
    let selected = text_selection_pixel(
        Some(TextSelection { start: 0, end: 10 }),
        false,
        Point::new(-10.0, -10.0),
    );
    let masked = text_selection_pixel(
        Some(TextSelection { start: 0, end: 10 }),
        true,
        Point::new(165.0, 40.0),
    );

    assert_ne!(selected, idle);
    assert_eq!(masked, idle);
}

#[test]
fn dropdown_surface_masks_lower_overlay_hover_routes() {
    let pointer = Point::new(40.0, 60.0);
    let routes = OverlayHoverRoutes::from_coverage(
        pointer,
        Some(71),
        OverlayHoverCoverage {
            dropdown: true,
            ..OverlayHoverCoverage::default()
        },
    );

    assert_eq!(routes.dropdown.mouse, pointer);
    assert_eq!(routes.dropdown.focused_id, Some(71));
    assert!(routes.dropdown.shows_interaction_effects);
    assert_hidden(routes.search.mouse);
    assert_hidden(routes.select.mouse);
    assert_hidden(routes.popover.mouse);
    assert_hidden(routes.base.mouse);
    assert_eq!(routes.base.focused_id, None);
    assert!(!routes.base.shows_interaction_effects);
    assert_eq!(routes.context_menu.mouse, pointer);
}

#[test]
fn popover_surface_masks_base_hover() {
    let widget = popover_widget();
    let states = HashMap::from([(POPOVER_ID, WidgetState::Popover(open_popover_state()))]);
    let (taffy, root) = layout_widget(&widget, &states, PhysicalSize::new(320, 240));
    let input_states = HashMap::new();
    let pointer = Point::new(30.0, 60.0);

    let routes = overlay_hover_routes(OverlayHoverInput {
        taffy: &taffy,
        root,
        widget: &widget,
        widget_states: &states,
        input_states: &input_states,
        focused_id: None,
        mouse: pointer,
        viewport: (320.0, 240.0),
        font_size: Theme::light().font_body,
        direction: LayoutDirection::Ltr,
    });

    assert_hidden(routes.base.mouse);
    assert_eq!(routes.popover.mouse, pointer);
}

fn button_pixel(mouse: Point, menu_open: bool, focused: bool) -> Color {
    let entries = [ContextMenuEntry::item("Open", ())];
    let widget = context_menu_button(&entries);
    let states = HashMap::from([(
        CONTEXT_MENU_ID,
        WidgetState::ContextMenu(context_menu_state(menu_open)),
    )]);
    let focused_id = focused.then(|| context_menu_child_focus_id(&widget));
    rendered_pixel(&widget, &states, &HashMap::new(), mouse, focused_id, (2, 2))
}

fn virtual_list_pixel(mouse: Point, menu_open: bool, hovered_row: Option<usize>) -> Color {
    let entries = [ContextMenuEntry::item("Open", ())];
    let widget = Widget::ContextMenu {
        id: CONTEXT_MENU_ID,
        child: Box::new(Widget::VirtualList {
            id: VIRTUAL_LIST_ID,
            item_height: 20.0,
            item_count: 5,
            items: &virtual_list_item,
            on_select: select_virtual_row,
            style: fixed_style(220.0, 100.0),
        }),
        entries: &entries,
        style: fixed_style(220.0, 100.0),
    };
    let states = HashMap::from([
        (
            CONTEXT_MENU_ID,
            WidgetState::ContextMenu(context_menu_state(menu_open)),
        ),
        (
            VIRTUAL_LIST_ID,
            WidgetState::VList(VirtualListState {
                hovered_row,
                ..VirtualListState::default()
            }),
        ),
    ]);

    rendered_pixel(&widget, &states, &HashMap::new(), mouse, None, (190, 40))
}

fn virtual_grid_pixel(mouse: Point, menu_open: bool, hovered_item: Option<usize>) -> Color {
    let entries = [ContextMenuEntry::item("Open", ())];
    let widget = Widget::ContextMenu {
        id: CONTEXT_MENU_ID,
        child: Box::new(Widget::VirtualGrid {
            id: VIRTUAL_GRID_ID,
            columns: 2,
            item_height: 80.0,
            item_count: 4,
            items: &virtual_list_item,
            on_select: select_virtual_row,
            style: fixed_style(220.0, 100.0),
        }),
        entries: &entries,
        style: fixed_style(220.0, 100.0),
    };
    let states = HashMap::from([
        (
            CONTEXT_MENU_ID,
            WidgetState::ContextMenu(context_menu_state(menu_open)),
        ),
        (
            VIRTUAL_GRID_ID,
            WidgetState::VGrid(VirtualGridState {
                hovered_item,
                ..VirtualGridState::default()
            }),
        ),
    ]);

    rendered_pixel(&widget, &states, &HashMap::new(), mouse, None, (190, 40))
}

fn slider_pixel(dragging: bool, menu_open: bool, mouse: Point) -> Color {
    let entries = [ContextMenuEntry::item("Open", ())];
    let widget = Widget::ContextMenu {
        id: CONTEXT_MENU_ID,
        child: Box::new(Widget::Slider {
            id: SLIDER_ID,
            value: 0.2,
            min: 0.0,
            max: 1.0,
            step: 0.1,
            on_change: change_slider,
            style: fixed_style(320.0, 100.0),
            label: "Volume",
        }),
        entries: &entries,
        style: fixed_style(320.0, 100.0),
    };
    let states = HashMap::from([
        (
            CONTEXT_MENU_ID,
            WidgetState::ContextMenu(context_menu_state_at(menu_open, Point::new(280.0, 20.0))),
        ),
        (
            SLIDER_ID,
            WidgetState::Slider(SliderState {
                dragging,
                ..SliderState::default()
            }),
        ),
    ]);

    rendered_pixel_in_viewport(
        &widget,
        &states,
        &HashMap::new(),
        mouse,
        None,
        PhysicalSize::new(320, 100),
        (74, 50),
    )
}

fn text_selection_pixel(selection: Option<TextSelection>, menu_open: bool, mouse: Point) -> Color {
    let entries = [ContextMenuEntry::item("Open", ())];
    let widget = Widget::ContextMenu {
        id: CONTEXT_MENU_ID,
        child: Box::new(Widget::TextInput {
            id: TEXT_INPUT_ID,
            on_change: change_text,
            on_submit: None,
            style: fixed_style(320.0, 100.0),
            label: "",
            placeholder: "",
            state: InputState::Idle,
            error_msg: None,
            is_password: false,
        }),
        entries: &entries,
        style: fixed_style(320.0, 100.0),
    };
    let states = HashMap::from([(
        CONTEXT_MENU_ID,
        WidgetState::ContextMenu(context_menu_state_at(menu_open, Point::new(280.0, 20.0))),
    )]);
    let mut input_state = selected_input_state();
    input_state.selection = selection;
    let input_states = HashMap::from([(TEXT_INPUT_ID, input_state)]);

    rendered_pixel_in_viewport(
        &widget,
        &states,
        &input_states,
        mouse,
        None,
        PhysicalSize::new(320, 100),
        (20, 41),
    )
}

fn rendered_pixel(
    widget: &Widget<'_, ()>,
    states: &HashMap<u64, WidgetState>,
    input_states: &HashMap<u64, InputWidgetState>,
    mouse: Point,
    focused_id: Option<u64>,
    pixel: (i32, i32),
) -> Color {
    rendered_pixel_in_viewport(
        widget,
        states,
        input_states,
        mouse,
        focused_id,
        PhysicalSize::new(220, 100),
        pixel,
    )
}

fn rendered_pixel_in_viewport(
    widget: &Widget<'_, ()>,
    states: &HashMap<u64, WidgetState>,
    input_states: &HashMap<u64, InputWidgetState>,
    mouse: Point,
    focused_id: Option<u64>,
    viewport: PhysicalSize<u32>,
    pixel: (i32, i32),
) -> Color {
    let (taffy, root) = layout_widget(widget, states, viewport);
    let mut surface =
        surfaces::raster_n32_premul((viewport.width as i32, viewport.height as i32)).unwrap();
    let mut fonts = FontSystem::new();
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::<(String, u32), Font>::new();
    let mut text_cache = TextBufferCache::default();

    draw_widgets(
        surface.canvas(),
        &taffy,
        root,
        widget,
        &mut fonts,
        &mut swash,
        mouse,
        focused_id,
        input_states,
        states,
        &mut font_cache,
        &mut text_cache,
        true,
        &Theme::light(),
        1.0,
    );
    surface.peek_pixels().unwrap().get_color(pixel)
}

fn context_menu_child_focus_id(widget: &Widget<'_, ()>) -> u64 {
    let Widget::ContextMenu { child, .. } = widget else {
        unreachable!("expected a ContextMenu widget for the focus regression test");
    };
    child
        .keyboard_focus_id(&[0])
        .expect("expected a focusable ContextMenu child at path [0]")
}

fn virtual_list_item(index: usize) -> Option<String> {
    Some(format!("Item {index}"))
}

fn select_virtual_row(_: usize) {}

fn change_slider(_: f32) {}

fn change_text(_: String) {}

fn context_menu_button<'a>(entries: &'a [ContextMenuEntry<'a, ()>]) -> Widget<'a, ()> {
    Widget::ContextMenu {
        id: CONTEXT_MENU_ID,
        child: Box::new(Widget::Button {
            text: "Background",
            on_press: (),
            style: fixed_style(220.0, 100.0),
            color: None,
            variant: ButtonVariant::Primary,
        }),
        entries,
        style: fixed_style(220.0, 100.0),
    }
}

fn context_menu_state(is_open: bool) -> ContextMenuState {
    context_menu_state_at(is_open, Point::new(20.0, 20.0))
}

fn context_menu_state_at(is_open: bool, anchor: Point) -> ContextMenuState {
    let mut state = ContextMenuState::default();
    if is_open {
        state.open_at(anchor.x, anchor.y);
    }
    state
}

fn selected_input_state() -> InputWidgetState {
    let mut fonts = FontSystem::new();
    let mut state =
        InputWidgetState::new_with_limits(&mut fonts, InputLimits::for_kind(InputKind::TextInput));
    state.set_text(&mut fonts, "Background");
    state
}

fn popover_widget() -> Widget<'static, ()> {
    Widget::Popover {
        id: POPOVER_ID,
        open: true,
        anchor: Box::new(Widget::Spacer {
            style: fixed_style(100.0, 32.0),
        }),
        content: Box::new(Widget::Spacer {
            style: fixed_style(120.0, 40.0),
        }),
        on_dismiss: None,
        style: fixed_style(100.0, 32.0),
        popup_style: fixed_style(120.0, 40.0),
    }
}

fn open_popover_state() -> PopoverState {
    let mut state = PopoverState::default();
    state.set_open(true);
    state.set_anchor_rect(20.0, 20.0, 100.0, 32.0);
    state
}

fn layout_widget<Message>(
    widget: &Widget<'_, Message>,
    states: &HashMap<u64, WidgetState>,
    size: PhysicalSize<u32>,
) -> (TaffyTree<crate::layout::RutterContext>, taffy::NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), states);
    compute_layout(&mut taffy, root, size, fonts, &RichTextRenderer::default());
    (taffy, root)
}

fn fixed_style(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(height),
        },
        ..Style::default()
    }
}

fn assert_hidden(point: Point) {
    assert_eq!(point.x, HIDDEN_HOVER_COORDINATE);
    assert_eq!(point.y, HIDDEN_HOVER_COORDINATE);
}
