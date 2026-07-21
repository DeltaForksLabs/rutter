use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rutter::cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::WidgetState;
use rutter::input_state::InputWidgetState;
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::draw_widgets;
use rutter::render::text::TextBufferCache;
use rutter::skia_safe::{Color, Font, Point, Surface, surfaces};
use rutter::taffy::prelude::{Dimension, Size, Style};
use rutter::{InputState, Theme, Widget};
use winit::dpi::PhysicalSize;

const ITEM_INPUT_ID: u64 = 42;

fn fixed_style(width: f32, height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(height),
        },
        ..Style::default()
    }
}

fn virtual_text_area(_: usize) -> Option<Widget<'static, ()>> {
    Some(Widget::TextArea {
        id: ITEM_INPUT_ID,
        on_change: |_| (),
        on_submit: None,
        style: fixed_style(100.0, 48.0),
        label: "",
        state: InputState::Idle,
        placeholder: "",
        error_msg: None,
    })
}

fn virtual_list() -> Widget<'static, ()> {
    Widget::VirtualListContent {
        id: 100,
        item_height: 52.0,
        item_count: 1,
        items: &virtual_text_area,
        on_select: |_| (),
        style: fixed_style(120.0, 60.0),
    }
}

fn render_virtual_list(include_colliding_secret: bool) -> Vec<Color> {
    let widget = virtual_list();
    let widget_states = HashMap::<u64, WidgetState>::new();
    let layout_fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = rutter::taffy::TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &widget, layout_fonts.clone(), &widget_states);
    compute_layout(&mut taffy, root, PhysicalSize::new(120, 60), layout_fonts);

    let mut font_system = FontSystem::new();
    let mut input_states = HashMap::new();
    if include_colliding_secret {
        let mut state = InputWidgetState::new(&mut font_system);
        state.set_text(&mut font_system, "virtual-secret-value");
        input_states.insert(ITEM_INPUT_ID, state);
    }
    draw_virtual_test_surface(
        &widget,
        &taffy,
        root,
        &mut font_system,
        &input_states,
        &widget_states,
    )
}

fn draw_virtual_test_surface(
    widget: &Widget<'_, ()>,
    taffy: &rutter::taffy::TaffyTree<rutter::layout::RutterContext>,
    root: rutter::taffy::NodeId,
    font_system: &mut FontSystem,
    input_states: &HashMap<u64, InputWidgetState>,
    widget_states: &HashMap<u64, WidgetState>,
) -> Vec<Color> {
    let mut surface = surfaces::raster_n32_premul((120, 60)).unwrap();
    surface.canvas().clear(Color::TRANSPARENT);
    let mut swash = SwashCache::new();
    let mut font_cache = HashMap::<(String, u32), Font>::new();
    let mut text_cache = TextBufferCache::default();
    draw_widgets(
        surface.canvas(),
        taffy,
        root,
        widget,
        font_system,
        &mut swash,
        Point::new(-1.0, -1.0),
        None,
        input_states,
        widget_states,
        &mut font_cache,
        &mut text_cache,
        true,
        &Theme::default(),
        1.0,
    );
    surface_pixels(&mut surface)
}

fn surface_pixels(surface: &mut Surface) -> Vec<Color> {
    let pixels = surface.peek_pixels().unwrap();
    let mut colors = Vec::with_capacity(120 * 60);
    for y in 0..60 {
        for x in 0..120 {
            colors.push(pixels.get_color((x, y)));
        }
    }
    colors
}

#[test]
fn virtual_item_cannot_read_colliding_global_input_state() {
    let without_secret = render_virtual_list(false);
    let with_secret = render_virtual_list(true);

    assert_eq!(with_secret, without_secret);
}
