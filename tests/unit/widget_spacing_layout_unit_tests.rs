use super::*;
use crate::widget::InputState;

fn field_widget(kind: usize, style: Style, annotated: bool) -> Widget<'static, ()> {
    let label = if annotated { "Channel" } else { "" };
    let error = annotated.then(|| "Invalid channel".to_string());
    match kind {
        0 => Widget::text_input(
            |_| (),
            None,
            style,
            label,
            "",
            InputState::Idle,
            error,
            false,
        ),
        1 => Widget::text_area(|_| (), None, style, label, InputState::Idle, "", error),
        2 => Widget::select(&["Choice"], 0, |_| (), style, label, ""),
        _ => unreachable!(),
    }
    .with_id(17)
}

fn child_height(kind: usize, style: Style, annotated: bool, stretch: bool) -> (f32, Style) {
    let widget = Widget::Row {
        children: vec![field_widget(kind, style, annotated)],
        style: Style {
            size: Size::from_lengths(240.0, 200.0),
            align_items: Some(if stretch {
                AlignItems::Stretch
            } else {
                AlignItems::FlexStart
            }),
            ..Style::default()
        },
    };
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &widget, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(240, 240),
        fonts,
        &RichTextRenderer::default(),
    );
    let child = taffy.children(root).unwrap()[0];
    (
        taffy.layout(child).unwrap().size.height,
        taffy.style(child).unwrap().clone(),
    )
}

fn annotations(kind: usize, annotated: bool) -> f32 {
    if !annotated {
        0.0
    } else if kind == 2 {
        20.0
    } else {
        40.0
    }
}

#[test]
fn auto_fields_stretch_to_the_definite_row_cross_axis_with_and_without_annotations() {
    for kind in 0..3 {
        for annotated in [false, true] {
            let (height, style) = child_height(kind, Style::default(), annotated, true);
            assert!(style.size.height.is_auto(), "caller Auto was replaced");
            assert_eq!(height, 200.0);
            assert!(style.min_size.height.is_auto());
            assert!(style.max_size.height.is_auto());
        }
    }
}

#[test]
fn nonstretch_auto_fields_keep_their_intrinsic_body_height_and_annotations() {
    for kind in 0..3 {
        for annotated in [false, true] {
            let (height, style) = child_height(kind, Style::default(), annotated, false);
            assert!(style.size.height.is_auto());
            assert_eq!(
                height,
                (if kind == 1 { 120.0 } else { 40.0 }) + annotations(kind, annotated)
            );
        }
    }
}

#[test]
fn auto_intrinsic_height_respects_mixed_length_and_percentage_min_max_constraints() {
    let cases = [
        (Dimension::auto(), Dimension::length(25.0), 65.0, 65.0),
        (Dimension::length(80.0), Dimension::auto(), 120.0, 200.0),
        (
            Dimension::length(10.0),
            Dimension::percent(0.25),
            50.0,
            50.0,
        ),
        (
            Dimension::percent(0.5),
            Dimension::length(70.0),
            100.0,
            110.0,
        ),
        // CSS min wins over max even when they use different units.
        (
            Dimension::percent(0.75),
            Dimension::length(40.0),
            150.0,
            150.0,
        ),
    ];
    for (min, max, intrinsic_height, stretched_height) in cases {
        for stretch in [false, true] {
            let (height, style) = child_height(
                0,
                Style {
                    min_size: Size {
                        width: Dimension::auto(),
                        height: min,
                    },
                    max_size: Size {
                        width: Dimension::auto(),
                        height: max,
                    },
                    ..Style::default()
                },
                true,
                stretch,
            );
            let expected = if stretch {
                stretched_height
            } else {
                intrinsic_height
            };
            assert!(style.size.height.is_auto());
            assert_eq!(
                height, expected,
                "min={min:?}, max={max:?}, stretch={stretch}"
            );
        }
    }
}

#[test]
fn percentage_heights_stay_parent_relative_while_absolute_body_bounds_reserve_annotations() {
    for kind in 0..3 {
        for annotated in [false, true] {
            let extra = annotations(kind, annotated);
            for (min, max, expected) in [
                (Dimension::auto(), Dimension::auto(), 100.0),
                (Dimension::length(110.0), Dimension::auto(), 110.0 + extra),
                (Dimension::auto(), Dimension::length(30.0), 30.0 + extra),
                (Dimension::length(10.0), Dimension::percent(0.4), 80.0),
            ] {
                let (height, style) = child_height(
                    kind,
                    Style {
                        size: Size {
                            width: Dimension::length(80.0),
                            height: Dimension::percent(0.5),
                        },
                        min_size: Size {
                            width: Dimension::auto(),
                            height: min,
                        },
                        max_size: Size {
                            width: Dimension::auto(),
                            height: max,
                        },
                        ..Style::default()
                    },
                    annotated,
                    true,
                );
                assert_eq!(style.size.height, Dimension::percent(0.5));
                assert_eq!(height, expected);
            }
        }
    }
}

#[test]
fn explicit_length_heights_and_body_min_max_keep_annotation_reservation() {
    for kind in 0..3 {
        for annotated in [false, true] {
            for (min, max, expected_body) in [
                (Dimension::auto(), Dimension::auto(), 40.0),
                (Dimension::length(60.0), Dimension::auto(), 60.0),
                (Dimension::auto(), Dimension::length(30.0), 30.0),
            ] {
                let (height, _) = child_height(
                    kind,
                    Style {
                        size: Size::from_lengths(80.0, 40.0),
                        min_size: Size {
                            width: Dimension::auto(),
                            height: min,
                        },
                        max_size: Size {
                            width: Dimension::auto(),
                            height: max,
                        },
                        ..Style::default()
                    },
                    annotated,
                    true,
                );
                assert_eq!(height, expected_body + annotations(kind, annotated));
            }
        }
    }
}
