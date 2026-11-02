//! Pointer dispatch must use current keyed item layouts, not press-time tracks.
use super::*;
use crate::engine::widget_state::{virtual_grid_cell_left, virtual_grid_cell_width};
use crate::layout::{SCROLLBAR_W, VIRTUAL_GRID_GAP};
use crate::render::hit_test::interactive_carousel_card_rect;
use crate::widgets::carousel::geometry::carousel_item_frames;
use crate::{CarouselConfig, KeyedVirtualItems, VirtualItemKey};
use skia_safe::Rect;
use taffy::{Dimension, LengthPercentage};

const COLLECTION_ID: u64 = 700;
const ITEM_HEIGHT: f32 = 80.0;
const ITEM_COUNT: usize = 8;

#[derive(Clone, Copy, Debug)]
enum Collection {
    List,
    Grid,
    Carousel,
}

#[derive(Clone, Debug, PartialEq)]
enum SliderMessage {
    Changed {
        index: usize,
        epoch: u64,
        value: f32,
    },
    Selected,
}

struct VirtualSliderModel {
    collection: Collection,
    keys: Vec<VirtualItemKey>,
    legacy: bool,
    epoch: u64,
    width: f32,
    messages: Vec<SliderMessage>,
}

struct VirtualSliderApp;

fn legacy_change(value: f32) -> SliderMessage {
    SliderMessage::Changed {
        index: usize::MAX,
        epoch: 0,
        value,
    }
}

fn fluid_style(height: Dimension) -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height,
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn build_slider_item<'a, const EPOCH: u64, const LEGACY: bool>(
    index: usize,
) -> Option<Widget<'a, SliderMessage>> {
    let slider = if LEGACY {
        Widget::slider(
            0.0,
            0.0,
            100.0,
            1.0,
            legacy_change,
            fluid_style(Dimension::length(44.0)),
            "Amount",
        )
    } else {
        Widget::slider_with(
            0.0,
            0.0,
            100.0,
            1.0,
            move |value| SliderMessage::Changed {
                index,
                epoch: EPOCH,
                value,
            },
            fluid_style(Dimension::length(44.0)),
            "Amount",
        )
    };
    Some(Widget::Column {
        children: vec![slider],
        style: Style {
            padding: taffy::Rect {
                left: LengthPercentage::length(8.0),
                right: LengthPercentage::length(8.0),
                top: LengthPercentage::length(8.0),
                bottom: LengthPercentage::length(8.0),
            },
            ..fluid_style(Dimension::percent(1.0))
        },
    })
}

impl AppLogic for VirtualSliderApp {
    type State = VirtualSliderModel;
    type Message = SliderMessage;

    fn new(_: &mut FontSystem) -> Self::State {
        VirtualSliderModel {
            collection: Collection::List,
            keys: (1..=ITEM_COUNT as u64)
                .map(|key| VirtualItemKey::new(key).unwrap())
                .collect(),
            legacy: false,
            epoch: 0,
            width: 280.0,
            messages: Vec::new(),
        }
    }

    fn view(model: &mut Self::State) -> Widget<'_, SliderMessage> {
        let items = match (model.legacy, model.epoch) {
            (true, _) => KeyedVirtualItems::try_new(&model.keys, &build_slider_item::<0, true>),
            (false, 0) => KeyedVirtualItems::try_new(&model.keys, &build_slider_item::<0, false>),
            (false, 7) => KeyedVirtualItems::try_new(&model.keys, &build_slider_item::<7, false>),
            _ => unreachable!("test publishes only epochs 0 and 7"),
        }
        .unwrap();
        let style = control_style(model.width, 120.0);
        let collection = match model.collection {
            Collection::List => Widget::interactive_virtual_list_content(
                ITEM_HEIGHT,
                items,
                |_| SliderMessage::Selected,
                style,
            ),
            Collection::Grid => Widget::interactive_virtual_grid_content(
                2,
                ITEM_HEIGHT,
                items,
                |_| SliderMessage::Selected,
                style,
            ),
            Collection::Carousel => Widget::interactive_carousel_view(
                items,
                |_| SliderMessage::Selected,
                CarouselConfig::uncontained(200.0).unwrap(),
                style,
            ),
        }
        .with_id(COLLECTION_ID);
        Widget::Column {
            children: vec![collection],
            style: Style {
                padding: taffy::Rect {
                    left: LengthPercentage::length(12.0),
                    right: LengthPercentage::length(0.0),
                    top: LengthPercentage::length(24.0),
                    bottom: LengthPercentage::length(0.0),
                },
                ..Style::default()
            },
        }
    }

    fn update(model: &mut Self::State, message: SliderMessage, _: &mut arboard::Clipboard) {
        model.messages.push(message);
    }
}

