// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use cosmic_text::FontSystem;
use winit::event_loop::{ControlFlow, EventLoop};

use super::*;

impl<A: MultiWindowAppLogic + 'static> MultiWindowRunner<A> {
    /// Runs with state and startup surfaces returned by the application hooks.
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

    /// Runs with an injected state factory and caller-provided startup surfaces.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner, SurfaceRequest};
    /// # fn launch<A: MultiWindowAppLogic + 'static>(
    /// #     state: A::State,
    /// #     surfaces: Vec<SurfaceRequest>,
    /// # ) {
    /// MultiWindowRunner::<A>::run_with(
    ///     move |_| Ok::<_, std::convert::Infallible>(state),
    ///     surfaces,
    /// );
    /// # }
    /// ```
    pub fn run_with<CreateState, StartupError>(
        create_state: CreateState,
        surfaces: Vec<SurfaceRequest>,
    ) where
        CreateState: FnOnce(&mut FontSystem) -> Result<A::State, StartupError>,
        StartupError: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        if let Err(error) = Self::try_run_with(create_state, surfaces) {
            eprintln!("Rutter multi-window application failed: {error}");
        }
    }

    /// Runs with an already constructed state and caller-provided surfaces.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner, SurfaceRequest};
    /// # fn launch<A: MultiWindowAppLogic + 'static>(
    /// #     state: A::State,
    /// #     surfaces: Vec<SurfaceRequest>,
    /// # ) {
    /// MultiWindowRunner::<A>::run_with_state(state, surfaces);
    /// # }
    /// ```
    pub fn run_with_state(state: A::State, surfaces: Vec<SurfaceRequest>) {
        Self::run_with(move |_| Ok::<_, std::convert::Infallible>(state), surfaces);
    }

    /// Runs through the application startup hooks and preserves typed failures.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner};
    /// # fn launch<A: MultiWindowAppLogic + 'static>() {
    /// MultiWindowRunner::<A>::try_run().expect("multi-window application failed");
    /// # }
    /// ```
    pub fn try_run() -> Result<(), MultiWindowRunError> {
        Self::launch_runtime(Self::initialize)
    }

    /// Runs injected startup state and surfaces while preserving typed failures.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner, SurfaceRequest};
    /// # fn launch<A: MultiWindowAppLogic + 'static>(
    /// #     state: A::State,
    /// #     surfaces: Vec<SurfaceRequest>,
    /// # ) -> Result<(), rutter::MultiWindowRunError> {
    /// MultiWindowRunner::<A>::try_run_with(
    ///     move |_| Ok::<_, std::convert::Infallible>(state),
    ///     surfaces,
    /// )
    /// # }
    /// ```
    pub fn try_run_with<CreateState, StartupError>(
        create_state: CreateState,
        surfaces: Vec<SurfaceRequest>,
    ) -> Result<(), MultiWindowRunError>
    where
        CreateState: FnOnce(&mut FontSystem) -> Result<A::State, StartupError>,
        StartupError: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let runtime = Self::initialize_with(create_state, surfaces)?;
        Self::launch_initialized_runtime(runtime)
    }

    /// Runs an already constructed state and surfaces with typed runtime errors.
    ///
    /// ```no_run
    /// # use rutter::{MultiWindowAppLogic, MultiWindowRunner, SurfaceRequest};
    /// # fn launch<A: MultiWindowAppLogic + 'static>(
    /// #     state: A::State,
    /// #     surfaces: Vec<SurfaceRequest>,
    /// # ) -> Result<(), rutter::MultiWindowRunError> {
    /// MultiWindowRunner::<A>::try_run_with_state(state, surfaces)
    /// # }
    /// ```
    pub fn try_run_with_state(
        state: A::State,
        surfaces: Vec<SurfaceRequest>,
    ) -> Result<(), MultiWindowRunError> {
        Self::try_run_with(move |_| Ok::<_, std::convert::Infallible>(state), surfaces)
    }

    fn launch_runtime<CreateRuntime>(
        create_runtime: CreateRuntime,
    ) -> Result<(), MultiWindowRunError>
    where
        CreateRuntime: FnOnce() -> Result<Self, MultiWindowRunError>,
    {
        let event_loop = EventLoop::new().map_err(MultiWindowRunError::from)?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let runtime = create_runtime()?;
        Self::run_initialized_runtime(event_loop, runtime)
    }

    fn launch_initialized_runtime(runtime: Self) -> Result<(), MultiWindowRunError> {
        let event_loop = EventLoop::new().map_err(MultiWindowRunError::from)?;
        event_loop.set_control_flow(ControlFlow::Wait);
        Self::run_initialized_runtime(event_loop, runtime)
    }

    fn run_initialized_runtime(
        event_loop: EventLoop<()>,
        mut runtime: Self,
    ) -> Result<(), MultiWindowRunError> {
        runtime.accessibility_waker = Some(event_loop.create_proxy());
        let event_result = event_loop.run_app(&mut runtime);
        runtime
            .fatal_error
            .map_or_else(|| event_result.map_err(MultiWindowRunError::from), Err)
    }

    pub(super) fn initialize() -> Result<Self, MultiWindowRunError> {
        let mut font_system = FontSystem::new();
        let canonical_state = A::new(&mut font_system);
        let pending_surfaces = A::initial_surfaces();
        Self::initialize_from_bootstrap(font_system, canonical_state, pending_surfaces)
    }

    pub(super) fn initialize_with<CreateState, StartupError>(
        create_state: CreateState,
        pending_surfaces: Vec<SurfaceRequest>,
    ) -> Result<Self, MultiWindowRunError>
    where
        CreateState: FnOnce(&mut FontSystem) -> Result<A::State, StartupError>,
        StartupError: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        validate_initial_surfaces(&pending_surfaces)?;
        let mut font_system = FontSystem::new();
        let canonical_state = create_state(&mut font_system)
            .map_err(|error| MultiWindowRunError::Startup(error.into()))?;
        Self::initialize_from_bootstrap(font_system, canonical_state, pending_surfaces)
    }

    fn initialize_from_bootstrap(
        font_system: FontSystem,
        canonical_state: A::State,
        pending_surfaces: Vec<SurfaceRequest>,
    ) -> Result<Self, MultiWindowRunError> {
        validate_initial_surfaces(&pending_surfaces)?;
        Ok(Self {
            font_system: Rc::new(RefCell::new(font_system)),
            canonical_state,
            revision: 0,
            pending_surfaces,
            surface_configs: BTreeMap::new(),
            surface_runners: BTreeMap::new(),
            routes: SurfaceRoutes::new(),
            focus_acquired_surfaces: HashSet::new(),
            backend_preference: MultiWindowBackendPreference::default(),
            native_surfaces_active: false,
            fatal_error: None,
            accessibility_waker: None,
        })
    }
}
