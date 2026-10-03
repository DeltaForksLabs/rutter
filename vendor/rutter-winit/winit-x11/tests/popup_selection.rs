//! Run only on a disposable X11 display: XTEST moves the pointer and clicks.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::{ButtonSource, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xinput::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest::ConnectionExt as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuOption {
    New,
    Open,
}

fn option_at(position: dpi::PhysicalPosition<f64>) -> Option<MenuOption> {
    if !(0.0..80.0).contains(&position.x) {
        return None;
    }
    if (0.0..20.0).contains(&position.y) {
        Some(MenuOption::New)
    } else if (20.0..40.0).contains(&position.y) {
        Some(MenuOption::Open)
    } else {
        None
    }
}

struct MenuSelectionTest {
    parent: Option<Box<dyn Window>>,
    popup: Option<Box<dyn Window>>,
    selected: Rc<Cell<Option<MenuOption>>>,
    pressed: bool,
    released: bool,
    deadline: Instant,
}

impl ApplicationHandler for MenuSelectionTest {
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

        // Use the actual X11 position rather than assuming where a compositor placed it.
        let (connection, _) = x11rb::connect(None).unwrap();
        let popup_id = popup.id().into_raw() as u32;
        let root = connection.query_tree(popup_id).unwrap().reply().unwrap().parent;
        let origin =
            connection.translate_coordinates(popup_id, root, 0, 0).unwrap().reply().unwrap();
        connection
            .warp_pointer(0u32, root, 0, 0, 0, 0, origin.dst_x + 20, origin.dst_y + 30)
            .unwrap()
            .check()
            .unwrap();
        connection.xtest_fake_input(4, 1, 0, root, 0, 0, 0).unwrap().check().unwrap();
        connection.xtest_fake_input(5, 1, 0, root, 0, 0, 0).unwrap().check().unwrap();

        self.parent = Some(parent);
        self.popup = Some(popup);
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) != self.popup.as_ref().map(|popup| popup.id()) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => panic!("clicking a menu option dismissed the popup"),
            WindowEvent::PointerButton {
                button: ButtonSource::Mouse(MouseButton::Left),
                state,
                position,
                ..
            } => match state {
                ElementState::Pressed => {
                    assert!(!self.pressed, "duplicate press for menu option");
                    assert_eq!(option_at(position), Some(MenuOption::Open));
                    self.pressed = true;
                },
                ElementState::Released => {
                    assert!(self.pressed, "release without menu option press");
                    assert!(!self.released, "duplicate release for menu option");
                    assert_eq!(option_at(position), Some(MenuOption::Open));
                    self.selected.set(option_at(position));
                    self.released = true;
                },
            },
            _ => {},
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(Instant::now() < self.deadline, "menu option click was not delivered in time");
        if self.released {
            assert_eq!(self.selected.get(), Some(MenuOption::Open));
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
            if grab.status == x11rb::protocol::xproto::GrabStatus::SUCCESS {
                connection
                    .xinput_xi_ungrab_device(x11rb::CURRENT_TIME, 2u16)
                    .unwrap()
                    .check()
                    .unwrap();
            }
            assert_eq!(
                grab.status,
                x11rb::protocol::xproto::GrabStatus::ALREADY_GRABBED,
                "selecting an option released the popup pointer grab"
            );
            event_loop.exit();
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
        }
    }
}

#[test]
#[ignore = "requires a disposable X11 display with XTEST"]
fn clicking_inside_popup_selects_option_without_dismissal() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let selected = Rc::new(Cell::new(None));
    let app = MenuSelectionTest {
        parent: None,
        popup: None,
        selected: selected.clone(),
        pressed: false,
        released: false,
        deadline: Instant::now() + Duration::from_secs(5),
    };
    builder.build().unwrap().run_app(app).unwrap();
    assert_eq!(selected.get(), Some(MenuOption::Open));
}