fn virtual_runner(collection: Collection, legacy: bool) -> RutterRunner<VirtualSliderApp> {
    let mut engine = RutterEngine::<VirtualSliderApp>::new().expect("owned Xvfb clipboard");
    engine.app_state.collection = collection;
    engine.app_state.legacy = legacy;
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
    set_collection_scroll(&mut runner, 32.0);
    runner
}

fn set_collection_scroll(runner: &mut RutterRunner<VirtualSliderApp>, pixels: f32) {
    let state = runner.engine.widget_states.get_mut(&COLLECTION_ID).unwrap();
    match runner.engine.app_state.collection {
        Collection::List => state.as_vlist_mut().unwrap().scroll_y = pixels,
        Collection::Grid => state.as_vgrid_mut().unwrap().scroll_y = pixels,
        Collection::Carousel => {
            let config = CarouselConfig::uncontained(200.0).unwrap();
            let state = state.as_carousel_mut().unwrap();
            // Set an absolute fractional position through the native state API.
            state.scroll_by_pixels(-10_000.0, &config, ITEM_COUNT);
            state.scroll_by_pixels(pixels, &config, ITEM_COUNT);
        }
    }
}

// Derive the painted item allocation independently of the slider geometry map.
fn slider_frame(runner: &RutterRunner<VirtualSliderApp>, index: usize) -> Rect {
    let width = runner.engine.app_state.width;
    let state = &runner.engine.widget_states[&COLLECTION_ID];
    let item = match runner.engine.app_state.collection {
        Collection::List => Rect::from_xywh(
            0.0,
            index as f32 * ITEM_HEIGHT - state.as_vlist().unwrap().scroll_y,
            width - SCROLLBAR_W - 4.0,
            ITEM_HEIGHT,
        ),
        Collection::Grid => Rect::from_xywh(
            virtual_grid_cell_left(index % 2, width, 2),
            (index / 2) as f32 * ITEM_HEIGHT - state.as_vgrid().unwrap().scroll_y
                + VIRTUAL_GRID_GAP * 0.5,
            virtual_grid_cell_width(width, 2),
            ITEM_HEIGHT - VIRTUAL_GRID_GAP,
        ),
        Collection::Carousel => {
            let frames = carousel_item_frames(
                &CarouselConfig::uncontained(200.0).unwrap(),
                state.as_carousel().unwrap().position,
                width,
                ITEM_COUNT,
                LayoutDirection::Ltr,
            );
            interactive_carousel_card_rect(
                *frames.iter().find(|frame| frame.index == index).unwrap(),
                120.0,
            )
        }
    };
    Rect::from_xywh(
        12.0 + item.left + 8.0,
        24.0 + item.top + 8.0,
        item.width() - 16.0,
        44.0,
    )
}

fn pointer_event(
    runner: &mut RutterRunner<VirtualSliderApp>,
    frame: Rect,
    fraction: f32,
    press: bool,
) {
    let position = PhysicalPosition::new(
        (frame.left + 16.0 + fraction * (frame.width() - 32.0)) as f64,
        frame.center_y() as f64,
    );
    let event = if press {
        WindowEvent::PointerButton {
            device_id: None,
            state: ElementState::Pressed,
            primary: true,
            position,
            button: ButtonSource::Mouse(MouseButton::Left),
            is_macos_activation_click: false,
        }
    } else {
        WindowEvent::PointerMoved {
            device_id: None,
            position,
            primary: true,
            source: PointerSource::Mouse,
        }
    };
    runner.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
}

fn assert_live_frame(runner: &RutterRunner<VirtualSliderApp>, id: u64, expected: Rect) {
    let actual = runner.engine.runtime_caches.viewport_geometry.sliders[&id];
    for (actual, expected) in [
        (actual.left, expected.left),
        (actual.top, expected.top),
        (actual.right, expected.right),
        (actual.bottom, expected.bottom),
    ] {
        assert!(
            (actual - expected).abs() <= 1.0,
            "edge {actual}, expected {expected}"
        );
    }
}

