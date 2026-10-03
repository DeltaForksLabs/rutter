//! Run only on a disposable X11 display with a working EGL or GLX driver.

use std::cell::Cell;
use std::rc::Rc;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::ContextAttributesBuilder;
use glutin::display::GetGlDisplay;
use glutin::prelude::{GlDisplay, NotCurrentGlContext};
use glutin::surface::SurfaceAttributesBuilder;
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::x11::{EventLoopBuilderExtX11, WindowAttributesX11};
use winit::window::{Window, WindowAttributes, WindowId};

struct DisplayApp {
    completed: Rc<Cell<bool>>,
    window: Option<Box<dyn Window>>,
}

impl ApplicationHandler for DisplayApp {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let attributes = WindowAttributes::default()
            .with_visible(false)
            .with_surface_size(winit::dpi::PhysicalSize::new(80, 60))
            .with_platform_attributes(Box::new(
                WindowAttributesX11::default().with_name("glutin-bridge", "glutin-bridge"),
            ));
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(event_loop, ConfigTemplateBuilder::new(), |mut configs| {
                configs
                    .next()
                    .expect("driver offered no GL window configurations")
            })
            .expect("failed to create a GL-compatible Winit window");
        let window = window.expect("requested a GL-compatible window");
        let surface_attributes = window
            .as_ref()
            .build_surface_attributes(SurfaceAttributesBuilder::new())
            .unwrap();
        let surface = unsafe {
            config
                .display()
                .create_window_surface(&config, &surface_attributes)
        }
        .expect("failed to create a GL surface for the Winit window");
        let handle = window.window_handle().unwrap();
        let context_attributes = ContextAttributesBuilder::new().build(Some(handle.as_raw()));
        let context = unsafe {
            config
                .display()
                .create_context(&config, &context_attributes)
        }
        .expect("failed to create a GL context for the Winit window");
        let _context = context
            .make_current(&surface)
            .expect("failed to activate the GL context");
        self.window = Some(window);
        self.completed.set(true);
        event_loop.exit();
    }

    fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

#[test]
#[ignore = "requires a disposable X11 display with EGL or GLX"]
fn display_builder_creates_compatible_window_and_current_gl_context() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let completed = Rc::new(Cell::new(false));
    builder
        .build()
        .unwrap()
        .run_app(DisplayApp {
            completed: completed.clone(),
            window: None,
        })
        .unwrap();
    assert!(completed.get());
}
