use super::*;
use rutter::{
    ShortcutEvent, ShortcutKey, ShortcutNamedKey, ShortcutOutcome,
    activity::{Transfer, TransferDirection, TransferState},
};

fn fixture() -> (State, ActivityId) {
    let mut state = State {
        overlay: Some(Overlay::default()),
        ..State::default()
    };
    let owner = state.registry.register_owner().unwrap();
    let id = state
        .registry
        .start(
            owner,
            Transfer::new(
                "Focus test",
                TransferDirection::Download,
                0,
                Some(100),
                TransferState::Running,
            )
            .unwrap(),
        )
        .unwrap();
    (state, id)
}

fn apply(state: &mut State, message: Message, now: Instant) -> Vec<SurfaceCommand> {
    state.apply_at(SurfaceId::PRIMARY, message, now, |_, _| Ok(()))
}

fn keyboard(commands: &[SurfaceCommand]) -> Vec<LayerKeyboardInteractivity> {
    commands
        .iter()
        .filter_map(|command| match command {
            SurfaceCommand::SetLayerKeyboardInteractivity {
                surface: SurfaceId::PRIMARY,
                interactivity,
            } => Some(*interactivity),
            _ => None,
        })
        .collect()
}

fn deadline(state: &State) -> Option<Instant> {
    state.overlay.as_ref().unwrap().focus.deadline
}

#[test]
fn toggle_and_selection_request_once_per_user_expansion_without_deadline_restart() {
    let now = Instant::now();
    for select in [false, true] {
        let (mut state, id) = fixture();
        let message = if select {
            Message::Select(id)
        } else {
            Message::ToggleDetails
        };
        assert_eq!(
            keyboard(&apply(&mut state, message, now)),
            [LayerKeyboardInteractivity::Exclusive]
        );
        assert_eq!(deadline(&state), Some(now + GRANT_TIMEOUT));
        for message in [
            Message::Select(id),
            Message::Action(id, ActivityAction::Pause),
            Message::ToggleTheme,
        ] {
            assert!(
                keyboard(&apply(
                    &mut state,
                    message,
                    now + Duration::from_millis(400)
                ))
                .is_empty()
            );
            assert_eq!(deadline(&state), Some(now + GRANT_TIMEOUT));
        }
        assert_eq!(
            keyboard(&apply(&mut state, Message::Collapse, now)),
            [LayerKeyboardInteractivity::OnDemand]
        );
        assert_eq!(deadline(&state), None);
        assert_eq!(
            keyboard(&apply(&mut state, Message::ToggleDetails, now)),
            [LayerKeyboardInteractivity::Exclusive]
        );
    }
}

#[test]
fn grant_cancels_deadline_and_focus_loss_falls_back_without_collapsing_or_regrab() {
    let now = Instant::now();
    let (mut state, id) = fixture();
    apply(&mut state, Message::ToggleDetails, now);
    assert!(
        Host::surface_event(
            &mut state,
            SurfaceId::new(42),
            SurfaceEvent::FocusChanged(true)
        )
        .is_empty()
    );
    assert_eq!(deadline(&state), Some(now + GRANT_TIMEOUT));
    assert!(
        Host::surface_event(
            &mut state,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(true)
        )
        .is_empty()
    );
    assert_eq!(deadline(&state), None);
    assert_eq!(Host::next_wakeup(&state, now), None);
    assert!(Host::wakeup(&mut state, now + GRANT_TIMEOUT).is_empty());
    assert_eq!(
        keyboard(&Host::surface_event(
            &mut state,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(false)
        )),
        [LayerKeyboardInteractivity::OnDemand]
    );
    assert!(state.island.is_expanded());
    assert!(
        Host::surface_event(
            &mut state,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(false)
        )
        .is_empty()
    );
    for message in [
        Message::Select(id),
        Message::ToggleTheme,
        Message::Action(id, ActivityAction::Pause),
    ] {
        assert!(apply(&mut state, message, now + GRANT_TIMEOUT).is_empty());
        assert_eq!(deadline(&state), None);
    }
}

