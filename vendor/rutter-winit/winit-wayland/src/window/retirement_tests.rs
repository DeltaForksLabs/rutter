//! Real native objects retained across terminal closure; run only on an owned compositor.
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dpi::{LogicalSize, PhysicalSize};
use rwh_06::{HandleError, HasDisplayHandle, HasWindowHandle};
use sctk::reexports::client::protocol::wl_shm::Format;
use sctk::shell::WaylandSurface;
use sctk::shell::wlr_layer::LayerShellHandler;
use sctk::shell::xdg::window::WindowHandler;
use sctk::shm::slot::{Buffer, SlotPool};
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;
use winit_core::application::ApplicationHandler;
use winit_core::event::{StartCause, WindowEvent};
use winit_core::event_loop::{ActiveEventLoop as CoreActiveEventLoop, ControlFlow};
use winit_core::window::{
    CursorGrabMode, LayerAnchors, LayerExclusiveZone, LayerOutput, LayerShellConfig,
    Window as CoreWindow, WindowAttributes, WindowId, WindowType as CoreWindowType,
};

use super::state::{WindowState, WindowType};
use crate::{ActiveEventLoop, EventLoop, Window};

struct RetainedLayerTest {
    windows: Vec<Box<dyn CoreWindow>>,
    retained_states: Vec<Arc<Mutex<WindowState>>>,
    surfaces: Vec<WlSurface>,
    buffers: Vec<Buffer>,
    pools: Vec<SlotPool>,
    destroyed: Vec<WindowId>,
    normal_close_seen: bool,
    triggered: bool,
    compositor_output_removal: bool,
    start: Instant,
    retired_at: Option<Instant>,
}

impl RetainedLayerTest {
    fn present(&mut self, event_loop: &ActiveEventLoop, window: &dyn CoreWindow) {
        let state = event_loop.state.borrow();
        let window_state = state.windows.borrow().get(&window.id()).unwrap().clone();
        let surface = window_state.lock().unwrap().window.wl_surface().clone();
        let size = window.surface_size();
        let mut pool = SlotPool::new((size.width * size.height * 4) as usize, &state.shm).unwrap();
        let (buffer, pixels) = pool
            .create_buffer(
                size.width as i32,
                size.height as i32,
                size.width as i32 * 4,
                Format::Argb8888,
            )
            .unwrap();
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&0xff335577_u32.to_ne_bytes());
        }
        window.pre_present_notify();
        buffer.attach_to(&surface).unwrap();
        surface.damage_buffer(0, 0, size.width as i32, size.height as i32);
        surface.commit();
        self.retained_states.push(window_state);
        self.surfaces.push(surface);
        self.buffers.push(buffer);
        self.pools.push(pool);
    }

    fn assert_frontends_revoked(&self) {
        for window in &self.windows[..3] {
            assert!(matches!(window.window_handle(), Err(HandleError::Unavailable)));
            assert!(matches!(window.display_handle(), Err(HandleError::Unavailable)));
            assert_eq!(window.surface_size(), PhysicalSize::default());
            assert!(window.request_surface_size(LogicalSize::new(200, 60).into()).is_none());
            assert!(window.set_cursor_hittest(false).is_err());
            for policy in [
                winit_core::window::LayerKeyboardInteractivity::None,
                winit_core::window::LayerKeyboardInteractivity::OnDemand,
                winit_core::window::LayerKeyboardInteractivity::Exclusive,
            ] {
                assert!(matches!(
                    window.set_layer_keyboard_interactivity(policy),
                    Err(winit_core::error::RequestError::Ignored)
                ));
            }
            for policy in [
                winit_core::window::SurfaceInputRegion::Full,
                winit_core::window::SurfaceInputRegion::Empty,
                winit_core::window::SurfaceInputRegion::rectangles([
                    winit_core::window::SurfaceInputRect::new(-4, 0, 16, 8).unwrap(),
                ])
                .unwrap(),
            ] {
                assert!(matches!(
                    window.set_surface_input_region(&policy),
                    Err(winit_core::error::RequestError::Ignored)
                ));
            }
            assert!(window.set_cursor_grab(CursorGrabMode::None).is_err());
            window.request_redraw();
            window.pre_present_notify();
            window.set_transparent(true);
            window.set_blur(true);
            window.set_cursor_visible(false);
            window.request_user_attention(None);
        }
        let layer = self.windows[0].cast_ref::<Window>().unwrap();
        assert!(layer.request_activation_token().is_err());
        assert_eq!(layer.layer_shell_version(), None);
        assert!(
            self.windows[3].window_handle().is_ok(),
            "ordinary close requests must remain vetoable"
        );
    }

    fn trigger_callbacks(&self, event_loop: &ActiveEventLoop) {
        let normal = {
            let state = self.retained_states[3].lock().unwrap();
            let WindowType::Window { window, .. } = &state.window else {
                panic!("expected toplevel")
            };
            window.clone()
        };
        let connection = &event_loop.handle.connection;
        let queue = &event_loop.queue_handle;
        WindowHandler::request_close(
            &mut *event_loop.state.borrow_mut(),
            connection,
            queue,
            &normal,
        );
        if self.compositor_output_removal {
            return;
        }
        let layer = {
            let state = self.retained_states[0].lock().unwrap();
            let WindowType::Layer { layer, .. } = &state.window else { panic!("expected layer") };
            layer.clone()
        };
        // Invoke the production SCTK callback on an actual configured/mapped role.
        LayerShellHandler::closed(&mut *event_loop.state.borrow_mut(), connection, queue, &layer);
        self.assert_frontends_revoked();
    }
}

