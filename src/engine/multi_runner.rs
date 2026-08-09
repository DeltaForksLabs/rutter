// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use cosmic_text::FontSystem;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

mod app_adapter;

use self::app_adapter::{SurfaceAppAdapter, SurfaceAppState};
use super::RutterEngine;
use super::gpu::BackendType;
use super::runner::RutterRunner;
use crate::multi_window::{
    CloseBehavior, MultiWindowAppLogic, MultiWindowRunError, SurfaceCommand, SurfaceId,
    SurfaceRequest, SurfaceRouteRegistrationError, SurfaceRoutes, WindowConfig,
};

type SurfaceRunner<A> = RutterRunner<SurfaceAppAdapter<A>>;

#[derive(Debug, Default)]
struct MultiWindowBackendPreference {
    retained: Option<BackendType>,
}

impl MultiWindowBackendPreference {
    fn required_backend(&self) -> Option<BackendType> {
        self.retained
    }

    fn retain_committed(&mut self, backend: BackendType) {
        self.retained.get_or_insert(backend);
    }
}

/// Owns multiple Rutter windows and routes each native event to one logical surface.
pub struct MultiWindowRunner<A: MultiWindowAppLogic> {
    font_system: Rc<RefCell<FontSystem>>,
    canonical_state: A::State,
    revision: u128,
    pending_surfaces: Vec<SurfaceRequest>,
    surface_configs: BTreeMap<SurfaceId, WindowConfig>,
    surface_runners: BTreeMap<SurfaceId, SurfaceRunner<A>>,
    routes: SurfaceRoutes<WindowId>,
    backend_preference: MultiWindowBackendPreference,
    native_surfaces_active: bool,
    fatal_error: Option<MultiWindowRunError>,
}

