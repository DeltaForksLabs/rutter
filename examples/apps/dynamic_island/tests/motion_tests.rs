use super::*;

#[test]
fn interactive_header_immediately_uses_current_semantic_and_visual_text() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    state.no_animation = false;
    state.apply_at(Message::Expand, now);
    assert_eq!(state.displayed_summary, None);
    assert!(state.text_slide.is_entry());
    let view = island_view(&state, state.summary(), &Theme::dark());
    let Widget::Container { child, .. } = &view else {
        panic!()
    };
    let Widget::Container { child, .. } = child.as_ref() else {
        panic!()
    };
    let Widget::ScrollView { child, .. } = child.as_ref() else {
        panic!()
    };
    let Widget::Column { children, .. } = child.as_ref() else {
        panic!()
    };
    let Widget::ButtonContent { label, child, .. } = &children[0] else {
        panic!()
    };
    assert_eq!(*label, state.summary());
    let Widget::RichText { content, .. } = child.as_ref() else {
        panic!()
    };
    assert_eq!(content.plain_text(), state.summary());
}

#[test]
fn all_text_directions_and_passive_changes_keep_capsule_stationary() {
    use DynamicIslandSlide::*;
    for (slide, expected) in [
        (SlideInLeft, (-1, 0)),
        (SlideInRight, (1, 0)),
        (SlideInUp, (0, 1)),
        (SlideInDown, (0, -1)),
        (SlideOutLeft, (-1, 0)),
        (SlideOutRight, (1, 0)),
        (SlideOutUp, (0, -1)),
        (SlideOutDown, (0, 1)),
    ] {
        let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
        let bounds = capsule_bounds(&state);
        state.text_slide = slide;
        let (x, y) = slide_offset(slide, 0.5);
        state.text_x = Animation::new(x).unwrap();
        state.text_y = Animation::new(y).unwrap();
        let offset = state.text_offset();
        assert_eq!(
            (
                offset.0.signum() as i32 * i32::from(offset.0 != 0.0),
                offset.1.signum() as i32 * i32::from(offset.1 != 0.0)
            ),
            expected
        );
        assert_eq!(capsule_bounds(&state), bounds);
    }
    let mut passive = configured(DynamicIslandMode::Passive, 320, 300);
    passive.no_animation = false;
    let original = passive.summary();
    let bounds = capsule_bounds(&passive);
    let deadline = passive.deadline.unwrap();
    IslandDemo::wakeup(&mut passive, deadline);
    assert_ne!(original, passive.summary());
    assert_eq!(passive.displayed_summary, Some(original));
    assert!(passive.text_is_animating());
    assert_eq!(capsule_bounds(&passive), bounds);
    IslandDemo::wakeup(&mut passive, deadline + Duration::from_millis(280));
    assert!(passive.text_slide.is_entry());
    assert_eq!(passive.displayed_summary, None);
    assert_eq!(capsule_bounds(&passive), bounds);
}

#[test]
fn text_slide_changes_only_clipped_pixels_and_preserves_header_hit_and_region() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    state.no_animation = false;
    let region = state.publish_region();
    let bounds_before = capsule_bounds(&state);
    let render = |state: &Model| {
        let view = island_view(state, state.summary(), &Theme::dark());
        let (tree, root) = layout(&view, 320, 300);
        let text = bounds(&tree, root, &[0, 0, 0, 0, 0]);
        let header = bounds(&tree, root, &[0, 0, 0, 0]);
        assert!(matches!(
            hit_test(
                &view,
                &tree,
                root,
                Point::new(header.center_x(), header.center_y()),
                Point::default(),
                &HashMap::new()
            ),
            Some(HitResult::Message {
                msg: Message::Expand,
                ..
            })
        ));
        let mut raster = surfaces::raster_n32_premul((320, 300)).unwrap();
        raster.canvas().clear(Color::TRANSPARENT);
        draw_widgets(
            raster.canvas(),
            &tree,
            root,
            &view,
            &mut FontSystem::new(),
            &mut SwashCache::new(),
            Point::new(-1.0, -1.0),
            None,
            &HashMap::new(),
            &HashMap::new(),
            &mut HashMap::new(),
            &mut TextBufferCache::default(),
            true,
            &Theme::dark(),
            1.0,
        );
        let pixels = raster.peek_pixels().unwrap();
        let colors: Vec<_> = (0..300)
            .flat_map(|y| (0..320).map(move |x| (x, y)))
            .map(|p| pixels.get_color(p))
            .collect();
        (text, colors)
    };
    let (text, before) = render(&state);
    state.start_text_entry(now);
    state.sample_text(now + Duration::from_millis(140));
    let (shifted_text, after) = render(&state);
    assert_eq!(text, shifted_text);
    assert_eq!(capsule_bounds(&state), bounds_before);
    assert_eq!(state.publish_region(), region);
    let mut changed = 0;
    for (i, (a, b)) in before.iter().zip(&after).enumerate() {
        if a != b {
            changed += 1;
            let (x, y) = ((i % 320) as f32, (i / 320) as f32);
            assert!(
                x >= text.left.floor()
                    && x < text.right.ceil()
                    && y >= text.top.floor()
                    && y < text.bottom.ceil()
            );
        }
    }
    assert!(changed > 0);
}

