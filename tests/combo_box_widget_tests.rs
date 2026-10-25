//! Public API geometry, annotation raster and input hit-coordinate integration.
use cosmic_text::{FontSystem, SwashCache};
use rutter::engine::widget_state::{SearchState, WidgetState};
use rutter::input_limits::{InputKind, InputLimits};
use rutter::input_state::InputWidgetState;
use rutter::layout::{build_taffy_tree_with_direction, compute_layout};
use rutter::render::hit_test::{HitResult, hit_test};
use rutter::render::{RichTextRenderer, draw_widgets, text::TextBufferCache};
use rutter::{InputState, LayoutDirection, Theme, Widget, combo_box::ComboBox};
use skia_safe::{Color, Point, surfaces};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

#[test]
fn combo_box_annotations_body_and_indicator_are_contained_across_themes_scales_and_rtl() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    for width in [320, 800] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let widget = Widget::combo_box(
                62,
                "",
                |_| (),
                ComboBox::new(&["São Paulo", "東京", "القاهرة"], |_| ())
                    .with_label("Destination — long localized field label — 行き先 القاهرة")
                    .with_placeholder("行き先 القاهرة — type or browse")
                    .with_state(InputState::Error)
                    .with_error(
                        "Validation feedback — 東京 القاهرة — choose a canonical destination or keep this draft",
                    ),
                Style {
                    size: Size::from_lengths(width as f32 - 32.0, 44.0),
                    ..Style::default()
                },
            );
            let mut tree = TaffyTree::new();
            let root = build_taffy_tree_with_direction(
                &mut tree,
                &widget,
                fonts.clone(),
                &HashMap::new(),
                direction,
            );
            compute_layout(
                &mut tree,
                root,
                PhysicalSize::new(width, 120),
                fonts.clone(),
                &RichTextRenderer::default(),
            );
            let bounds = tree.layout(root).unwrap();
            let inputs = HashMap::from([(
                62,
                InputWidgetState::new_with_limits(
                    &mut fonts.borrow_mut(),
                    InputLimits::for_kind(InputKind::TextInput),
                ),
            )]);
            assert_eq!(bounds.size.height, 84.0);
            let left = bounds.location.x;
            let point = Point::new(left + 60.0, 30.0);
            assert!(matches!(
                hit_test(
                    &widget,
                    &tree,
                    root,
                    point,
                    Point::default(),
                    &HashMap::new()
                ),
                Some(HitResult::InputFocus {
                    id: 62,
                    local_x: 60.0,
                    local_y: 10.0,
                    height: 44.0,
                    ..
                })
            ));
            for y in [10.0, 74.0] {
                assert!(
                    hit_test(
                        &widget,
                        &tree,
                        root,
                        Point::new(left + 60.0, y),
                        Point::default(),
                        &HashMap::new()
                    )
                    .is_none()
                );
            }
            for theme in [Theme::light(), Theme::dark()] {
                for scale in [1.0, 1.5, 2.0] {
                    let mut closed_chevron = None;
                    for dismissed in [true, false] {
                        let states = HashMap::from([(
                            62,
                            WidgetState::Search(SearchState {
                                dismissed,
                                hovered_option: None,
                            }),
                        )]);
                        let mut surface = surfaces::raster_n32_premul((
                            (width as f32 * scale) as i32,
                            (120.0 * scale) as i32,
                        ))
                        .unwrap();
                        surface.canvas().clear(Color::MAGENTA);
                        surface.canvas().scale((scale, scale));
                        draw_widgets(
                            surface.canvas(),
                            &tree,
                            root,
                            &widget,
                            &mut fonts.borrow_mut(),
                            &mut SwashCache::new(),
                            Point::new(-10.0, -10.0),
                            Some(62),
                            &inputs,
                            &states,
                            &mut HashMap::new(),
                            &mut TextBufferCache::default(),
                            true,
                            &theme,
                            scale,
                        );
                        let pixels = surface.peek_pixels().unwrap();
                        for (top, bottom) in [(0, 20), (64, 84)] {
                            assert!(
                                ((top as f32 * scale) as i32..(bottom as f32 * scale) as i32).any(
                                    |y| ((left * scale) as i32
                                        ..((left + bounds.size.width) * scale) as i32)
                                        .any(|x| pixels.get_color((x, y)) != Color::MAGENTA)
                                ),
                                "annotation painted at {width}, {direction:?}, scale {scale}"
                            );
                        }
                        assert_eq!(
                            pixels.get_color((
                                ((if left > 0.0 { 8.0 } else { width as f32 - 8.0 }) * scale)
                                    as i32,
                                (40.0 * scale) as i32
                            )),
                            Color::MAGENTA
                        );
                        if dismissed {
                            assert_eq!(
                                pixels.get_color(((20.0 * scale) as i32, (90.0 * scale) as i32)),
                                Color::MAGENTA
                            );
                        }
                        // Physical right placement is stable even in RTL.
                        let x = left + bounds.size.width - 20.0;
                        assert!((((40.0 * scale) as i32)..((46.0 * scale) as i32)).any(|y| {
                            (((x - 5.0) * scale) as i32..((x + 5.0) * scale) as i32)
                                .any(|x| pixels.get_color((x, y)) != theme.surface)
                        }));
                        assert_ne!(
                            pixels.get_color((((x + 10.0) * scale) as i32, (42.0 * scale) as i32)),
                            theme.surface,
                            "square backing is distinct from the input surface"
                        );
                        let chevron: Vec<_> = ((37.0 * scale) as i32..(48.0 * scale) as i32)
                            .flat_map(|y| {
                                (((x - 6.0) * scale) as i32..((x + 6.0) * scale) as i32)
                                    .map(move |x| (x, y))
                            })
                            .map(|point| pixels.get_color(point))
                            .collect();
                        if dismissed {
                            closed_chevron = Some(chevron);
                        } else {
                            assert_ne!(
                                closed_chevron.as_ref().unwrap(),
                                &chevron,
                                "open arrow differs at {width}, {direction:?}, scale {scale}"
                            );
                        }
                    }
                }
            }
            let disabled = widget.enabled(false);
            assert!(
                hit_test(
                    &disabled,
                    &tree,
                    root,
                    point,
                    Point::default(),
                    &HashMap::new()
                )
                .is_none()
            );
        }
    }
}

#[test]
fn combo_box_configuration_rejects_zero_window_and_drops_stale_selection() {
    assert!(
        ComboBox::new(&["one"], |_| ())
            .with_visible_rows(0)
            .is_err()
    );
    let widget = Widget::combo_box(
        62,
        "unmatched",
        |_| (),
        ComboBox::new(&["one"], |_| ()).with_selected_index(Some(10)),
        Style::default(),
    );
    assert!(matches!(
        widget,
        Widget::ComboBox {
            selected_index: None,
            model_value: Some("unmatched"),
            ..
        }
    ));
}
