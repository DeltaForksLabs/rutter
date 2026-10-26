use super::*;
use crate::engine::widget_state::SelectState;
use crate::layout::{build_taffy_tree, field::ANNOTATION_HEIGHT};
use crate::render::hit_test::{HitResult, find_input_geometry, hit_test};
use crate::widgets::table_of_contents::HeadingLevel;
use skia_safe::{Surface, surfaces};
use taffy::prelude::{Dimension, LengthPercentage, Rect, Size, Style};

fn layout_widget(widget: &Widget<'_, ()>, width: u32) -> (TaffyTree<RutterContext>, NodeId) {
    let mut taffy = TaffyTree::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(width, 400),
        fonts,
        &RichTextRenderer::default(),
    );
    (taffy, root)
}

fn raster_widget(
    widget: &Widget<'_, ()>,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    theme: &Theme,
    input_text: Option<&str>,
) -> Surface {
    let mut surface = surfaces::raster_n32_premul((320, 400)).unwrap();
    let mut fonts = FontSystem::new();
    let mut inputs = HashMap::new();
    if let Some(text) = input_text {
        let mut state = InputWidgetState::new(&mut fonts);
        state.try_set_text(&mut fonts, text).unwrap();
        inputs.insert(17, state);
    }
    let canvas = surface.canvas();
    canvas.clear(SkiaColor::TRANSPARENT);
    canvas.translate((12.0, 12.0));
    draw_widgets(
        canvas,
        taffy,
        root,
        widget,
        &mut fonts,
        &mut SwashCache::new(),
        Point::new(-100.0, -100.0),
        None,
        &inputs,
        &HashMap::new(),
        &mut HashMap::new(),
        &mut TextBufferCache::default(),
        false,
        theme,
        1.0,
    );
    surface
}

fn ink_in(surface: &mut Surface, bounds: SkiaRect) -> bool {
    let pixels = surface.peek_pixels().unwrap();
    (bounds.top as i32..bounds.bottom as i32).any(|y| {
        (bounds.left as i32..bounds.right as i32)
            .any(|x| pixels.get_color((x, y)) != SkiaColor::TRANSPARENT)
    })
}

