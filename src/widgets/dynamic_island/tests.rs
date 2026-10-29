use super::*;
use crate::{SurfaceInputRegion, skia_safe::Rect};

#[test]
fn translated_summary_keeps_complete_semantic_label() {
    let summary = "Long localized summary — تفاصيل كاملة";
    let view = DynamicIsland::interactive()
        .interactive_view_with_offset(
            summary,
            (),
            Widget::Spacer {
                style: Style::default(),
            },
            (35.0, -12.0),
            &Theme::light(),
        )
        .unwrap();
    let Widget::Container { child, .. } = view else {
        panic!("capsule")
    };
    let Widget::Column { children, .. } = *child else {
        panic!("document")
    };
    let Widget::ButtonContent { label, child, .. } = &children[0] else {
        panic!("header")
    };
    assert_eq!(*label, summary);
    let Widget::RichText { content, .. } = child.as_ref() else {
        panic!("summary")
    };
    assert_eq!(content.plain_text(), summary);
    assert_eq!(content.visual_offset(), (35.0, -12.0));
}

#[test]
fn dynamic_island_owned_mode_and_expansion_are_independent_of_views() {
    let mut passive = DynamicIsland::passive();
    for requested in [false, true] {
        assert_eq!(
            passive.set_expanded(requested),
            Err(DynamicIslandError::PassiveExpansion)
        );
    }
    assert_eq!(passive.toggle(), Err(DynamicIslandError::PassiveExpansion));
    assert!(!passive.is_expanded());
    assert_eq!(passive.clone().mode(), DynamicIslandMode::Passive);
    let mut interactive = DynamicIsland::interactive();
    interactive.toggle().unwrap();
    let mut copy = interactive.clone();
    copy.toggle().unwrap();
    assert!(interactive.is_expanded());
    assert!(!copy.is_expanded());
    for summary in ["Downloading", "تم التنزيل — إغلاق التفاصيل"] {
        interactive
            .interactive_view(
                summary,
                (),
                Widget::Spacer {
                    style: Style::default(),
                },
                &Theme::light(),
            )
            .unwrap();
        assert!(interactive.is_expanded());
    }
    assert!(matches!(
        interactive.passive_view::<()>("x", &Theme::light()),
        Err(DynamicIslandError::ModeMismatch)
    ));
    assert!(matches!(
        passive.interactive_view(
            "x",
            (),
            Widget::Spacer {
                style: Style::default()
            },
            &Theme::light()
        ),
        Err(DynamicIslandError::ModeMismatch)
    ));
}

#[test]
fn dynamic_island_region_rejects_malformed_and_excessive_geometry() {
    let island = DynamicIsland::interactive();
    for bounds in [
        Rect::from_xywh(0.0, 0.0, 0.0, 10.0),
        Rect::from_xywh(0.0, 0.0, -1.0, 10.0),
        Rect::from_xywh(f32::NAN, 0.0, 30.0, 20.0),
        Rect::from_xywh(0.0, 0.0, f32::INFINITY, 10.0),
        Rect::from_xywh(i32::MAX as f32, 0.0, 256.0, 10.0),
    ] {
        for state in [DynamicIsland::passive(), island.clone()] {
            assert_eq!(
                state.input_region(bounds, 24.0),
                Err(DynamicIslandError::InvalidGeometry)
            );
        }
    }
    let bounds = Rect::from_xywh(0.0, 0.0, 800.0, 800.0);
    for radius in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            island.input_region(bounds, radius),
            Err(DynamicIslandError::InvalidGeometry)
        );
    }
    assert_eq!(
        island.input_region(bounds, 128.0),
        Err(DynamicIslandError::GeometryTooComplex)
    );
    assert_eq!(
        DynamicIsland::passive()
            .input_region(bounds, 128.0)
            .unwrap(),
        SurfaceInputRegion::Empty
    );
    assert!(
        island
            .input_region(Rect::from_xywh(-20.0, -10.0, 320.0, 1_000_000.0), 127.0)
            .is_ok()
    );
}

