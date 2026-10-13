//! Opt-in regression driven only by tests/native/x11_panel_smoke.py.
use crate::engine::gpu::{GraphicsBackend, gl::GlBackend};
use crate::{PanelConfig, PanelEdge};
use raw_window_handle::HasWindowHandle;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{WindowAttributes, WindowId, WindowType};

#[test]
#[ignore = "requires the owned Openbox/Xwayland driver"]
fn native_x11_panel_managed_reservation_focus_and_retained_renderer() {
    assert_eq!(std::env::var("RUTTER_X11_PANEL_OWNED").as_deref(), Ok("1"));
    struct Probe {
        directory: PathBuf,
        settings: Option<Box<dyn GraphicsBackend>>,
        panels: Vec<Box<dyn GraphicsBackend>>,
        popup: Option<Box<dyn GraphicsBackend>>,
        nested_popup: Option<Box<dyn GraphicsBackend>>,
        retained_descendant: Option<Box<dyn GraphicsBackend>>,
        retirement_parents: Vec<raw_window_handle::RawWindowHandle>,
        retirement_checked: bool,
        retired_nested_id: Option<WindowId>,
        destroyed: Vec<WindowId>,
        started: Instant,
        ready: bool,
        command: String,
    }
    impl Probe {
        fn report(&self, stage: &str) {
            std::fs::write(self.directory.join("stage"), stage).unwrap();
        }
        fn render(backend: &mut dyn GraphicsBackend) {
            backend.begin_frame().unwrap().clear(skia_safe::Color::BLUE);
            backend.end_frame().unwrap();
        }
        fn popup(&mut self, event_loop: &dyn ActiveEventLoop, index: usize, active: bool) {
            let parent = self.panels[index]
                .window()
                .window_handle()
                .unwrap()
                .as_raw();
            // SAFETY: parent renderer/window is retained beyond this child.
            let attrs = unsafe {
                WindowAttributes::default()
                    .with_window_type(WindowType::Popup)
                    .with_parent_window(Some(parent))
            }
            .with_surface_size(winit::dpi::LogicalSize::new(120, 80))
            .with_active(active);
            let mut popup = GlBackend::try_new(event_loop, attrs).unwrap();
            Self::render(&mut *popup);
            std::fs::write(
                self.directory.join("popup"),
                popup.window().id().into_raw().to_string(),
            )
            .unwrap();
            self.popup = Some(popup);
        }
    }
    impl ApplicationHandler for Probe {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let missing = PanelConfig::new("missing", PanelEdge::Top, 0, 40)
                .unwrap()
                .with_output(crate::PanelOutput::Name("rutter-nonexistent-output".into()))
                .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(missing))
                    .is_err()
            );
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_window_type(WindowType::Panel))
                    .is_err()
            );
            let explicit_layer =
                crate::LayerShellConfig::new("strict-wayland", crate::LayerAnchors::TOP, 200, 40)
                    .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_layer_shell(explicit_layer))
                    .is_err()
            );
            let mut settings = GlBackend::try_new(
                event_loop,
                WindowAttributes::default()
                    .with_title("Panel Settings")
                    .with_surface_size(winit::dpi::LogicalSize::new(300, 200)),
            )
            .unwrap();
            Self::render(&mut *settings);
            settings.window().focus_window();
            self.settings = Some(settings);
        }
        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            id: WindowId,
            event: WindowEvent,
        ) {
            if self.retirement_parents.iter().any(|parent| matches!(parent,
                raw_window_handle::RawWindowHandle::Xlib(handle) if handle.window as usize == id.into_raw())) {
                assert!(!self.destroyed.contains(&id), "terminal-tree event delivered after Destroyed: {event:?}");
            }
            if matches!(event, WindowEvent::CloseRequested)
                && self.command == "retire"
                && !self.retirement_checked
            {
                for window in [
                    self.panels[1].window(),
                    self.popup.as_ref().unwrap().window(),
                    self.nested_popup.as_ref().unwrap().window(),
                    self.retained_descendant.as_ref().unwrap().window(),
                ] {
                    assert!(
                        window.window_handle().is_err(),
                        "tree handle still live at first retirement callback"
                    );
                }
                for parent in &self.retirement_parents {
                    // SAFETY: these retained owners outlive this deliberately rejected request.
                    let attributes = unsafe {
                        WindowAttributes::default()
                            .with_window_type(WindowType::Popup)
                            .with_parent_window(Some(*parent))
                    };
                    assert!(
                        event_loop.create_window(attributes).is_err(),
                        "popup escaped the retiring tree"
                    );
                    use x11rb::connection::Connection;
                    use x11rb::protocol::xproto::{
                        ConnectionExt as _, EventMask, KEY_PRESS_EVENT, KeyPressEvent,
                    };
                    let (connection, screen) = x11rb::connect(None).unwrap();
                    let raw_window_handle::RawWindowHandle::Xlib(handle) = parent else {
                        panic!("expected Xlib parent");
                    };
                    let key = KeyPressEvent {
                        response_type: KEY_PRESS_EVENT,
                        detail: 38,
                        sequence: 0,
                        time: x11rb::CURRENT_TIME,
                        root: connection.setup().roots[screen].root,
                        event: handle.window as u32,
                        child: 0,
                        root_x: 0,
                        root_y: 0,
                        event_x: 0,
                        event_y: 0,
                        state: Default::default(),
                        same_screen: true,
                    };
                    connection
                        .send_event(false, handle.window as u32, EventMask::NO_EVENT, key)
                        .unwrap()
                        .check()
                        .unwrap();
                }
                self.retirement_checked = true;
                // Global XKB notifications and a core key addressed to a live
                // window must not route through the just-retired active menu.
                use x11rb::protocol::{
                    xkb,
                    xproto::{ConnectionExt as _, EventMask},
                };
                let (connection, _) = x11rb::connect(None).unwrap();
                let extension = connection
                    .query_extension(b"XKEYBOARD")
                    .unwrap()
                    .reply()
                    .unwrap();
                assert!(extension.present);
                let settings = self.settings.as_ref().unwrap().window().id().into_raw() as u32;
                connection
                    .send_event(
                        false,
                        settings,
                        EventMask::NO_EVENT,
                        xkb::StateNotifyEvent {
                            response_type: extension.first_event,
                            xkb_type: xkb::STATE_NOTIFY_EVENT,
                            base_mods: x11rb::protocol::xproto::ModMask::SHIFT,
                            ..Default::default()
                        },
                    )
                    .unwrap()
                    .check()
                    .unwrap();
                connection
                    .send_event(
                        false,
                        settings,
                        EventMask::NO_EVENT,
                        xkb::MapNotifyEvent {
                            response_type: extension.first_event,
                            xkb_type: xkb::MAP_NOTIFY_EVENT,
                            ..Default::default()
                        },
                    )
                    .unwrap()
                    .check()
                    .unwrap();
            }
            if matches!(event, WindowEvent::Destroyed) {
                self.destroyed.push(id);
                if self.retirement_checked
                    && self
                        .panels
                        .get(1)
                        .is_some_and(|panel| id == panel.window().id())
                {
                    self.report("retired");
                }
                if self
                    .nested_popup
                    .as_ref()
                    .is_some_and(|nested| nested.window().id() == id)
                {
                    let mut nested = self.nested_popup.take().unwrap();
                    assert!(nested.window().window_handle().is_err());
                    assert!(nested.begin_frame().is_err());
                    assert!(nested.end_frame().is_err());
                    assert!(
                        nested
                            .resize(winit::dpi::PhysicalSize::new(80, 60))
                            .is_err()
                    );
                    self.retired_nested_id = Some(id);
                    // Drop during dispatch: queued key/XI2 focus events must be
                    // suppressed even after the weak native route has disappeared.
                    drop(nested);
                }
            }
        }
        fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
            assert!(
                self.started.elapsed() < Duration::from_secs(30),
                "panel smoke timed out"
            );
            if !self.ready && self.started.elapsed() > Duration::from_millis(400) {
                self.settings.as_ref().unwrap().window().focus_window();
                for edge in [
                    PanelEdge::Top,
                    PanelEdge::Right,
                    PanelEdge::Bottom,
                    PanelEdge::Left,
                ] {
                    let panel = PanelConfig::new(
                        "rutter-x11-panel",
                        edge,
                        match edge {
                            PanelEdge::Top => 0,
                            PanelEdge::Bottom => 400,
                            _ => 200,
                        },
                        40,
                    )
                    .unwrap()
                    .with_reservation(if edge == PanelEdge::Top {
                        Some(40)
                    } else {
                        None
                    })
                    .unwrap();
                    let mut backend = GlBackend::try_new(
                        event_loop,
                        WindowAttributes::default().with_panel(panel),
                    )
                    .unwrap();
                    assert_eq!(backend.window().window_type(), WindowType::Panel);
                    assert_eq!(backend.window().layer_shell_version(), None);
                    assert!(backend.window().panel_output_name().is_some());
                    Self::render(&mut *backend);
                    self.panels.push(backend);
                }
                self.settings.as_ref().unwrap().window().set_maximized(true);
                let ids = self
                    .panels
                    .iter()
                    .map(|panel| panel.window().id().into_raw().to_string())
                    .chain(std::iter::once(
                        self.settings
                            .as_ref()
                            .unwrap()
                            .window()
                            .id()
                            .into_raw()
                            .to_string(),
                    ))
                    .collect::<Vec<_>>()
                    .join(" ");
                std::fs::write(self.directory.join("windows"), ids).unwrap();
                self.ready = true;
                self.report("ready");
            }
            let command =
                std::fs::read_to_string(self.directory.join("command")).unwrap_or_default();
            if !command.is_empty() && command != self.command {
                self.command = command.clone();
                match command.as_str() {
                    "popup" => {
                        self.popup(event_loop, 0, false);
                        self.report("popup");
                    }
                    "hide" => {
                        self.popup.take();
                        self.panels[0].window().set_visible(false);
                        self.report("hidden");
                    }
                    "active" => {
                        self.popup.take();
                        self.popup(event_loop, 0, true);
                        self.report("active");
                    }
                    "dismiss" => {
                        self.popup.take();
                        self.report("dismissed");
                    }
                    "immediate-active" => {
                        self.popup(event_loop, 0, true);
                        self.popup.take();
                        self.report("immediate-dismissed");
                    }
                    "show" => {
                        self.panels[0].window().set_visible(true);
                        self.report("shown");
                    }
                    "quick-show" => {
                        for _ in 0..8 {
                            self.panels[0].window().set_visible(false);
                            self.panels[0].window().set_visible(true);
                        }
                        self.report("quick-shown");
                    }
                    "quick-hide" => {
                        self.panels[0].window().set_visible(true);
                        self.panels[0].window().set_visible(false);
                        self.report("quick-hidden");
                    }
                    "close" => {
                        self.panels.remove(0);
                        self.report("closed");
                    }
                    "retire" => {
                        self.popup(event_loop, 1, true);
                        let parent = self
                            .popup
                            .as_ref()
                            .unwrap()
                            .window()
                            .window_handle()
                            .unwrap()
                            .as_raw();
                        // SAFETY: both popup and panel owners are retained for renderer cleanup.
                        let attributes = unsafe {
                            WindowAttributes::default()
                                .with_window_type(WindowType::Popup)
                                .with_parent_window(Some(parent))
                        }
                        .with_surface_size(winit::dpi::LogicalSize::new(80, 60))
                        .with_active(true);
                        let mut nested = GlBackend::try_new(event_loop, attributes).unwrap();
                        Self::render(&mut *nested);
                        self.retirement_parents = vec![
                            self.panels[1].window().window_handle().unwrap().as_raw(),
                            parent,
                            nested.window().window_handle().unwrap().as_raw(),
                        ];
                        std::fs::write(
                            self.directory.join("nested-popup"),
                            nested.window().id().into_raw().to_string(),
                        )
                        .unwrap();
                        self.nested_popup = Some(nested);
                        // Retain a sibling descendant after dropping their retired
                        // ancestor, so its late DestroyNotify must be cleanup-only.
                        let attributes = unsafe {
                            WindowAttributes::default()
                                .with_window_type(WindowType::Popup)
                                .with_parent_window(Some(parent))
                        }
                        .with_surface_size(winit::dpi::LogicalSize::new(60, 40))
                        .with_active(false);
                        let mut descendant = GlBackend::try_new(event_loop, attributes).unwrap();
                        Self::render(&mut *descendant);
                        self.retirement_parents
                            .push(descendant.window().window_handle().unwrap().as_raw());
                        self.retained_descendant = Some(descendant);
                        self.report("retire-ready");
                    }
                    "drop-retired-ancestor" => {
                        assert!(self.retirement_checked);
                        assert!(
                            self.retained_descendant
                                .as_ref()
                                .unwrap()
                                .window()
                                .window_handle()
                                .is_err()
                        );
                        self.popup.take();
                        self.report("ancestor-dropped");
                    }
                    "finish" => {
                        let panel = &mut self.panels[1];
                        let popup = self.retained_descendant.as_mut().unwrap();
                        assert!(self.retirement_checked);
                        let nested = self.retired_nested_id.unwrap();
                        assert!(self.destroyed.contains(&nested));
                        let raw_window_handle::RawWindowHandle::Xlib(ancestor) =
                            self.retirement_parents[1]
                        else {
                            panic!("expected Xlib ancestor")
                        };
                        let ancestor_id = WindowId::from_raw(ancestor.window as usize);
                        assert!(
                            self.destroyed.iter().position(|id| *id == nested).unwrap()
                                < self
                                    .destroyed
                                    .iter()
                                    .position(|id| *id == ancestor_id)
                                    .unwrap()
                        );
                        assert!(self.destroyed.contains(&panel.window().id()));
                        assert!(self.destroyed.contains(&popup.window().id()));
                        assert!(
                            self.destroyed
                                .iter()
                                .position(|id| *id == popup.window().id())
                                .unwrap()
                                < self
                                    .destroyed
                                    .iter()
                                    .position(|id| *id == panel.window().id())
                                    .unwrap()
                        );
                        for backend in [panel, popup] {
                            assert!(backend.window().window_handle().is_err());
                            assert!(backend.begin_frame().is_err());
                            assert!(backend.end_frame().is_err());
                            assert!(
                                backend
                                    .resize(winit::dpi::PhysicalSize::new(100, 60))
                                    .is_err()
                            );
                            backend.window().set_visible(true);
                            assert!(backend.window().window_handle().is_err());
                        }
                        self.nested_popup.take();
                        self.retained_descendant.take();
                        self.popup.take();
                        self.panels.clear();
                        self.report("finished");
                        event_loop.exit();
                    }
                    _ => panic!("unknown panel smoke command"),
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(20),
            ));
        }
    }
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    builder
        .build()
        .unwrap()
        .run_app(Probe {
            directory: std::env::var_os("RUTTER_X11_PANEL_DIRECTORY")
                .unwrap()
                .into(),
            settings: None,
            panels: Vec::new(),
            popup: None,
            nested_popup: None,
            retained_descendant: None,
            retirement_parents: Vec::new(),
            retirement_checked: false,
            retired_nested_id: None,
            destroyed: Vec::new(),
            started: Instant::now(),
            ready: false,
            command: String::new(),
        })
        .unwrap();
}