fn animated(now: Instant, width: u32, height: u32) -> Model {
    let mut state = Model::new(DynamicIslandMode::Interactive, now);
    IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(width, height));
    IslandDemo::wakeup(&mut state, now);
    state
}

#[test]
fn compact_expansion_and_contraction_keep_both_axes_centered_and_input_sampled() {
    for (width, height) in [(320, 300), (800, 300), (320, 110), (800, 110)] {
        for scale in [1.0, 1.5, 2.0] {
            let now = Instant::now();
            let mut state = configured(DynamicIslandMode::Interactive, width, height);
            state.metrics.as_mut().unwrap().scale_factor = scale;
            state.no_animation = false;
            let compact = capsule_bounds(&state).unwrap();
            assert_eq!(compact.width(), 240.0);
            state.apply_at(Message::Expand, now);
            let expanded = resting_bounds(&state).unwrap();
            assert!(expanded.width() > compact.width());
            assert!(expanded.height() > compact.height());
            for (start, from, to, message) in [
                (now, compact, expanded, Message::Expand),
                (
                    now + Duration::from_millis(240),
                    expanded,
                    compact,
                    Message::Collapse,
                ),
            ] {
                state.apply_at(message, start);
                let half = start + Duration::from_millis(120);
                let commands = IslandDemo::wakeup(&mut state, half);
                let sampled = capsule_bounds(&state).unwrap();
                assert_eq!(sampled.width(), (from.width() + to.width()) / 2.0);
                assert_eq!(sampled.height(), (from.height() + to.height()) / 2.0);
                assert!((sampled.center_x() - width as f32 / 2.0).abs() <= 1.0);
                assert_eq!(sampled.top, spacing::SM);
                assert_eq!(
                    published(&commands),
                    &state
                        .island
                        .input_region(sampled, DynamicIsland::RADIUS)
                        .unwrap()
                );
                let view = island_view(&state, state.summary(), &Theme::dark());
                let (tree, root) = layout(&view, width, height);
                let actual = bounds(&tree, root, &[0]);
                assert!((actual.width() - sampled.width()).abs() <= 1.0);
                assert!((actual.height() - sampled.height()).abs() <= 1.0);
                assert!((actual.center_x() - width as f32 / 2.0).abs() <= 1.0);
                let header = bounds(&tree, root, &[0, 0, 0, 0]);
                assert!(matches!(
                    hit_test(
                        &view,
                        &tree,
                        root,
                        Point::new(header.center_x(), header.top + 12.0),
                        Point::default(),
                        &HashMap::new()
                    ),
                    Some(HitResult::Message {
                        msg: Message::Expand,
                        ..
                    })
                ));
                drop(view);
                IslandDemo::wakeup(&mut state, start + Duration::from_millis(240));
                assert_eq!(capsule_bounds(&state), Some(to));
            }
        }
    }
}

fn assert_action_composition(state: &Model, clicks: usize) {
    // The example cannot call crate-private accessibility/focus traversal. Check
    // its composition boundary here; library tests exercise the engine's actual
    // disabled/hidden focus and accessibility behavior for these same widgets.
    let view = island_view(state, state.summary(), &Theme::dark());
    fn active_buttons(view: &Widget<'_, Message>) -> usize {
        match view {
            Widget::Disabled { .. } => 0,
            Widget::Button { .. } | Widget::ButtonContent { .. } => 1,
            Widget::Column { children, .. } => children.iter().map(active_buttons).sum(),
            Widget::Container { child, .. } | Widget::ScrollView { child, .. } => {
                active_buttons(child)
            }
            Widget::RichText { .. } => 0,
            _ => panic!("unexpected demo composition"),
        }
    }
    assert_eq!(active_buttons(&view), clicks);
}

