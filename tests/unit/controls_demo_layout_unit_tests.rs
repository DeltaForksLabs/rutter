use super::*;
use rutter::engine::widget_state::{ScrollState, WidgetState};
use rutter::layout::{build_taffy_tree, compute_layout};
use rutter::render::{
    RichTextRenderer,
    hit_test::{HitResult, hit_test},
};
use skia_safe::{Point, Rect as Bounds};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use winit::dpi::PhysicalSize;

#[test]
fn sections_reserve_focus_space_inside_the_viewport_and_keep_scrolled_hits() {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut state = ControlsDemoState::default();
    let view = ControlsDemo::view(&mut state);
    for width in [320, 469, 800] {
        for height in [260, 700] {
            let mut tree = TaffyTree::new();
            let root = build_taffy_tree(&mut tree, &view, fonts.clone(), &HashMap::new());
            compute_layout(
                &mut tree,
                root,
                PhysicalSize::new(width, height),
                fonts.clone(),
                &RichTextRenderer::default(),
            );
            let Widget::Column { children, .. } = &view else {
                unreachable!()
            };
            let index = children
                .iter()
                .position(|child| matches!(child, Widget::ScrollView { .. }))
                .unwrap();
            let viewport_node = tree.children(root).unwrap()[index];
            let viewport = tree.layout(viewport_node).unwrap();
            let document_node = tree.children(viewport_node).unwrap()[0];
            let document = tree.layout(document_node).unwrap();
            let document_children = super::super::layout::document_children(&view);
            let nodes = tree.children(document_node).unwrap();
            let mut controls = Vec::new();
            for (widget, node) in document_children.iter().zip(nodes) {
                collect_control_bounds(
                    widget,
                    &tree,
                    node,
                    Point::new(viewport.location.x, viewport.location.y),
                    &mut controls,
                );
            }
            assert_eq!(controls.len(), 7);
            for (bounds, select) in controls {
                assert!(bounds.left >= viewport.location.x + spacing::SM);
                assert!(
                    bounds.right <= viewport.location.x + viewport.size.width - spacing::SM + 1.0
                );
                assert!(bounds.width() > 0.0 && bounds.height() >= 28.0);
                let mut scroll = ScrollState {
                    offset_y: 0.0,
                    content_height: document.size.height,
                    viewport_h: viewport.size.height,
                };
                scroll.scroll_by(
                    (bounds.center_y() - viewport.location.y - viewport.size.height / 2.0).max(0.0),
                );
                let states = HashMap::from([(
                    super::super::layout::PAGE_SCROLL_ID,
                    WidgetState::Scroll(scroll.clone()),
                )]);
                let mouse = Point::new(bounds.left + 9.0, bounds.center_y() - scroll.offset_y);
                let hit = hit_test(&view, &tree, root, mouse, Point::default(), &states);
                if select {
                    assert!(matches!(hit, Some(HitResult::SelectToggle(10))));
                } else {
                    assert!(matches!(hit, Some(HitResult::Message { .. })));
                }
            }
        }
    }
}

fn collect_control_bounds(
    widget: &Widget<'_, Msg>,
    tree: &TaffyTree<rutter::layout::RutterContext>,
    node: NodeId,
    origin: Point,
    controls: &mut Vec<(Bounds, bool)>,
) {
    let layout = tree.layout(node).unwrap();
    let origin = Point::new(origin.x + layout.location.x, origin.y + layout.location.y);
    match widget {
        Widget::Checkbox { .. }
        | Widget::Radio { .. }
        | Widget::Switch { .. }
        | Widget::Select { .. } => {
            controls.push((
                Bounds::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height),
                matches!(widget, Widget::Select { .. }),
            ));
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (child, node) in children.iter().zip(tree.children(node).unwrap()) {
                collect_control_bounds(child, tree, node, origin, controls);
            }
        }
        _ => {}
    }
}

#[test]
fn checkbox_row_wraps_and_uses_bounded_fluid_widths() {
    let mut state = ControlsDemoState::default();
    let view = ControlsDemo::view(&mut state);
    let children = super::super::layout::document_children(&view);
    let (checkboxes, style) = checkbox_row(children)
        .expect("controls demo must contain one Row with only Checkbox children");

    assert_eq!(style.flex_wrap, FlexWrap::Wrap);
    assert_eq!(style.gap.width, LengthPercentage::length(24.0));
    assert_eq!(style.gap.height, LengthPercentage::length(12.0));
    assert_eq!(style.size.width, Dimension::percent(1.0));
    assert_eq!(style.max_size.width, Dimension::length(384.0));
    assert_eq!(checkboxes.len(), 2);
    for checkbox in checkboxes {
        let Widget::Checkbox { style, .. } = checkbox else {
            unreachable!()
        };
        assert_eq!(style.size.width, Dimension::percent(1.0));
        assert_eq!(style.max_size.width, Dimension::length(180.0));
    }
}

fn checkbox_row<'view, 'widget>(
    children: &'view [Widget<'widget, Msg>],
) -> Option<(&'view [Widget<'widget, Msg>], &'view Style)> {
    children.iter().find_map(|child| match child {
        Widget::Row { children, style }
            if children
                .iter()
                .all(|child| matches!(child, Widget::Checkbox { .. })) =>
        {
            Some((children.as_slice(), style))
        }
        Widget::Column { children, .. } => checkbox_row(children),
        _ => None,
    })
}
