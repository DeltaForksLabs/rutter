//! Run only on a disposable X11 display; no AT-SPI service is required.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_winit::Adapter;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId};

struct NoInitialTree;

impl ActivationHandler for NoInitialTree {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        None
    }
}

struct NoActions;

impl ActionHandler for NoActions {
    fn do_action(&mut self, _: ActionRequest) {}
}

struct NoDeactivation;

impl DeactivationHandler for NoDeactivation {
    fn deactivate_accessibility(&mut self) {}
}

struct AccessKitApp {
    completed: Rc<Cell<bool>>,
    window: Option<Box<dyn Window>>,
    adapter: Option<Adapter>,
    deadline: Instant,
}

impl ApplicationHandler for AccessKitApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window = event_loop
            .create_window(WindowAttributes::default().with_visible(false))
            .unwrap();
        let mut adapter = Adapter::with_direct_handlers(
            event_loop,
            window.as_ref(),
            NoInitialTree,
            NoActions,
            NoDeactivation,
        );
        adapter.process_event(window.as_ref(), &WindowEvent::Moved((30, 40).into()));
        adapter.process_event(
            window.as_ref(),
            &WindowEvent::SurfaceResized((80, 60).into()),
        );
        adapter.process_event(window.as_ref(), &WindowEvent::Focused(false));
        adapter.update_if_active(|| panic!("inactive accessibility tree should not be requested"));
        window.set_visible(true);
        self.adapter = Some(adapter);
        self.window = Some(window);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(
            Instant::now() < self.deadline,
            "window was not mapped in time"
        );
        let window = self.window.as_ref().unwrap();
        if window.is_visible() != Some(true) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
            return;
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                Adapter::with_direct_handlers(
                    event_loop,
                    window.as_ref(),
                    NoInitialTree,
                    NoActions,
                    NoDeactivation,
                )
            }))
            .is_err(),
            "adapters must be installed before the window becomes visible"
        );
        self.completed.set(true);
        event_loop.exit();
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

#[test]
#[ignore = "requires a disposable X11 display"]
fn direct_adapter_accepts_hidden_winit_trait_window_and_surface_events() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let completed = Rc::new(Cell::new(false));
    builder
        .build()
        .unwrap()
        .run_app(AccessKitApp {
            completed: completed.clone(),
            window: None,
            adapter: None,
            deadline: Instant::now() + Duration::from_secs(5),
        })
        .unwrap();
    assert!(completed.get());
}
