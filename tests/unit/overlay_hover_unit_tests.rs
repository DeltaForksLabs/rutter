use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use skia_safe::{Color, Font, Point, surfaces};
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

use super::*;
use crate::engine::widget_state::{ContextMenuState, PopoverState, VirtualListState, WidgetState};
use crate::i18n::LayoutDirection;
use crate::layout::{build_taffy_tree, compute_layout};
use crate::render::text::TextBufferCache;
use crate::render::{RichTextRenderer, draw_widgets};
use crate::theme::Theme;
use crate::widget::{ButtonVariant, ContextMenuEntry, Widget};

const CONTEXT_MENU_ID: u64 = 41;
const POPOVER_ID: u64 = 42;
const VIRTUAL_LIST_ID: u64 = 43;

#[test]
fn context_menu_surface_masks_underlying_button_hover() {
    let pointer = Point::new(25.0, 40.0);
    let idle = button_pixel(Point::new(-10.0, -10.0), false);
    let hovered = button_pixel(pointer, false);
    let masked = button_pixel(pointer, true);

    assert_ne!(hovered, idle);
    assert_eq!(masked, idle);
}

#[test]
fn context_menu_surface_masks_underlying_virtual_row_hover() {
    let pointer = Point::new(25.0, 40.0);
    let idle = virtual_list_pixel(Point::new(-10.0, -10.0), false);
    let hovered = virtual_list_pixel(pointer, false);
    let masked = virtual_list_pixel(pointer, true);

    assert_ne!(hovered, idle);
    assert_eq!(masked, idle);
}

#[test]
fn dropdown_surface_masks_lower_overlay_hover_routes() {
    let pointer = Point::new(40.0, 60.0);
    let routes = OverlayHoverRoutes::from_coverage(
        pointer,
        OverlayHoverCoverage {
            dropdown: true,
            ..OverlayHoverCoverage::default()
        },
    );

    assert_eq!(routes.dropdown, pointer);
    assert_hidden(routes.search);
    assert_hidden(routes.select);
    assert_hidden(routes.popover);
    assert_hidden(routes.base);
    assert_eq!(routes.context_menu, pointer);
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

    assert_hidden(routes.base);
    assert_eq!(routes.popover, pointer);
}

fn button_pixel(mouse: Point, menu_open: bool) -> Color {
    let entries = [ContextMenuEntry::item("Open", ())];
    let widget = context_menu_button(&entries);
    let states = HashMap::from([(
        CONTEXT_MENU_ID,
        WidgetState::ContextMenu(context_menu_state(menu_open)),
    )]);
    rendered_pixel(&widget, &states, mouse, (190, 80))
}

fn virtual_list_pixel(mouse: Point, menu_open: bool) -> Color {
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
            WidgetState::VList(VirtualListState::default()),
        ),
    ]);

    rendered_pixel(&widget, &states, mouse, (190, 40))
}

fn rendered_pixel(
    widget: &Widget<'_, ()>,
    states: &HashMap<u64, WidgetState>,
    mouse: Point,
    pixel: (i32, i32),
) -> Color {
    let (taffy, root) = layout_widget(widget, states, PhysicalSize::new(220, 100));
    let mut surface = surfaces::raster_n32_premul((220, 100)).unwrap();
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
        None,
        &HashMap::new(),
        states,
        &mut font_cache,
        &mut text_cache,
        true,
        &Theme::light(),
        1.0,
    );
    surface.peek_pixels().unwrap().get_color(pixel)
}

fn virtual_list_item(index: usize) -> Option<String> {
    Some(format!("Item {index}"))
}

fn select_virtual_row(_: usize) {}

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
    let mut state = ContextMenuState::default();
    if is_open {
        state.open_at(20.0, 20.0);
    }
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