impl ApplicationHandler for RetainedLayerTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn CoreActiveEventLoop) {
        let native = event_loop.cast_ref::<ActiveEventLoop>().unwrap();
        let target =
            std::env::var("RUTTER_LAYER_OUTPUT").expect("owned output name must be supplied");
        let config = LayerShellConfig::new("rutter-retained-layer", LayerAnchors::TOP, 320, 40)
            .unwrap()
            .with_output(LayerOutput::Name(target))
            .unwrap()
            .with_exclusive_zone(LayerExclusiveZone::Reserve(40))
            .unwrap();
        let layer =
            event_loop.create_window(WindowAttributes::default().with_layer_shell(config)).unwrap();
        self.present(native, layer.as_ref());
        self.windows.push(layer);
        for parent in 0..2 {
            let parent_handle = self.windows[parent].window_handle().unwrap().as_raw();
            // SAFETY: all parents are retained through both their children and this test.
            let attributes = unsafe {
                WindowAttributes::default()
                    .with_window_type(CoreWindowType::Popup)
                    .with_parent_window(Some(parent_handle))
            }
            .with_surface_size(LogicalSize::new(100, 60))
            .with_active(false);
            let popup = event_loop.create_window(attributes).unwrap();
            self.present(native, popup.as_ref());
            self.windows.push(popup);
        }
        let normal = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("Retained ordinary Settings")
                    .with_decorations(false),
            )
            .unwrap();
        self.present(native, normal.as_ref());
        self.windows.push(normal);
    }

    fn new_events(&mut self, _: &dyn CoreActiveEventLoop, _: StartCause) {
        assert!(self.start.elapsed() < Duration::from_secs(15), "retained lifetime test timed out");
        if self.triggered && !self.compositor_output_removal {
            self.assert_frontends_revoked();
        }
    }

    fn window_event(&mut self, _: &dyn CoreActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            // Deliberately ignore every close request and retain every frontend and Arc.
            WindowEvent::CloseRequested if id == self.windows[3].id() => {
                self.normal_close_seen = true;
            },
            WindowEvent::Destroyed => {
                assert_ne!(id, self.windows[3].id());
                self.assert_frontends_revoked();
                self.destroyed.push(id);
                self.retired_at.get_or_insert_with(Instant::now);
            },
            WindowEvent::RedrawRequested if self.destroyed.contains(&id) => {
                panic!("redraw after retirement")
            },
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn CoreActiveEventLoop) {
        let native = event_loop.cast_ref::<ActiveEventLoop>().unwrap();
        if !self.triggered && self.start.elapsed() > Duration::from_millis(300) {
            self.trigger_callbacks(native);
            self.triggered = true;
            writeln!(std::io::stdout(), "RETAINED_LAYER_READY").unwrap();
        }
        if self.destroyed.len() == 3 {
            assert_eq!(self.destroyed, vec![
                self.windows[2].id(),
                self.windows[1].id(),
                self.windows[0].id()
            ]);
            assert!(self.normal_close_seen);
            for (state, surface) in self.retained_states[..3].iter().zip(&self.surfaces[..3]) {
                let mut state = state.lock().unwrap();
                assert!(matches!(state.window, WindowType::Retired { .. }));
                assert!(surface.is_alive(), "renderer cleanup still needs the backing wl_surface");
                let WindowType::Retired { _owner: Some(owner), .. } = &state.window else {
                    panic!("missing cleanup owner")
                };
                match owner.as_ref() {
                    WindowType::Layer { layer, .. } => {
                        let sctk::shell::wlr_layer::SurfaceKind::Wlr(role) = layer.kind() else {
                            panic!("unknown layer role")
                        };
                        assert!(!role.is_alive());
                    },
                    WindowType::Popup { popup, .. } => {
                        assert!(!popup.xdg_popup().is_alive());
                        assert!(!popup.xdg_surface().is_alive());
                    },
                    _ => panic!("unexpected retired role"),
                }
                assert!(state.children().is_empty());
                state.request_frame_callback();
                state.reload_transparency_hint();
                state.set_scale_factor(2.0);
                assert!(!state.set_blur(true));
            }
            assert!(self.windows[0].cast_ref::<Window>().unwrap().surface().is_alive());
            self.assert_frontends_revoked();
            if self.retired_at.unwrap().elapsed() > Duration::from_millis(300) {
                writeln!(std::io::stdout(), "RETAINED_LAYER_RETIRED_CHILDREN_FIRST").unwrap();
                event_loop.exit();
            }
        }
        event_loop
            .set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(20)));
    }
}

fn run_retained_layer_test(compositor_output_removal: bool) {
    let mut event_loop = EventLoop::new().unwrap();
    let mut test = RetainedLayerTest {
        windows: Vec::new(),
        retained_states: Vec::new(),
        surfaces: Vec::new(),
        buffers: Vec::new(),
        pools: Vec::new(),
        destroyed: Vec::new(),
        normal_close_seen: false,
        triggered: false,
        compositor_output_removal,
        start: Instant::now(),
        retired_at: None,
    };
    event_loop.run_app_on_demand(&mut test).unwrap();
    assert_eq!(test.windows.len(), 4, "frontends must still be retained at test completion");
    assert_eq!(test.destroyed.len(), 3);
    // Render resources are released before their retained windows/states.
    test.buffers.clear();
    test.pools.clear();
    test.windows.clear();
    test.retained_states.clear();
    assert!(test.surfaces[..3].iter().all(|surface| !surface.is_alive()));
}

#[test]
#[ignore = "requires an owned disposable layer compositor; production closed callback is invoked \
            explicitly"]
fn retained_layer_closed_callback_retires_native_lifetime_and_children() {
    run_retained_layer_test(false);
}

#[test]
#[ignore = "requires an owned disposable two-output compositor and external targeted output removal"]
fn retained_layer_output_removal_retires_native_lifetime_and_children() {
    run_retained_layer_test(true);
}