#[test]
fn timeout_fallback_is_once_only_and_wakeup_orders_timeout_before_dirty_region() {
    let now = Instant::now();
    let (mut state, id) = fixture();
    apply(&mut state, Message::Select(id), now);
    assert_eq!(Host::next_wakeup(&state, now), Some(now + GRANT_TIMEOUT));
    assert!(Host::wakeup(&mut state, now + GRANT_TIMEOUT - Duration::from_millis(1)).is_empty());
    Host::surface_created(&mut state, SurfaceId::PRIMARY);
    assert_eq!(Host::next_wakeup(&state, now), Some(now));
    let commands = Host::wakeup(&mut state, now + GRANT_TIMEOUT);
    assert!(matches!(
        commands.as_slice(),
        [
            SurfaceCommand::SetLayerKeyboardInteractivity {
                interactivity: LayerKeyboardInteractivity::OnDemand,
                ..
            },
            SurfaceCommand::SetInputRegion { .. },
            SurfaceCommand::RequestRedraw(_)
        ]
    ));
    assert!(state.island.is_expanded());
    assert_eq!(deadline(&state), None);
    assert_eq!(Host::next_wakeup(&state, now), None);
    assert!(Host::wakeup(&mut state, now + GRANT_TIMEOUT * 2).is_empty());
    assert!(apply(&mut state, Message::Select(id), now + GRANT_TIMEOUT).is_empty());
    assert_eq!(deadline(&state), None);
}

#[test]
fn already_focused_expansion_has_no_deadline_and_escape_is_surface_local() {
    let now = Instant::now();
    let (mut state, _) = fixture();
    Host::surface_event(
        &mut state,
        SurfaceId::PRIMARY,
        SurfaceEvent::FocusChanged(true),
    );
    assert_eq!(
        keyboard(&apply(&mut state, Message::ToggleDetails, now)),
        [LayerKeyboardInteractivity::Exclusive]
    );
    assert_eq!(deadline(&state), None);
    let escape = ShortcutEvent {
        key: ShortcutKey::Named(ShortcutNamedKey::Escape),
        control: false,
        alt: false,
        shift: false,
        super_key: false,
        repeat: false,
    };
    assert!(matches!(
        Host::shortcut(&state, SurfaceId::new(42), escape.clone()),
        ShortcutOutcome::Ignored
    ));
    assert!(
        state
            .apply_at(
                SurfaceId::new(42),
                Message::Collapse,
                now,
                |_, _| unreachable!()
            )
            .is_empty()
    );
    assert!(state.island.is_expanded());
    let ShortcutOutcome::Message(message) = Host::shortcut(&state, SurfaceId::PRIMARY, escape)
    else {
        panic!()
    };
    assert_eq!(
        keyboard(&apply(&mut state, message, now)),
        [LayerKeyboardInteractivity::OnDemand]
    );
    assert!(!state.island.is_expanded());
    let mut ordinary = State::default();
    assert!(apply(&mut ordinary, Message::ToggleDetails, now).is_empty());
    assert!(
        Host::surface_event(
            &mut ordinary,
            SurfaceId::PRIMARY,
            SurfaceEvent::FocusChanged(false)
        )
        .is_empty()
    );
    assert_eq!(Host::next_wakeup(&ordinary, now), None);
}

#[test]
fn explicit_close_releases_exclusive_before_empty_and_close_and_cancels_pending() {
    let now = Instant::now();
    for explicit in [false, true] {
        let (mut state, _) = fixture();
        apply(&mut state, Message::ToggleDetails, now);
        Host::surface_created(&mut state, SurfaceId::PRIMARY);
        if explicit {
            assert!(matches!(
                apply(&mut state, Message::Close, now).as_slice(),
                [
                    SurfaceCommand::SetLayerKeyboardInteractivity {
                        interactivity: LayerKeyboardInteractivity::OnDemand,
                        ..
                    },
                    SurfaceCommand::SetInputRegion {
                        region: SurfaceInputRegion::Empty,
                        ..
                    },
                    SurfaceCommand::Close(_)
                ]
            ));
        } else {
            Host::surface_closed(&mut state, SurfaceId::PRIMARY);
        }
        assert_eq!(deadline(&state), None);
        assert_eq!(Host::next_wakeup(&state, now), None);
        assert!(Host::wakeup(&mut state, now + GRANT_TIMEOUT).is_empty());
        assert!(
            Host::surface_event(
                &mut state,
                SurfaceId::PRIMARY,
                SurfaceEvent::FocusChanged(false)
            )
            .is_empty()
        );
    }
}
