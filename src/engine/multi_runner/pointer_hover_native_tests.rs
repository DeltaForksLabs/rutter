//! Opt-in X11 integration: actual unpressed native mouse motion and leave.
use super::pointer_hover_tests::{HoverApp, HoverModel};
use super::*;
use std::time::Duration;
use winit::event_loop::EventLoop;
use winit::platform::x11::EventLoopBuilderExtX11;
use x11rb::protocol::xproto::ConnectionExt as _;

struct NativeHoverProbe {
    runtime: MultiWindowRunner<HoverApp>,
    stage: u8,
    rendered: bool,
    deadline: Instant,
}

impl NativeHoverProbe {
    fn move_pointer(&self, x: i16, y: i16) {
        let id = self.runtime.routes.native_for(SurfaceId::PRIMARY).unwrap();
        self.move_on(id, x, y);
    }

    fn move_on(&self, id: WindowId, x: i16, y: i16) {
        let (connection, _) = x11rb::connect(None).unwrap();
        let window = id.into_raw() as u32;
        let root = connection.query_tree(window).unwrap().reply().unwrap().root;
        let origin = connection
            .translate_coordinates(window, root, 0, 0)
            .unwrap()
            .reply()
            .unwrap();
        connection
            .warp_pointer(0u32, root, 0, 0, 0, 0, origin.dst_x + x, origin.dst_y + y)
            .unwrap()
            .check()
            .unwrap();
    }
}

impl ApplicationHandler for NativeHoverProbe {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.can_create_surfaces(event_loop);
        let window = self.runtime.surface_runners[&SurfaceId::PRIMARY]
            .native_window()
            .unwrap();
        for policy in [
            crate::LayerKeyboardInteractivity::None,
            crate::LayerKeyboardInteractivity::OnDemand,
            crate::LayerKeyboardInteractivity::Exclusive,
        ] {
            assert!(
                matches!(
                    window.set_layer_keyboard_interactivity(policy),
                    Err(winit::error::RequestError::NotSupported(_))
                ),
                "X11 must explicitly reject layer-only keyboard policy"
            );
        }
    }
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.destroy_surfaces(event_loop);
    }
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "native unpressed mouse hover/leave timed out at stage {}",
            self.stage
        );
        self.runtime.new_events(event_loop, cause);
    }
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        native: WindowId,
        event: WindowEvent,
    ) {
        self.rendered |= matches!(event, WindowEvent::RedrawRequested);
        assert!(
            !matches!(event, WindowEvent::PointerButton { .. }),
            "hover must need no press"
        );
        let menu_motion = self.stage == 5
            && matches!(event, WindowEvent::PointerMoved { .. })
            && self.runtime.surface_runners[&SurfaceId::PRIMARY].native_menu_window_id()
                == Some(native);
        let reports_before = self.runtime.canonical_state.reports.clone();
        self.runtime.window_event(event_loop, native, event);
        assert!(self.runtime.fatal_error.is_none());
        if menu_motion {
            assert_eq!(
                self.runtime.canonical_state.reports, reports_before,
                "native menu motion must not be observed as its parent surface"
            );
            self.stage = 6;
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.about_to_wait(event_loop);
        if !self.rendered {
            return;
        }
        let reports = &self.runtime.canonical_state.reports;
        let at = |x: f32, y: f32| {
            reports.iter().any(|(surface, point)| {
                *surface == SurfaceId::PRIMARY
                    && point.is_some_and(|point| {
                        (point.x() - x).abs() < 1.0 && (point.y() - y).abs() < 1.0
                    })
            })
        };
        match self.stage {
            0 => {
                self.stage = 1;
                self.move_pointer(60, 22);
            }
            1 if at(60.0, 22.0) => {
                self.stage = 2;
                self.move_pointer(80, 30);
            }
            2 if at(80.0, 30.0) => {
                self.runtime.canonical_state.reports.clear();
                self.stage = 3;
                self.move_pointer(300, 200);
            }
            3 if reports.iter().any(|(_, point)| point.is_none()) => {
                assert!(self.runtime.canonical_state.actions.is_empty());
                self.runtime.canonical_state.menu = true;
                self.runtime.publish_surface_state(
                    self.runtime.canonical_state.clone(),
                    self.runtime.revision + 1,
                );
                // Open a real native menu through normal runner input. No button
                // was needed for the preceding native hover/leave assertions.
                let native = self.runtime.routes.native_for(SurfaceId::PRIMARY).unwrap();
                for state in [
                    winit::event::ElementState::Pressed,
                    winit::event::ElementState::Released,
                ] {
                    self.runtime.window_event(
                        event_loop,
                        native,
                        WindowEvent::PointerButton {
                            device_id: None,
                            state,
                            position: winit::dpi::PhysicalPosition::new(60.0, 22.0),
                            primary: true,
                            button: winit::event::ButtonSource::Mouse(
                                winit::event::MouseButton::Left,
                            ),
                            is_macos_activation_click: false,
                        },
                    );
                }
                self.stage = 4;
            }
            4 => {
                let runner = &self.runtime.surface_runners[&SurfaceId::PRIMARY];
                if let Some(native) = runner.native_menu_window_id()
                    && runner.pending_native_menu_deadline().is_none()
                {
                    self.stage = 5;
                    self.move_on(native, 20, 22);
                }
            }
            _ => {}
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
    }
}

#[test]
#[ignore = "requires an isolated X11 display; run through /tmp/opencode/with_xvfb.py"]
fn native_unpressed_mouse_reports_hover_leave_and_isolates_native_menu_motion() {
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let mut runtime = MultiWindowRunner::<HoverApp>::initialize().unwrap();
    runtime.canonical_state = HoverModel::default();
    runtime.pending_surfaces = vec![SurfaceRequest::new(
        SurfaceId::PRIMARY,
        WindowConfig::default()
            .with_position(100, 100)
            .with_decorations(false)
            .with_inner_size(200, 100)
            .unwrap(),
    )];
    runtime.backend_preference.retained = Some(BackendType::OpenGl);
    builder
        .build()
        .unwrap()
        .run_app(NativeHoverProbe {
            runtime,
            stage: 0,
            rendered: false,
            deadline: Instant::now() + Duration::from_secs(10),
        })
        .unwrap();
}
