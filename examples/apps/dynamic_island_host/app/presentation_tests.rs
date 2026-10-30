use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rutter::{
    activity::{Transfer, TransferDirection, TransferState},
    render::hit_test::{HitResult, hit_test},
    skia_safe::Point,
};

use super::*;

fn fixture() -> (State, [ActivityId; 2]) {
    let mut state = State::default();
    let owner = state.registry.register_owner().unwrap();
    let ids = ["First · téléchargement", "Second · تحميل"].map(|title| {
        state
            .registry
            .start(
                owner,
                Transfer::new(
                    title,
                    TransferDirection::Download,
                    10,
                    Some(100),
                    TransferState::Running,
                )
                .unwrap(),
            )
            .unwrap()
    });
    state
        .errors
        .insert(ids[0], "Pause delivery failed. Try again.".into());
    (state, ids)
}

#[test]
fn selectors_name_the_activity_and_selection_in_the_accessible_button_label() {
    let (mut state, ids) = fixture();
    for selected in [ids[0], ids[1], ids[0]] {
        state.reduce(Message::Select(selected), |_, _| unreachable!());
        let expected: Vec<_> = state
            .registry
            .iter()
            .map(|activity| {
                (
                    activity.id(),
                    format!(
                        "{} · {}",
                        if activity.id() == selected {
                            "Selected"
                        } else {
                            "Select"
                        },
                        view::summary(activity)
                    ),
                )
            })
            .collect();
        let expected_header = format!(
            "{} · Collapse details",
            view::summary(state.registry.get(selected).unwrap())
        );
        let Widget::ScrollView { child, .. } = Host::view(&mut state, SurfaceId::PRIMARY) else {
            panic!()
        };
        let Widget::Column { children, .. } = *child else {
            panic!()
        };
        let Widget::Column {
            children: selectors,
            ..
        } = &children[2]
        else {
            panic!()
        };
        assert_eq!(selectors.len(), 2);
        for (selector, (id, expected)) in selectors.iter().zip(&expected) {
            let Widget::ButtonContent {
                label,
                on_press: Message::Select(target),
                child,
                ..
            } = selector
            else {
                panic!()
            };
            // ButtonContent.label is the full accessible name, not merely child paint.
            assert_eq!(*label, expected);
            assert_eq!(target, id);
            let Widget::Text { content, .. } = &**child else {
                panic!()
            };
            assert_eq!(content, expected);
        }
        let Widget::Container { child, .. } = children.last().unwrap() else {
            panic!()
        };
        let Widget::Column {
            children: capsule_children,
            ..
        } = &**child
        else {
            panic!()
        };
        assert!(
            matches!(&capsule_children[0], Widget::ButtonContent { label, on_press: Message::ToggleDetails, .. } if *label == expected_header)
        );
    }
}

#[test]
fn real_capsule_header_toggles_and_collapsing_removes_detail_hits_and_error() {
    for dark in [false, true] {
        for width in [320, 800] {
            let (mut state, ids) = fixture();
            state.dark = dark;
            let mut expanded_height = None;
            for expanded in [true, false, true, false] {
                state.reduce(Message::ToggleDetails, |_, _| unreachable!());
                assert_eq!(state.island.is_expanded(), expanded);
                let expected_summary = format!(
                    "{} · {} details",
                    view::summary(state.registry.get(ids[0]).unwrap()),
                    if expanded { "Collapse" } else { "Expand" }
                );
                let Widget::ScrollView { child, .. } = Host::view(&mut state, SurfaceId::PRIMARY)
                else {
                    panic!()
                };
                let Widget::Column { mut children, .. } = *child else {
                    panic!()
                };
                let capsule = children.pop().unwrap();
                let Widget::Container { radius, child, .. } = &capsule else {
                    panic!("host must mount an actual capsule");
                };
                assert_eq!(*radius, DynamicIsland::RADIUS);
                let Widget::Column {
                    children: capsule_children,
                    ..
                } = &**child
                else {
                    panic!()
                };
                assert_eq!(capsule_children.len(), if expanded { 2 } else { 1 });
                let Widget::ButtonContent {
                    label,
                    on_press: Message::ToggleDetails,
                    ..
                } = &capsule_children[0]
                else {
                    panic!()
                };
                assert_eq!(*label, expected_summary);

                let fonts = Rc::new(RefCell::new(FontSystem::new()));
                let mut tree = TaffyTree::new();
                let root = rutter::layout::build_taffy_tree(
                    &mut tree,
                    &capsule,
                    fonts.clone(),
                    &HashMap::new(),
                );
                rutter::layout::compute_layout(
                    &mut tree,
                    root,
                    winit::dpi::PhysicalSize::new(width, 120),
                    fonts,
                    &rutter::render::RichTextRenderer::default(),
                );
                let content = tree.children(root).unwrap()[0];
                let content_bounds = tree.layout(content).unwrap();
                let header = tree.children(content).unwrap()[0];
                let header_bounds = tree.layout(header).unwrap();
                assert!(header_bounds.size.height >= 44.0);
                let header_point = Point::new(
                    content_bounds.location.x
                        + header_bounds.location.x
                        + header_bounds.size.width / 2.0,
                    content_bounds.location.y
                        + header_bounds.location.y
                        + header_bounds.size.height / 2.0,
                );
                assert!(matches!(
                    hit_test(
                        &capsule,
                        &tree,
                        root,
                        header_point,
                        Point::default(),
                        &HashMap::new()
                    ),
                    Some(HitResult::Message {
                        msg: Message::ToggleDetails,
                        ..
                    })
                ));
                let height = tree.layout(root).unwrap().size.height;
                if expanded {
                    expanded_height = Some(height);
                    let details = tree.children(content).unwrap()[1];
                    let details_bounds = tree.layout(details).unwrap();
                    assert!(
                        details_bounds.location.y
                            >= header_bounds.location.y + header_bounds.size.height + spacing::MD
                                - 1.0
                    );
                    assert!(
                        details_bounds.location.x + details_bounds.size.width
                            <= content_bounds.size.width + 1.0
                    );
                    let Widget::Column {
                        children: details_widgets,
                        ..
                    } = &capsule_children[1]
                    else {
                        panic!()
                    };
                    assert!(
                        matches!(&details_widgets[1], Widget::Text { content, .. } if content == "Pause delivery failed. Try again.")
                    );
                } else {
                    assert!(height < expanded_height.unwrap());
                }
                // Sample the entire capsule, including where removed details were.
                // Only expanded details may expose typed producer action messages.
                let mut actions = 0;
                for y in (0..expanded_height.unwrap().ceil() as usize).step_by(8) {
                    for x in (0..width as usize).step_by(8) {
                        if let Some(HitResult::Message {
                            msg: Message::Action(id, _),
                            ..
                        }) = hit_test(
                            &capsule,
                            &tree,
                            root,
                            Point::new(x as f32, y as f32),
                            Point::default(),
                            &HashMap::new(),
                        ) {
                            assert_eq!(id, ids[0]);
                            actions += 1;
                        }
                    }
                }
                assert_eq!(actions > 0, expanded);
            }
        }
    }
}
