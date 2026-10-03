//! Run manually with `DISPLAY=:0 cargo test -p winit-x11 --test popup -- --ignored`.

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{
    WindowAnchor, WindowAttributes, WindowConstraintAdjustment, WindowGravity, WindowId,
    WindowPositioner, WindowType,
};
use x11rb::protocol::xinput::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, WindowClass};

struct PopupSmokeTest;

impl ApplicationHandler for PopupSmokeTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(
            event_loop
                .create_window(WindowAttributes::default().with_window_type(WindowType::Popup))
                .is_err()
        );
        let parent =
            event_loop.create_window(WindowAttributes::default().with_visible(false)).unwrap();
        let parent_handle = parent.window_handle().unwrap().as_raw();
        let attributes = unsafe {
            WindowAttributes::default()
                .with_window_type(WindowType::Popup)
                .with_parent_window(Some(parent_handle))
        }
        .with_position(dpi::PhysicalPosition::new(20, 30))
        .with_surface_size(dpi::PhysicalSize::new(120, 60))
        .with_active(false);
        let popup = event_loop.create_window(attributes).unwrap();

        assert_eq!(popup.window_type(), WindowType::Popup);
        let (connection, _) = x11rb::connect(None).unwrap();
        let popup_id = match popup.window_handle().unwrap().as_raw() {
            rwh_06::RawWindowHandle::Xlib(handle) => handle.window as u32,
            rwh_06::RawWindowHandle::Xcb(handle) => handle.window.get(),
            _ => panic!("expected an X11 popup handle"),
        };
        let properties = connection.get_window_attributes(popup_id).unwrap().reply().unwrap();
        assert!(properties.override_redirect);
        assert_eq!(properties.class, WindowClass::INPUT_OUTPUT);
        let root =
            connection.query_tree(parent.id().into_raw() as u32).unwrap().reply().unwrap().parent;
        assert_eq!(connection.query_tree(popup_id).unwrap().reply().unwrap().parent, root);

        let transient_atom =
            connection.intern_atom(false, b"WM_TRANSIENT_FOR").unwrap().reply().unwrap().atom;
        let transient_for = connection
            .get_property(
                false,
                popup_id,
                transient_atom,
                x11rb::protocol::xproto::AtomEnum::WINDOW,
                0,
                1,
            )
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(
            transient_for.value32().unwrap().next().unwrap() as usize,
            parent.id().into_raw()
        );
        let menu_atom = connection
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_POPUP_MENU")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let type_atom =
            connection.intern_atom(false, b"_NET_WM_WINDOW_TYPE").unwrap().reply().unwrap().atom;
        let window_type = connection
            .get_property(false, popup_id, type_atom, x11rb::protocol::xproto::AtomEnum::ATOM, 0, 1)
            .unwrap()
            .reply()
            .unwrap();
        assert_eq!(window_type.value32().unwrap().next(), Some(menu_atom));

        let grab = connection
            .xinput_xi_grab_device(
                popup_id,
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
        assert_eq!(grab.status, x11rb::protocol::xproto::GrabStatus::ALREADY_GRABBED);

        let parent_origin = connection
            .translate_coordinates(parent.id().into_raw() as u32, root, 0, 0)
            .unwrap()
            .reply()
            .unwrap();
        let popup_origin = popup.outer_position().unwrap();
        assert_eq!(popup_origin.x, parent_origin.dst_x as i32 + 20);
        assert_eq!(popup_origin.y, parent_origin.dst_y as i32 + 30);

        let positioner = WindowPositioner::new(
            WindowAnchor::TopLeft,
            (dpi::PhysicalPosition::new(50, 70).into(), dpi::PhysicalSize::new(1, 1).into()),
            dpi::PhysicalPosition::new(0, 0).into(),
            WindowGravity::BottomRight,
            WindowConstraintAdjustment::empty(),
        );
        popup.set_positioner(positioner);
        assert_eq!(popup.positioner(), positioner);
        let repositioned =
            connection.translate_coordinates(popup_id, root, 0, 0).unwrap().reply().unwrap();
        assert_eq!(repositioned.dst_x as i32, parent_origin.dst_x as i32 + 50);
        assert_eq!(repositioned.dst_y as i32, parent_origin.dst_y as i32 + 70);
        popup.set_visible(false);
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
        popup.set_visible(true);
        event_loop.exit();
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

#[test]
#[ignore = "requires a live X11 display"]
fn creates_root_level_transient_popup() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    builder.build().unwrap().run_app(PopupSmokeTest).unwrap();
}
