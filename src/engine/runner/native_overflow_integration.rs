//! Disposable-display regressions for native menus beyond thin parents.

use super::*;

#[path = "native_menu_review_integration.rs"]
mod review;

#[derive(Clone)]
struct OverflowState {
    selected: Rc<Cell<u32>>,
    failures: Rc<RefCell<Vec<crate::NativeMenuFailure>>>,
    entries: Vec<ContextMenuEntry<'static, bool>>,
}

struct OverflowApp;

impl AppLogic for OverflowApp {
    type State = OverflowState;
    type Message = bool;

    fn new(_: &mut FontSystem) -> Self::State {
        OverflowState {
            selected: Rc::new(Cell::new(0)),
            failures: Rc::new(RefCell::new(Vec::new())),
            entries: vec![ContextMenuEntry::item("Settings", true)],
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, bool> {
        Widget::context_menu(
            Widget::Spacer {
                style: Style::default(),
            },
            &state.entries,
            Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::percent(1.0),
                },
                ..Style::default()
            },
        )
        .with_id(980)
    }

    fn update(state: &mut Self::State, selected: bool, _: &mut arboard::Clipboard) {
        if selected {
            state.selected.set(state.selected.get() + 1);
        }
    }

    fn native_menu_unavailable(
        state: &Self::State,
        failure: crate::NativeMenuFailure,
    ) -> Option<bool> {
        state.failures.borrow_mut().push(failure);
        Some(false)
    }
}

struct OverflowTest {
    runner: RutterRunner<OverflowApp>,
    logical_size: (u32, u32),
    backend: BackendType,
    wayland: bool,
    compositor_input: bool,
    opened: bool,
    selected: bool,
    popup_presses: u32,
    parent_presses: u32,
    nested: bool,
    hovered_levels: usize,
    long: bool,
    scrolled: bool,
    resize_requested: bool,
    first_frame: Option<Instant>,
    deadline: Instant,
}