#[test]
fn cli_accepts_all_directions_and_rejects_malformed_or_misplaced_flags() {
    for entry in ["SlideInLeft", "SlideInRight", "SlideInUp", "SlideInDown"] {
        for exit in [
            "SlideOutLeft",
            "SlideOutRight",
            "SlideOutUp",
            "SlideOutDown",
        ] {
            let options = parse_args(
                [
                    "interactive",
                    "DP-2",
                    "--exit",
                    exit,
                    "--enter",
                    entry,
                    "--no-animation",
                ]
                .map(OsString::from),
            )
            .unwrap();
            assert!(options.enter.is_entry() && !options.exit.is_entry() && options.no_animation);
        }
    }
    for args in [
        vec!["--enter"],
        vec!["--exit", "SlideInUp"],
        vec!["--enter", "SlideOutDown"],
        vec!["--enter", "slideindown"],
        vec!["--no-animation", "DP-2"],
        vec!["--no-animation", "--no-animation"],
        vec!["--enter", "SlideInUp", "--enter", "SlideInDown"],
        vec!["--no-animation=true"],
        vec!["--exit", "--no-animation"],
    ] {
        assert!(
            parse_args(
                std::iter::once("interactive")
                    .chain(args)
                    .map(OsString::from)
            )
            .is_err()
        );
    }
}

#[test]
fn wake_clock_drives_queued_reversal_late_frames_idle_and_exit_safety() {
    let now = Instant::now();
    let mut state = animated(now, 320, 300);
    assert_eq!(capsule_bounds(&state), resting_bounds(&state));
    assert_eq!(IslandDemo::next_wakeup(&state, now), Some(now + FRAME));
    let late = now + Duration::from_millis(75);
    IslandDemo::wakeup(&mut state, late);
    assert_eq!(
        IslandDemo::next_wakeup(&state, late),
        Some(now + Duration::from_millis(80))
    );
    let before_action = capsule_bounds(&state).unwrap();
    let action_time = now + Duration::from_millis(77);
    state.queued.push(Message::PauseResume);
    IslandDemo::wakeup(&mut state, action_time);
    assert_eq!(capsule_bounds(&state).unwrap(), before_action);
    assert_eq!(
        IslandDemo::next_wakeup(&state, action_time),
        Some(now + Duration::from_millis(80))
    );
    let settled = now + Duration::from_secs(1);
    IslandDemo::wakeup(&mut state, settled);
    assert!(!state.text_is_animating());
    assert_eq!(IslandDemo::next_wakeup(&state, settled), None);
    state.queued.push(Message::Expand);
    assert_eq!(IslandDemo::next_wakeup(&state, settled), Some(settled));
    IslandDemo::wakeup(&mut state, settled);
    assert!(state.island.is_expanded());
    assert_action_composition(&state, 3);
    let half = settled + Duration::from_millis(120);
    IslandDemo::wakeup(&mut state, half);
    let midpoint = capsule_bounds(&state).unwrap();
    assert_eq!(midpoint.height(), 164.0);
    state.queued.push(Message::Collapse);
    IslandDemo::wakeup(&mut state, half);
    assert!(!state.island.is_expanded());
    assert_eq!(capsule_bounds(&state), Some(midpoint));
    assert_action_composition(&state, 1); // details disappear before the body shrinks
    state.queued.push(Message::Expand);
    IslandDemo::wakeup(&mut state, half);
    assert!(state.island.is_expanded()); // expansion reverses collapse during motion
    state.queued.push(Message::Close);
    let exit = IslandDemo::wakeup(&mut state, half);
    assert!(exit.contains(&SurfaceCommand::Close(SurfaceId::PRIMARY)));
    assert_eq!(published(&exit[..2]), &SurfaceInputRegion::Empty);
    assert_action_composition(&state, 0);
    assert_eq!(
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape()),
        ShortcutOutcome::Ignored
    );
    assert!(state.apply_at(Message::PauseResume, half).is_empty());
    let view = island_view(&state, state.summary(), &Theme::dark());
    let (tree, root) = layout(&view, 320, 300);
    assert!(
        hit_test(
            &view,
            &tree,
            root,
            Point::new(160.0, 40.0),
            Point::default(),
            &HashMap::new()
        )
        .is_none()
    );
    drop(view);
    let complete = IslandDemo::wakeup(&mut state, half + Duration::from_secs(10));
    assert!(
        complete.is_empty(),
        "close has no pending exit phase or commands"
    );
    assert!(capsule_bounds(&state).is_none());
    assert_action_composition(&state, 0);
    assert_eq!(
        IslandDemo::next_wakeup(&state, half + Duration::from_secs(10)),
        None
    );
}