impl<A: MultiWindowAppLogic + 'static> MultiWindowRunner<A> {
    /// Runs a multi-window application and prints controlled failures.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner};
    /// # fn launch<A: MultiWindowAppLogic + 'static>() {
    /// MultiWindowRunner::<A>::run();
    /// # }
    /// ```
    pub fn run() {
        if let Err(error) = Self::try_run() {
            eprintln!("Rutter multi-window application failed: {error}");
        }
    }

    /// Runs a multi-window application while preserving typed lifecycle failures.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner};
    /// # fn launch<A: MultiWindowAppLogic + 'static>() {
    /// MultiWindowRunner::<A>::try_run().expect("multi-window application failed");
    /// # }
    /// ```
    pub fn try_run() -> Result<(), MultiWindowRunError> {
        let event_loop = EventLoop::new().map_err(MultiWindowRunError::from)?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut runtime = Self::initialize()?;
        let event_result = event_loop.run_app(&mut runtime);
        if let Some(error) = runtime.fatal_error {
            return Err(error);
        }
        event_result.map_err(MultiWindowRunError::from)
    }

    fn initialize() -> Result<Self, MultiWindowRunError> {
        let mut font_system = FontSystem::new();
        let canonical_state = A::new(&mut font_system);
        let pending_surfaces = A::initial_surfaces();
        validate_initial_surfaces(&pending_surfaces)?;
        Ok(Self {
            font_system: Rc::new(RefCell::new(font_system)),
            canonical_state,
            revision: 0,
            pending_surfaces,
            surface_configs: BTreeMap::new(),
            surface_runners: BTreeMap::new(),
            routes: SurfaceRoutes::new(),
            backend_preference: MultiWindowBackendPreference::default(),
            native_surfaces_active: false,
            fatal_error: None,
        })
    }

    fn open_surface(
        &mut self,
        event_loop: &ActiveEventLoop,
        request: SurfaceRequest,
    ) -> Result<(), MultiWindowRunError> {
        self.ensure_surface_is_available(request.surface)?;
        let mut runner = self.build_surface_runner(&request)?;
        let required_backend = self.backend_preference.required_backend();
        let (native, committed_backend) = runner
            .resume_surface(
                event_loop,
                Some(request.window.window_attributes()),
                required_backend,
            )
            .map_err(|error| surface_error(request.surface, error.into()))?;
        let surface = request.surface;
        self.register_committed_surface(request, native, runner)?;
        self.backend_preference.retain_committed(committed_backend);
        self.notify_surface_created(surface);
        Ok(())
    }

    fn ensure_surface_is_available(&self, surface: SurfaceId) -> Result<(), MultiWindowRunError> {
        if self.surface_configs.contains_key(&surface) {
            return Err(MultiWindowRunError::DuplicateLogicalSurface(surface));
        }
        Ok(())
    }

    fn build_surface_runner(
        &self,
        request: &SurfaceRequest,
    ) -> Result<SurfaceRunner<A>, MultiWindowRunError> {
        let app_state =
            SurfaceAppState::new(request.surface, self.canonical_state.clone(), self.revision);
        let engine = RutterEngine::with_shared_font_system(
            app_state,
            self.font_system.clone(),
            request.window.surface_config(),
        )
        .map_err(|error| surface_error(request.surface, error))?;
        Ok(RutterRunner::with_engine(engine))
    }

    fn register_committed_surface(
        &mut self,
        request: SurfaceRequest,
        native: WindowId,
        runner: SurfaceRunner<A>,
    ) -> Result<(), MultiWindowRunError> {
        self.routes
            .register_committed(request.surface, native)
            .map_err(|error| route_error(request.surface, error))?;
        self.surface_configs.insert(request.surface, request.window);
        self.surface_runners.insert(request.surface, runner);
        Ok(())
    }

    fn resume_registered_surfaces(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), MultiWindowRunError> {
        let surfaces: Vec<SurfaceId> = self.surface_configs.keys().copied().collect();
        for surface in surfaces {
            self.resume_registered_surface(event_loop, surface)?;
        }
        Ok(())
    }

    fn resume_registered_surface(
        &mut self,
        event_loop: &ActiveEventLoop,
        surface: SurfaceId,
    ) -> Result<(), MultiWindowRunError> {
        let attributes = self.config_for(surface)?.window_attributes();
        let required_backend = self.backend_preference.required_backend();
        let runner = self.runner_for_mut(surface)?;
        let (native, committed_backend) = runner
            .resume_surface(event_loop, Some(attributes), required_backend)
            .map_err(|error| surface_error(surface, error.into()))?;
        self.routes
            .register_committed(surface, native)
            .map_err(|error| route_error(surface, error))?;
        self.backend_preference.retain_committed(committed_backend);
        Ok(())
    }

    fn start_pending_surfaces(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), MultiWindowRunError> {
        let requests = std::mem::take(&mut self.pending_surfaces);
        for request in requests {
            self.open_surface(event_loop, request)?;
        }
        Ok(())
    }

    fn config_for(&self, surface: SurfaceId) -> Result<&WindowConfig, MultiWindowRunError> {
        self.surface_configs
            .get(&surface)
            .ok_or(MultiWindowRunError::UnknownLogicalSurface(surface))
    }

    fn runner_for_mut(
        &mut self,
        surface: SurfaceId,
    ) -> Result<&mut SurfaceRunner<A>, MultiWindowRunError> {
        self.surface_runners
            .get_mut(&surface)
            .ok_or(MultiWindowRunError::UnknownLogicalSurface(surface))
    }

    fn dispatch_surface_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        surface: SurfaceId,
        native: WindowId,
        event: WindowEvent,
    ) {
        let runner = match self.runner_for_mut(surface) {
            Ok(runner) => runner,
            Err(error) => return self.terminate_for_error(event_loop, error),
        };
        runner.window_event(event_loop, native, event);
        if let Some(error) = runner.take_fatal_error() {
            return self.terminate_for_error(event_loop, surface_error(surface, error));
        }
        self.synchronize_and_apply(event_loop, surface);
    }

    fn synchronize_and_apply(&mut self, event_loop: &ActiveEventLoop, surface: SurfaceId) {
        let commands = match self.synchronize_surface_state(surface) {
            Ok(commands) => commands,
            Err(error) => return self.terminate_for_error(event_loop, error),
        };
        if let Err(error) = self.apply_surface_commands(event_loop, commands) {
            self.terminate_for_error(event_loop, error);
        }
    }

    fn synchronize_surface_state(
        &mut self,
        surface: SurfaceId,
    ) -> Result<Vec<SurfaceCommand>, MultiWindowRunError> {
        let published_revision = self.revision;
        let source = self.runner_for_mut(surface)?.app_state_mut();
        if source.revision == published_revision {
            return Ok(Vec::new());
        }
        let model = source.model.clone();
        let revision = source.revision;
        let commands = std::mem::take(&mut source.commands);
        self.publish_surface_state(model, revision);
        Ok(commands)
    }

    fn publish_surface_state(&mut self, model: A::State, revision: u128) {
        self.canonical_state = model.clone();
        self.revision = revision;
        for runner in self.surface_runners.values_mut() {
            let state = runner.app_state_mut();
            state.model = model.clone();
            state.revision = revision;
            runner.invalidate_and_redraw();
        }
    }

    fn notify_surface_created(&mut self, surface: SurfaceId) {
        A::surface_created(&mut self.canonical_state, surface);
        self.revision += 1;
        let model = self.canonical_state.clone();
        self.publish_surface_state(model, self.revision);
    }

    fn notify_surface_closed(&mut self, surface: SurfaceId) {
        A::surface_closed(&mut self.canonical_state, surface);
        self.revision += 1;
        let model = self.canonical_state.clone();
        self.publish_surface_state(model, self.revision);
    }

    fn apply_surface_commands(
        &mut self,
        event_loop: &ActiveEventLoop,
        commands: Vec<SurfaceCommand>,
    ) -> Result<(), MultiWindowRunError> {
        for command in commands {
            match command {
                SurfaceCommand::Open(request) => self.open_surface(event_loop, request)?,
                SurfaceCommand::Close(surface) => self.close_surface(surface)?,
                SurfaceCommand::Exit => {
                    event_loop.exit();
                    return Ok(());
                }
            }
        }
        self.exit_if_no_surfaces(event_loop);
        Ok(())
    }

    fn close_surface(&mut self, surface: SurfaceId) -> Result<(), MultiWindowRunError> {
        if !self.surface_configs.contains_key(&surface) {
            return Err(MultiWindowRunError::UnknownLogicalSurface(surface));
        }
        self.routes.remove_surface(surface);
        self.surface_configs.remove(&surface);
        self.surface_runners.remove(&surface);
        self.notify_surface_closed(surface);
        Ok(())
    }

    fn handle_close_request(&mut self, event_loop: &ActiveEventLoop, surface: SurfaceId) {
        let behavior = match self.config_for(surface) {
            Ok(config) => config.close_behavior(),
            Err(error) => return self.terminate_for_error(event_loop, error),
        };
        match behavior {
            CloseBehavior::CloseSurface => {
                if let Err(error) = self.close_surface(surface) {
                    self.terminate_for_error(event_loop, error);
                    return;
                }
                self.exit_if_no_surfaces(event_loop);
            }
            CloseBehavior::ExitApplication => event_loop.exit(),
        }
    }

    fn handle_destroyed(&mut self, event_loop: &ActiveEventLoop, native: WindowId) {
        let Some(surface) = self.routes.remove_native(native) else {
            return;
        };
        self.surface_configs.remove(&surface);
        self.surface_runners.remove(&surface);
        self.notify_surface_closed(surface);
        self.exit_if_no_surfaces(event_loop);
    }

    fn process_all_schedules(&mut self, event_loop: &ActiveEventLoop) -> Option<Instant> {
        let surfaces: Vec<SurfaceId> = self.surface_runners.keys().copied().collect();
        let mut earliest = None;
        for surface in surfaces {
            let Some(runner) = self.surface_runners.get_mut(&surface) else {
                continue;
            };
            earliest = minimum_deadline(earliest, runner.process_scheduled_work());
            self.synchronize_and_apply(event_loop, surface);
            if self.fatal_error.is_some() {
                return None;
            }
        }
        earliest
    }

    fn release_native_surfaces(&mut self) {
        self.routes.clear();
        for runner in self.surface_runners.values_mut() {
            runner.release_surface();
        }
        self.native_surfaces_active = false;
    }

    fn exit_if_no_surfaces(&self, event_loop: &ActiveEventLoop) {
        if self.surface_runners.is_empty() {
            event_loop.exit();
        }
    }

    fn terminate_for_error(&mut self, event_loop: &ActiveEventLoop, error: MultiWindowRunError) {
        if self.fatal_error.is_none() {
            self.fatal_error = Some(error);
        }
        event_loop.exit();
    }
}