fn assert_ink_contained(surface: &mut Surface, bounds: SkiaRect) {
    let pixels = surface.peek_pixels().unwrap();
    for y in 0..400 {
        for x in 0..320 {
            if !bounds.contains(Point::new(x as f32, y as f32)) {
                assert_eq!(
                    pixels.get_color((x, y)),
                    SkiaColor::TRANSPARENT,
                    "ink outside {bounds:?} at ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn padded_text_and_headings_wrap_and_paint_inside_the_measured_content_box() {
    for theme in [Theme::light(), Theme::dark()] {
        for width in [90.0, 160.0] {
            for heading in [false, true] {
                let style = Style {
                    size: Size {
                        width: Dimension::length(width),
                        height: Dimension::auto(),
                    },
                    padding: Rect {
                        left: LengthPercentage::length(8.0),
                        right: LengthPercentage::length(12.0),
                        top: LengthPercentage::length(8.0),
                        bottom: LengthPercentage::length(12.0),
                    },
                    ..Style::default()
                };
                let content = "alpha beta gamma delta\nsecond explicit line";
                let widget = if heading {
                    Widget::heading(HeadingLevel::H3, content, style)
                } else {
                    Widget::Text {
                        content: content.into(),
                        style,
                        color: None,
                        size: 16.0,
                    }
                };
                let (taffy, root) = layout_widget(&widget, width as u32);
                let layout = taffy.layout(root).unwrap();
                let content_bounds = SkiaRect::from_xywh(
                    20.0,
                    20.0,
                    layout.content_box_width(),
                    layout.content_box_height(),
                );
                let mut surface = raster_widget(&widget, &taffy, root, &theme, None);
                assert_ink_contained(&mut surface, content_bounds);
                // More than the two explicit lines must be measured AND painted
                // at this narrow width (the old painter only drew those two).
                let font_size = if heading {
                    HeadingLevel::H3.font_size()
                } else {
                    16.0
                };
                assert!(layout.content_box_height() > font_size * 2.4);
                assert!(ink_in(
                    &mut surface,
                    SkiaRect::from_ltrb(
                        content_bounds.left,
                        content_bounds.top + font_size * 2.4,
                        content_bounds.right,
                        content_bounds.bottom
                    )
                ));
            }
        }
    }
}

fn input_widget(width: f32, multiline: bool, error: bool) -> Widget<'static, ()> {
    let style = Style {
        size: Size::from_lengths(width, if multiline { 100.0 } else { 40.0 }),
        ..Style::default()
    };
    let error = error.then(|| "A long validation error that must stay in this field".to_string());
    if multiline {
        Widget::text_area(
            |_| (),
            None,
            style,
            "Saturation (%) — a long channel label",
            InputState::Error,
            "Enter a value",
            error,
        )
        .with_id(17)
    } else {
        Widget::text_input(
            |_| (),
            None,
            style,
            "Saturation (%) — a long channel label",
            "Enter a value",
            InputState::Error,
            error,
            false,
        )
        .with_id(17)
    }
}

#[test]
fn input_annotations_are_contained_even_before_state_creation_and_on_early_returns() {
    for theme in [Theme::light(), Theme::dark()] {
        for width in [60.0, 140.0] {
            for multiline in [false, true] {
                let widget = input_widget(width, multiline, true);
                let (taffy, root) = layout_widget(&widget, width as u32);
                let size = taffy.layout(root).unwrap().size;
                let body_height = if multiline { 100.0 } else { 40.0 };
                assert_eq!(size.height, body_height + ANNOTATION_HEIGHT * 2.0);
                for text in [None, Some(""), Some("12345")] {
                    let mut surface = raster_widget(&widget, &taffy, root, &theme, text);
                    assert_ink_contained(
                        &mut surface,
                        SkiaRect::from_xywh(12.0, 12.0, size.width, size.height),
                    );
                    assert!(ink_in(
                        &mut surface,
                        SkiaRect::from_xywh(16.0, 12.0, width - 8.0, ANNOTATION_HEIGHT - 2.0)
                    ));
                    assert!(ink_in(
                        &mut surface,
                        SkiaRect::from_xywh(
                            16.0,
                            12.0 + ANNOTATION_HEIGHT + body_height + 2.0,
                            width - 8.0,
                            ANNOTATION_HEIGHT - 2.0
                        )
                    ));
                }
            }
        }
    }
}

#[test]
fn input_hits_geometry_and_drag_origins_use_the_rendered_body() {
    for width in [60.0, 140.0] {
        for multiline in [false, true] {
            let widget = input_widget(width, multiline, true);
            let (taffy, root) = layout_widget(&widget, width as u32);
            let origin = Point::new(30.0, 45.0);
            let body_height = if multiline { 100.0 } else { 40.0 };
            let mouse = Point::new(origin.x + 20.0, origin.y + ANNOTATION_HEIGHT + 11.0);
            let Some(HitResult::InputFocus {
                id,
                local_x,
                local_y,
                width: hit_width,
                height,
            }) = hit_test(&widget, &taffy, root, mouse, origin, &HashMap::new())
            else {
                panic!("editable body must focus the original manual ID");
            };
            assert_eq!(
                (id, local_x, local_y, hit_width, height),
                (17, 20.0, 11.0, width, body_height)
            );
            // The runner retains this origin for subsequent pointer-drag events.
            assert_eq!(mouse.y - local_y, origin.y + ANNOTATION_HEIGHT);
            let geometry = find_input_geometry(&widget, &taffy, root, 17).unwrap();
            assert_eq!((geometry.width, geometry.height), (width, body_height));
            for y in [10.0, ANNOTATION_HEIGHT + body_height + 10.0] {
                assert!(
                    hit_test(
                        &widget,
                        &taffy,
                        root,
                        Point::new(origin.x + 20.0, origin.y + y),
                        origin,
                        &HashMap::new()
                    )
                    .is_none()
                );
            }
        }
    }
}

#[test]
fn select_label_trigger_hit_and_popup_anchor_share_the_reserved_geometry() {
    for theme in [Theme::light(), Theme::dark()] {
        for width in [60.0, 140.0] {
            let widget = Widget::select(
                &["A long selected option"],
                0,
                |_| (),
                Style {
                    size: Size::from_lengths(width, 40.0),
                    ..Style::default()
                },
                "A long color format label",
                "Choose",
            )
            .with_id(19);
            let (taffy, root) = layout_widget(&widget, width as u32);
            let mut surface = raster_widget(&widget, &taffy, root, &theme, None);
            assert_ink_contained(&mut surface, SkiaRect::from_xywh(12.0, 12.0, width, 60.0));
            assert!(ink_in(
                &mut surface,
                SkiaRect::from_xywh(16.0, 12.0, width - 8.0, 18.0)
            ));
            assert!(
                hit_test(
                    &widget,
                    &taffy,
                    root,
                    Point::new(20.0, 10.0),
                    Point::default(),
                    &HashMap::new()
                )
                .is_none()
            );
            assert!(matches!(
                hit_test(
                    &widget,
                    &taffy,
                    root,
                    Point::new(20.0, 35.0),
                    Point::default(),
                    &HashMap::new()
                ),
                Some(HitResult::SelectToggle(19))
            ));
            let states = HashMap::from([(
                19,
                WidgetState::Select(SelectState {
                    is_open: true,
                    ..SelectState::default()
                }),
            )]);
            let overlays = select_overlay::collector::collect_open_select_overlays(
                &widget,
                &taffy,
                root,
                &states,
                (320.0, 400.0),
            );
            assert_eq!(overlays.len(), 1);
            assert_eq!(
                overlays[0].anchor,
                SkiaRect::from_xywh(0.0, ANNOTATION_HEIGHT, width, 40.0)
            );
        }
    }
}

#[test]
fn checkbox_and_radio_long_labels_do_not_paint_into_the_next_control() {
    for theme in [Theme::light(), Theme::dark()] {
        for radio in [false, true] {
            let mut surface = surfaces::raster_n32_premul((320, 400)).unwrap();
            surface.canvas().clear(SkiaColor::TRANSPARENT);
            let mut fonts = HashMap::new();
            let input = LabeledControlRenderInput {
                canvas: surface.canvas(),
                label: "A very long label that would overlap the next control",
                is_focused: false,
                size: (70.0, 32.0),
                mouse: Point::new(-100.0, -100.0),
                font_cache: &mut fonts,
                theme: &theme,
            };
            if radio {
                draw_radio(true, input);
            } else {
                draw_checkbox(true, input);
            }
            assert!(!ink_in(
                &mut surface,
                SkiaRect::from_xywh(71.0, 0.0, 240.0, 40.0)
            ));
            assert!(ink_in(
                &mut surface,
                SkiaRect::from_xywh(26.0, 0.0, 44.0, 32.0)
            ));
        }
    }
}

#[test]
fn compact_control_bodies_keep_edge_hits_and_disabled_paint_in_bounds() {
    for theme in [Theme::light(), Theme::dark()] {
        for enabled in [false, true] {
            for active in [false, true] {
                let style = Style {
                    size: Size::from_lengths(140.0, 28.0),
                    ..Style::default()
                };
                let controls = [
                    Widget::Checkbox {
                        checked: active,
                        label: "Lembrar",
                        on_change: |_| (),
                        style: style.clone(),
                    },
                    Widget::Radio {
                        selected: active,
                        label: "Opção",
                        on_select: || (),
                        style: style.clone(),
                    },
                    Widget::Switch {
                        checked: active,
                        on_change: |_| (),
                        style: style.clone(),
                    },
                    Widget::Select {
                        id: 19,
                        options: &["Rust"],
                        selected_index: 0,
                        on_change: |_| (),
                        label: "",
                        placeholder: "Escolha...",
                        style,
                    },
                ];
                for control in controls {
                    let control = control.enabled(enabled);
                    let (taffy, root) = layout_widget(&control, 140);
                    let mut surface = raster_widget(&control, &taffy, root, &theme, None);
                    assert_ink_contained(
                        &mut surface,
                        SkiaRect::from_xywh(12.0, 12.0, 140.0, 28.0),
                    );
                    assert!(ink_in(
                        &mut surface,
                        SkiaRect::from_xywh(12.0, 16.0, 18.0, 18.0)
                    ));
                    // Both edges of the allocated hit target still activate;
                    // optical stroke insets must not move pointer coordinates.
                    for x in [0.5, 139.5] {
                        let hit = hit_test(
                            &control,
                            &taffy,
                            root,
                            Point::new(x, 14.0),
                            Point::default(),
                            &HashMap::new(),
                        );
                        assert_eq!(hit.is_some(), enabled);
                    }
                }
            }
        }
    }
}

#[test]
fn changing_field_annotations_reserves_sibling_space_without_replacing_keyed_nodes() {
    let column = |error| Widget::Column {
        children: vec![
            input_widget(90.0, false, error),
            Widget::Spacer {
                style: Style {
                    size: Size::from_lengths(90.0, 16.0),
                    ..Style::default()
                },
            },
        ],
        style: Style::default(),
    };
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = taffy.new_leaf(Style::default()).unwrap();
    let mut tree = crate::layout::SyncedLayoutTree::placeholder(root);
    let mut first_node = None;
    for error in [false, true, false] {
        crate::layout::sync_taffy_tree(&mut taffy, &mut tree, &column(error), &HashMap::new());
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(120, 300),
            fonts.clone(),
            &RichTextRenderer::default(),
        );
        let children = taffy.children(root).unwrap();
        assert_eq!(*first_node.get_or_insert(children[0]), children[0]);
        assert_eq!(taffy.total_node_count(), 3);
        let field = taffy.layout(children[0]).unwrap();
        let sibling = taffy.layout(children[1]).unwrap();
        assert_eq!(field.size.height, if error { 80.0 } else { 60.0 });
        assert_eq!(sibling.location.y, field.location.y + field.size.height);
    }
}

#[test]
fn adjacent_narrow_channel_labels_and_errors_leave_the_column_gap_clear() {
    let widget = Widget::Row {
        children: vec![
            input_widget(60.0, false, true),
            input_widget(60.0, false, true).with_id(18),
        ],
        style: Style {
            gap: Size::length(8.0_f32),
            ..Style::default()
        },
    };
    let (taffy, root) = layout_widget(&widget, 128);
    for theme in [Theme::light(), Theme::dark()] {
        let mut surface = raster_widget(&widget, &taffy, root, &theme, None);
        assert!(!ink_in(
            &mut surface,
            SkiaRect::from_xywh(72.0, 12.0, 8.0, 80.0)
        ));
        assert!(ink_in(
            &mut surface,
            SkiaRect::from_xywh(84.0, 12.0, 52.0, 18.0)
        ));
    }
}

#[test]
fn unlabeled_fields_and_empty_errors_keep_the_original_control_size_and_hit_coordinates() {
    let widget = Widget::text_input(
        |_| (),
        None,
        Style {
            size: Size::from_lengths(100.0, 40.0),
            ..Style::default()
        },
        "",
        "",
        InputState::Idle,
        Some(String::new()),
        false,
    )
    .with_id(17);
    let (taffy, root) = layout_widget(&widget, 100);
    assert_eq!(taffy.layout(root).unwrap().size.height, 40.0);
    assert!(matches!(
        hit_test(
            &widget,
            &taffy,
            root,
            Point::new(20.0, 10.0),
            Point::default(),
            &HashMap::new()
        ),
        Some(HitResult::InputFocus {
            id: 17,
            local_y: 10.0,
            height: 40.0,
            ..
        })
    ));
}