#[test]
fn metrics_during_motion_reconcile_and_zero_duration_never_schedules_frames() {
    let now = Instant::now();
    let mut state = animated(now, 420, 300);
    IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(320, 110));
    let halfway = now + Duration::from_millis(140);
    IslandDemo::wakeup(&mut state, halfway);
    // Resize maps the current sample into the actual host before converging on
    // its new inset target; the capsule must fit the host throughout.
    assert!(capsule_bounds(&state).unwrap().width() <= 320.0);
    IslandDemo::wakeup(&mut state, now + Duration::from_secs(1));
    assert_eq!(capsule_bounds(&state), resting_bounds(&state));
    assert_eq!(IslandDemo::next_wakeup(&state, now), None);
    for (width, height) in [(20, 20), (20000, 300), (320, 0)] {
        IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(width, height));
        assert_eq!(
            published(&IslandDemo::wakeup(&mut state, now)),
            &SurfaceInputRegion::Empty
        );
        assert!(capsule_bounds(&state).is_none());
        assert_action_composition(&state, 0);
    }
    let mut immediate = configured(DynamicIslandMode::Interactive, 320, 110);
    assert_eq!(IslandDemo::next_wakeup(&immediate, now), None);
    immediate.apply_at(Message::Expand, now);
    assert_eq!(capsule_bounds(&immediate), resting_bounds(&immediate));
    assert_eq!(immediate.frame, None);
    assert_eq!(
        IslandDemo::next_wakeup(&immediate, now),
        Some(now + FOCUS_GRANT_TIMEOUT)
    );
    let commands = immediate.apply_at(Message::Close, now);
    assert!(commands.contains(&SurfaceCommand::Close(SurfaceId::PRIMARY)));
    assert!(capsule_bounds(&immediate).is_none());
    assert_action_composition(&immediate, 0);
}

#[test]
fn short_host_resize_keeps_compact_body_layout_paint_and_native_input_together() {
    use DynamicIslandSlide::*;
    for slide in [SlideInLeft, SlideInRight, SlideInUp, SlideInDown] {
        for theme in [Theme::light(), Theme::dark()] {
            let now = Instant::now();
            let mut state = configured(DynamicIslandMode::Interactive, 420, 300);
            state.no_animation = false;
            state
                .motion
                .as_mut()
                .unwrap()
                .slide_at(slide, Duration::from_millis(280), now);
            IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(320, 110));
            let commands = IslandDemo::wakeup(&mut state, now + Duration::from_millis(140));
            let sampled = capsule_bounds(&state).unwrap();
            assert_eq!(sampled.height(), 76.0);
            assert_eq!(
                published(&commands),
                &state
                    .island
                    .input_region(sampled, DynamicIsland::RADIUS)
                    .unwrap()
            );
            let view = island_view(&state, state.summary(), &theme);
            let (tree, root) = layout(&view, 320, 110);
            let actual = bounds(&tree, root, &[0]);
            for (painted, stored) in [
                (actual.left, sampled.left),
                (actual.top, sampled.top),
                (actual.right, sampled.right),
                (actual.bottom, sampled.bottom),
            ] {
                assert!((painted - stored).abs() <= 1.0);
            }
            let viewport = bounds(&tree, root, &[0, 0]);
            let header = bounds(&tree, root, &[0, 0, 0, 0]);
            assert_eq!(viewport.height(), 44.0);
            assert_eq!(header.height(), 44.0);
            let point = Point::new(header.center_x(), header.center_y());
            if (0.0..320.0).contains(&point.x) && (0.0..110.0).contains(&point.y) {
                assert!(matches!(
                    hit_test(&view, &tree, root, point, Point::default(), &HashMap::new()),
                    Some(HitResult::Message {
                        msg: Message::Expand,
                        ..
                    })
                ));
            }
            let mut raster = surfaces::raster_n32_premul((320, 110)).unwrap();
            raster.canvas().clear(Color::TRANSPARENT);
            draw_widgets(
                raster.canvas(),
                &tree,
                root,
                &view,
                &mut FontSystem::new(),
                &mut SwashCache::new(),
                Point::new(-1.0, -1.0),
                None,
                &HashMap::new(),
                &HashMap::new(),
                &mut HashMap::new(),
                &mut TextBufferCache::default(),
                true,
                &theme,
                1.0,
            );
            let pixels = raster.peek_pixels().unwrap();
            let x = actual.center_x().clamp(1.0, 318.0) as i32;
            let y = if actual.top < 0.0 {
                actual.bottom - 4.0
            } else {
                actual.top + 4.0
            };
            assert_eq!(pixels.get_color((x, y as i32)), theme.surface);
            if actual.bottom + 2.0 < 110.0 {
                assert_eq!(pixels.get_color((x, (actual.bottom + 2.0) as i32)).a(), 0);
            }
            drop(view);
            IslandDemo::wakeup(&mut state, now + Duration::from_millis(280));
            assert_eq!(capsule_bounds(&state), resting_bounds(&state));
            assert_eq!(IslandDemo::next_wakeup(&state, now), None);
        }
    }
}

