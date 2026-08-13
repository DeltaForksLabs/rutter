// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::*;

impl<A: MultiWindowAppLogic + 'static> MultiWindowRunner<A> {
    pub(super) fn apply_surface_commands(
        &mut self,
        event_loop: &ActiveEventLoop,
        commands: Vec<SurfaceCommand>,
    ) -> Result<(), MultiWindowRunError> {
        for command in commands {
            if self.apply_surface_command(event_loop, command)? {
                return Ok(());
            }
        }
        self.exit_if_no_surfaces(event_loop);
        Ok(())
    }

    fn apply_surface_command(
        &mut self,
        event_loop: &ActiveEventLoop,
        command: SurfaceCommand,
    ) -> Result<bool, MultiWindowRunError> {
        match command {
            SurfaceCommand::Open(request) => self.open_surface(event_loop, request)?,
            SurfaceCommand::Close(surface) => self.close_surface(surface)?,
            SurfaceCommand::SetVisible { surface, visible } => {
                self.set_surface_visibility(surface, visible)?;
            }
            SurfaceCommand::RequestRedraw(surface) => self.request_surface_redraw(surface)?,
            SurfaceCommand::Exit => {
                event_loop.exit();
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn close_surface(&mut self, surface: SurfaceId) -> Result<(), MultiWindowRunError> {
        if !self.surface_configs.contains_key(&surface) {
            return Err(MultiWindowRunError::UnknownLogicalSurface(surface));
        }
        self.routes.remove_surface(surface);
        self.focus_acquired_surfaces.remove(&surface);
        self.surface_configs.remove(&surface);
        self.surface_runners.remove(&surface);
        self.notify_surface_closed(surface);
        Ok(())
    }

    pub(super) fn apply_automatic_surface_close(
        &mut self,
        surface: SurfaceId,
        closes_automatically: bool,
    ) -> Result<bool, MultiWindowRunError> {
        if !closes_automatically {
            return Ok(false);
        }
        self.close_surface(surface)?;
        Ok(true)
    }

    pub(super) fn set_surface_visibility(
        &mut self,
        surface: SurfaceId,
        visible: bool,
    ) -> Result<(), MultiWindowRunError> {
        self.config_for(surface)?;
        self.runner_for_mut(surface)?.set_surface_visible(visible);
        self.surface_configs
            .get_mut(&surface)
            .expect("registered surface config must remain present")
            .set_visible(visible);
        Ok(())
    }

    pub(super) fn request_surface_redraw(
        &mut self,
        surface: SurfaceId,
    ) -> Result<(), MultiWindowRunError> {
        self.config_for(surface)?;
        self.runner_for_mut(surface)?.request_surface_redraw();
        Ok(())
    }
}
