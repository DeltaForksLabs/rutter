//! Run only on a dedicated X11 display.

use std::time::{Duration, Instant};

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::protocol::xproto::ConnectionExt as _;

struct ParentMovementTest {
    parent: Option<Box<dyn Window>>,
    popup: Option<Box<dyn Window>>,
    configured: bool,
    repositioned: bool,
    deadline: Instant,
}

impl Default for ParentMovementTest {
    fn default() -> Self {
        Self {
            parent: None,
            popup: None,
            configured: false,
            repositioned: false,
            deadline: Instant::now() + Duration::from_secs(5),
        }
    }
}

impl ApplicationHandler for ParentMovementTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        event_loop
            .set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        let parent = event_loop.create_window(WindowAttributes::default()).unwrap();
        let handle = parent.window_handle().unwrap().as_raw();
        let popup = event_loop
            .create_window(
                unsafe {
                    WindowAttributes::default()
                        .with_window_type(WindowType::Popup)
                        .with_parent_window(Some(handle))
                }
                .with_position(dpi::PhysicalPosition::new(20, 30))
                .with_surface_size(dpi::PhysicalSize::new(120, 60)),
            )
            .unwrap();
        self.popup = Some(popup);
        self.parent = Some(parent);
        self.parent
            .as_ref()
            .unwrap()
            .set_outer_position(dpi::PhysicalPosition::new(150, 150).into());
        let _ = self
            .parent
            .as_ref()
            .unwrap()
            .request_surface_size(dpi::PhysicalSize::new(320, 240).into());
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) == self.parent.as_ref().map(|window| window.id())
            && matches!(event, WindowEvent::SurfaceResized(_))
        {
            self.configured = true;
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        event_loop
            .set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        assert!(Instant::now() < self.deadline, "parent configure was not delivered in time");
        if !self.configured {
            return;
        }
        let (parent, popup) = (self.parent.as_ref().unwrap(), self.popup.as_ref().unwrap());
        let (connection, _) = x11rb::connect(None).unwrap();
        let root =
            connection.query_tree(parent.id().into_raw() as u32).unwrap().reply().unwrap().parent;
        let origin = connection
            .translate_coordinates(parent.id().into_raw() as u32, root, 0, 0)
            .unwrap()
            .reply()
            .unwrap();
        if origin.dst_x < 100 || origin.dst_y < 100 {
            return;
        }
        if !self.repositioned {
            assert_eq!(popup.outer_position().unwrap(), dpi::PhysicalPosition::new(20, 30));
            popup.set_outer_position(dpi::PhysicalPosition::new(60, 45).into());
            self.repositioned = true;
            return;
        }
        let popup_origin = connection
            .translate_coordinates(popup.id().into_raw() as u32, root, 0, 0)
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(popup.outer_position().unwrap(), dpi::PhysicalPosition::new(60, 45));
        assert_eq!(popup_origin.dst_x as i32, origin.dst_x as i32 + 60);
        assert_eq!(popup_origin.dst_y as i32, origin.dst_y as i32 + 45);
        event_loop.exit();
    }
}

#[test]
#[ignore = "requires a live X11 display"]
fn popup_follows_parent_movement() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    builder.build().unwrap().run_app(ParentMovementTest::default()).unwrap();
}
