use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Instant};

use rutter::{
    ShortcutEvent, ShortcutKey, ShortcutNamedKey, ShortcutOutcome,
    activity::{Transfer, TransferDirection, TransferState},
    render::hit_test::{HitResult, hit_test},
    skia_safe::Point,
};

use super::*;

fn metrics(width: u32, height: u32, scale: f64) -> SurfaceMetrics {
    SurfaceMetrics {
        physical_size: (
            (width as f64 * scale) as u32,
            (height as f64 * scale) as u32,
        ),
        logical_size: (width as f64, height as f64),
        scale_factor: scale,
        output_name: None,
        layer_shell_version: Some(4),
    }
}

fn state() -> State {
    State {
        overlay: Some(Overlay::default()),
        ..State::default()
    }
}

fn contains(region: &SurfaceInputRegion, point: Point) -> bool {
    let SurfaceInputRegion::Rectangles(rects) = region else {
        return false;
    };
    rects.as_slice().iter().any(|rect| {
        let (x, y, width, height) = rect.logical_rect();
        point.x >= x as f32
            && point.x < (x + width) as f32
            && point.y >= y as f32
            && point.y < (y + height) as f32
    })
}

#[test]
fn publication_is_deduplicated_but_metrics_and_recreation_force_one_shot_publish() {
    let now = Instant::now();
    let mut state = state();
    assert!(!state.island.is_expanded());
    for _ in 0..2 {
        Host::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(420, 300, 1.5));
        assert_eq!(Host::next_wakeup(&state, now), Some(now));
        assert!(matches!(
            Host::wakeup(&mut state, now).as_slice(),
            [
                SurfaceCommand::SetInputRegion { .. },
                SurfaceCommand::RequestRedraw(_)
            ]
        ));
        assert_eq!(Host::next_wakeup(&state, now), None);
        assert!(publish(&mut state).is_empty());
    }
    Host::surface_created(&mut state, SurfaceId::PRIMARY);
    assert_eq!(Host::wakeup(&mut state, now).len(), 2);
    state.reduce(Message::ToggleTheme, |_, _| unreachable!());
    assert!(publish(&mut state).is_empty());
    state.reduce(Message::ToggleDetails, |_, _| unreachable!());
    assert_eq!(publish(&mut state).len(), 2);
    let escape = ShortcutEvent {
        key: ShortcutKey::Named(ShortcutNamedKey::Escape),
        control: false,
        alt: false,
        shift: false,
        super_key: false,
        repeat: false,
    };
    assert!(matches!(
        Host::shortcut(&state, SurfaceId::PRIMARY, escape.clone()),
        ShortcutOutcome::Message(Message::Collapse)
    ));
    assert!(matches!(
        Host::shortcut(&state, SurfaceId::new(42), escape.clone()),
        ShortcutOutcome::Ignored
    ));
    let mut modified_escape = escape.clone();
    modified_escape.control = true;
    assert!(matches!(
        Host::shortcut(&state, SurfaceId::PRIMARY, modified_escape),
        ShortcutOutcome::Ignored
    ));
    assert!(matches!(
        Host::shortcut(&State::default(), SurfaceId::PRIMARY, escape),
        ShortcutOutcome::Ignored
    ));
    state.reduce(Message::Collapse, |_, _| unreachable!());
    assert!(publish(&mut state).iter().all(|command| matches!(
        command,
        SurfaceCommand::SetInputRegion { .. } | SurfaceCommand::RequestRedraw(_)
    )));
    Host::surface_created(&mut state, SurfaceId::PRIMARY);
    assert!(matches!(
        close(&mut state).as_slice(),
        [
            SurfaceCommand::SetInputRegion {
                region: SurfaceInputRegion::Empty,
                ..
            },
            SurfaceCommand::Close(_)
        ]
    ));
    assert_eq!(Host::next_wakeup(&state, now), None);
    Host::surface_created(&mut state, SurfaceId::PRIMARY);
    assert!(publish(&mut state).is_empty());
    let mut externally_closed = self::state();
    Host::surface_created(&mut externally_closed, SurfaceId::PRIMARY);
    Host::surface_closed(&mut externally_closed, SurfaceId::PRIMARY);
    assert_eq!(Host::next_wakeup(&externally_closed, now), None);
    assert!(publish(&mut externally_closed).is_empty());
    assert!(publish(&mut State::default()).is_empty());
}