#[test]
fn translated_layout_paint_and_original_rounded_input_agree_across_host_matrix() {
    use DynamicIslandSlide::*;
    for (width, height) in [(320, 300), (800, 300), (320, 110), (800, 110)] {
        for theme in [Theme::light(), Theme::dark()] {
            for slide in [
                SlideInLeft,
                SlideInRight,
                SlideInUp,
                SlideInDown,
                SlideOutLeft,
                SlideOutRight,
                SlideOutUp,
                SlideOutDown,
            ] {
                let now = Instant::now();
                let mut state = configured(DynamicIslandMode::Interactive, width, height);
                state
                    .motion
                    .as_mut()
                    .unwrap()
                    .slide_at(slide, Duration::from_millis(280), now);
                state
                    .motion
                    .as_mut()
                    .unwrap()
                    .sample_at(now + Duration::from_millis(140));
                let sampled = capsule_bounds(&state).unwrap();
                assert_eq!(
                    published(&state.publish_region()),
                    &state
                        .island
                        .input_region(sampled, DynamicIsland::RADIUS)
                        .unwrap()
                );
                let view = island_view(&state, state.summary(), &theme);
                let (tree, root) = layout(&view, width, height);
                let actual = bounds(&tree, root, &[0]);
                for (a, b) in [
                    (actual.left, sampled.left),
                    (actual.top, sampled.top),
                    (actual.right, sampled.right),
                    (actual.bottom, sampled.bottom),
                ] {
                    assert!((a - b).abs() <= 1.0, "layout must follow stored sample");
                }
                let header = bounds(&tree, root, &[0, 0, 0, 0]);
                let point = Point::new(header.center_x(), header.center_y());
                if point.x >= 0.0
                    && point.x < width as f32
                    && point.y >= 0.0
                    && point.y < height as f32
                {
                    assert!(matches!(
                        hit_test(&view, &tree, root, point, Point::default(), &HashMap::new()),
                        Some(HitResult::Message {
                            msg: Message::Expand,
                            ..
                        })
                    ));
                }
                for scale in [1.0_f32, 1.5, 2.0] {
                    let mut raster = surfaces::raster_n32_premul((
                        (width as f32 * scale) as i32,
                        (height as f32 * scale) as i32,
                    ))
                    .unwrap();
                    raster.canvas().clear(Color::TRANSPARENT);
                    raster.canvas().scale((scale, scale));
                    draw_widgets(
                        raster.canvas(),
                        &tree,
                        root,
                        &view,
                        &mut FontSystem::new(),
                        &mut SwashCache::new(),
                        Point::new(-1.0, -1.0),
                        None,
                        &HashMap::new(),
                        &HashMap::new(),
                        &mut HashMap::new(),
                        &mut TextBufferCache::default(),
                        true,
                        &theme,
                        scale,
                    );
                    let pixels = raster.peek_pixels().unwrap();
                    // Surface edges clip the ORIGINAL translated shape. Its center
                    // is visible at each midpoint, even on the short host.
                    let x = (actual.center_x().clamp(1.0, width as f32 - 2.0) * scale) as i32;
                    let padding_y = if actual.top < 0.0 {
                        actual.bottom - 4.0
                    } else {
                        actual.top + 4.0
                    };
                    let y = (padding_y * scale) as i32;
                    assert_eq!(
                        pixels.get_color((x, y)),
                        theme.surface,
                        "{slide:?} at {width}x{height}, scale {scale}"
                    );
                    let outside_x = if actual.right < width as f32 - 2.0 {
                        actual.right + 2.0
                    } else {
                        actual.left - 2.0
                    };
                    if outside_x >= 0.0 && outside_x < width as f32 {
                        assert_eq!(pixels.get_color(((outside_x * scale) as i32, y)).a(), 0);
                    }
                }
            }
        }
    }
}
