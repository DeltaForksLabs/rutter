//! Run only on a dedicated X11 display.

use std::time::{Duration, Instant};

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::ConnectionExt as _;

struct ParentDestructionTest {
    parent_id: Option<WindowId>,
    popup: Option<Box<dyn Window>>,
    close_requested: bool,
    deadline: Instant,
}

impl Default for ParentDestructionTest {
    fn default() -> Self {
        Self {
            parent_id: None,
            popup: None,
            close_requested: false,
            deadline: Instant::now() + Duration::from_secs(5),
        }
    }
}

impl ApplicationHandler for ParentDestructionTest {
    fn can_create_surfaces(&mut self, loop_: &dyn ActiveEventLoop) {
        loop_.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        let parent = loop_.create_window(WindowAttributes::default()).unwrap();
        let handle = parent.window_handle().unwrap().as_raw();
        let popup = loop_
            .create_window(unsafe {
                WindowAttributes::default()
                    .with_window_type(WindowType::Popup)
                    .with_parent_window(Some(handle))
            })
            .unwrap();
        self.parent_id = Some(parent.id());
        self.popup = Some(popup);
        drop(parent);
    }

    fn window_event(&mut self, loop_: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) == self.popup.as_ref().map(|popup| popup.id())
            && matches!(event, WindowEvent::CloseRequested)
        {
            assert!(!self.close_requested);
            self.close_requested = true;
            let popup = self.popup.take().unwrap();
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
            drop(popup);
        }
        if Some(id) == self.parent_id && matches!(event, WindowEvent::Destroyed) {
            assert!(
                self.close_requested,
                "popup must close before parent destruction is delivered"
            );
            loop_.exit();
        }
    }

    fn about_to_wait(&mut self, loop_: &dyn ActiveEventLoop) {
        assert!(Instant::now() < self.deadline, "parent destruction was not delivered in time");
        loop_.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
    }
}

#[test]
#[ignore = "requires a live X11 display"]
fn destroying_parent_dismisses_popup_and_releases_grab() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    builder.build().unwrap().run_app(ParentDestructionTest::default()).unwrap();
}
