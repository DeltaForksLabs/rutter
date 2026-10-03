//! Run only on a disposable X11 display: this test changes focus and injects a key.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use rwh_06::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::Key;
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, InputFocus};
use x11rb::protocol::xtest::ConnectionExt as _;

fn input_focus() -> u32 {
    let (connection, _) = x11rb::connect(None).unwrap();
    connection.get_input_focus().unwrap().reply().unwrap().focus
}

fn focus(window: &dyn Window) {
    let (connection, _) = x11rb::connect(None).unwrap();
    connection
        .set_input_focus(InputFocus::PARENT, window.id().into_raw() as u32, x11rb::CURRENT_TIME)
        .unwrap()
        .check()
        .unwrap();
}

fn assert_focus(window: &dyn Window, operation: &str) {
    assert_eq!(input_focus(), window.id().into_raw() as u32, "{operation}");
}

fn create_popup(
    event_loop: &dyn ActiveEventLoop,
    parent: &dyn Window,
    active: bool,
) -> Box<dyn Window> {
    // SAFETY: all test popups are dropped before the parent.
    let attributes = unsafe {
        WindowAttributes::default()
            .with_window_type(WindowType::Popup)
            .with_parent_window(Some(parent.window_handle().unwrap().as_raw()))
    }
    .with_active(active)
    .with_surface_size(dpi::PhysicalSize::new(120, 80));
    event_loop.create_window(attributes).unwrap()
}

fn check_popup_focus_lifecycle(event_loop: &dyn ActiveEventLoop, parent: &dyn Window) {
    focus(parent);
    let first = create_popup(event_loop, parent, true);
    assert_focus(first.as_ref(), "active popup takes keyboard focus");
    let second = create_popup(event_loop, parent, true);
    assert_focus(second.as_ref(), "topmost popup takes keyboard focus");

    second.set_visible(false);
    assert_focus(first.as_ref(), "hiding topmost popup restores preceding popup focus");
    second.set_visible(true);
    assert_focus(second.as_ref(), "showing an active popup reacquires keyboard focus");
    drop(second);
    assert_focus(first.as_ref(), "dropping topmost popup restores preceding popup focus");

    first.set_visible(false);
    assert_focus(parent, "hiding last popup restores parent focus");
    first.set_visible(true);
    assert_focus(first.as_ref(), "showing last popup reacquires keyboard focus");
    drop(first);
    assert_focus(parent, "dropping last popup restores parent focus");

    let first = create_popup(event_loop, parent, true);
    let second = create_popup(event_loop, parent, true);
    drop(first);
    assert_focus(second.as_ref(), "dropping a lower popup must not take focus from the top popup");
    drop(second);
    assert_focus(parent, "dead lower popup is skipped when restoring focus");

    let first = create_popup(event_loop, parent, true);
    let second = create_popup(event_loop, parent, true);
    first.set_visible(false);
    assert_focus(second.as_ref(), "hiding a lower popup must not take focus from the top popup");
    drop(second);
    assert_focus(parent, "hidden lower popup is skipped when restoring focus");
}

fn check_external_and_inactive_focus(event_loop: &dyn ActiveEventLoop, parent: &dyn Window) {
    let other = event_loop.create_window(WindowAttributes::default()).unwrap();
    let popup = create_popup(event_loop, parent, true);
    focus(other.as_ref());
    popup.set_visible(false);
    assert_focus(other.as_ref(), "hiding a popup must not steal another window's focus");
    popup.set_visible(true);
    focus(other.as_ref());
    drop(popup);
    assert_focus(other.as_ref(), "dropping a popup must not steal another window's focus");

    focus(parent);
    let inactive = create_popup(event_loop, parent, false);
    assert_focus(parent, "inactive popup does not take keyboard focus");
    inactive.set_visible(false);
    inactive.set_visible(true);
    assert_focus(parent, "showing inactive popup does not take keyboard focus");
    drop(inactive);
    assert_focus(parent, "dropping inactive popup preserves focus");
}

fn check_hidden_parent(event_loop: &dyn ActiveEventLoop, parent: &dyn Window) {
    let popup = create_popup(event_loop, parent, true);
    parent.set_visible(false);
    popup.set_visible(false);
    assert_ne!(input_focus(), parent.id().into_raw() as u32, "hidden parent cannot receive focus");
    drop(popup);
    parent.set_visible(true);
    focus(parent);
}

fn inject_letter_a() {
    let (connection, screen) = x11rb::connect(None).unwrap();
    let setup = connection.setup();
    let first = setup.min_keycode;
    let mapping = connection
        .get_keyboard_mapping(first, setup.max_keycode - first + 1)
        .unwrap()
        .reply()
        .unwrap();
    let index = mapping
        .keysyms
        .chunks(mapping.keysyms_per_keycode as usize)
        .position(|symbols| symbols[0] == u32::from(b'a'))
        .expect("disposable display needs an unmodified a key");
    let code = first + u8::try_from(index).unwrap();
    let root = setup.roots[screen].root;
    for event_type in [2, 3] {
        connection.xtest_fake_input(event_type, code, 0, root, 0, 0, 0).unwrap().check().unwrap();
    }
}

struct KeyboardFocusTest {
    parent: Option<Box<dyn Window>>,
    received_key: Rc<Cell<bool>>,
    deadline: Instant,
}

impl ApplicationHandler for KeyboardFocusTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let parent = event_loop.create_window(WindowAttributes::default()).unwrap();
        check_popup_focus_lifecycle(event_loop, parent.as_ref());
        check_external_and_inactive_focus(event_loop, parent.as_ref());
        check_hidden_parent(event_loop, parent.as_ref());
        let popup = create_popup(event_loop, parent.as_ref(), true);
        drop(popup);
        assert_focus(parent.as_ref(), "parent focus is restored before injecting a real key");
        self.parent = Some(parent);
        inject_letter_a();
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if event.state == ElementState::Pressed
                && event.logical_key == Key::Character("a".into())
            {
                assert_eq!(Some(id), self.parent.as_ref().map(|parent| parent.id()));
                self.received_key.set(true);
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _: &dyn ActiveEventLoop) {
        assert!(Instant::now() < self.deadline, "restored parent did not receive the XTEST key");
    }
}

#[test]
#[ignore = "requires a disposable X11 display with XTEST"]
fn popup_lifecycle_restores_keyboard_focus_without_stealing_external_focus() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let received_key = Rc::new(Cell::new(false));
    builder
        .build()
        .unwrap()
        .run_app(KeyboardFocusTest {
            parent: None,
            received_key: received_key.clone(),
            deadline: Instant::now() + Duration::from_secs(10),
        })
        .unwrap();
    assert!(received_key.get());
}
