// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::time::Instant;

use crate::multi_window::{MultiWindowAppLogic, SurfaceCommand};

#[derive(Debug, Default)]
pub(super) struct ApplicationWakeupScheduler {
    due_callback_ran: bool,
}

pub(super) struct ApplicationWakeupPoll {
    pub(super) commands: Option<Vec<SurfaceCommand>>,
    pub(super) deadline: Option<Instant>,
}

impl ApplicationWakeupScheduler {
    pub(super) fn begin_event_cycle(&mut self) {
        self.due_callback_ran = false;
    }

    pub(super) fn poll<A: MultiWindowAppLogic>(
        &mut self,
        state: &mut A::State,
        now: Instant,
    ) -> ApplicationWakeupPoll {
        let Some(deadline) = A::next_wakeup(state, now) else {
            return ApplicationWakeupPoll::disabled();
        };
        if deadline > now {
            return ApplicationWakeupPoll::waiting_until(deadline);
        }
        self.run_due_wakeup::<A>(state, now)
    }

    fn run_due_wakeup<A: MultiWindowAppLogic>(
        &mut self,
        state: &mut A::State,
        now: Instant,
    ) -> ApplicationWakeupPoll {
        if self.due_callback_ran {
            return ApplicationWakeupPoll::disabled();
        }
        self.due_callback_ran = true;
        let commands = A::wakeup(state, now);
        let deadline = A::next_wakeup(state, now).filter(|deadline| *deadline > now);
        ApplicationWakeupPoll::after_wakeup(commands, deadline)
    }
}

impl ApplicationWakeupPoll {
    fn disabled() -> Self {
        Self {
            commands: None,
            deadline: None,
        }
    }

    fn waiting_until(deadline: Instant) -> Self {
        Self {
            commands: None,
            deadline: Some(deadline),
        }
    }

    fn after_wakeup(commands: Vec<SurfaceCommand>, deadline: Option<Instant>) -> Self {
        Self {
            commands: Some(commands),
            deadline,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use arboard::Clipboard;
    use cosmic_text::FontSystem;
    use taffy::prelude::Style;

    use super::*;
    use crate::multi_window::{SurfaceId, SurfaceRequest, WindowConfig};
    use crate::widget::Widget;

    #[derive(Clone, Debug, Default)]
    struct TimerState {
        deadline: Option<Instant>,
        replacement_deadline: Option<Instant>,
        wakeup_count: usize,
        wakeup_commands: Vec<SurfaceCommand>,
    }

    #[derive(Clone, Debug)]
    enum TimerMessage {
        Schedule(Instant),
    }

    struct TimerApp;

    impl MultiWindowAppLogic for TimerApp {
        type State = TimerState;
        type Message = TimerMessage;

        fn new(_: &mut FontSystem) -> Self::State {
            TimerState::default()
        }

        fn view<'a>(_: &'a mut Self::State, _: SurfaceId) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Style::default(),
            }
        }

        fn update(
            state: &mut Self::State,
            _: SurfaceId,
            message: Self::Message,
            _: &mut Clipboard,
        ) -> Vec<SurfaceCommand> {
            apply_timer_message(state, message);
            Vec::new()
        }

        fn next_wakeup(state: &Self::State, _: Instant) -> Option<Instant> {
            state.deadline
        }

        fn wakeup(state: &mut Self::State, _: Instant) -> Vec<SurfaceCommand> {
            state.wakeup_count += 1;
            state.deadline = state.replacement_deadline;
            state.wakeup_commands.clone()
        }
    }

    #[test]
    fn no_declared_deadline_disables_application_wakeup_work() {
        let now = Instant::now();
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState::default();

        let poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert!(poll.commands.is_none());
        assert_eq!(poll.deadline, None);
        assert_eq!(state.wakeup_count, 0);
    }

    #[test]
    fn future_deadline_waits_without_running_the_callback() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(30);
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState {
            deadline: Some(deadline),
            ..Default::default()
        };

        let poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert!(poll.commands.is_none());
        assert_eq!(poll.deadline, Some(deadline));
        assert_eq!(state.wakeup_count, 0);
    }

    #[test]
    fn due_wakeup_runs_once_and_uses_its_replacement_deadline() {
        let now = Instant::now();
        let replacement = now + Duration::from_secs(60);
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState {
            deadline: Some(now),
            replacement_deadline: Some(replacement),
            ..Default::default()
        };

        let poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert_eq!(poll.commands, Some(Vec::new()));
        assert_eq!(poll.deadline, Some(replacement));
        assert_eq!(state.wakeup_count, 1);
    }

    #[test]
    fn update_replaces_a_later_wait_with_an_earlier_deadline() {
        let now = Instant::now();
        let late = now + Duration::from_secs(60);
        let early = now + Duration::from_secs(10);
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState {
            deadline: Some(late),
            ..Default::default()
        };

        let first_poll = scheduler.poll::<TimerApp>(&mut state, now);
        apply_timer_message(&mut state, TimerMessage::Schedule(early));
        let second_poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert_eq!(first_poll.deadline, Some(late));
        assert_eq!(second_poll.deadline, Some(early));
    }

    #[test]
    fn past_replacement_does_not_spin_inside_one_event_cycle() {
        let now = Instant::now();
        let past = now - Duration::from_secs(1);
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState {
            deadline: Some(past),
            replacement_deadline: Some(past),
            ..Default::default()
        };

        let first_poll = scheduler.poll::<TimerApp>(&mut state, now);
        let second_poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert_eq!(first_poll.deadline, None);
        assert_eq!(second_poll.deadline, None);
        assert_eq!(state.wakeup_count, 1);
    }

    #[test]
    fn suspended_deadline_produces_one_wakeup_after_resumption() {
        let now = Instant::now();
        let overdue = now - Duration::from_secs(120);
        let next = now + Duration::from_secs(60);
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState {
            deadline: Some(overdue),
            replacement_deadline: Some(next),
            ..Default::default()
        };

        scheduler.begin_event_cycle();
        let poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert_eq!(poll.deadline, Some(next));
        assert_eq!(state.wakeup_count, 1);
    }

    #[test]
    fn wakeup_preserves_commands_for_the_normal_surface_command_router() {
        let now = Instant::now();
        let surface = SurfaceId::new(9);
        let commands = vec![
            SurfaceCommand::Open(SurfaceRequest::new(surface, WindowConfig::default())),
            SurfaceCommand::RequestRedraw(surface),
            SurfaceCommand::Close(surface),
        ];
        let mut scheduler = ApplicationWakeupScheduler::default();
        let mut state = TimerState {
            deadline: Some(now),
            wakeup_commands: commands.clone(),
            ..Default::default()
        };

        let poll = scheduler.poll::<TimerApp>(&mut state, now);

        assert_eq!(poll.commands, Some(commands));
    }

    fn apply_timer_message(state: &mut TimerState, message: TimerMessage) {
        let TimerMessage::Schedule(deadline) = message;
        state.deadline = Some(deadline);
    }
}