#[test]
fn dynamic_island_bands_follow_curve_and_live_bounds_with_bounded_work() {
    let island = DynamicIsland::interactive();
    for height in [76.0, 230.0, 1000.0] {
        for radius in [0.0_f32, 0.5, 24.0, 127.0, 10000.0] {
            let effective = radius.min(160.0_f32).min(height / 2.0);
            if effective > 127.0 {
                continue;
            }
            let bounds = Rect::from_xywh(8.25, 12.25, 320.0, height);
            let SurfaceInputRegion::Rectangles(rects) =
                island.input_region(bounds, radius).unwrap()
            else {
                panic!()
            };
            assert!(rects.as_slice().len() <= 256);
            for rect in rects.as_slice() {
                let (x, y, w, h) = rect.logical_rect();
                assert!(x as f32 >= bounds.left - 0.5 && (x + w) as f32 <= bounds.right + 0.5);
                assert!(y as f32 >= bounds.top - 0.5 && (y + h) as f32 <= bounds.bottom + 0.5);
                for row in y..y + h {
                    let dy = (bounds.top + effective - (row as f32 + 0.5))
                        .max(row as f32 + 0.5 - (bounds.bottom - effective))
                        .max(0.0);
                    let inset = effective - (effective * effective - dy * dy).max(0.0).sqrt();
                    assert!((x as f32 - (bounds.left + inset)).abs() <= 0.501);
                    assert!(((x + w) as f32 - (bounds.right - inset)).abs() <= 0.501);
                }
            }
        }
    }
}

#[test]
fn dynamic_island_accessibility_retains_full_summary_and_only_visible_actions() {
    use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
    use crate::layout::{build_taffy_tree, compute_layout};
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    let summary = "تم التنزيل — عرض التفاصيل الكاملة — Download complete — expand details";
    let theme = Theme::dark();
    let mut interactive = DynamicIsland::interactive();
    for expanded in [false, true, false, true] {
        interactive.set_expanded(expanded).unwrap();
        for passive in [false, true] {
            let view = if passive {
                DynamicIsland::passive()
                    .passive_view(summary, &theme)
                    .unwrap()
            } else {
                interactive
                    .interactive_view(
                        summary,
                        (),
                        Widget::Button {
                            text: "Child action",
                            on_press: (),
                            style: Style::default(),
                            color: None,
                            variant: ButtonVariant::Primary,
                        },
                        &theme,
                    )
                    .unwrap()
            };
            let fonts = Rc::new(RefCell::new(cosmic_text::FontSystem::new()));
            let states = HashMap::new();
            let mut taffy = TaffyTree::new();
            let root = build_taffy_tree(&mut taffy, &view, fonts.clone(), &states);
            compute_layout(
                &mut taffy,
                root,
                winit::dpi::PhysicalSize::new(320, 180),
                fonts,
                &crate::render::RichTextRenderer::default(),
            );
            let update = build_accessibility_update(
                &taffy,
                &view,
                root,
                AccessibilityInputs {
                    input_states: &HashMap::new(),
                    widget_states: &states,
                    focused_widget_id: None,
                    viewport: (320.0, 180.0),
                    direction: crate::LayoutDirection::Rtl,
                },
            );
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(summary))
            );
            let actions: Vec<_> = update
                .nodes
                .iter()
                .filter(|(_, node)| node.supports_action(accesskit::Action::Click))
                .collect();
            assert_eq!(
                actions.len(),
                if passive {
                    0
                } else if expanded {
                    2
                } else {
                    1
                }
            );
            if passive {
                assert!(
                    update
                        .nodes
                        .iter()
                        .all(|(_, node)| !node.supports_action(accesskit::Action::Focus))
                );
            }
            assert_eq!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some("Child action")),
                !passive && expanded
            );
        }
    }
}
