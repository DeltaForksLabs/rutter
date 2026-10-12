//! Real popup backend boundaries with controlled presentation/resize failures.

use super::*;
use crate::engine::gpu::{GraphicsBackend, GraphicsError};
use skia_safe::Canvas;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    FirstFrameFailure,
    OversizedConfigure,
    InputBeforeRedraw,
    InitialAllocationLimit,
}

struct ObservedPopupBackend {
    inner: Box<dyn GraphicsBackend>,
    fail_presentation: bool,
    presentations: Rc<Cell<u32>>,
    resizes: Rc<Cell<u32>>,
}

impl GraphicsBackend for ObservedPopupBackend {
    fn backend_type(&self) -> BackendType {
        self.inner.backend_type()
    }
    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError> {
        self.inner.begin_frame()
    }
    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        if self.fail_presentation {
            return Err(GraphicsError::Frame {
                backend: BackendType::OpenGl,
                reason: "controlled first popup presentation failure".into(),
            });
        }
        self.inner.end_frame()?;
        self.presentations.set(self.presentations.get() + 1);
        Ok(())
    }
    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        assert!(
            size.width <= 4096
                && size.height <= 4096
                && u64::from(size.width) * u64::from(size.height) <= 4 * 1024 * 1024,
            "an over-budget configure reached graphics allocation"
        );
        self.resizes.set(self.resizes.get() + 1);
        self.inner.resize(size)
    }
    fn window(&self) -> &Rc<dyn Window> {
        self.inner.window()
    }
}

struct ReviewTest {
    runner: RutterRunner<OverflowApp>,
    scenario: Scenario,
    first_frame: Option<Instant>,
    opened: bool,
    installed: bool,
    adjustment_requested: bool,
    presentations: Rc<Cell<u32>>,
    resizes: Rc<Cell<u32>>,
    deadline: Instant,
}