#[test]
fn native_virtual_slider_drag_uses_live_keyed_layout_and_collection_offsets() {
    for collection in [Collection::List, Collection::Grid, Collection::Carousel] {
        for legacy in [false, true] {
            let mut runner = virtual_runner(collection, legacy);
            let index = if matches!(collection, Collection::Grid) {
                3
            } else {
                1
            };
            let frame = slider_frame(&runner, index);
            pointer_event(&mut runner, frame, 0.25, true);
            let id = runner
                .engine
                .drag_slider_id
                .expect("native slider press captures");
            let expected = |epoch, value| SliderMessage::Changed {
                index: if legacy { usize::MAX } else { index },
                epoch: if legacy { 0 } else { epoch },
                value,
            };
            assert_eq!(runner.engine.app_state.messages, vec![expected(0, 25.0)]);

            runner.engine.app_state.width = 240.0;
            runner.engine.app_state.epoch = 7;
            runner.engine.layout_dirty = true;
            set_collection_scroll(&mut runner, 48.0);
            let frame = slider_frame(&runner, index);
            pointer_event(&mut runner, frame, 0.5, false);
            assert_eq!(
                runner.engine.drag_slider_id,
                Some(id),
                "{collection:?}, legacy={legacy}"
            );
            assert_eq!(
                runner.engine.app_state.messages.last(),
                Some(&expected(7, 50.0))
            );
            assert_live_frame(&runner, id, frame);

            // Reconcile scroll-only movement without a retained layout rebuild.
            runner.refresh_layout_before_keyboard_input().unwrap();
            set_collection_scroll(&mut runner, 64.0);
            let frame = slider_frame(&runner, index);
            pointer_event(&mut runner, frame, 0.75, false);
            assert_eq!(runner.engine.drag_slider_id, Some(id));
            assert_live_frame(&runner, id, frame);
            assert_eq!(
                runner.engine.app_state.messages,
                vec![expected(0, 25.0), expected(7, 50.0), expected(7, 75.0)]
            );

            // An item scrolled out of the live collection cannot retain its track.
            runner.refresh_layout_before_keyboard_input().unwrap();
            set_collection_scroll(&mut runner, 10_000.0);
            pointer_event(&mut runner, frame, 0.9, false);
            assert_eq!(runner.engine.drag_slider_id, None);
            assert_eq!(runner.engine.app_state.messages.len(), 3);
            assert!(
                !runner
                    .engine
                    .runtime_caches
                    .viewport_geometry
                    .sliders
                    .contains_key(&id)
            );
        }
    }
}

#[test]
fn native_virtual_slider_drag_cancels_when_slider_is_clipped_before_item() {
    for collection in [Collection::List, Collection::Grid] {
        for legacy in [false, true] {
            let mut runner = virtual_runner(collection, legacy);
            let index = if matches!(collection, Collection::Grid) {
                3
            } else {
                1
            };
            let frame = slider_frame(&runner, index);
            pointer_event(&mut runner, frame, 0.25, true);
            let id = runner.engine.drag_slider_id.expect("slider press captures");
            assert_eq!(runner.engine.app_state.messages.len(), 1);

            // Partial clipping keeps capture and the original, unshortened track.
            runner.refresh_layout_before_keyboard_input().unwrap();
            set_collection_scroll(&mut runner, ITEM_HEIGHT + 32.0);
            let frame = slider_frame(&runner, index);
            assert!(frame.top < 24.0 && frame.bottom > 24.0);
            pointer_event(&mut runner, frame, 0.5, false);
            assert_eq!(runner.engine.drag_slider_id, Some(id));
            assert_live_frame(&runner, id, frame);
            assert_eq!(
                runner.engine.app_state.messages,
                [25.0, 50.0].map(|value| SliderMessage::Changed {
                    index: if legacy { usize::MAX } else { index },
                    epoch: 0,
                    value,
                })
            );

            // The slider is now above the viewport, but its item's bottom remains visible.
            runner.refresh_layout_before_keyboard_input().unwrap();
            let scroll = ITEM_HEIGHT + 60.0;
            set_collection_scroll(&mut runner, scroll);
            let frame = slider_frame(&runner, index);
            assert!(frame.bottom < 24.0);
            let item_bottom = 24.0 + 2.0 * ITEM_HEIGHT
                - scroll
                - if matches!(collection, Collection::Grid) {
                    VIRTUAL_GRID_GAP * 0.5
                } else {
                    0.0
                };
            assert!(item_bottom > 24.0, "item must still intersect the viewport");
            pointer_event(&mut runner, frame, 0.9, false);
            assert_eq!(runner.engine.drag_slider_id, None);
            assert_eq!(runner.engine.app_state.messages.len(), 2);
            assert!(
                !runner
                    .engine
                    .runtime_caches
                    .viewport_geometry
                    .sliders
                    .contains_key(&id)
            );
        }
    }
}
