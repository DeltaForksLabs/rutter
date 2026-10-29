//! Deterministic command/lifetime tests; only constructor clipboard uses isolated Xvfb.
use std::cell::Cell;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::*;
use crate::engine::InitializedSurface;
use crate::engine::gpu::{GraphicsBackend, GraphicsError};
use crate::engine::test_surface::{InputRegionState, MemoryEventLoop, MemoryWindow, RasterPopup};
use crate::{SurfaceInputRect, SurfaceInputRegion, Widget};

struct InputApp;

impl MultiWindowAppLogic for InputApp {
    type State = ();
    type Message = ();
    fn new(_: &mut FontSystem) {}
    fn view<'a>(_: &'a mut (), _: SurfaceId) -> Widget<'a, ()> {
        Widget::Spacer {
            style: Default::default(),
        }
    }
    fn update(_: &mut (), _: SurfaceId, _: (), _: &mut arboard::Clipboard) -> Vec<SurfaceCommand> {
        Vec::new()
    }
}

fn rectangles() -> SurfaceInputRegion {
    SurfaceInputRegion::rectangles([
        SurfaceInputRect::new(-8, 12, 40, 24).unwrap(),
        SurfaceInputRect::new(60, -4, 12, 20).unwrap(),
    ])
    .unwrap()
}

fn surface_runner(
    runtime: &MultiWindowRunner<InputApp>,
    input_state: Option<Arc<InputRegionState>>,
) -> (SurfaceRunner<InputApp>, Rc<Cell<usize>>) {
    let (engine, drops) = surface_engine(runtime, input_state);
    (RutterRunner::with_engine(engine), drops)
}

fn surface_engine(
    runtime: &MultiWindowRunner<InputApp>,
    input_state: Option<Arc<InputRegionState>>,
) -> (RutterEngine<SurfaceAppAdapter<InputApp>>, Rc<Cell<usize>>) {
    let window: Rc<dyn winit::window::Window> = Rc::new(MemoryWindow {
        size: winit::dpi::PhysicalSize::new(800, 600),
        scale: 1.5,
        popup: false,
        popup_state: None,
        input_state,
    });
    let mut engine = RutterEngine::with_shared_font_system(
        SurfaceAppState::new(SurfaceId::PRIMARY, (), 0),
        runtime.font_system.clone(),
        Default::default(),
    )
    .unwrap();
    let drops = Rc::new(Cell::new(0));
    engine.window = Some(window.clone());
    engine.graphics_backend = Some(Box::new(RasterPopup {
        window,
        surface: skia_safe::surfaces::raster_n32_premul((1, 1)).unwrap(),
        frames: Rc::new(Cell::new(0)),
        drops: drops.clone(),
        configure_on_frame: None,
    }));
    (engine, drops)
}

fn runtime(input_state: Option<Arc<InputRegionState>>) -> MultiWindowRunner<InputApp> {
    let mut runtime = MultiWindowRunner::<InputApp>::initialize().unwrap();
    runtime.pending_surfaces.clear();
    let (runner, _) = surface_runner(&runtime, input_state);
    runtime
        .register_committed_surface(
            SurfaceRequest::new(SurfaceId::PRIMARY, WindowConfig::default()),
            runner.native_window().unwrap().id(),
            runner,
        )
        .unwrap();
    runtime
}

fn command(
    runtime: &mut MultiWindowRunner<InputApp>,
    region: SurfaceInputRegion,
) -> Result<(), MultiWindowRunError> {
    runtime.apply_surface_commands(
        &MemoryEventLoop::default(),
        vec![SurfaceCommand::SetInputRegion {
            surface: SurfaceId::PRIMARY,
            region,
        }],
    )
}

struct InitialFrameBackend {
    backend: Box<dyn GraphicsBackend>,
    operations: Rc<RefCell<Vec<&'static str>>>,
}

impl GraphicsBackend for InitialFrameBackend {
    fn backend_type(&self) -> BackendType {
        self.backend.backend_type()
    }

    fn resize(&mut self, _: winit::dpi::PhysicalSize<u32>) -> Result<(), GraphicsError> {
        // The backing raster has fixed dimensions, unlike a native swapchain.
        self.operations.borrow_mut().push("resize");
        Ok(())
    }