impl ApplicationHandler for ReviewTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner
            .resume_surface(
                event_loop,
                Some(
                    WindowAttributes::default()
                        .with_decorations(false)
                        .with_title("Rutter native menu review")
                        .with_surface_size(PhysicalSize::new(800, 30)),
                ),
                Some(BackendType::OpenGl),
            )
            .unwrap();
        if self.scenario == Scenario::InitialAllocationLimit {
            let parent = self.runner.engine.window.as_ref().unwrap();
            let handle = parent.window_handle().unwrap();
            // The probe is dropped before its live parent, including the error path.
            let attributes =
                unsafe { WindowAttributes::default().with_parent_window(Some(handle.as_raw())) }
                    .with_window_type(WindowType::Popup)
                    .with_active(false)
                    .with_transparent(true)
                    .with_position(PhysicalPosition::new(20, 20))
                    .with_surface_size(PhysicalSize::new(4096, 1025));
            match crate::engine::gpu::gl::GlBackend::try_new(event_loop, attributes) {
                Err(error) => assert!(error.reason.contains("physical pixels per buffer")),
                Ok(_) => panic!("an oversized initial configure reached GL surface allocation"),
            }
            event_loop.exit();
        }
    }
    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        self.runner.release_surface();
    }
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "native review fixture timed out"
        );
        self.runner.new_events(event_loop, cause);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.about_to_wait(event_loop);
        assert!(self.runner.fatal_error.is_none());
        assert_eq!(
            self.runner.engine.window.as_ref().unwrap().surface_size(),
            PhysicalSize::new(800, 30)
        );
        if !self.runner.engine.app_state.failures.borrow().is_empty() {
            let failure = self.runner.engine.app_state.failures.borrow()[0].clone();
            assert!(!failure.overlay_usable);
            assert!(self.runner.native_menu.is_none());
            assert!(!self.runner.engine.any_context_menu_open());
            assert_eq!(self.runner.engine.app_state.selected.get(), 0);
            if self.scenario == Scenario::FirstFrameFailure {
                assert!(
                    failure
                        .reason
                        .contains("controlled first popup presentation failure")
                );
                assert_eq!(self.presentations.get(), 0);
            } else {
                assert_eq!(self.scenario, Scenario::OversizedConfigure);
                assert!(failure.reason.contains("physical pixels per buffer"));
                assert_eq!(self.resizes.get(), 0);
            }
            event_loop.exit();
            return;
        }
        if self.runner.engine.app_state.selected.get() == 1 {
            assert_eq!(self.scenario, Scenario::InputBeforeRedraw);
            assert!(self.runner.native_menu.is_none());
            event_loop.exit();
            return;
        }
        if !self.opened
            && self
                .first_frame
                .is_some_and(|frame| frame.elapsed() > Duration::from_millis(200))
        {
            self.opened = true;
            self.runner.window_event(
                event_loop,
                self.runner.active_window_id.unwrap(),
                press(PhysicalPosition::new(20.0, 20.0), MouseButton::Right),
            );
            self.runner.about_to_wait(event_loop);
        }
        if !self.installed && self.runner.native_menu.is_some() {
            let mut popup = self.runner.native_menu.take().unwrap();
            assert!(
                !popup.ready,
                "a configure must not suppress the overlay before first presentation"
            );
            assert!(self.runner.engine.native_menu_layout.is_some());
            popup.backend = Box::new(ObservedPopupBackend {
                inner: popup.backend,
                fail_presentation: self.scenario == Scenario::FirstFrameFailure,
                presentations: self.presentations.clone(),
                resizes: self.resizes.clone(),
            });
            self.runner.native_menu = Some(popup);
            self.installed = true;
        }
        if let Some(popup) = self.runner.native_menu.as_ref().filter(|popup| popup.ready)
            && !self.adjustment_requested
        {
            self.adjustment_requested = true;
            if self.scenario == Scenario::OversizedConfigure {
                let id = popup.id();
                self.resizes.set(0);
                self.runner.window_event(
                    event_loop,
                    id,
                    WindowEvent::SurfaceResized(PhysicalSize::new(4096, 1025)),
                );
            } else if self.scenario == Scenario::InputBeforeRedraw {
                self.runner.handle_key(&Key::Named(NamedKey::End), false);
                let window = self.runner.native_menu.as_ref().unwrap().backend.window();
                let mut positioner = window.positioner();
                positioner.offset = winit::dpi::LogicalPosition::new(-100.0, -80.0).into();
                window.request_popup_layout(
                    positioner,
                    winit::dpi::LogicalSize::new(120.0, 72.0).into(),
                );
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(50),
        ));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let parent = self.runner.active_window_id.unwrap();
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        if self.scenario == Scenario::InputBeforeRedraw
            && self.adjustment_requested
            && Some(id) == self.runner.native_menu_window_id()
            && matches!(event, WindowEvent::SurfaceResized(size) if size == PhysicalSize::new(120, 72))
        {
            // Winit has received the configure, but the runner has seen neither
            // SurfaceResized nor RedrawRequested for it. Click the new last row.
            let before = self.presentations.get();
            self.runner.window_event(
                event_loop,
                id,
                press(PhysicalPosition::new(20.0, 50.0), MouseButton::Left),
            );
            assert_eq!(
                self.presentations.get(),
                before + 1,
                "new geometry must be presented before row activation"
            );
            assert_eq!(
                self.runner.engine.app_state.selected.get(),
                1,
                "only the displayed last row sends the selection message"
            );
            return;
        }
        self.runner.window_event(event_loop, id, event);
        assert!(self.runner.fatal_error.is_none());
        if id == parent && redraw {
            self.first_frame.get_or_insert_with(Instant::now);
        }
    }
}

fn run_review_test(scenario: Scenario) {
    let mut builder = EventLoop::builder();
    winit::platform::wayland::EventLoopBuilderExtWayland::with_wayland(&mut builder);
    winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    let mut state = OverflowApp::new(&mut FontSystem::new());
    if scenario == Scenario::InputBeforeRedraw {
        state.entries = (0..30)
            .map(|index| ContextMenuEntry::item("Row", index == 29))
            .collect();
    }
    let engine = RutterEngine::<OverflowApp>::with_shared_font_system(
        state,
        Rc::new(RefCell::new(FontSystem::new())),
        SurfaceConfig::default(),
    )
    .unwrap();
    builder
        .build()
        .unwrap()
        .run_app(ReviewTest {
            runner: RutterRunner::with_engine(engine),
            scenario,
            first_frame: None,
            opened: false,
            installed: false,
            adjustment_requested: false,
            presentations: Rc::new(Cell::new(0)),
            resizes: Rc::new(Cell::new(0)),
            deadline: Instant::now() + Duration::from_secs(20),
        })
        .unwrap();
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL; controlled presentation failure"]
fn native_first_frame_failure_keeps_overlay_pending_and_reports_failure() {
    run_review_test(Scenario::FirstFrameFailure);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL; oversized event boundary"]
fn native_oversized_configure_is_reported_before_backend_resize() {
    run_review_test(Scenario::OversizedConfigure);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL; pointer is injected before resize/redraw delivery"]
fn native_configure_before_redraw_pointer_selects_the_new_scrolled_row() {
    run_review_test(Scenario::InputBeforeRedraw);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL; initial backend allocation boundary"]
fn native_backend_rejects_oversized_initial_popup_before_gl_surface() {
    run_review_test(Scenario::InitialAllocationLimit);
}