impl ApplicationHandler for OverflowTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner
            .resume_surface(
                event_loop,
                Some(
                    WindowAttributes::default()
                        .with_title("Rutter native overflow acceptance")
                        .with_decorations(false)
                        .with_position(PhysicalPosition::new(100, 100))
                        .with_surface_size(winit::dpi::LogicalSize::new(
                            self.logical_size.0,
                            self.logical_size.1,
                        )),
                ),
                Some(self.backend),
            )
            .unwrap();
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        self.runner.release_surface();
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(Instant::now() < self.deadline, "thin-parent menu timed out");
        self.runner.new_events(event_loop, cause);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.about_to_wait(event_loop);
        let parent = self.runner.engine.window.as_ref().unwrap();
        let scale = parent.scale_factor();
        let size = parent.surface_size().to_logical::<u32>(scale);
        assert_eq!(
            (size.width, size.height),
            self.logical_size,
            "opening a menu must not enlarge its parent"
        );
        if self.runner.engine.app_state.selected.get() == 1 {
            assert!(self.runner.native_menu.is_none());
            assert!(!self.runner.engine.any_context_menu_open());
            if self.compositor_input {
                assert_eq!((self.parent_presses, self.popup_presses), (1, 1));
            }
            event_loop.exit();
            return;
        }
        if !self.runner.engine.app_state.failures.borrow().is_empty() {
            assert_eq!(self.backend, BackendType::CpuSoftbuffer);
            let failure = self.runner.engine.app_state.failures.borrow()[0].clone();
            assert_eq!(failure.menu_id, 980);
            assert!(!failure.overlay_usable);
            assert!(!self.runner.engine.any_context_menu_open());
            assert!(self.runner.native_menu.is_none());
            event_loop.exit();
            return;
        }
        if !self.opened
            && !self.compositor_input
            && self
                .first_frame
                .is_some_and(|time| time.elapsed() > Duration::from_millis(200))
        {
            self.opened = true;
            let parent = self.runner.active_window_id.unwrap();
            if self.wayland {
                self.runner.window_event(
                    event_loop,
                    parent,
                    press(
                        PhysicalPosition::new(20.0 * scale, 20.0 * scale),
                        MouseButton::Right,
                    ),
                );
            } else {
                click_at_button(parent, (20.0 * scale) as i16, (20.0 * scale) as i16, 3);
            }
        }
        if let Some(popup) = self.runner.native_menu.as_ref().filter(|popup| popup.ready) {
            let window = popup.backend.window();
            let popup_scale = window.scale_factor();
            let panels = &self.runner.engine.native_menu_layout.as_ref().unwrap().1;
            if panels.len() != self.hovered_levels + 1 {
                return;
            }
            let panel = &panels[self.hovered_levels];
            if self.long && !self.resize_requested {
                self.resize_requested = true;
                window.request_popup_layout(
                    window.positioner(),
                    winit::dpi::LogicalSize::new(120.0, 72.0).into(),
                );
                return;
            }
            if self.long && panel.rect.height() >= 320.0 {
                return;
            }
            assert!(panel.rect.width() >= 100.0 && panel.rect.height() >= 44.0);
            let mut point = Point::new(panel.rect.left + 20.0, panel.rect.top + 22.0);
            if self.long && self.scrolled {
                assert!(panel.scroll_y > 0.0);
                point = crate::widgets::dropdown_menu::row_rect(
                    panel,
                    &self.runner.engine.app_state.entries,
                    29,
                )
                .unwrap()
                .center();
            }
            assert!(
                point.x < 0.0
                    || point.y < 0.0
                    || point.x > self.logical_size.0 as f32
                    || point.y > self.logical_size.1 as f32,
                "selection must be outside the parent"
            );
            if !self.selected && !self.compositor_input {
                let origin = window.outer_position().unwrap();
                let local = PhysicalPosition::new(
                    f64::from(point.x) * popup_scale - f64::from(origin.x),
                    f64::from(point.y) * popup_scale - f64::from(origin.y),
                );
                let id = window.id();
                if self.long && !self.scrolled {
                    assert!(
                        panel.rect.height() < 320.0,
                        "test compositor must resize the requested menu"
                    );
                    assert!(panel.max_scroll > 0.0);
                    self.scrolled = true;
                    self.runner.window_event(
                        event_loop,
                        id,
                        WindowEvent::PointerMoved {
                            device_id: None,
                            position: local,
                            primary: true,
                            source: winit::event::PointerSource::Mouse,
                        },
                    );
                    self.runner.window_event(
                        event_loop,
                        id,
                        WindowEvent::MouseWheel {
                            device_id: None,
                            delta: winit::event::MouseScrollDelta::LineDelta(0.0, -3.0),
                            phase: winit::event::TouchPhase::Moved,
                        },
                    );
                    assert!(
                        self.runner.engine.widget_states[&980]
                            .as_context_menu()
                            .unwrap()
                            .navigation
                            .scroll_offset(0)
                            > 0.0
                    );
                    self.runner.handle_key(&Key::Named(NamedKey::End), false);
                    return;
                } else if self.nested && self.hovered_levels < 2 {
                    self.hovered_levels += 1;
                    if self.wayland {
                        self.runner.window_event(
                            event_loop,
                            id,
                            WindowEvent::PointerMoved {
                                device_id: None,
                                position: local,
                                primary: true,
                                source: winit::event::PointerSource::Mouse,
                            },
                        );
                    } else {
                        move_at(id, local.x as i16, local.y as i16);
                    }
                } else if self.wayland {
                    self.selected = true;
                    self.runner
                        .window_event(event_loop, id, press(local, MouseButton::Left));
                } else {
                    self.selected = true;
                    click_at(id, local.x as i16, local.y as i16);
                }
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(50),
        ));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let parent = self.runner.active_window_id.unwrap();
        if matches!(
            event,
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                ..
            }
        ) {
            if id == parent {
                self.parent_presses += 1;
            } else {
                self.popup_presses += 1;
            }
        }
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        self.runner.window_event(event_loop, id, event);
        assert!(self.runner.fatal_error.is_none());
        if id == parent && redraw {
            self.first_frame.get_or_insert_with(Instant::now);
        }
    }
}

fn press(position: PhysicalPosition<f64>, button: MouseButton) -> WindowEvent {
    WindowEvent::PointerButton {
        device_id: None,
        state: ElementState::Pressed,
        position,
        primary: true,
        button: ButtonSource::Mouse(button),
        is_macos_activation_click: false,
    }
}

