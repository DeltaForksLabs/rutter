// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::event::WindowEvent;

use super::*;

impl<A: MultiWindowAppLogic + 'static> MultiWindowRunner<A> {
    pub(super) fn dispatch_surface_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        surface: SurfaceId,
        native: WindowId,
        event: WindowEvent,
    ) {
        let surface_event = translate_surface_event(&event);
        if !self.forward_native_surface_event(event_loop, surface, native, event) {
            return;
        }
        self.synchronize_and_apply(event_loop, surface);
        if event_loop.exiting()
            || self.fatal_error.is_some()
            || !self.surface_configs.contains_key(&surface)
        {
            return;
        }
        if let Some(surface_event) = surface_event {
            self.dispatch_application_surface_event(event_loop, surface, surface_event);
        }
    }

    fn forward_native_surface_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        surface: SurfaceId,
        native: WindowId,
        event: WindowEvent,
    ) -> bool {
        let runner = match self.runner_for_mut(surface) {
            Ok(runner) => runner,
            Err(error) => {
                self.terminate_for_error(event_loop, error);
                return false;
            }
        };
        runner.window_event(event_loop, native, event);
        let Some(error) = runner.take_fatal_error() else {
            return true;
        };
        self.terminate_for_error(event_loop, surface_error(surface, error));
        false
    }

    fn dispatch_application_surface_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        surface: SurfaceId,
        event: SurfaceEvent,
    ) {
        let closes_automatically = match self.focus_loss_closes_surface(surface, event) {
            Ok(closes) => closes,
            Err(error) => return self.terminate_for_error(event_loop, error),
        };
        let commands = self.notify_surface_event(surface, event);
        let lifecycle_ends = commands_end_surface_lifecycle(&commands, surface);
        if let Err(error) = self.apply_surface_commands(event_loop, commands) {
            return self.terminate_for_error(event_loop, error);
        }
        if event_loop.exiting() || lifecycle_ends {
            return;
        }
        if let Err(error) = self.apply_automatic_surface_close(surface, closes_automatically) {
            return self.terminate_for_error(event_loop, error);
        }
        self.exit_if_no_surfaces(event_loop);
    }

    pub(super) fn notify_surface_event(
        &mut self,
        surface: SurfaceId,
        event: SurfaceEvent,
    ) -> Vec<SurfaceCommand> {
        let commands = A::surface_event(&mut self.canonical_state, surface, event);
        self.revision += 1;
        let model = self.canonical_state.clone();
        self.publish_surface_state(model, self.revision);
        commands
    }

    pub(super) fn focus_loss_closes_surface(
        &mut self,
        surface: SurfaceId,
        event: SurfaceEvent,
    ) -> Result<bool, MultiWindowRunError> {
        match event {
            SurfaceEvent::FocusChanged(true) => {
                self.focus_acquired_surfaces.insert(surface);
                Ok(false)
            }
            SurfaceEvent::FocusChanged(false) => {
                let had_focus = self.focus_acquired_surfaces.remove(&surface);
                Ok(had_focus && self.config_for(surface)?.closes_on_focus_loss())
            }
        }
    }
}

pub(super) fn translate_surface_event(event: &WindowEvent) -> Option<SurfaceEvent> {
    match event {
        WindowEvent::Focused(focused) => Some(SurfaceEvent::FocusChanged(*focused)),
        _ => None,
    }
}

pub(super) fn commands_end_surface_lifecycle(
    commands: &[SurfaceCommand],
    surface: SurfaceId,
) -> bool {
    commands.iter().any(|command| {
        matches!(command, SurfaceCommand::Close(target) if *target == surface)
            || matches!(command, SurfaceCommand::Exit)
    })
}
