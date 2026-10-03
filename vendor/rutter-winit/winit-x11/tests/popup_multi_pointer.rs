//! Run only on a disposable X11 display: this test changes the device hierarchy.

use std::time::{Duration, Instant};

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::{
    ConnectionExt as _, DeviceType, HierarchyChange, HierarchyChangeData,
    HierarchyChangeDataAddMaster, XIEventMask,
};
use x11rb::protocol::xproto::{GrabMode, GrabStatus};

fn add_master(name: &[u8]) {
    let (connection, _) = x11rb::connect(None).unwrap();
    let name_len = name.len();
    let change = HierarchyChange {
        len: (8 + name_len).div_ceil(4) as u16,
        data: HierarchyChangeData::AddMaster(HierarchyChangeDataAddMaster {
            send_core: false,
            enable: true,
            name: name.to_vec(),
        }),
    };
    connection.xinput_xi_change_hierarchy(&[change]).unwrap().check().unwrap();
}

struct MultiplePointersTest {
    parent: Option<Box<dyn Window>>,
    popup: Option<Box<dyn Window>>,
    deadline: Instant,
}

impl Default for MultiplePointersTest {
    fn default() -> Self {
        Self { parent: None, popup: None, deadline: Instant::now() + Duration::from_secs(5) }
    }
}

impl ApplicationHandler for MultiplePointersTest {
    fn can_create_surfaces(&mut self, loop_: &dyn ActiveEventLoop) {
        loop_.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        add_master(b"rutter popup second");
        let parent = loop_.create_window(WindowAttributes::default()).unwrap();
        let handle = parent.window_handle().unwrap().as_raw();
        let popup = loop_
            .create_window(unsafe {
                WindowAttributes::default()
                    .with_window_type(WindowType::Popup)
                    .with_parent_window(Some(handle))
            })
            .unwrap();
        self.parent = Some(parent);
        self.popup = Some(popup);
        add_master(b"rutter popup third");
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        assert!(
            !matches!(event, WindowEvent::CloseRequested),
            "popup grab failed after hierarchy change"
        );
    }

    fn about_to_wait(&mut self, loop_: &dyn ActiveEventLoop) {
        assert!(Instant::now() < self.deadline, "new master pointers were not grabbed in time");
        loop_.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50)));
        let (connection, screen) = x11rb::connect(None).unwrap();
        let root = connection.setup().roots[screen].root;
        let masters = connection
            .xinput_xi_query_device(1u16)
            .unwrap()
            .reply()
            .unwrap()
            .infos
            .into_iter()
            .filter(|info| info.enabled && info.type_ == DeviceType::MASTER_POINTER)
            .collect::<Vec<_>>();
        if masters.len() < 3 {
            return;
        }
        for master in masters {
            let reply = connection
                .xinput_xi_grab_device(
                    root,
                    x11rb::CURRENT_TIME,
                    0u32,
                    master.deviceid,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                    false.into(),
                    &[u32::from(XIEventMask::BUTTON_PRESS)],
                )
                .unwrap()
                .reply()
                .unwrap();
            if reply.status == GrabStatus::SUCCESS {
                connection
                    .xinput_xi_ungrab_device(x11rb::CURRENT_TIME, master.deviceid)
                    .unwrap()
                    .check()
                    .unwrap();
            }
            assert_eq!(
                reply.status,
                GrabStatus::ALREADY_GRABBED,
                "pointer {} not captured",
                master.deviceid
            );
        }
        self.popup.take();
        loop_.exit();
    }
}

#[test]
#[ignore = "requires a disposable X11 display with XI2 master-device changes"]
fn popup_grabs_existing_and_new_master_pointers() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    builder.build().unwrap().run_app(MultiplePointersTest::default()).unwrap();
}
