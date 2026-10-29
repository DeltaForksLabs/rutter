use super::collect_focus_order;
use crate::{ButtonVariant, DynamicIsland, Theme, Widget};
use taffy::prelude::*;

#[test]
fn dynamic_island_tab_order_excludes_collapsed_children_and_preserves_ids_on_reopen() {
    let mut state = DynamicIsland::interactive();
    let mut expanded_order = None;
    let mut collapsed_order = None;
    for expanded in [false, true, false, true] {
        state.set_expanded(expanded).unwrap();
        let view = state
            .interactive_view(
                if expanded {
                    "تم التنزيل — Collapse details"
                } else {
                    "Downloading — Expand details"
                },
                (),
                Widget::Button {
                    text: "Child",
                    on_press: (),
                    style: Style::default(),
                    color: None,
                    variant: ButtonVariant::Primary,
                },
                &Theme::light(),
            )
            .unwrap();
        let mut order = Vec::new();
        collect_focus_order(&view, &mut order);
        assert_eq!(order.len(), if expanded { 2 } else { 1 });
        let previous = if expanded {
            &mut expanded_order
        } else {
            &mut collapsed_order
        };
        if let Some(previous) = previous {
            assert_eq!(*previous, order);
        } else {
            *previous = Some(order);
        }
    }
    assert_eq!(expanded_order.unwrap()[0], collapsed_order.unwrap()[0]);
    let passive: Widget<'_, ()> = DynamicIsland::passive()
        .passive_view("Summary", &Theme::light())
        .unwrap();
    let mut order = Vec::new();
    collect_focus_order(&passive, &mut order);
    assert!(order.is_empty());
}

#[test]
fn dynamic_island_motion_exit_disabled_and_hidden_compositions_have_no_tab_stops() {
    use crate::{DynamicIslandMotion, DynamicIslandSlide};
    use std::time::{Duration, Instant};
    let mut island = DynamicIsland::interactive();
    island.set_expanded(true).unwrap();
    let now = Instant::now();
    let mut motion = DynamicIslandMotion::new(
        skia_safe::Rect::from_xywh(0.0, 0.0, 420.0, 300.0),
        skia_safe::Rect::from_xywh(8.0, 8.0, 404.0, 252.0),
    )
    .unwrap();
    motion.slide_at(
        DynamicIslandSlide::SlideOutUp,
        Duration::from_millis(280),
        now,
    );
    for at in [
        now,
        now + Duration::from_millis(140),
        now + Duration::from_secs(1),
    ] {
        let view = if motion.sample_at(at).is_some() {
            island
                .interactive_view(
                    "Closing",
                    (),
                    Widget::Button {
                        text: "Child",
                        on_press: (),
                        style: Style::default(),
                        color: None,
                        variant: ButtonVariant::Primary,
                    },
                    &Theme::dark(),
                )
                .unwrap()
                .enabled(false)
        } else {
            Widget::Column {
                children: vec![],
                style: Style::default(),
            }
        };
        let mut order = Vec::new();
        collect_focus_order(&view, &mut order);
        assert!(order.is_empty());
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        let mut tree = TaffyTree::new();
        let root = crate::layout::build_taffy_tree(
            &mut tree,
            &view,
            fonts.clone(),
            &std::collections::HashMap::new(),
        );
        crate::layout::compute_layout(
            &mut tree,
            root,
            winit::dpi::PhysicalSize::new(420, 300),
            fonts,
            &crate::render::RichTextRenderer::default(),
        );
        let update = crate::accessibility::build_accessibility_update(
            &tree,
            &view,
            root,
            crate::accessibility::AccessibilityInputs {
                input_states: &std::collections::HashMap::new(),
                widget_states: &std::collections::HashMap::new(),
                focused_widget_id: None,
                viewport: (420.0, 300.0),
                direction: crate::LayoutDirection::Ltr,
            },
        );
        assert!(
            update
                .nodes
                .iter()
                .all(|(_, node)| !node.supports_action(accesskit::Action::Click)
                    && !node.supports_action(accesskit::Action::Focus))
        );
        if motion.bounds().is_none() {
            assert!(
                update
                    .nodes
                    .iter()
                    .all(|(_, node)| node.label() != Some("Child"))
            );
        }
    }
}
