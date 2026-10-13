//! Disposable-compositor regression: real roles, rendering and layer-owned menus.
use super::*;
use crate::{LayerAnchors, LayerExclusiveZone, LayerOutput, LayerShellConfig, SurfaceMetrics};
use std::time::Duration;
use taffy::prelude::{Dimension, Size, Style};
use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton};
use winit::event_loop::EventLoop;

#[test]
#[ignore = "requires the owned layer-shell smoke driver"]
fn native_neutral_panel_wayland_uses_real_layer_role() {
    use crate::engine::gpu::gl::GlBackend;
    use winit::application::ApplicationHandler;
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    use winit::window::{WindowAttributes, WindowType};
    struct NeutralPanel;
    impl ApplicationHandler for NeutralPanel {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let panel = crate::PanelConfig::new("rutter-neutral", crate::PanelEdge::Top, 320, 32)
                .unwrap()
                .with_reservation(Some(32))
                .unwrap()
                .with_margins(crate::PanelMargins {
                    top: 8,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(panel.reservation(), Some(32));
            let activation = winit::platform::wayland::WindowAttributesWayland::default()
                .with_activation_token(winit::window::ActivationToken::from_raw(
                    "owned-rejected-token".into(),
                ));
            assert!(
                event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_panel(panel.clone())
                            .with_platform_attributes(Box::new(activation))
                    )
                    .is_err()
            );
            let mut backend =
                GlBackend::try_new(event_loop, WindowAttributes::default().with_panel(panel))
                    .unwrap();
            assert_eq!(backend.window().window_type(), WindowType::Panel);
            assert!(backend.window().layer_shell_version().is_some());
            backend.begin_frame().unwrap().clear(skia_safe::Color::BLUE);
            backend.end_frame().unwrap();
            event_loop.exit();
        }
        fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut builder = EventLoop::builder();
    builder.with_wayland().with_any_thread(true);
    builder.build().unwrap().run_app(NeutralPanel).unwrap();
}

#[test]
#[ignore = "requires an owned two-output layer compositor and external targeted output removal"]
fn native_layer_retained_opengl_renderer_rejects_frames_after_output_removal() {
    use crate::engine::gpu::{GraphicsBackend, gl::GlBackend};
    use raw_window_handle::HasWindowHandle;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::window::{WindowAttributes, WindowType};

    struct RetainedRenderers {
        backends: Vec<Box<dyn GraphicsBackend>>,
        destroyed: Vec<WindowId>,
        start: Instant,
    }
    impl ApplicationHandler for RetainedRenderers {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let target = std::env::var("RUTTER_LAYER_OUTPUT").unwrap();
            let config = LayerShellConfig::new("rutter-retained-gl", LayerAnchors::TOP, 320, 40)
                .unwrap()
                .with_output(LayerOutput::Name(target))
                .unwrap();
            let layer = GlBackend::try_new(
                event_loop,
                WindowAttributes::default().with_layer_shell(config),
            )
            .unwrap();
            let parent = layer.window().window_handle().unwrap().as_raw();
            self.backends.push(layer);
            // SAFETY: the parent's renderer retains its window throughout this test.
            let attributes = unsafe {
                WindowAttributes::default()
                    .with_window_type(WindowType::Popup)
                    .with_parent_window(Some(parent))
            }
            .with_surface_size(winit::dpi::LogicalSize::new(100, 60))
            .with_active(false);
            self.backends
                .push(GlBackend::try_new(event_loop, attributes).unwrap());
            for backend in &mut self.backends {
                backend.begin_frame().unwrap().clear(skia_safe::Color::BLUE);
                backend.end_frame().unwrap();
            }
            println!("RETAINED_LAYER_READY");
        }
        fn window_event(&mut self, _: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
            if matches!(event, WindowEvent::Destroyed) {
                self.destroyed.push(id);
            }
            // Deliberately retain both renderers and ignore every CloseRequested.
        }
        fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
            assert!(self.start.elapsed() < Duration::from_secs(15));
            if self.destroyed.len() == 2 {
                assert_eq!(
                    self.destroyed,
                    vec![
                        self.backends[1].window().id(),
                        self.backends[0].window().id()
                    ]
                );
                for backend in &mut self.backends {
                    assert!(backend.window().window_handle().is_err());
                    assert!(backend.begin_frame().is_err());
                    assert!(backend.end_frame().is_err());
                    assert!(
                        backend
                            .resize(winit::dpi::PhysicalSize::new(100, 60))
                            .is_err()
                    );
                }
                // Actual EGL/Skia resource destruction after native role retirement.
                while let Some(backend) = self.backends.pop() {
                    drop(backend);
                }
                println!("RETAINED_LAYER_RETIRED_OPENGL_CLEANUP");
                event_loop.exit();
            }
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(20),
            ));
        }
    }
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    let mut builder = EventLoop::builder();
    builder.with_wayland().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let test = RetainedRenderers {
        backends: Vec::new(),
        destroyed: Vec::new(),
        start: Instant::now(),
    };
    event_loop.run_app(test).unwrap();
}