impl<A: MultiWindowAppLogic + 'static> ApplicationHandler for MultiWindowRunner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.fatal_error.is_some() || self.native_surfaces_active {
            return;
        }
        let result = if self.surface_runners.is_empty() {
            self.start_pending_surfaces(event_loop)
        } else {
            self.resume_registered_surfaces(event_loop)
        };
        match result {
            Ok(()) => self.native_surfaces_active = true,
            Err(error) => self.terminate_for_error(event_loop, error),
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.release_native_surfaces();
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _: StartCause) {
        if self.fatal_error.is_some() {
            event_loop.exit();
            return;
        }
        if !self.native_surfaces_active {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        match self.process_all_schedules(event_loop) {
            Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
            None => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, native: WindowId, event: WindowEvent) {
        let Some(surface) = self.routes.surface_for(native) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => self.handle_close_request(event_loop, surface),
            WindowEvent::Destroyed => self.handle_destroyed(event_loop, native),
            event => self.dispatch_surface_event(event_loop, surface, native, event),
        }
    }
}

fn validate_initial_surfaces(requests: &[SurfaceRequest]) -> Result<(), MultiWindowRunError> {
    if requests.is_empty() {
        return Err(MultiWindowRunError::EmptyInitialSurfaces);
    }
    let mut surfaces = HashSet::new();
    for request in requests {
        if !surfaces.insert(request.surface) {
            return Err(MultiWindowRunError::DuplicateLogicalSurface(
                request.surface,
            ));
        }
    }
    Ok(())
}

fn route_error<NativeId: std::fmt::Debug>(
    surface: SurfaceId,
    error: SurfaceRouteRegistrationError<NativeId>,
) -> MultiWindowRunError {
    match error {
        SurfaceRouteRegistrationError::DuplicateLogical(surface) => {
            MultiWindowRunError::DuplicateLogicalSurface(surface)
        }
        SurfaceRouteRegistrationError::DuplicateNative(native) => {
            MultiWindowRunError::NativeRouteConflict {
                surface,
                native: format!("{native:?}"),
            }
        }
    }
}

fn surface_error(
    surface: SurfaceId,
    error: crate::engine::run_error::RutterRunError,
) -> MultiWindowRunError {
    MultiWindowRunError::Surface(surface, error)
}

fn minimum_deadline(current: Option<Instant>, candidate: Option<Instant>) -> Option<Instant> {
    match (current, candidate) {
        (Some(current), Some(candidate)) => Some(current.min(candidate)),
        (Some(current), None) => Some(current),
        (None, candidate) => candidate,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/multi_runner_unit_tests.rs"]
mod tests;
