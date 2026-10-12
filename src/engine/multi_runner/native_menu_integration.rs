//! Native overflow routing through two independent parent engines.

use std::time::Duration;
use taffy::prelude::{Dimension, Size, Style};
use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton};
use winit::event_loop::EventLoop;

use super::*;

#[derive(Clone)]
struct MenuModel {
    entries: Vec<crate::ContextMenuEntry<'static, MenuMessage>>,
    sources: Vec<SurfaceId>,
}

#[derive(Clone, Debug)]
enum MenuMessage {
    Select,
    Failure(crate::NativeMenuFailure),
}

struct MenuApp;

impl MultiWindowAppLogic for MenuApp {
    type State = MenuModel;
    type Message = MenuMessage;

    fn new(_: &mut FontSystem) -> MenuModel {
        MenuModel {
            entries: vec![crate::ContextMenuEntry::item(
                "Settings",
                MenuMessage::Select,
            )],
            sources: Vec::new(),
        }
    }

    fn initial_surfaces() -> Vec<SurfaceRequest> {
        [(1, 800, 30), (2, 30, 600)]
            .into_iter()
            .map(|(id, width, height)| {
                SurfaceRequest::new(
                    SurfaceId::new(id),
                    WindowConfig::default()
                        .with_decorations(false)
                        .with_inner_size(width, height)
                        .unwrap(),
                )
            })
            .collect()
    }

    fn view<'a>(state: &'a mut MenuModel, _: SurfaceId) -> crate::Widget<'a, MenuMessage> {
        crate::Widget::context_menu(
            crate::Widget::Spacer {
                style: Default::default(),
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
        .with_id(980)
    }

    fn update(
        state: &mut MenuModel,
        surface: SurfaceId,
        message: MenuMessage,
        _: &mut arboard::Clipboard,
    ) -> Vec<SurfaceCommand> {
        if let MenuMessage::Failure(failure) = message {
            assert_eq!(failure.menu_id, 980);
            assert!(!failure.overlay_usable);
        }
        state.sources.push(surface);
        Vec::new()
    }

    fn native_menu_unavailable(
        _: &MenuModel,
        _: SurfaceId,
        failure: crate::NativeMenuFailure,
    ) -> Option<MenuMessage> {
        Some(MenuMessage::Failure(failure))
    }
}

struct MenuRoutingTest {
    runtime: MultiWindowRunner<MenuApp>,
    opened: usize,
    pressed: usize,
    popup_frames: HashSet<WindowId>,
    start: Instant,
    deadline: Instant,
    cpu: bool,
}

impl ApplicationHandler for MenuRoutingTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.can_create_surfaces(event_loop);
    }
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.destroy_surfaces(event_loop);
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "multi-window native menu routing timed out"
        );
        self.runtime.new_events(event_loop, cause);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested)
            && self.runtime.routes.surface_for(id).is_none()
        {
            self.popup_frames.insert(id);
        }
        self.runtime.window_event(event_loop, id, event);
        assert!(self.runtime.fatal_error.is_none());
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.about_to_wait(event_loop);
        let sources = &self.runtime.canonical_state.sources;
        if sources.len() == 2 {
            assert_eq!(sources, &[SurfaceId::new(1), SurfaceId::new(2)]);
            assert!(
                self.runtime
                    .surface_runners
                    .values()
                    .all(|runner| runner.native_menu_window_id().is_none())
            );
            event_loop.exit();
            return;
        }
        if self.start.elapsed() >= Duration::from_millis(200) && self.opened == sources.len() {
            let surface = SurfaceId::new(self.opened as u64 + 1);
            let native = self.runtime.routes.native_for(surface).unwrap();
            self.opened += 1;
            self.runtime
                .window_event(event_loop, native, press(20.0, 20.0, MouseButton::Right));
        }
        if !self.cpu && self.pressed < self.opened {
            let surface = SurfaceId::new(self.opened as u64);
            let runner = &self.runtime.surface_runners[&surface];
            if let Some(popup) = runner.native_menu_window_id()
                && runner.pending_native_menu_deadline().is_none()
                && self.popup_frames.contains(&popup)
            {
                self.pressed += 1;
                self.runtime
                    .window_event(event_loop, popup, press(20.0, 22.0, MouseButton::Left));
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(50),
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

fn run_routing_test(cpu: bool) {
    let mut builder = EventLoop::builder();
    winit::platform::wayland::EventLoopBuilderExtWayland::with_wayland(&mut builder);
    winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    let mut runtime = MultiWindowRunner::<MenuApp>::initialize().unwrap();
    runtime.backend_preference.retained = Some(if cpu {
        BackendType::CpuSoftbuffer
    } else {
        BackendType::OpenGl
    });
    builder
        .build()
        .unwrap()
        .run_app(MenuRoutingTest {
            runtime,
            opened: 0,
            pressed: 0,
            popup_frames: HashSet::new(),
            start: Instant::now(),
            deadline: Instant::now() + Duration::from_secs(20),
            cpu,
        })
        .unwrap();
}

#[test]
#[ignore = "requires a disposable Wayland compositor and alpha-capable EGL; synthetic runner input"]
fn native_multi_window_thin_parents_route_selection_to_invoking_surface() {
    run_routing_test(false);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with wl_shm; synthetic runner input"]
fn native_multi_window_failures_route_to_invoking_surface() {
    run_routing_test(true);
}
