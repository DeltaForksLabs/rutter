//! Run only on a disposable X11 display: XTEST moves the pointer and clicks.

use std::time::{Duration, Instant};

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest::ConnectionExt as _;

struct DismissalTest {
    parent: Option<Box<dyn Window>>,
    popup: Option<Box<dyn Window>>,
    close_requests: usize,
    deadline: Instant,
}

impl Default for DismissalTest {
    fn default() -> Self {
        Self {
            parent: None,
            popup: None,
            close_requests: 0,
            deadline: Instant::now() + Duration::from_secs(5),
        }
    }
}

impl ApplicationHandler for DismissalTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        event_loop
            .set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        let parent =
            event_loop.create_window(WindowAttributes::default().with_visible(false)).unwrap();
        let handle = parent.window_handle().unwrap().as_raw();
        let popup = event_loop
            .create_window(
                unsafe {
                    WindowAttributes::default()
                        .with_window_type(WindowType::Popup)
                        .with_parent_window(Some(handle))
                }
                .with_position(dpi::PhysicalPosition::new(100, 100))
                .with_surface_size(dpi::PhysicalSize::new(80, 40)),
            )
            .unwrap();
        let (connection, screen) = x11rb::connect(None).unwrap();
        let root = connection.setup().roots[screen].root;
        connection.warp_pointer(0u32, root, 0, 0, 0, 0, 0, 0).unwrap().check().unwrap();
        connection.xtest_fake_input(4, 1, 0, root, 0, 0, 0).unwrap().check().unwrap();
        connection.xtest_fake_input(5, 1, 0, root, 0, 0, 0).unwrap().check().unwrap();
        self.parent = Some(parent);
        self.popup = Some(popup);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) == self.popup.as_ref().map(|popup| popup.id())
            && matches!(event, WindowEvent::CloseRequested)
        {
            self.close_requests += 1;
            assert_eq!(self.close_requests, 1);
            self.popup.take();
            let (connection, screen) = x11rb::connect(None).unwrap();
            let root = connection.setup().roots[screen].root;
            let grab = connection
                .xinput_xi_grab_device(
                    root,
                    x11rb::CURRENT_TIME,
                    0u32,
                    2u16,
                    x11rb::protocol::xproto::GrabMode::ASYNC,
                    x11rb::protocol::xproto::GrabMode::ASYNC,
                    false.into(),
                    &[u32::from(x11rb::protocol::xinput::XIEventMask::BUTTON_PRESS)],
                )
                .unwrap()
                .reply()
                .unwrap();
            assert_eq!(grab.status, x11rb::protocol::xproto::GrabStatus::SUCCESS);
            connection.xinput_xi_ungrab_device(x11rb::CURRENT_TIME, 2u16).unwrap().check().unwrap();
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(Instant::now() < self.deadline, "outside click was not delivered in time");
        event_loop
            .set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
    }
}

#[test]
#[ignore = "requires a disposable X11 display with XTEST"]
fn outside_click_requests_close_and_releases_grab() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    builder.build().unwrap().run_app(DismissalTest::default()).unwrap();
}