#[derive(Clone)]
struct PanelModel {
    entries: Vec<crate::ContextMenuEntry<'static, ()>>,
    selected: bool,
    metrics: BTreeMap<SurfaceId, SurfaceMetrics>,
    closed: Vec<SurfaceId>,
}
struct PanelApp;
impl MultiWindowAppLogic for PanelApp {
    type State = PanelModel;
    type Message = ();
    fn new(_: &mut FontSystem) -> PanelModel {
        PanelModel {
            entries: vec![crate::ContextMenuEntry::item("Settings", ())],
            selected: false,
            metrics: BTreeMap::new(),
            closed: Vec::new(),
        }
    }
    fn view<'a>(state: &'a mut PanelModel, surface: SurfaceId) -> crate::Widget<'a, ()> {
        let color = match surface.get() {
            1 => skia_safe::Color::RED,
            2 => skia_safe::Color::GREEN,
            3 => skia_safe::Color::BLUE,
            4 => skia_safe::Color::MAGENTA,
            _ => skia_safe::Color::GRAY,
        };
        crate::Widget::context_menu(
            crate::Widget::Container {
                child: Box::new(crate::Widget::Spacer {
                    style: Default::default(),
                }),
                style: Style {
                    size: Size {
                        width: Dimension::percent(1.0),
                        height: Dimension::percent(1.0),
                    },
                    ..Default::default()
                },
                color: Some(color),
                radius: 0.0,
            },
            &state.entries,
            Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::percent(1.0),
                },
                ..Default::default()
            },
        )
        .with_id(981)
    }
    fn update(
        state: &mut PanelModel,
        surface: SurfaceId,
        _: (),
        _: &mut arboard::Clipboard,
    ) -> Vec<SurfaceCommand> {
        assert_eq!(surface, SurfaceId::new(1));
        state.selected = true;
        Vec::new()
    }
    fn surface_metrics_changed(
        state: &mut PanelModel,
        surface: SurfaceId,
        metrics: SurfaceMetrics,
    ) {
        println!("LAYER_METRICS {} {:?}", surface.get(), metrics);
        state.metrics.insert(surface, metrics);
    }
    fn surface_closed(state: &mut PanelModel, surface: SurfaceId) {
        state.closed.push(surface);
    }
}

fn panel_requests() -> Vec<SurfaceRequest> {
    let target = std::env::var("RUTTER_LAYER_OUTPUT")
        .map(LayerOutput::Name)
        .unwrap_or_default();
    let settings = SurfaceRequest::new(
        SurfaceId::new(5),
        WindowConfig::default()
            .with_title("Rutter layer Settings")
            .with_decorations(false)
            .with_inner_size(500, 300)
            .unwrap(),
    );
    let mut requests: Vec<_> = [
        (
            1,
            LayerAnchors::TOP | LayerAnchors::LEFT | LayerAnchors::RIGHT,
            0,
            40,
        ),
        (2, LayerAnchors::BOTTOM, 400, 30),
        (
            3,
            LayerAnchors::LEFT | LayerAnchors::TOP | LayerAnchors::BOTTOM,
            30,
            0,
        ),
        (4, LayerAnchors::RIGHT, 30, 300),
    ]
    .into_iter()
    .map(|(id, anchors, width, height)| {
        let mut config =
            LayerShellConfig::new(format!("rutter-panel-{id}"), anchors, width, height)
                .unwrap()
                .with_output(target.clone())
                .unwrap();
        if id == 1 {
            config = config
                .with_exclusive_zone(LayerExclusiveZone::Reserve(40))
                .unwrap();
        }
        SurfaceRequest::new(
            SurfaceId::new(id),
            WindowConfig::default().with_layer_shell(config),
        )
    })
    .collect();
    requests.push(settings);
    requests
}