    fn begin_frame(&mut self) -> Result<&skia_safe::Canvas, GraphicsError> {
        self.operations.borrow_mut().push("begin");
        self.backend.begin_frame()
    }

    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        self.operations.borrow_mut().push("present");
        self.backend.end_frame()
    }

    fn window(&self) -> &Rc<dyn winit::window::Window> {
        self.backend.window()
    }
}

fn initialize(
    engine: &mut RutterEngine<SurfaceAppAdapter<InputApp>>,
    config: &WindowConfig,
    operations: Rc<RefCell<Vec<&'static str>>>,
) -> Result<InitializedSurface, MultiWindowRunError> {
    let backend = engine.graphics_backend.take().unwrap();
    engine.window = None;
    let backend = Box::new(InitialFrameBackend {
        backend,
        operations: operations.clone(),
    });
    engine.initialize_surface_backend(
        backend,
        |window| {
            MultiWindowRunner::<InputApp>::apply_initial_input_region(
                SurfaceId::PRIMARY,
                config,
                window,
            )?;
            operations.borrow_mut().push("input-region");
            Ok(())
        },
        |error| surface_error(SurfaceId::PRIMARY, error.into()),
    )
}

#[test]
fn input_region_initial_policy_precedes_first_buffer_presentation() {
    let runtime = runtime(None);
    for policy in [SurfaceInputRegion::Empty, rectangles()] {
        let state = Arc::new(InputRegionState::default());
        let (mut engine, drops) = surface_engine(&runtime, Some(state.clone()));
        let config = WindowConfig::default().with_input_region(policy.clone());
        let operations = Rc::new(RefCell::new(Vec::new()));
        let initialized = initialize(&mut engine, &config, operations.clone()).unwrap();
        assert_eq!(*state.policies.lock().unwrap(), [policy]);
        assert_eq!(
            *operations.borrow(),
            ["input-region", "resize", "begin", "present"]
        );
        drop(initialized);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn input_region_commands_apply_logical_union_empty_and_restore_full_with_frames() {
    let state = Arc::new(InputRegionState::default());
    let mut runtime = runtime(Some(state.clone()));
    for policy in [
        rectangles(),
        SurfaceInputRegion::Empty,
        SurfaceInputRegion::Full,
    ] {
        command(&mut runtime, policy.clone()).unwrap();
        assert_eq!(
            runtime
                .config_for(SurfaceId::PRIMARY)
                .unwrap()
                .input_region(),
            &policy
        );
    }
    assert_eq!(
        *state.policies.lock().unwrap(),
        [
            rectangles(),
            SurfaceInputRegion::Empty,
            SurfaceInputRegion::Full
        ]
    );
    assert_eq!(state.redraws.load(Ordering::Relaxed), 3);
}

#[test]
fn input_region_failure_preserves_policy_and_surfaces_context_and_cause() {
    let mut runtime = runtime(None);
    let before = runtime.config_for(SurfaceId::PRIMARY).unwrap().clone();
    let error = command(&mut runtime, SurfaceInputRegion::Empty).unwrap_err();
    assert!(matches!(
        &error,
        MultiWindowRunError::InputRegion {
            surface: SurfaceId::PRIMARY,
            source: winit::error::RequestError::NotSupported(_)
        }
    ));
    assert!(
        error
            .to_string()
            .contains("setting native input region for surface")
    );
    assert!(error.source().is_some());
    assert_eq!(runtime.config_for(SurfaceId::PRIMARY).unwrap(), &before);
}

#[test]
fn input_region_suspension_defers_latest_policy_and_reapplies_on_new_native_lifetime() {
    let old = Arc::new(InputRegionState::default());
    let mut runtime = runtime(Some(old.clone()));
    command(&mut runtime, SurfaceInputRegion::Empty).unwrap();
    runtime.release_native_surfaces();
    command(&mut runtime, SurfaceInputRegion::Full).unwrap();
    command(&mut runtime, rectangles()).unwrap();
    assert_eq!(*old.policies.lock().unwrap(), [SurfaceInputRegion::Empty]);
    assert_eq!(old.redraws.load(Ordering::Relaxed), 1);

    let resumed = Arc::new(InputRegionState::default());
    let (mut engine, _) = surface_engine(&runtime, Some(resumed.clone()));
    let operations = Rc::new(RefCell::new(Vec::new()));
    let _initialized = initialize(
        &mut engine,
        runtime.config_for(SurfaceId::PRIMARY).unwrap(),
        operations.clone(),
    )
    .unwrap();
    assert_eq!(*resumed.policies.lock().unwrap(), [rectangles()]);
    // Initialization presents directly; policy publication needs no extra frame.
    assert_eq!(resumed.redraws.load(Ordering::Relaxed), 0);
    assert_eq!(
        *operations.borrow(),
        ["input-region", "resize", "begin", "present"]
    );
    assert!(runtime.routes.is_empty());
}

#[test]
fn input_region_initial_default_skips_backend_and_opt_in_failure_releases_resources() {
    let runtime = runtime(None);
    let (mut engine, _) = surface_engine(&runtime, None);
    let operations = Rc::new(RefCell::new(Vec::new()));
    let _initialized =
        initialize(&mut engine, &WindowConfig::default(), operations.clone()).unwrap();
    assert_eq!(
        *operations.borrow(),
        ["input-region", "resize", "begin", "present"]
    );

    let (mut engine, drops) = surface_engine(&runtime, None);
    let native = Rc::downgrade(engine.window.as_ref().unwrap());
    let configured = WindowConfig::default().with_input_region(rectangles());
    assert_eq!(configured.clone().input_region(), &rectangles());
    operations.borrow_mut().clear();
    let error = initialize(&mut engine, &configured, operations.clone())
        .err()
        .expect("unsupported initial policy must fail before presentation");
    assert!(matches!(
        &error,
        MultiWindowRunError::InputRegion {
            surface: SurfaceId::PRIMARY,
            source: winit::error::RequestError::NotSupported(_),
        }
    ));
    assert!(error.source().is_some());
    assert!(error.to_string().contains("setting native input region"));
    assert!(operations.borrow().is_empty());
    assert!(engine.window.is_none());
    assert!(engine.graphics_backend.is_none());
    assert!(native.upgrade().is_none());
    assert_eq!(drops.get(), 1);
}

#[test]
fn input_region_terminal_and_closed_commands_do_not_mutate_or_schedule() {
    let state = Arc::new(InputRegionState::default());
    let mut runtime = runtime(Some(state.clone()));
    command(&mut runtime, rectangles()).unwrap();
    state.retired.store(true, Ordering::Relaxed);
    assert!(matches!(
        command(&mut runtime, SurfaceInputRegion::Empty),
        Err(MultiWindowRunError::InputRegion {
            source: winit::error::RequestError::Ignored,
            ..
        })
    ));
    assert_eq!(
        runtime
            .config_for(SurfaceId::PRIMARY)
            .unwrap()
            .input_region(),
        &rectangles()
    );
    runtime.close_surface(SurfaceId::PRIMARY).unwrap();
    assert!(matches!(
        command(&mut runtime, SurfaceInputRegion::Full),
        Err(MultiWindowRunError::UnknownLogicalSurface(
            SurfaceId::PRIMARY
        ))
    ));
    assert_eq!(*state.policies.lock().unwrap(), [rectangles()]);
    assert_eq!(state.redraws.load(Ordering::Relaxed), 1);
}

fn keyboard_command(
    runtime: &mut MultiWindowRunner<InputApp>,
    interactivity: crate::LayerKeyboardInteractivity,
) -> Result<(), MultiWindowRunError> {
    runtime.apply_surface_commands(
        &MemoryEventLoop::default(),
        vec![SurfaceCommand::SetLayerKeyboardInteractivity {
            surface: SurfaceId::PRIMARY,
            interactivity,
        }],
    )
}

fn configure_layer(runtime: &mut MultiWindowRunner<InputApp>) {
    let layer = crate::LayerShellConfig::new("keyboard-test", crate::LayerAnchors::TOP, 320, 80)
        .unwrap()
        .with_keyboard_interactivity(crate::LayerKeyboardInteractivity::OnDemand);
    runtime.surface_configs.insert(
        SurfaceId::PRIMARY,
        WindowConfig::default().with_layer_shell(layer),
    );
}

#[test]
fn layer_keyboard_commands_persist_defer_and_recreate_with_latest_policy() {
    use crate::LayerKeyboardInteractivity::{Exclusive, None, OnDemand};
    let native = Arc::new(InputRegionState::default());
    let mut runtime = runtime(Some(native.clone()));
    configure_layer(&mut runtime);
    keyboard_command(&mut runtime, Exclusive).unwrap();
    assert_eq!(
        runtime
            .config_for(SurfaceId::PRIMARY)
            .unwrap()
            .window_attributes()
            .layer_shell
            .unwrap()
            .keyboard_interactivity(),
        Exclusive
    );
    assert_eq!(*native.keyboard_policies.lock().unwrap(), [Exclusive]);
    assert_eq!(native.redraws.load(Ordering::Relaxed), 1);
    runtime.release_native_surfaces();
    assert_eq!(
        runtime
            .config_for(SurfaceId::PRIMARY)
            .unwrap()
            .window_attributes()
            .layer_shell
            .unwrap()
            .keyboard_interactivity(),
        Exclusive
    );
    keyboard_command(&mut runtime, None).unwrap();
    keyboard_command(&mut runtime, OnDemand).unwrap();
    // Resume builds WindowAttributes from the retained config, not a stale native window.
    assert_eq!(
        runtime
            .config_for(SurfaceId::PRIMARY)
            .unwrap()
            .window_attributes()
            .layer_shell
            .unwrap()
            .keyboard_interactivity(),
        OnDemand
    );
    assert_eq!(*native.keyboard_policies.lock().unwrap(), [Exclusive]);
    assert_eq!(native.redraws.load(Ordering::Relaxed), 1);
}

#[test]
fn layer_keyboard_errors_retain_config_and_cause_without_redraw_or_silent_acceptance() {
    use crate::LayerKeyboardInteractivity::{Exclusive, OnDemand};
    let native = Arc::new(InputRegionState::default());
    let mut runtime = runtime(Some(native.clone()));
    let before = runtime.config_for(SurfaceId::PRIMARY).unwrap().clone();
    assert!(matches!(
        keyboard_command(&mut runtime, Exclusive),
        Err(MultiWindowRunError::LayerKeyboardInteractivity {
            source: winit::error::RequestError::NotSupported(_),
            ..
        })
    ));
    assert_eq!(runtime.config_for(SurfaceId::PRIMARY).unwrap(), &before);
    configure_layer(&mut runtime);
    keyboard_command(&mut runtime, Exclusive).unwrap();
    let before = runtime.config_for(SurfaceId::PRIMARY).unwrap().clone();
    native.retired.store(true, Ordering::Relaxed);
    let error = keyboard_command(&mut runtime, OnDemand).unwrap_err();
    assert!(error.source().is_some());
    assert!(
        error
            .to_string()
            .contains("setting layer keyboard interactivity")
    );
    assert!(matches!(
        error,
        MultiWindowRunError::LayerKeyboardInteractivity {
            source: winit::error::RequestError::Ignored,
            ..
        }
    ));
    assert_eq!(runtime.config_for(SurfaceId::PRIMARY).unwrap(), &before);
    assert_eq!(*native.keyboard_policies.lock().unwrap(), [Exclusive]);
    assert_eq!(native.redraws.load(Ordering::Relaxed), 1);
    runtime.close_surface(SurfaceId::PRIMARY).unwrap();
    assert!(matches!(
        keyboard_command(&mut runtime, Exclusive),
        Err(MultiWindowRunError::UnknownLogicalSurface(_))
    ));

    let mut unsupported = self::runtime(None);
    configure_layer(&mut unsupported);
    let before = unsupported.config_for(SurfaceId::PRIMARY).unwrap().clone();
    assert!(matches!(
        keyboard_command(&mut unsupported, Exclusive),
        Err(MultiWindowRunError::LayerKeyboardInteractivity {
            source: winit::error::RequestError::NotSupported(_),
            ..
        })
    ));
    assert_eq!(unsupported.config_for(SurfaceId::PRIMARY).unwrap(), &before);
}