#[test]
fn bounded_capsule_matches_native_input_and_scrolls_the_entire_long_document() {
    for dark in [false, true] {
        for width in [320, 800] {
            for height in [120, 300] {
                for scale in [1.0, 1.5, 2.0] {
                    for expanded in [false, true] {
                        let mut state = state();
                        state.dark = dark;
                        let owner = state.registry.register_owner().unwrap();
                        let title = "Long localized téléchargement تحميل summary ".repeat(5);
                        let id = state
                            .registry
                            .start(
                                owner,
                                Transfer::new(
                                    title,
                                    TransferDirection::Download,
                                    1,
                                    Some(100),
                                    TransferState::Running,
                                )
                                .unwrap(),
                            )
                            .unwrap();
                        state.errors.insert(
                            id,
                            "Pause was not delivered. Retry after the next update.".into(),
                        );
                        state.island.set_expanded(expanded).unwrap();
                        Host::surface_metrics_changed(
                            &mut state,
                            SurfaceId::PRIMARY,
                            metrics(width, height, scale),
                        );
                        let expected = bounds(&state).unwrap();
                        let region = state
                            .island
                            .input_region(expected, DynamicIsland::RADIUS)
                            .unwrap();
                        let theme = Host::theme_for(&state);
                        let widget = Host::view(&mut state, SurfaceId::PRIMARY);
                        rutter::WidgetIdSnapshot::capture(&widget).unwrap();
                        let fonts = Rc::new(RefCell::new(FontSystem::new()));
                        let mut tree = TaffyTree::new();
                        let root = rutter::layout::build_taffy_tree(
                            &mut tree,
                            &widget,
                            fonts.clone(),
                            &HashMap::new(),
                        );
                        rutter::layout::compute_layout(
                            &mut tree,
                            root,
                            winit::dpi::PhysicalSize::new(width, height),
                            fonts.clone(),
                            &rutter::render::RichTextRenderer::default(),
                        );
                        let capsule = tree.children(root).unwrap()[0];
                        let allocation = tree.layout(capsule).unwrap();
                        assert!((allocation.location.x - expected.left).abs() <= 1.0);
                        assert!((allocation.location.y - expected.top).abs() <= 1.0);
                        assert!((allocation.size.width - expected.width()).abs() <= 1.0);
                        assert!((allocation.size.height - expected.height()).abs() <= 1.0);
                        assert!(!contains(&region, Point::new(expected.left, expected.top)));
                        assert!(!contains(&region, Point::new(0.0, 0.0)));
                        assert!(contains(
                            &region,
                            Point::new(expected.center_x(), expected.top + 4.0)
                        ));
                        let viewport = tree.children(capsule).unwrap()[0];
                        let document = tree.children(viewport).unwrap()[0];
                        let viewport_bounds = tree.layout(viewport).unwrap();
                        let document_bounds = tree.layout(document).unwrap();
                        assert!(viewport_bounds.size.height > 0.0);
                        assert!(document_bounds.size.height > viewport_bounds.size.height);
                        let Widget::Container {
                            child: capsule_widget,
                            ..
                        } = &widget
                        else {
                            panic!()
                        };
                        let Widget::Container { child: scroll, .. } = &**capsule_widget else {
                            panic!()
                        };
                        let Widget::ScrollView {
                            child: document_widget,
                            ..
                        } = &**scroll
                        else {
                            panic!()
                        };
                        let Widget::Column { children, .. } = &**document_widget else {
                            panic!()
                        };
                        assert_eq!(children.len(), if expanded { 2 } else { 1 });
                        assert!(
                            matches!(&children[0], Widget::ButtonContent { label, .. } if label.contains("localized"))
                        );
                        // Scroll to the bottom: Close must remain reachable after a
                        // header taller than the entire viewport. Collapsed trees
                        // must contain no hidden action/Close accessibility nodes.
                        let offset = document_bounds.size.height - viewport_bounds.size.height;
                        let scrolls = HashMap::from([(
                            DOCUMENT,
                            rutter::engine::widget_state::WidgetState::Scroll(
                                rutter::engine::widget_state::ScrollState {
                                    offset_y: offset,
                                    content_height: document_bounds.size.height,
                                    viewport_h: viewport_bounds.size.height,
                                },
                            ),
                        )]);
                        let mut close_hits = 0;
                        let mut action_hits = 0;
                        for y in (0..height).step_by(4) {
                            for x in (0..width).step_by(4) {
                                match hit_test(
                                    &widget,
                                    &tree,
                                    root,
                                    Point::new(x as f32, y as f32),
                                    Point::default(),
                                    &scrolls,
                                ) {
                                    Some(HitResult::Message {
                                        msg: Message::Close,
                                        ..
                                    }) => close_hits += 1,
                                    Some(HitResult::Message {
                                        msg: Message::Action(..),
                                        ..
                                    }) => action_hits += 1,
                                    _ => {}
                                }
                            }
                        }
                        assert_eq!(close_hits > 0, expanded);
                        if !expanded {
                            assert_eq!(action_hits, 0);
                        }
                        if expanded && !dark && width == 320 && height == 120 && scale == 1.0 {
                            let mut reached_action = false;
                            for offset_y in
                                (0..document_bounds.size.height.ceil() as usize).step_by(24)
                            {
                                let states = HashMap::from([(
                                    DOCUMENT,
                                    rutter::engine::widget_state::WidgetState::Scroll(
                                        rutter::engine::widget_state::ScrollState {
                                            offset_y: (offset_y as f32).min(offset),
                                            content_height: document_bounds.size.height,
                                            viewport_h: viewport_bounds.size.height,
                                        },
                                    ),
                                )]);
                                for y in (0..height).step_by(8) {
                                    for x in (0..width).step_by(8) {
                                        let point = Point::new(x as f32, y as f32);
                                        if let Some(HitResult::Message {
                                            msg: Message::Action(target, _),
                                            ..
                                        }) = hit_test(
                                            &widget,
                                            &tree,
                                            root,
                                            point,
                                            Point::default(),
                                            &states,
                                        ) {
                                            assert_eq!(target, id);
                                            assert!(contains(&region, point));
                                            reached_action = true;
                                        }
                                    }
                                }
                            }
                            assert!(
                                reached_action,
                                "selected producer controls must remain reachable through scrolling"
                            );
                        }
                        let mut surface = rutter::skia_safe::surfaces::raster_n32_premul((
                            (width as f64 * scale) as i32,
                            (height as f64 * scale) as i32,
                        ))
                        .unwrap();
                        let canvas = surface.canvas();
                        canvas.clear(rutter::skia_safe::Color::TRANSPARENT);
                        canvas.scale((scale as f32, scale as f32));
                        rutter::render::draw_widgets(
                            canvas,
                            &tree,
                            root,
                            &widget,
                            &mut fonts.borrow_mut(),
                            &mut cosmic_text::SwashCache::new(),
                            Point::new(-100.0, -100.0),
                            None,
                            &HashMap::new(),
                            &scrolls,
                            &mut HashMap::new(),
                            &mut rutter::render::text::TextBufferCache::default(),
                            false,
                            &theme,
                            scale as f32,
                        );
                        let pixels = surface.peek_pixels().unwrap();
                        assert_ne!(
                            pixels.get_color((
                                (expected.center_x() as f64 * scale) as i32,
                                ((expected.top + 2.0) as f64 * scale) as i32
                            )),
                            rutter::skia_safe::Color::TRANSPARENT
                        );
                        // Long scrolled text cannot paint into the transparent host
                        // padding or below the bounded capsule at any raster scale.
                        for y in 0..(height as f64 * scale) as i32 {
                            for x in [0, (width as f64 * scale) as i32 - 1] {
                                assert_eq!(
                                    pixels.get_color((x, y)),
                                    rutter::skia_safe::Color::TRANSPARENT
                                );
                            }
                        }
                        let bottom = ((expected.bottom + 1.0) as f64 * scale).ceil() as i32;
                        for y in bottom..(height as f64 * scale) as i32 {
                            assert_eq!(
                                pixels.get_color(((expected.center_x() as f64 * scale) as i32, y)),
                                rutter::skia_safe::Color::TRANSPARENT
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn empty_overlay_keeps_a_header_and_expanded_controls() {
    let mut state = state();
    Host::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(420, 300, 1.0));
    for expanded in [false, true] {
        state.island.set_expanded(expanded).unwrap();
        let widget = Host::view(&mut state, SurfaceId::PRIMARY);
        rutter::WidgetIdSnapshot::capture(&widget).unwrap();
        let Widget::Container { child: capsule, .. } = widget else {
            panic!()
        };
        let Widget::Container { child: scroll, .. } = *capsule else {
            panic!()
        };
        let Widget::ScrollView {
            child: document, ..
        } = *scroll
        else {
            panic!()
        };
        let Widget::Column { children, .. } = *document else {
            panic!()
        };
        assert!(
            matches!(&children[0], Widget::ButtonContent { label, on_press: Message::ToggleDetails, .. } if label.contains("Activity host"))
        );
        assert_eq!(children.len(), if expanded { 2 } else { 1 });
        if expanded {
            let Widget::Column {
                children: details, ..
            } = &children[1]
            else {
                panic!()
            };
            assert!(matches!(
                &details[0],
                Widget::Button {
                    on_press: Message::ToggleTheme,
                    ..
                }
            ));
            assert!(matches!(
                details.last().unwrap(),
                Widget::Button {
                    on_press: Message::Close,
                    ..
                }
            ));
        }
    }
}