struct PanelTest {
    runtime: MultiWindowRunner<PanelApp>,
    deadline: Instant,
    stage: u8,
    stage_started: Instant,
    frames: HashSet<WindowId>,
    live_input: bool,
    native_presses: usize,
}
impl ApplicationHandler for PanelTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(event_loop.layer_shell_version().is_some());
        let missing_output =
            LayerShellConfig::new("missing-output-probe", LayerAnchors::TOP, 200, 30)
                .unwrap()
                .with_output(LayerOutput::Name(
                    "rutter-test-output-that-does-not-exist".into(),
                ))
                .unwrap();
        let error = event_loop
            .create_window(
                winit::window::WindowAttributes::default().with_layer_shell(missing_output),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("output name is unavailable"),
            "{error}"
        );
        let conflicting =
            LayerShellConfig::new("conflicting-role-probe", LayerAnchors::TOP, 200, 30).unwrap();
        let error = event_loop
            .create_window(
                winit::window::WindowAttributes::default()
                    .with_layer_shell(conflicting)
                    .with_window_type(winit::window::WindowType::Popup),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("cannot also be a popup"),
            "{error}"
        );
        self.runtime.can_create_surfaces(event_loop);
    }
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.destroy_surfaces(event_loop);
    }
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "layer integration timed out at stage {}",
            self.stage
        );
        self.runtime.new_events(event_loop, cause);
    }
    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            self.frames.insert(id);
        }
        if matches!(
            event,
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                ..
            }
        ) {
            self.native_presses += 1;
        }
        self.runtime.window_event(event_loop, id, event);
        assert!(
            self.runtime.fatal_error.is_none(),
            "{:?}",
            self.runtime.fatal_error
        );
    }
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.about_to_wait(event_loop);
        assert!(
            self.runtime.fatal_error.is_none(),
            "{:?}",
            self.runtime.fatal_error
        );
        let surface = SurfaceId::new(1);
        if self.stage == 0
            && self.runtime.canonical_state.metrics.len() == 5
            && self.frames.len() >= 5
            && self.stage_started.elapsed() > Duration::from_millis(400)
        {
            if std::env::var_os("RUTTER_LAYER_MAXIMIZE").is_some() {
                let settings = self.runtime.surface_runners[&SurfaceId::new(5)]
                    .native_window()
                    .unwrap();
                if !settings.is_maximized() {
                    settings.set_maximized(true);
                    return;
                }
            }
            for id in 1..=4 {
                let metrics = &self.runtime.canonical_state.metrics[&SurfaceId::new(id)];
                assert!(metrics.layer_shell_version.is_some());
                assert_eq!(
                    self.runtime.surface_runners[&SurfaceId::new(id)]
                        .native_window()
                        .unwrap()
                        .window_type(),
                    winit::window::WindowType::LayerShell
                );
                assert!(metrics.logical_size.0 > 0.0 && metrics.logical_size.1 > 0.0);
                if let Ok(target) = std::env::var("RUTTER_LAYER_OUTPUT") {
                    assert_eq!(metrics.output_name.as_deref(), Some(target.as_str()));
                }
            }
            assert_eq!(
                self.runtime.canonical_state.metrics[&SurfaceId::new(2)].logical_size,
                (400.0, 30.0)
            );
            assert_eq!(
                self.runtime.canonical_state.metrics[&SurfaceId::new(4)].logical_size,
                (30.0, 300.0)
            );
            assert_eq!(
                self.runtime.canonical_state.metrics[&SurfaceId::new(5)].layer_shell_version,
                None
            );
            println!("LAYER_READY {:?}", self.runtime.canonical_state.metrics);
            self.stage = 1;
            if !self.live_input && std::env::var_os("RUTTER_LAYER_REMOVE_OUTPUT").is_none() {
                let native = self.runtime.routes.native_for(surface).unwrap();
                self.runtime.window_event(
                    event_loop,
                    native,
                    press(20.0, 20.0, MouseButton::Right),
                );
            }
        }
        if std::env::var_os("RUTTER_LAYER_REMOVE_OUTPUT").is_some() {
            if self.stage == 1 && self.runtime.canonical_state.closed.len() == 4 {
                let mut closed = self.runtime.canonical_state.closed.clone();
                closed.sort();
                assert_eq!(closed, (1..=4).map(SurfaceId::new).collect::<Vec<_>>());
                assert_eq!(self.runtime.surface_runners.len(), 1);
                for id in 1..=4 {
                    assert!(self.runtime.routes.native_for(SurfaceId::new(id)).is_none());
                }
                assert!(
                    self.runtime.surface_runners[&SurfaceId::new(5)]
                        .native_window()
                        .unwrap()
                        .layer_shell_version()
                        .is_none()
                );
                self.stage = 3;
                self.stage_started = Instant::now();
                println!("LAYER_OUTPUT_REMOVED");
            }
            if self.stage == 3 && self.stage_started.elapsed() > Duration::from_secs(2) {
                assert_eq!(
                    self.runtime.canonical_state.closed.len(),
                    4,
                    "output removal must not recreate targeted panels"
                );
                event_loop.exit();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(25),
            ));
            return;
        }
        if self.stage == 1 {
            let runner = &self.runtime.surface_runners[&surface];
            if let Some(popup) = runner.native_menu_window_id()
                && runner.pending_native_menu_deadline().is_none()
                && self.frames.contains(&popup)
            {
                assert!(
                    runner.native_window().unwrap().surface_size().height <= 80,
                    "menu must not enlarge layer parent"
                );
                println!("LAYER_POPUP_READY");
                self.stage = 2;
                if !self.live_input {
                    self.runtime.window_event(
                        event_loop,
                        popup,
                        press(20.0, 22.0, MouseButton::Left),
                    );
                }
            }
        }
        if self.stage == 2 && self.runtime.canonical_state.selected {
            assert!(
                self.runtime.surface_runners[&surface]
                    .native_menu_window_id()
                    .is_none()
            );
            if self.live_input {
                assert!(self.native_presses >= 2);
            }
            self.runtime.close_surface(surface).unwrap();
            assert_eq!(self.runtime.canonical_state.closed, vec![surface]);
            assert!(self.runtime.routes.native_for(surface).is_none());
            self.stage = 3;
            self.stage_started = Instant::now();
            println!("LAYER_CLOSED");
        }
        // Leave time for the external compositor probe to verify restored work area.
        if self.stage == 3 && self.stage_started.elapsed() > Duration::from_secs(2) {
            if self.live_input {
                assert!(
                    self.runtime.surface_runners[&SurfaceId::new(5)]
                        .native_window()
                        .unwrap()
                        .has_focus(),
                    "Settings focus must survive nonexclusive panel menu selection"
                );
            }
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(25),
        ));
    }
}
fn press(x: f64, y: f64, button: MouseButton) -> WindowEvent {
    WindowEvent::PointerButton {
        device_id: None,
        state: ElementState::Pressed,
        position: PhysicalPosition::new(x, y),
        primary: true,
        button: ButtonSource::Mouse(button),
        is_macos_activation_click: false,
    }
}

