use super::*;

fn report(state: &mut Model, position: Option<(f32, f32)>) -> bool {
    let response = IslandDemo::surface_pointer_hovered(
        state,
        SurfaceId::PRIMARY,
        position.map(|(x, y)| LogicalPointerPosition::new(x, y)),
    );
    if let Some(commands) = &response {
        assert!(commands.is_empty());
    }
    response.is_some()
}

#[test]
fn fresh_hover_expands_repeat_does_not_restart_and_escape_requires_reentry() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    state.no_animation = false;
    assert!(report(&mut state, Some((160.0, 40.0))));
    assert_eq!(state.queued, [Message::Expand]);
    IslandDemo::wakeup(&mut state, now);
    assert!(state.island.is_expanded());
    let half = now + Duration::from_millis(120);
    IslandDemo::wakeup(&mut state, half);
    let offset = state.text_offset();
    let bounds = capsule_bounds(&state);
    assert!(!report(&mut state, Some((161.0, 41.0))));
    assert!(state.queued.is_empty());
    assert!(state.apply_at(Message::Expand, half).is_empty());
    assert_eq!(state.text_offset(), offset);
    assert_eq!(capsule_bounds(&state), bounds);
    IslandDemo::wakeup(&mut state, now + Duration::from_millis(300));
    // Header-to-details movement remains inside the one rounded capsule.
    assert!(!report(&mut state, Some((160.0, 200.0))));
    assert!(state.pointer_inside && state.queued.is_empty());
    let ShortcutOutcome::Message(collapse) =
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape())
    else {
        panic!()
    };
    state.apply_at(collapse, now + Duration::from_millis(300));
    assert!(!report(&mut state, Some((160.0, 40.0))));
    assert!(state.queued.is_empty());
    assert!(!state.island.is_expanded());
    assert!(report(&mut state, None));
    assert!(!report(&mut state, None));
    assert!(!state.pointer_inside && !state.island.is_expanded());
    assert!(report(&mut state, Some((160.0, 40.0))));
    IslandDemo::wakeup(&mut state, now + Duration::from_millis(320));
    assert!(state.island.is_expanded());
    assert!(report(&mut state, None));
    assert!(state.island.is_expanded(), "leave only clears membership");
}

#[test]
fn hover_excludes_corners_other_surfaces_and_passive_and_uses_current_bands() {
    for scale in [1.0, 1.5, 2.0] {
        let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
        state.metrics.as_mut().unwrap().scale_factor = scale;
        let bounds = capsule_bounds(&state).unwrap();
        assert!(!report(&mut state, Some((bounds.left, bounds.top))));
        assert!(!state.pointer_inside && state.queued.is_empty());
        assert_eq!(
            IslandDemo::surface_pointer_hovered(
                &mut state,
                SurfaceId::new(9),
                Some(LogicalPointerPosition::new(160.0, 40.0)),
            ),
            None
        );
        assert!(!state.pointer_inside && state.queued.is_empty());
        assert!(report(&mut state, Some((160.0, 40.0))));
        assert_eq!(state.queued, [Message::Expand]);
        let SurfaceInputRegion::Rectangles(rects) = state.input_region() else {
            panic!()
        };
        assert!(rects.as_slice().len() <= 256);
    }
    let mut passive = configured(DynamicIslandMode::Passive, 320, 300);
    let deadline = passive.deadline;
    assert!(!report(&mut passive, Some((160.0, 40.0))));
    assert!(!report(&mut passive, None));
    assert!(!passive.pointer_inside && passive.queued.is_empty());
    assert_eq!(passive.deadline, deadline);
}

#[test]
fn interactive_changes_ignore_exit_flags_and_close_during_entry_without_pending_exit() {
    let now = Instant::now();
    for enter in [
        DynamicIslandSlide::SlideInLeft,
        DynamicIslandSlide::SlideInRight,
        DynamicIslandSlide::SlideInUp,
        DynamicIslandSlide::SlideInDown,
    ] {
        let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
        state.no_animation = false;
        state.enter = enter;
        state.exit = DynamicIslandSlide::SlideOutRight;
        for message in [Message::Expand, Message::PauseResume, Message::Collapse] {
            state.apply_at(message, now);
            assert_eq!(state.text_slide, enter);
            assert_eq!(state.displayed_summary, None);
            assert!(state.text_is_animating());
        }
        let commands = state.apply_at(Message::Close, now);
        assert_eq!(published(&commands[..2]), &SurfaceInputRegion::Empty);
        assert_eq!(
            commands.last(),
            Some(&SurfaceCommand::Close(SurfaceId::PRIMARY))
        );
        assert!(!state.text_is_animating());
        assert!(state.motion.is_none() && state.frame.is_none());
        assert_eq!(IslandDemo::next_wakeup(&state, now), None);
    }
}
