use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::{PopoverState, WidgetState};
use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
use rutter::render::hit_test::{HitResult, PopoverOverlayHit, hit_test, hit_test_popover_overlay};
use rutter::render::text::TextBufferCache;
use rutter::render::{RichTextRenderer, draw_widgets};
use rutter::{
    ColorFormat, ColorPickerCallbacks, ColorPickerEvent, ColorPickerIds, ColorPickerState,
    PickerColor, Theme, Widget, WidgetId,
};
use skia_safe::{Color, Point, Rect, surfaces};
use taffy::prelude::{Dimension, NodeId, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

type Msg = ColorPickerEvent;

// Plane, hue, alpha, preview: the strips are nested inside the preview row.
const CUSTOM_PATHS: [&[usize]; 4] = [&[0], &[1, 1, 0], &[1, 1, 1], &[1, 0]];

#[test]
fn every_format_preserves_custom_sizes_and_wraps_fields_without_overflow() {
    for width in [360, 240] {
        for format in ColorFormat::ALL {
            let state = picker_state(format);
            let widget = picker(&state, width);
            let (taffy, root) = layout(&widget, width, &HashMap::new());
            assert_eq!(bounds_at(&taffy, root, &[]).width(), width as f32);
            for (path, height) in CUSTOM_PATHS.into_iter().zip([190.0, 24.0, 24.0, 58.0]) {
                let bounds = bounds_at(&taffy, root, path);
                assert_eq!(bounds.height(), height, "{format:?} at {width}: {path:?}");
                assert!(bounds.left >= 0.0 && bounds.right <= width as f32);
            }
            let fields: Vec<_> = editor_paths(&state)
                .iter()
                .map(|(path, _)| bounds_at(&taffy, root, path))
                .collect();
            for bounds in &fields {
                assert!(bounds.left >= 0.0 && bounds.right <= width as f32);
                assert!(bounds.width() >= 60.0 && bounds.height() >= 68.0);
            }
            if format != ColorFormat::Hex {
                let first = fields.first().unwrap();
                let last = fields.last().unwrap();
                // Five 66px CMYK fields plus four 8px gaps also wrap at 360px.
                if width == 240 || format == ColorFormat::Cmyk {
                    assert!(last.top >= first.bottom, "{format:?} must wrap at {width}");
                } else {
                    assert_eq!(first.top, last.top, "{format:?} fits one row at {width}");
                }
            }
        }
    }
}

#[test]
fn public_hits_resolve_stable_ids_and_disabling_the_picker_blocks_them() {
    for width in [360, 240] {
        for format in ColorFormat::ALL {
            let state = picker_state(format);
            let widget = picker(&state, width);
            let states = HashMap::new();
            let (taffy, root) = layout(&widget, width, &states);
            let ids = ColorPickerIds::new(100).unwrap();
            let mut points = Vec::new();
            for (path, id) in CUSTOM_PATHS[..3]
                .iter()
                .zip([ids.plane(), ids.hue(), ids.alpha()])
            {
                let bounds = bounds_at(&taffy, root, path);
                let point = Point::new(bounds.center_x(), bounds.center_y());
                assert!(matches!(
                    hit_test(&widget, &taffy, root, point, Point::new(0.0, 0.0), &states),
                    Some(HitResult::CustomPointer {
                        id: actual, position, bounds: size, focuses_keyboard: true,
                    }) if actual == id.get()
                        && position == Point::new(bounds.width() / 2.0, bounds.height() / 2.0)
                        && size == (bounds.width(), bounds.height())
                ));
                points.push(point);
            }
            for (path, id) in editor_paths(&state) {
                let bounds = bounds_at(&taffy, root, &path);
                let point = Point::new(bounds.center_x(), bounds.center_y());
                assert!(matches!(
                    hit_test(&widget, &taffy, root, point, Point::new(0.0, 0.0), &states),
                    Some(HitResult::InputFocus { id: actual, .. }) if actual == id
                ));
                points.push(point);
            }
            let select = bounds_at(&taffy, root, &[2]);
            let point = Point::new(select.center_x(), select.center_y());
            assert!(matches!(
                hit_test(&widget, &taffy, root, point, Point::new(0.0, 0.0), &states),
                Some(HitResult::SelectToggle(id)) if id == ids.format().get()
            ));
            points.push(point);
            let preview = bounds_at(&taffy, root, CUSTOM_PATHS[3]);
            let point = Point::new(preview.center_x(), preview.center_y());
            assert!(
                hit_test(&widget, &taffy, root, point, Point::new(0.0, 0.0), &states).is_none()
            );
            points.push(point);
            let disabled = widget.enabled(false);
            for point in points {
                assert!(
                    hit_test(
                        &disabled,
                        &taffy,
                        root,
                        point,
                        Point::new(0.0, 0.0),
                        &states
                    )
                    .is_none()
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CopyMessage {
    Edit,
    Copy,
}

fn picker_with_copy(state: &ColorPickerState, width: u32) -> Widget<'_, CopyMessage> {
    let callbacks = ColorPickerCallbacks::new(
        |_| CopyMessage::Edit,
        |_| CopyMessage::Edit,
        |_| CopyMessage::Edit,
        |_| CopyMessage::Edit,
        |_| CopyMessage::Edit,
        [|_| CopyMessage::Edit; 5],
    )
    .with_copy_button(CopyMessage::Copy);
    Widget::color_picker(
        state,
        ColorPickerIds::new(100).unwrap(),
        callbacks,
        width_style(width),
    )
}

#[test]
fn copy_button_stays_right_of_editors_hits_its_message_and_is_disabled_with_picker() {
    for width in [360, 240] {
        for format in ColorFormat::ALL {
            let state = picker_state(format);
            let widget = picker_with_copy(&state, width);
            let states = HashMap::new();
            let (taffy, root) = layout(&widget, width, &states);
            let copy = bounds_at(&taffy, root, &[3, 1]);
            assert_eq!(copy.width(), 44.0);
            assert_eq!(copy.height(), 44.0);
            assert_eq!(copy.right, width as f32);
            for (path, _) in editor_paths(&state) {
                let path = [vec![3, 0], path[1..].to_vec()].concat();
                let field = bounds_at(&taffy, root, &path);
                assert!(field.width() >= 60.0 && field.right <= copy.left - 8.0);
                assert!(field.height() >= 68.0);
            }
            let point = Point::new(copy.center_x(), copy.center_y());
            assert!(matches!(
                hit_test(&widget, &taffy, root, point, Point::default(), &states),
                Some(HitResult::Message {
                    msg: CopyMessage::Copy,
                    focus_id: Some(_),
                    ..
                })
            ));
            let disabled = widget.enabled(false);
            assert!(hit_test(&disabled, &taffy, root, point, Point::default(), &states).is_none());
        }
    }
}

#[test]
fn popover_routes_the_nested_svg_copy_button_to_the_application() {
    let state = picker_state(ColorFormat::Hex);
    let anchor = Widget::color_swatch(
        state.color(),
        WidgetId::manual(20).unwrap(),
        Style {
            size: Size::from_lengths(80.0, 32.0),
            ..Style::default()
        },
    );
    let widget = Widget::popover(
        true,
        anchor,
        picker_with_copy(&state, 360),
        None,
        Style {
            size: Size::from_lengths(80.0, 32.0),
            ..Style::default()
        },
        width_style(360),
    )
    .with_id(21);
    let mut popover = PopoverState::default();
    popover.set_open(true);
    popover.set_anchor_rect(0.0, 0.0, 80.0, 32.0);
    let states = HashMap::from([(21, WidgetState::Popover(popover))]);
    let (taffy, root) = layout(&widget, 800, &states);
    let copy = bounds_at(&taffy, root, &[1, 0, 3, 1]);
    let mouse = Point::new(8.0 + copy.center_x(), 40.0 + copy.center_y());
    assert!(matches!(
        hit_test_popover_overlay(&widget, &taffy, root, mouse, (800.0, 700.0), &states),
        Some(PopoverOverlayHit::Content(HitResult::Message {
            msg: CopyMessage::Copy,
            ..
        }))
    ));
}

#[test]
fn open_popover_routes_to_nested_picker_custom_controls() {
    for width in [360, 240] {
        let state = picker_state(ColorFormat::Cmyk);
        let anchor = Widget::color_swatch(
            state.color(),
            WidgetId::manual(20).unwrap(),
            Style {
                size: Size::from_lengths(80.0, 32.0),
                ..Style::default()
            },
        );
        let widget = Widget::popover(
            true,
            anchor,
            picker(&state, width),
            None,
            Style {
                size: Size::from_lengths(80.0, 32.0),
                ..Style::default()
            },
            width_style(width),
        )
        .with_id(21);
        let mut popover = PopoverState::default();
        popover.set_open(true);
        popover.set_anchor_rect(0.0, 0.0, 80.0, 32.0);
        let states = HashMap::from([(21, WidgetState::Popover(popover))]);
        let (taffy, root) = layout(&widget, 800, &states);
        for (index, path) in CUSTOM_PATHS[..3].iter().enumerate() {
            let content_path = [vec![1, 0], path.to_vec()].concat();
            let bounds = bounds_at(&taffy, root, &content_path);
            // Overlay placement adds the 8px viewport margin and anchor gap.
            let mouse = Point::new(8.0 + bounds.center_x(), 40.0 + bounds.center_y());
            assert!(matches!(
                hit_test_popover_overlay(&widget, &taffy, root, mouse, (800.0, 700.0), &states),
                Some(PopoverOverlayHit::Content(HitResult::CustomPointer {
                    id, position, bounds: size, focuses_keyboard: true,
                })) if id == 100 + index as u64
                    && position == Point::new(bounds.width() / 2.0, bounds.height() / 2.0)
                    && size == (bounds.width(), bounds.height())
            ));
        }
    }
}

#[test]
fn software_rendering_paints_gradients_and_uniform_partial_alpha_preview_in_both_themes() {
    let state = picker_state(ColorFormat::Rgb);
    for width in [360, 240] {
        let widget = picker(&state, width);
        let states = HashMap::new();
        let (taffy, root) = layout(&widget, width, &states);
        for theme in [Theme::dark(), Theme::light()] {
            let mut surface = surfaces::raster_n32_premul((width as i32, 700)).unwrap();
            surface.canvas().clear(Color::TRANSPARENT);
            draw_widgets(
                surface.canvas(),
                &taffy,
                root,
                &widget,
                &mut FontSystem::new(),
                &mut SwashCache::new(),
                Point::new(-1.0, -1.0),
                None,
                &HashMap::new(),
                &states,
                &mut HashMap::new(),
                &mut TextBufferCache::default(),
                true,
                &theme,
                1.0,
            );
            let pixels = surface.peek_pixels().unwrap();
            for path in CUSTOM_PATHS {
                let bounds = bounds_at(&taffy, root, path);
                let sample = |x: f32, y: f32| {
                    pixels.get_color(((bounds.left + x) as i32, (bounds.top + y) as i32))
                };
                let (left, right) = if path == CUSTOM_PATHS[3] {
                    // Partial-alpha previews composite over a uniform theme surface.
                    (sample(4.0, 4.0), sample(12.0, 4.0))
                } else {
                    (
                        sample(bounds.width() * 0.2, 7.0),
                        sample(bounds.width() * 0.8, 7.0),
                    )
                };
                assert_eq!(left.a(), 255, "unpainted custom leaf at {width}: {path:?}");
                assert_eq!(right.a(), 255);
                if path == CUSTOM_PATHS[3] {
                    assert_eq!(left, right, "nonuniform preview at {width}");
                } else {
                    assert_ne!(left, right, "missing gradient at {width}: {path:?}");
                }
                if path == CUSTOM_PATHS[0] {
                    let bottom = sample(bounds.width() * 0.2, bounds.height() - 7.0);
                    assert!(
                        bottom.r() < left.r() && bottom.g() < left.g() && bottom.b() < left.b()
                    );
                }
            }
        }
    }
}

fn picker_state(format: ColorFormat) -> ColorPickerState {
    let mut state = ColorPickerState::new(PickerColor::from_rgba8(103, 80, 164, 128));
    state
        .update(ColorPickerEvent::Format(format.index()))
        .unwrap();
    state
}

fn picker(state: &ColorPickerState, width: u32) -> Widget<'_, Msg> {
    let callbacks = ColorPickerCallbacks::new(
        |(saturation, value)| Msg::SaturationValue { saturation, value },
        Msg::Hue,
        Msg::Alpha,
        Msg::Format,
        Msg::Hex,
        [
            |text| Msg::Channel { index: 0, text },
            |text| Msg::Channel { index: 1, text },
            |text| Msg::Channel { index: 2, text },
            |text| Msg::Channel { index: 3, text },
            |text| Msg::Channel { index: 4, text },
        ],
    );
    Widget::color_picker(
        state,
        ColorPickerIds::new(100).unwrap(),
        callbacks,
        width_style(width),
    )
}

fn width_style(width: u32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width as f32),
            height: Dimension::auto(),
        },
        ..Style::default()
    }
}

fn layout<Message>(
    widget: &Widget<'_, Message>,
    width: u32,
    states: &HashMap<u64, WidgetState>,
) -> (TaffyTree<RutterContext>, NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(width, 700),
        fonts,
        &RichTextRenderer::default(),
    );
    (taffy, root)
}

fn bounds_at(taffy: &TaffyTree<RutterContext>, root: NodeId, path: &[usize]) -> Rect {
    let mut node = root;
    let mut origin = Point::new(0.0, 0.0);
    for index in path {
        let layout = taffy.layout(node).unwrap();
        origin.x += layout.location.x;
        origin.y += layout.location.y;
        node = taffy.children(node).unwrap()[*index];
    }
    let layout = taffy.layout(node).unwrap();
    let (width, height) = (layout.size.width, layout.size.height);
    assert!(
        width.is_finite() && width > 0.0,
        "invalid width at {path:?}: {width}"
    );
    assert!(
        height.is_finite() && height > 0.0,
        "invalid height at {path:?}: {height}"
    );
    Rect::from_xywh(
        origin.x + layout.location.x,
        origin.y + layout.location.y,
        width,
        height,
    )
}

fn editor_paths(state: &ColorPickerState) -> Vec<(Vec<usize>, u64)> {
    let ids = ColorPickerIds::new(100).unwrap();
    if state.format() == ColorFormat::Hex {
        return vec![(vec![3], ids.hex().get())];
    }
    (0..state.channel_count())
        .map(|index| (vec![3, index], ids.channel(index).unwrap().get()))
        .collect()
}