#[test]
#[ignore = "requires an owned disposable layer-supporting compositor and private X11 clipboard display"]
fn native_layer_shell_panels_settings_and_popup_share_runner() {
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    let mut builder = EventLoop::builder();
    builder.with_wayland().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let runtime = MultiWindowRunner::<PanelApp>::initialize_with(
        |fonts| Ok::<_, std::convert::Infallible>(PanelApp::new(fonts)),
        panel_requests(),
    )
    .unwrap();
    event_loop
        .run_app(PanelTest {
            runtime,
            deadline: Instant::now() + Duration::from_secs(25),
            stage: 0,
            stage_started: Instant::now(),
            frames: HashSet::new(),
            live_input: std::env::var_os("RUTTER_LAYER_LIVE_INPUT").is_some(),
            native_presses: 0,
        })
        .unwrap();
}

#[test]
#[ignore = "requires an owned disposable Wayland compositor without layer-shell"]
fn native_layer_shell_unavailable_is_an_error_without_toplevel_fallback() {
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    struct UnsupportedTest;
    impl ApplicationHandler for UnsupportedTest {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            assert_eq!(event_loop.layer_shell_version(), None);
            let config = LayerShellConfig::new("unsupported", LayerAnchors::TOP, 500, 30).unwrap();
            let error = event_loop
                .create_window(winit::window::WindowAttributes::default().with_layer_shell(config))
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("zwlr_layer_shell_v1 is unavailable"),
                "{error}"
            );
            let window = event_loop
                .create_window(winit::window::WindowAttributes::default())
                .unwrap();
            assert_eq!(window.layer_shell_version(), None);
            event_loop.exit();
        }
        fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut builder = EventLoop::builder();
    builder.with_wayland().with_any_thread(true);
    builder.build().unwrap().run_app(UnsupportedTest).unwrap();
}
