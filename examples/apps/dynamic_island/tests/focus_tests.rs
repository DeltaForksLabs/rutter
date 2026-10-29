use super::*;
use rutter::SurfaceEvent;

fn policy(interactivity: LayerKeyboardInteractivity) -> SurfaceCommand {
    SurfaceCommand::SetLayerKeyboardInteractivity {
        surface: SurfaceId::PRIMARY,
        interactivity,
    }
}

#[test]
fn hover_requests_exclusive_once_and_escape_restores_on_demand() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    IslandDemo::surface_pointer_hovered(
        &mut state,
        SurfaceId::PRIMARY,
        Some(LogicalPointerPosition::new(160.0, 40.0)),
    );
    let commands = IslandDemo::wakeup(&mut state, now);
    assert!(commands.contains(&policy(LayerKeyboardInteractivity::Exclusive)));
    assert_eq!(state.focus_deadline, Some(now + FOCUS_GRANT_TIMEOUT));
    assert_eq!(IslandDemo::next_wakeup(&state, now), state.focus_deadline);
    assert!(state.apply_at(Message::Expand, now).is_empty());
    let ShortcutOutcome::Message(message) =
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape())
    else {
        panic!()
    };
    assert!(
        state
            .apply_at(message, now)
            .contains(&policy(LayerKeyboardInteractivity::OnDemand))
    );
    assert_eq!(state.focus_deadline, None);
    assert_eq!(IslandDemo::next_wakeup(&state, now), None);
    assert!(IslandDemo::wakeup(&mut state, now + FOCUS_GRANT_TIMEOUT).is_empty());
    assert_eq!(state.focus_fallback, None);
}

#[test]
fn grant_cancels_timeout_and_focus_loss_falls_back_without_regrab() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    state.apply_at(Message::Expand, now);
    assert!(
        IslandDemo::surface_event(
            &mut state,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(true)
        )
        .is_empty()
    );
    assert_eq!(state.focus_deadline, None);
    assert!(IslandDemo::wakeup(&mut state, now + FOCUS_GRANT_TIMEOUT).is_empty());
    assert_eq!(state.focus_fallback, None);
    assert_eq!(
        IslandDemo::surface_event(
            &mut state,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(false)
        ),
        [policy(LayerKeyboardInteractivity::OnDemand)]
    );
    assert_eq!(state.focus_fallback, Some(FocusFallbackReason::FocusLost));
    assert!(state.island.is_expanded());
    assert!(
        IslandDemo::surface_event(
            &mut state,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(false)
        )
        .is_empty()
    );
    assert!(state.apply_at(Message::Expand, now).is_empty());
    assert_eq!(IslandDemo::next_wakeup(&state, now), None);
}

#[test]
fn no_grant_invokes_application_fallback_once_and_later_click_focus_allows_escape() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 800, 300);
    state.apply_at(Message::Expand, now);
    assert!(
        IslandDemo::wakeup(
            &mut state,
            now + FOCUS_GRANT_TIMEOUT - Duration::from_nanos(1)
        )
        .is_empty()
    );
    assert_eq!(state.focus_fallback, None);
    assert_eq!(
        IslandDemo::wakeup(&mut state, now + FOCUS_GRANT_TIMEOUT),
        [policy(LayerKeyboardInteractivity::OnDemand)]
    );
    assert_eq!(
        state.focus_fallback,
        Some(FocusFallbackReason::GrantDeadlineElapsed)
    );
    assert!(state.island.is_expanded());
    assert_eq!(IslandDemo::next_wakeup(&state, now), None);
    assert!(IslandDemo::wakeup(&mut state, now + Duration::from_secs(10)).is_empty());
    // A real click's focus arrives as a native event; no synthesized key or grab.
    IslandDemo::surface_event(
        &mut state,
        SurfaceId::PRIMARY,
        SurfaceEvent::FocusChanged(true),
    );
    assert_eq!(state.keyboard_policy, LayerKeyboardInteractivity::OnDemand);
    let ShortcutOutcome::Message(message) =
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape())
    else {
        panic!()
    };
    state.apply_at(message, now);
    assert!(!state.island.is_expanded());
}

#[test]
fn close_cancels_focus_deadline_and_passive_and_other_surfaces_are_unaffected() {
    let now = Instant::now();
    for native_close in [false, true] {
        let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
        state.apply_at(Message::Expand, now);
        if native_close {
            IslandDemo::surface_closed(&mut state, SurfaceId::PRIMARY);
        } else {
            state.apply_at(Message::Close, now);
        }
        assert_eq!(state.focus_deadline, None);
        assert!(IslandDemo::wakeup(&mut state, now + FOCUS_GRANT_TIMEOUT).is_empty());
        assert!(
            IslandDemo::surface_event(
                &mut state,
                SurfaceId::PRIMARY,
                SurfaceEvent::FocusChanged(false)
            )
            .is_empty()
        );
        assert_eq!(state.focus_fallback, None);
    }
    let mut passive = configured(DynamicIslandMode::Passive, 320, 300);
    assert!(
        IslandDemo::surface_event(
            &mut passive,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(false)
        )
        .is_empty()
    );
    assert_eq!(passive.keyboard_policy, LayerKeyboardInteractivity::None);
    assert_eq!(passive.focus_deadline, None);
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    state.apply_at(Message::Expand, now);
    IslandDemo::surface_event(
        &mut state,
        SurfaceId::new(2),
        SurfaceEvent::FocusChanged(true),
    );
    assert_eq!(state.focus_deadline, Some(now + FOCUS_GRANT_TIMEOUT));
}
