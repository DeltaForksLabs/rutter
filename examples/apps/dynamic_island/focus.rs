//! Application focus policy: a bounded request, never an engine-level denial.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FocusFallbackReason {
    GrantDeadlineElapsed,
    FocusLost,
}

impl Model {
    fn keyboard_command(&mut self, policy: LayerKeyboardInteractivity) -> Vec<SurfaceCommand> {
        if self.keyboard_policy == policy {
            return vec![];
        }
        self.keyboard_policy = policy;
        vec![SurfaceCommand::SetLayerKeyboardInteractivity {
            surface: SurfaceId::PRIMARY,
            interactivity: policy,
        }]
    }

    pub(super) fn expansion_focus_changed(&mut self, now: Instant) -> Vec<SurfaceCommand> {
        self.focus_fallback = None;
        if self.island.is_expanded() {
            self.focus_deadline = (!self.focused).then_some(now + FOCUS_GRANT_TIMEOUT);
            self.keyboard_command(LayerKeyboardInteractivity::Exclusive)
        } else {
            self.focus_deadline = None;
            self.keyboard_command(LayerKeyboardInteractivity::OnDemand)
        }
    }
}

impl IslandDemo {
    /// Application callback for a focus request that did not grant focus in time,
    /// or focus subsequently lost. Timeout is inference, not compositor refusal:
    /// layer-shell has no explicit denial event. Keep details open for click focus.
    pub(super) fn focus_by_click_fallback(
        state: &mut Model,
        reason: FocusFallbackReason,
    ) -> Vec<SurfaceCommand> {
        state.focus_deadline = None;
        if state.closing || state.island.mode() == DynamicIslandMode::Passive {
            return vec![];
        }
        state.focus_fallback = Some(reason);
        state.keyboard_command(LayerKeyboardInteractivity::OnDemand)
    }
}
