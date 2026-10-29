use super::*;

fn summary_content<'a>(widget: &'a Widget<'_, Message>) -> &'a rutter::RichText<'a> {
    match widget {
        Widget::RichText { content, .. } => content,
        Widget::Disabled { child, .. }
        | Widget::Container { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::ButtonContent { child, .. } => summary_content(child),
        Widget::Column { children, .. } => summary_content(&children[0]),
        _ => panic!("expected summary composition"),
    }
}

#[test]
fn interactive_interruptions_sample_at_event_time_and_keep_both_axes_continuous() {
    use DynamicIslandSlide::*;
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    state.no_animation = false;
    state.enter = SlideInLeft;
    state.start_text_entry(now);
    let event = now + Duration::from_millis(93);
    let mut expected = state.clone();
    expected.sample_text(event);
    state.enter = SlideInUp;
    state.apply_at(Message::Expand, event);
    assert_eq!(state.text_offset(), expected.text_offset());
    assert!(state.text_offset().0 < 0.0);
    assert_eq!(state.text_offset().1, 0.0);
    assert_eq!(state.displayed_summary, None);
    let view = island_view(&state, state.summary(), &Theme::dark());
    assert_eq!(summary_content(&view).plain_text(), state.summary());
    assert_eq!(
        summary_content(&view).accessibility_label(),
        state.summary()
    );
    drop(view);

    let second = event + Duration::from_millis(65);
    let mut expected = state.clone();
    expected.sample_text(second);
    state.apply_at(Message::Collapse, second);
    assert_eq!(state.text_offset(), expected.text_offset());
    state.sample_text(second + Duration::from_millis(280));
    assert_eq!(state.text_offset(), (0.0, 0.0));
    assert!(!state.text_is_animating());
}

#[test]
fn passive_interruptions_coalesce_without_restarting_exit_and_publish_current_accessibility() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Passive, 320, 300);
    state.no_animation = false;
    state.enter = DynamicIslandSlide::SlideInLeft;
    state.exit = DynamicIslandSlide::SlideOutUp;
    state.start_text_entry(now);
    let event = now + Duration::from_millis(90);
    let mut expected = state.clone();
    expected.sample_text(event);
    let outgoing = state.summary();
    state.alternate = true;
    state.start_text_exit(outgoing, event);
    assert_eq!(state.text_offset(), expected.text_offset());
    assert!(state.text_offset().0 < 0.0);
    let boundary = state.text_exit_end.unwrap();
    let next = event + Duration::from_millis(100);
    let old = state.summary();
    state.alternate = false;
    state.start_text_exit(old, next);
    state.alternate = true;
    state.start_text_exit(outgoing, next + Duration::from_millis(10));
    assert_eq!(state.text_exit_end, Some(boundary));
    assert_eq!(state.displayed_summary, Some(outgoing));
    assert!(state.text_offset().0 < 0.0 && state.text_offset().1 < 0.0);
    let view = IslandDemo::view(&mut state, SurfaceId::PRIMARY);
    let content = summary_content(&view);
    assert_eq!(content.plain_text(), outgoing);
    assert_eq!(
        content.accessibility_label(),
        "Passive · Library up to date"
    );
    assert_eq!(
        content.clone().into_owned().accessibility_label(),
        "Passive · Library up to date"
    );
    drop(view);
    state.sample_text(boundary + Duration::from_millis(140));
    assert_eq!(state.displayed_summary, None);
    assert_eq!(state.text_offset(), (-0.5, 0.0));
    state.sample_text(boundary + Duration::from_millis(280));
    assert_eq!(state.text_offset(), (0.0, 0.0));
    assert!(!state.text_is_animating());
}

#[test]
fn late_wakeup_finishes_both_text_phases_without_scheduling_a_fresh_entry() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Passive, 320, 300);
    state.no_animation = false;
    let old = state.summary();
    state.alternate = true;
    state.start_text_exit(old, now);
    state.deadline = Some(now + INTERVAL);
    let late = now + Duration::from_millis(700);
    IslandDemo::wakeup(&mut state, late);
    assert_eq!(state.displayed_summary, None);
    assert_eq!(state.text_offset(), (0.0, 0.0));
    assert!(!state.text_is_animating());
    assert_eq!(state.frame, None);
    assert_eq!(IslandDemo::next_wakeup(&state, late), state.deadline);
}

#[test]
fn wrapped_summary_taller_than_44_is_fully_hidden_at_every_slide_endpoint() {
    use DynamicIslandSlide::*;
    const LONG: &str = "Playing — Details anzeigen — عرض التفاصيل — A deliberately long localized summary must remain fully hidden at the slide endpoint even when wrapping creates many lines. This text exceeds the old fixed forty-four pixel translation.";
    for theme in [Theme::light(), Theme::dark()] {
        for width in [320, 800] {
            let state = configured(DynamicIslandMode::Interactive, width, 300);
            let view = island_view(&state, LONG, &theme);
            let (tree, root) = layout(&view, width, 300);
            let text = bounds(&tree, root, &[0, 0, 0, 0, 0]);
            assert!(text.height() > 44.0);
            for scale in [1.0_f32, 1.5, 2.0] {
                for direction in [
                    SlideInLeft,
                    SlideInRight,
                    SlideInUp,
                    SlideInDown,
                    SlideOutLeft,
                    SlideOutRight,
                    SlideOutUp,
                    SlideOutDown,
                ] {
                    let mut hidden = state.clone();
                    let (x, y) =
                        slide_offset(direction, if direction.is_entry() { 0.0 } else { 1.0 });
                    hidden.text_x = Animation::new(x).unwrap();
                    hidden.text_y = Animation::new(y).unwrap();
                    let hidden_view = island_view(&hidden, LONG, &theme);
                    let mut raster = surfaces::raster_n32_premul((
                        (width as f32 * scale) as i32,
                        (300.0 * scale) as i32,
                    ))
                    .unwrap();
                    raster.canvas().clear(Color::TRANSPARENT);
                    raster.canvas().scale((scale, scale));
                    draw_widgets(
                        raster.canvas(),
                        &tree,
                        root,
                        &hidden_view,
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
                    let viewport = bounds(&tree, root, &[0, 0]);
                    for y in (text.top.max(viewport.top) * scale).ceil() as i32
                        ..(text.bottom.min(viewport.bottom) * scale).floor() as i32
                    {
                        for x in
                            (text.left * scale).ceil() as i32..(text.right * scale).floor() as i32
                        {
                            assert_eq!(
                                pixels.get_color((x, y)),
                                theme.surface,
                                "hidden wrapped text leaked: {direction:?}, width {width}, scale {scale}"
                            );
                        }
                    }
                }
            }
        }
    }
}