#[test]
#[ignore = "requires the owned Xwayland driver before a WM is started"]
fn native_x11_panel_requires_ewmh_without_toplevel_fallback() {
    assert_eq!(std::env::var("RUTTER_X11_PANEL_OWNED").as_deref(), Ok("1"));
    struct Unsupported;
    impl ApplicationHandler for Unsupported {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let panel = PanelConfig::new("unsupported", PanelEdge::Top, 0, 40).unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(panel.clone()))
                    .is_err()
            );
            use x11rb::connection::Connection;
            use x11rb::protocol::xproto::{
                AtomEnum, ConnectionExt as _, CreateWindowAux, PropMode, WindowClass,
            };
            use x11rb::wrapper::ConnectionExt as _;
            let (connection, screen) = x11rb::connect(None).unwrap();
            let root = connection.setup().roots[screen].root;
            let atom = |name: &[u8]| {
                connection
                    .intern_atom(false, name)
                    .unwrap()
                    .reply()
                    .unwrap()
                    .atom
            };
            let check = atom(b"_NET_SUPPORTING_WM_CHECK");
            let supported = atom(b"_NET_SUPPORTED");
            let advertised = [
                atom(b"_NET_WM_WINDOW_TYPE"),
                atom(b"_NET_WM_WINDOW_TYPE_DOCK"),
            ];
            connection
                .change_property32(
                    PropMode::REPLACE,
                    root,
                    supported,
                    AtomEnum::ATOM,
                    &advertised,
                )
                .unwrap()
                .check()
                .unwrap();
            connection
                .change_property32(PropMode::REPLACE, root, check, AtomEnum::WINDOW, &[root])
                .unwrap()
                .check()
                .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(panel.clone()))
                    .is_err(),
                "root-as-self EWMH check accepted"
            );
            let child = connection.generate_id().unwrap();
            let grandchild = connection.generate_id().unwrap();
            for (window, parent) in [(child, root), (grandchild, child)] {
                connection
                    .create_window(
                        x11rb::COPY_DEPTH_FROM_PARENT,
                        window,
                        parent,
                        0,
                        0,
                        1,
                        1,
                        0,
                        WindowClass::INPUT_OUTPUT,
                        0,
                        &CreateWindowAux::new(),
                    )
                    .unwrap()
                    .check()
                    .unwrap();
            }
            connection
                .change_property32(
                    PropMode::REPLACE,
                    root,
                    check,
                    AtomEnum::WINDOW,
                    &[grandchild],
                )
                .unwrap()
                .check()
                .unwrap();
            connection
                .change_property32(
                    PropMode::REPLACE,
                    grandchild,
                    check,
                    AtomEnum::WINDOW,
                    &[grandchild],
                )
                .unwrap()
                .check()
                .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(panel.clone()))
                    .is_err(),
                "non-immediate supporting WM child accepted"
            );
            connection
                .change_property8(
                    PropMode::REPLACE,
                    root,
                    check,
                    AtomEnum::WINDOW,
                    &[1, 0, 0, 0],
                )
                .unwrap()
                .check()
                .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(panel.clone()))
                    .is_err(),
                "non-WINDOW32 supporting WM check accepted"
            );
            connection
                .change_property32(PropMode::REPLACE, root, check, AtomEnum::WINDOW, &[child])
                .unwrap()
                .check()
                .unwrap();
            connection
                .change_property32(
                    PropMode::REPLACE,
                    child,
                    check,
                    AtomEnum::WINDOW,
                    &[grandchild],
                )
                .unwrap()
                .check()
                .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(panel.clone()))
                    .is_err(),
                "supporting WM self-reference mismatch accepted"
            );
            connection
                .change_property32(PropMode::REPLACE, child, check, AtomEnum::WINDOW, &[child])
                .unwrap()
                .check()
                .unwrap();
            connection
                .change_property8(
                    PropMode::REPLACE,
                    root,
                    supported,
                    AtomEnum::ATOM,
                    &[1, 0, 0, 0],
                )
                .unwrap()
                .check()
                .unwrap();
            assert!(
                event_loop
                    .create_window(WindowAttributes::default().with_panel(panel))
                    .is_err(),
                "non-ATOM32 capabilities accepted"
            );
            connection
                .delete_property(root, check)
                .unwrap()
                .check()
                .unwrap();
            connection
                .delete_property(root, supported)
                .unwrap()
                .check()
                .unwrap();
            connection.destroy_window(child).unwrap().check().unwrap();
            let ordinary = event_loop
                .create_window(WindowAttributes::default())
                .unwrap();
            assert_eq!(ordinary.window_type(), WindowType::Window);
            event_loop.exit();
        }
        fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    builder.build().unwrap().run_app(Unsupported).unwrap();
}