fn run_overflow_test(wayland: bool, vertical: bool, backend: BackendType, compositor_input: bool) {
    run_overflow_scenario(wayland, vertical, backend, compositor_input, false, false);
}

fn run_overflow_scenario(
    wayland: bool,
    vertical: bool,
    backend: BackendType,
    compositor_input: bool,
    nested: bool,
    long: bool,
) {
    let mut builder = EventLoop::builder();
    if wayland {
        winit::platform::wayland::EventLoopBuilderExtWayland::with_wayland(&mut builder);
        winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    } else {
        builder.with_x11();
        builder.with_any_thread(true);
    }
    let mut state = OverflowApp::new(&mut FontSystem::new());
    if nested {
        state.entries = vec![ContextMenuEntry::submenu(
            "More",
            vec![ContextMenuEntry::submenu(
                "Deeper",
                vec![ContextMenuEntry::item("Settings", true)],
            )],
        )];
    } else if long {
        state.entries = (0..30)
            .map(|_| ContextMenuEntry::item("Settings", true))
            .collect();
    }
    let selected = state.selected.clone();
    let failures = state.failures.clone();
    let engine = RutterEngine::<OverflowApp>::with_shared_font_system(
        state,
        Rc::new(RefCell::new(FontSystem::new())),
        SurfaceConfig::default(),
    )
    .unwrap();
    builder
        .build()
        .unwrap()
        .run_app(OverflowTest {
            runner: RutterRunner::with_engine(engine),
            logical_size: if vertical { (30, 600) } else { (800, 30) },
            backend,
            wayland,
            compositor_input,
            opened: false,
            selected: false,
            popup_presses: 0,
            parent_presses: 0,
            nested,
            hovered_levels: 0,
            long,
            scrolled: false,
            resize_requested: false,
            first_frame: None,
            deadline: Instant::now() + Duration::from_secs(20),
        })
        .unwrap();
    if backend == BackendType::CpuSoftbuffer {
        assert_eq!(failures.borrow().len(), 1);
    } else {
        assert_eq!(selected.get(), 1);
    }
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL"]
fn native_wayland_context_overflows_horizontal_parent() {
    run_overflow_test(true, false, BackendType::OpenGl, false);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL"]
fn native_wayland_context_overflows_vertical_parent() {
    run_overflow_test(true, true, BackendType::OpenGl, false);
}

#[test]
#[ignore = "requires a disposable Wayland compositor; uses synthetic opening input"]
fn wayland_cpu_thin_parent_reports_unusable_overlay_and_dismisses() {
    run_overflow_test(true, false, BackendType::CpuSoftbuffer, false);
}

#[test]
#[ignore = "requires a disposable X11 server with XTEST and alpha-capable GL"]
fn native_x11_context_selects_outside_horizontal_parent() {
    run_overflow_test(false, false, BackendType::OpenGl, false);
}

#[test]
#[ignore = "requires a disposable X11 server with XTEST and alpha-capable GL"]
fn native_x11_context_selects_outside_vertical_parent() {
    run_overflow_test(false, true, BackendType::OpenGl, false);
}

#[test]
#[ignore = "requires external compositor-delivered right-click and popup left-click on a disposable Wayland session"]
fn native_wayland_compositor_pointer_selects_outside_thin_parent() {
    run_overflow_test(true, false, BackendType::OpenGl, true);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with alpha-capable EGL; synthetic input"]
fn native_wayland_nested_context_overflows_thin_parent() {
    run_overflow_scenario(true, false, BackendType::OpenGl, false, true, false);
}

#[test]
#[ignore = "requires a disposable X11 display with alpha-capable GL and XTEST"]
fn native_x11_nested_context_overflows_thin_parent() {
    run_overflow_scenario(false, true, BackendType::OpenGl, false, true, false);
}

#[test]
#[ignore = "requires a disposable Wayland compositor; explicitly requests a smaller popup configure and uses synthetic input"]
fn native_wayland_configured_resize_scrolls_and_selects_outside_parent() {
    run_overflow_scenario(true, false, BackendType::OpenGl, false, false, true);
}
