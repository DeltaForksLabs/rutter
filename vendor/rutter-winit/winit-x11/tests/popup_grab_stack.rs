//! Run only on a dedicated X11 display.

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{WindowAttributes, WindowId, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::ConnectionExt as _;
use x11rb::protocol::xproto::{GrabMode, GrabStatus};

fn another_client_grab_status() -> GrabStatus {
    let (connection, screen) = x11rb::connect(None).unwrap();
    let root = connection.setup().roots[screen].root;
    let status = connection
        .xinput_xi_grab_device(
            root,
            x11rb::CURRENT_TIME,
            0u32,
            2u16,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            false.into(),
            &[u32::from(x11rb::protocol::xinput::XIEventMask::BUTTON_PRESS)],
        )
        .unwrap()
        .reply()
        .unwrap()
        .status;
    if status == GrabStatus::SUCCESS {
        connection.xinput_xi_ungrab_device(x11rb::CURRENT_TIME, 2u16).unwrap().check().unwrap();
    }
    status
}

struct GrabStackTest;

impl ApplicationHandler for GrabStackTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let parent = event_loop.create_window(WindowAttributes::default()).unwrap();
        let handle = parent.window_handle().unwrap().as_raw();
        let popup_attributes = || unsafe {
            WindowAttributes::default()
                .with_window_type(WindowType::Popup)
                .with_parent_window(Some(handle))
        };
        let first = event_loop.create_window(popup_attributes()).unwrap();
        let second = event_loop.create_window(popup_attributes()).unwrap();
        assert_eq!(another_client_grab_status(), GrabStatus::ALREADY_GRABBED);
        drop(second);
        assert_eq!(another_client_grab_status(), GrabStatus::ALREADY_GRABBED);
        drop(first);
        assert_eq!(another_client_grab_status(), GrabStatus::SUCCESS);
        event_loop.exit();
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

#[test]
#[ignore = "requires a live X11 display"]
fn dropping_topmost_popup_restores_previous_grab() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    builder.build().unwrap().run_app(GrabStackTest).unwrap();
}
