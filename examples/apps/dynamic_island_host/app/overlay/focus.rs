//! One bounded focus request per user expansion, with click-focus fallback.
use std::time::{Duration, Instant};

use rutter::{LayerKeyboardInteractivity, SurfaceEvent};

use super::*;

const GRANT_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Clone, Default)]
pub(in super::super) struct Focus {
    focused: bool,
    exclusive: bool,
    pub deadline: Option<Instant>,
}

impl Focus {
    fn policy(&mut self, exclusive: bool) -> Vec<SurfaceCommand> {
        if self.exclusive == exclusive {
            return vec![];
        }
        self.exclusive = exclusive;
        vec![SurfaceCommand::SetLayerKeyboardInteractivity {
            surface: SurfaceId::PRIMARY,
            interactivity: if exclusive {
                LayerKeyboardInteractivity::Exclusive
            } else {
                LayerKeyboardInteractivity::OnDemand
            },
        }]
    }

    pub fn release(&mut self) -> Vec<SurfaceCommand> {
        self.deadline = None;
        self.policy(false)
    }
}

pub(in super::super) fn transition(
    state: &mut State,
    was_expanded: bool,
    user_expansion: bool,
    now: Instant,
) -> Vec<SurfaceCommand> {
    let Some(overlay) = &mut state.overlay else {
        return vec![];
    };
    if overlay.closed || was_expanded == state.island.is_expanded() {
        return vec![];
    }
    if !state.island.is_expanded() {
        return overlay.focus.release();
    }
    if !user_expansion {
        return vec![];
    }
    overlay.focus.deadline = (!overlay.focus.focused).then_some(now + GRANT_TIMEOUT);
    overlay.focus.policy(true)
}

pub(in super::super) fn event(
    state: &mut State,
    surface: SurfaceId,
    event: SurfaceEvent,
) -> Vec<SurfaceCommand> {
    if surface != SurfaceId::PRIMARY {
        return vec![];
    }
    let Some(overlay) = &mut state.overlay else {
        return vec![];
    };
    if overlay.closed {
        return vec![];
    }
    let SurfaceEvent::FocusChanged(focused) = event;
    overlay.focus.focused = focused;
    if focused {
        overlay.focus.deadline = None;
        return vec![];
    }
    // A loss or inferred grant timeout ends this attempt. Details remain open;
    // only a later collapse/re-expansion may request Exclusive again.
    if overlay.focus.exclusive {
        overlay.focus.release()
    } else {
        vec![]
    }
}

pub(super) fn timeout(state: &mut State, now: Instant) -> Vec<SurfaceCommand> {
    let Some(overlay) = &mut state.overlay else {
        return vec![];
    };
    if !overlay.closed
        && overlay
            .focus
            .deadline
            .is_some_and(|deadline| now >= deadline)
    {
        overlay.focus.release()
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests;
