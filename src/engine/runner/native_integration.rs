//! Run only on disposable X11 displays or Wayland compositors, as specified by each test.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use cosmic_text::FontSystem;
use raw_window_handle::HasWindowHandle;
use taffy::prelude::{Dimension, Size, Style};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ButtonSource, ElementState, FingerId, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::x11::EventLoopBuilderExtX11;
use winit::window::{Window, WindowAttributes, WindowId, WindowType};
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest::ConnectionExt as _;

use super::*;
use crate::DropdownMenuEntry;
use crate::app::SurfaceConfig;
use crate::widget::{ButtonVariant, ContextMenuEntry, Widget};

const CONTEXT_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect x="2" y="2" width="12" height="12" rx="2" fill="#5086E7"/></svg>"##;

struct NativeButtonApp;

impl AppLogic for NativeButtonApp {
    type State = Rc<Cell<u32>>;
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {
        Rc::new(Cell::new(0))
    }

    fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Button {
            text: "Select",
            on_press: (),
            style: Style {
                size: Size {
                    width: Dimension::length(200.0),
                    height: Dimension::length(80.0),
                },
                ..Style::default()
            },
            color: None,
            variant: ButtonVariant::Primary,
        }
    }

    fn update(state: &mut Self::State, _: Self::Message, _: &mut arboard::Clipboard) {
        state.set(state.get() + 1);
    }
}

struct NativeButtonTest {
    runner: RutterRunner<NativeButtonApp>,
    clicks: Rc<Cell<u32>>,
    sent_click: bool,
    received_press: bool,
    recreated: bool,
    deadline: Instant,
}

impl NativeButtonTest {
    fn create_surface(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner
            .resume_surface(
                event_loop,
                Some(
                    WindowAttributes::default()
                        .with_title("Rutter native button integration")
                        .with_decorations(false)
                        .with_surface_size(PhysicalSize::new(240, 120)),
                ),
                Some(BackendType::OpenGl),
            )
            .expect("Rutter could not initialize its OpenGL surface");
        assert_eq!(self.runner.engine.backend_type(), Some(BackendType::OpenGl));
        assert!(self.runner.engine.accessibility_adapter.is_some());
        assert_eq!(
            self.runner.engine.window.as_ref().unwrap().surface_size(),
            PhysicalSize::new(240, 120)
        );
    }
}

impl ApplicationHandler for NativeButtonTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.create_surface(event_loop);
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        self.runner.release_surface();
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "Rutter button click was not delivered in time"
        );
        self.runner.new_events(event_loop, cause);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) != self.runner.active_window_id {
            return;
        }
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        if let WindowEvent::PointerButton {
            button: ButtonSource::Mouse(MouseButton::Left),
            state,
            position,
            ..
        } = &event
        {
            assert!((0.0..200.0).contains(&position.x) && (0.0..80.0).contains(&position.y));
            // A button event must use its own coordinates even when the last
            // movement event was missed or belonged to another pointer.
            self.runner.cursor_physical = PhysicalPosition::new(500.0, 500.0);
            self.runner.cursor_pos = skia_safe::Point::new(500.0, 500.0);
            self.runner.engine.last_mouse_pos = skia_safe::Point::new(500.0, 500.0);
            if *state == ElementState::Pressed {
                assert!(!self.received_press);
                self.received_press = true;
            }
        }
        let released = matches!(
            event,
            WindowEvent::PointerButton {
                button: ButtonSource::Mouse(MouseButton::Left),
                state: ElementState::Released,
                ..
            }
        );
        self.runner.window_event(event_loop, id, event);
        assert!(self.runner.fatal_error.is_none());
        if released {
            assert!(self.received_press);
            assert_eq!(
                self.clicks.get(),
                if self.recreated { 2 } else { 1 },
                "native button press must select the Rutter widget exactly once"
            );
            if !self.recreated {
                self.runner.release_surface();
                assert!(self.runner.engine.accessibility_adapter.is_none());
                self.create_surface(event_loop);
                assert_ne!(self.runner.active_window_id, Some(id));
                self.recreated = true;
                self.sent_click = false;
                self.received_press = false;
            } else {
                event_loop.exit();
            }
        } else if redraw && !self.sent_click {
            self.sent_click = true;
            self.runner.window_event(
                event_loop,
                id,
                WindowEvent::PointerButton {
                    device_id: None,
                    state: ElementState::Pressed,
                    position: PhysicalPosition::new(40.0, 35.0),
                    primary: true,
                    button: ButtonSource::Touch {
                        finger_id: FingerId::from_raw(1),
                        force: None,
                    },
                    is_macos_activation_click: false,
                },
            );
            assert_eq!(self.clicks.get(), if self.recreated { 1 } else { 0 });
            click_window(id);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.about_to_wait(event_loop);
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
    }
}

fn click_window(id: WindowId) {
    click_at(id, 40, 35);
}

fn click_at(id: WindowId, x: i16, y: i16) {
    click_at_button(id, x, y, 1);
}

fn click_at_button(id: WindowId, x: i16, y: i16, button: u8) {
    move_at(id, x, y);
    let (connection, _) = x11rb::connect(None).expect("connect to isolated X11 server");
    let root = connection
        .query_tree(id.into_raw() as u32)
        .unwrap()
        .reply()
        .unwrap()
        .root;
    connection
        .xtest_fake_input(4, button, 0, root, 0, 0, 0)
        .unwrap()
        .check()
        .unwrap();
    connection
        .xtest_fake_input(5, button, 0, root, 0, 0, 0)
        .unwrap()
        .check()
        .unwrap();
}

fn move_at(id: WindowId, x: i16, y: i16) {
    let (connection, _) = x11rb::connect(None).expect("connect to isolated X11 server");
    let window = id.into_raw() as u32;
    let root = connection.query_tree(window).unwrap().reply().unwrap().root;
    let origin = connection
        .translate_coordinates(window, root, 0, 0)
        .unwrap()
        .reply()
        .unwrap();
    connection
        .warp_pointer(0u32, root, 0, 0, 0, 0, origin.dst_x + x, origin.dst_y + y)
        .unwrap()
        .check()
        .unwrap();
}

struct NativeDropdownApp;

impl AppLogic for NativeDropdownApp {
    type State = Rc<Cell<u32>>;
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {
        Rc::new(Cell::new(0))
    }

    fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::dropdown_menu(
            "File",
            vec![
                DropdownMenuEntry::item("Open", ()),
                DropdownMenuEntry::submenu("Tools", vec![DropdownMenuEntry::item("Export", ())]),
            ],
            Style {
                size: Size {
                    width: Dimension::length(200.0),
                    height: Dimension::length(40.0),
                },
                ..Style::default()
            },
        )
        .with_id(900)
    }

    fn update(state: &mut Self::State, _: Self::Message, _: &mut arboard::Clipboard) {
        state.set(state.get() + 1);
    }
}

struct NativeDropdownTest {
    runner: RutterRunner<NativeDropdownApp>,
    selected: Rc<Cell<u32>>,
    opened: bool,
    pressed_popup: bool,
    dismiss_outside: bool,
    select_submenu: bool,
    moved_submenu: bool,
    deadline: Instant,
}

impl ApplicationHandler for NativeDropdownTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner
            .resume_surface(
                event_loop,
                Some(
                    WindowAttributes::default()
                        .with_title("Native Rutter dropdown")
                        .with_decorations(true)
                        .with_position(PhysicalPosition::new(190, 90))
                        .with_surface_size(PhysicalSize::new(320, 220)),
                ),
                Some(BackendType::OpenGl),
            )
            .expect("create dropdown parent surface");
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        self.runner.release_surface();
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "native menu did not deliver its selection"
        );
        self.runner.new_events(event_loop, cause);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.about_to_wait(event_loop);
        if self.dismiss_outside && self.pressed_popup && self.runner.native_menu.is_none() {
            assert_eq!(
                self.selected.get(),
                0,
                "outside click must not select an entry"
            );
            assert!(!self.runner.any_dropdown_menu_open());
            event_loop.exit();
        } else if !self.dismiss_outside && self.selected.get() == 1 {
            assert!(self.pressed_popup);
            assert!(
                self.runner.native_menu.is_none(),
                "selected menu must release native popup"
            );
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let parent = self.runner.active_window_id.unwrap();
        let popup = self.runner.native_menu.as_ref().map(|popup| popup.id());
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        self.runner.window_event(event_loop, id, event);
        assert!(self.runner.fatal_error.is_none());
        if id == parent && redraw && !self.opened {
            self.opened = true;
            click_at(parent, 40, 20);
        } else if Some(id) == popup && redraw && !self.pressed_popup {
            let (connection, _) = x11rb::connect(None).unwrap();
            let native_popup = id.into_raw() as u32;
            let tree = connection
                .query_tree(native_popup)
                .unwrap()
                .reply()
                .unwrap();
            assert_eq!(
                tree.parent, tree.root,
                "menu must use an unconfined X11 popup window"
            );
            assert_ne!(id, parent);
            assert_native_popup_fits_parent(&self.runner, id);
            if self.select_submenu && !self.moved_submenu {
                self.moved_submenu = true;
                move_at(id, 40, 52);
            } else if self.select_submenu
                && self
                    .runner
                    .native_menu
                    .as_ref()
                    .unwrap()
                    .backend
                    .window()
                    .surface_size()
                    .width
                    < 300
            {
                // Wait for the submenu's additional native pixels to be configured.
            } else if self.dismiss_outside {
                self.pressed_popup = true;
                click_at(parent, 300, 180);
            } else {
                self.pressed_popup = true;
                click_at(
                    id,
                    if self.select_submenu { 200 } else { 40 },
                    if self.select_submenu { 52 } else { 20 },
                );
            }
        }
    }
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_dropdown_selects_a_rutter_entry_on_its_popup_window() {
    run_native_dropdown_test(false, false);
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_dropdown_submenu_selects_a_rutter_entry() {
    run_native_dropdown_test(false, true);
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_dropdown_dismisses_on_outside_click_without_selection() {
    run_native_dropdown_test(true, false);
}

fn run_native_dropdown_test(dismiss_outside: bool, select_submenu: bool) {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let selected = Rc::new(Cell::new(0));
    let engine = RutterEngine::<NativeDropdownApp>::with_shared_font_system(
        selected.clone(),
        Rc::new(RefCell::new(FontSystem::new())),
        SurfaceConfig::default(),
    )
    .unwrap();
    builder
        .build()
        .unwrap()
        .run_app(NativeDropdownTest {
            runner: RutterRunner::with_engine(engine),
            selected: selected.clone(),
            opened: false,
            pressed_popup: false,
            dismiss_outside,
            select_submenu,
            moved_submenu: false,
            deadline: Instant::now() + Duration::from_secs(20),
        })
        .unwrap();
    assert_eq!(selected.get(), u32::from(!dismiss_outside));
}

struct NativeContextApp;

#[derive(Clone, Debug)]
enum NativeContextMessage {
    Opening,
    Select,
}

struct NativeContextState {
    selected: Rc<Cell<u32>>,
    entries: Vec<ContextMenuEntry<'static, NativeContextMessage>>,
    opening_entries: Option<Vec<ContextMenuEntry<'static, NativeContextMessage>>>,
}

impl AppLogic for NativeContextApp {
    type State = NativeContextState;
    type Message = NativeContextMessage;

    fn new(_: &mut FontSystem) -> Self::State {
        Self::State {
            selected: Rc::new(Cell::new(0)),
            entries: native_context_entries(false),
            opening_entries: None,
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::context_menu(
            Widget::Button {
                text: "Right click",
                on_press: NativeContextMessage::Select,
                style: Style {
                    size: Size {
                        width: Dimension::length(200.0),
                        height: Dimension::length(80.0),
                    },
                    ..Style::default()
                },
                color: None,
                variant: ButtonVariant::Primary,
            }
            .with_id(902),
            &state.entries,
            Style::default(),
        )
        .with_id(901)
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut arboard::Clipboard) {
        match message {
            NativeContextMessage::Opening => state.entries = state.opening_entries.take().unwrap(),
            NativeContextMessage::Select => state.selected.set(state.selected.get() + 1),
        }
    }

    fn context_menu_opening(
        state: &Self::State,
        _: crate::ContextMenuTarget,
    ) -> Option<Self::Message> {
        state
            .opening_entries
            .as_ref()
            .map(|_| NativeContextMessage::Opening)
    }
}

struct NativeContextTest {
    runner: RutterRunner<NativeContextApp>,
    selected: Rc<Cell<u32>>,
    opened: bool,
    pressed_popup: bool,
    nested: bool,
    hovered_levels: usize,
    keyboard_checked: bool,
    opening_change: bool,
    deadline: Instant,
}

impl ApplicationHandler for NativeContextTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner
            .resume_surface(
                event_loop,
                Some(
                    WindowAttributes::default()
                        .with_title("Native Rutter context menu")
                        .with_decorations(true)
                        .with_position(PhysicalPosition::new(190, 90))
                        .with_surface_size(if self.nested {
                            PhysicalSize::new(800, 500)
                        } else {
                            PhysicalSize::new(320, 220)
                        }),
                ),
                Some(BackendType::OpenGl),
            )
            .expect("create context menu parent surface");
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        self.runner.release_surface();
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "native context menu did not select"
        );
        self.runner.new_events(event_loop, cause);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.about_to_wait(event_loop);
        if self.selected.get() == 1 {
            assert!(self.pressed_popup);
            assert!(self.runner.native_menu.is_none());
            let (connection, _) = x11rb::connect(None).unwrap();
            assert_eq!(
                connection.get_input_focus().unwrap().reply().unwrap().focus,
                self.runner.active_window_id.unwrap().into_raw() as u32,
                "selecting a context-menu entry must restore native parent keyboard focus"
            );
            event_loop.exit();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let parent = self.runner.active_window_id.unwrap();
        let popup = self.runner.native_menu.as_ref().map(|popup| popup.id());
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        let opening_press = matches!(
            event,
            WindowEvent::PointerButton {
                button: ButtonSource::Mouse(MouseButton::Right),
                state: ElementState::Pressed,
                ..
            }
        );
        let popup_pointer_motion =
            Some(id) == popup && matches!(event, WindowEvent::PointerMoved { .. });
        self.runner.window_event(event_loop, id, event);
        assert!(self.runner.fatal_error.is_none());
        if popup_pointer_motion {
            assert_eq!(
                self.runner.engine.last_mouse_pos,
                logical_cursor_position(
                    self.runner.cursor_physical,
                    self.runner.engine.scale_factor
                ),
                "popup input and parent drawing must share the current pointer"
            );
            assert_eq!(
                self.runner.cursor_pos,
                Point::new(
                    self.runner.cursor_physical.x as f32,
                    self.runner.cursor_physical.y as f32
                ),
            );
        }
        if id == parent && opening_press && self.opening_change {
            assert!(self.runner.engine.app_state.opening_entries.is_none());
            let size = self.runner.engine.window.as_ref().unwrap().surface_size();
            self.runner.engine.try_ensure_widget_states().unwrap();
            self.runner.engine.try_ensure_layout(size).unwrap();
            assert!(
                self.runner.engine.any_context_menu_open(),
                "post-opening topology reconciliation closed the fresh menu"
            );
        }
        if id == parent && redraw && !self.opened {
            self.opened = true;
            click_at_button(parent, 40, 35, 3);
        } else if Some(id) == popup && redraw && !self.pressed_popup {
            if self.nested {
                self.step_submenu_popup(event_loop, id);
                return;
            }
            let (connection, _) = x11rb::connect(None).unwrap();
            let native_popup = id.into_raw() as u32;
            let tree = connection
                .query_tree(native_popup)
                .unwrap()
                .reply()
                .unwrap();
            assert_eq!(tree.parent, tree.root);
            assert_native_popup_fits_parent(&self.runner, id);
            self.pressed_popup = true;
            click_at(id, 40, 22);
        }
    }
}

impl NativeContextTest {
    fn step_submenu_popup(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId) {
        if !self.runner.native_menu.as_ref().unwrap().ready {
            return;
        }
        if !self.keyboard_checked {
            self.check_submenu_keyboard(event_loop);
            self.keyboard_checked = true;
        }
        let menu = self.runner.engine.widget_states[&901]
            .as_context_menu()
            .unwrap();
        if menu.navigation.open_submenu_path().len() != self.hovered_levels {
            return;
        }
        let scale = self.runner.engine.scale_factor;
        let size = self.runner.engine.window.as_ref().unwrap().surface_size();
        let font = crate::render::text::get_cached_font(
            &mut self.runner.engine.font_cache,
            "sans-serif",
            NativeContextApp::theme_for(&self.runner.engine.app_state).font_body,
        );
        let surfaces = crate::widgets::dropdown_menu::build_context_menu_surfaces(
            Point::new(menu.anchor_x, menu.anchor_y),
            &self.runner.engine.app_state.entries,
            &menu.navigation,
            SkiaRect::from_xywh(
                0.0,
                0.0,
                size.width as f32 / scale,
                size.height as f32 / scale,
            ),
            NativeContextApp::locale().direction(),
            &font,
        );
        let bounds = surfaces
            .iter()
            .skip(1)
            .fold(surfaces[0].rect, |bounds, panel| {
                SkiaRect::from_ltrb(
                    bounds.left.min(panel.rect.left),
                    bounds.top.min(panel.rect.top),
                    bounds.right.max(panel.rect.right),
                    bounds.bottom.max(panel.rect.bottom),
                )
            });
        let panel = &surfaces[self.hovered_levels];
        let entries = crate::widgets::dropdown_menu::entries_at_level(
            &self.runner.engine.app_state.entries,
            &panel.level_path,
        )
        .unwrap();
        let index = if self.hovered_levels == 0 {
            2
        } else if self.hovered_levels == 1 {
            1
        } else {
            0
        };
        let point = crate::widgets::dropdown_menu::row_rect(panel, entries, index)
            .unwrap()
            .center();
        let x = (point.x * scale - (bounds.left * scale).floor()) as i16;
        let y = (point.y * scale - (bounds.top * scale).floor()) as i16;
        assert_native_popup_fits_parent(&self.runner, id);
        if self.hovered_levels < 2 {
            self.hovered_levels += 1;
            move_at(id, x, y);
        } else {
            self.pressed_popup = true;
            click_at(id, x, y);
        }
    }

    fn check_submenu_keyboard(&mut self, event_loop: &dyn ActiveEventLoop) {
        let menu = self.runner.engine.widget_states[&901]
            .as_context_menu()
            .unwrap();
        let anchor = Point::new(menu.anchor_x, menu.anchor_y);
        self.runner.focus_widget(Some(902));
        self.runner.native_menu_event(
            event_loop,
            WindowEvent::ModifiersChanged(winit::keyboard::ModifiersState::CONTROL.into()),
        );
        assert!(self.runner.engine.modifiers.state().control_key());
        self.runner
            .handle_key(&Key::Character("More".into()), false);
        let navigation = &self.runner.engine.widget_states[&901]
            .as_context_menu()
            .unwrap()
            .navigation;
        assert_eq!(navigation.active_path(), Some([0].as_slice()));
        assert!(navigation.typeahead_buffer().is_empty());
        self.runner.native_menu_event(
            event_loop,
            WindowEvent::ModifiersChanged(winit::event::Modifiers::default()),
        );
        assert!(!self.runner.engine.modifiers.state().control_key());
        self.runner
            .handle_key(&Key::Character("More".into()), false);
        assert_eq!(
            self.runner.engine.widget_states[&901]
                .as_context_menu()
                .unwrap()
                .navigation
                .active_path(),
            Some([2].as_slice())
        );
        self.runner
            .engine
            .widget_states
            .get_mut(&901)
            .unwrap()
            .as_context_menu_mut()
            .unwrap()
            .navigation
            .open_at_index(Some(0));
        for key in [
            NamedKey::ArrowDown,
            NamedKey::ArrowRight,
            NamedKey::ArrowDown,
            NamedKey::Enter,
        ] {
            self.runner.handle_key(&Key::Named(key), false);
        }
        assert_eq!(
            self.runner.engine.widget_states[&901]
                .as_context_menu()
                .unwrap()
                .navigation
                .open_submenu_path(),
            [2, 1]
        );
        assert_eq!(self.runner.engine.focused_widget_id, Some(902));
        self.runner.handle_key(&Key::Named(NamedKey::Escape), false);
        assert!(!self.runner.engine.any_context_menu_open());
        assert_eq!(self.runner.engine.focused_widget_id, Some(902));
        self.runner.engine.open_context_menu(901, anchor);
        self.runner
            .handle_context_menu_hit(crate::render::hit_test::ContextMenuOverlayHit::Dismiss);
        assert!(!self.runner.engine.any_context_menu_open());
        assert_eq!(self.runner.engine.focused_widget_id, Some(902));
        self.runner.engine.open_context_menu(901, anchor);
    }
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_context_menu_selects_a_rutter_entry_on_its_popup_window() {
    run_native_context_menu(false, false);
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_context_menu_submenus_hover_select_and_restore_keyboard_focus() {
    run_native_context_menu(true, false);
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_context_menu_opening_reconciles_changed_disabled_status_and_submenu_structure() {
    run_native_context_menu(true, true);
}

fn native_context_entries(nested: bool) -> Vec<ContextMenuEntry<'static, NativeContextMessage>> {
    if nested {
        vec![
            ContextMenuEntry::disabled("Locked").with_svg_icon(CONTEXT_ICON_SVG),
            ContextMenuEntry::separator(),
            ContextMenuEntry::submenu(
                "More",
                vec![
                    ContextMenuEntry::disabled("Unavailable"),
                    ContextMenuEntry::submenu(
                        "Deeper",
                        vec![
                            ContextMenuEntry::item("Leaf", NativeContextMessage::Select)
                                .with_svg_icon(CONTEXT_ICON_SVG)
                                .with_shortcut_label("CTRL+C"),
                        ],
                    )
                    .with_svg_icon(CONTEXT_ICON_SVG),
                ],
            )
            .with_svg_icon(CONTEXT_ICON_SVG)
            .with_shortcut_label("ALT+M"),
        ]
    } else {
        vec![
            ContextMenuEntry::item("Copy", NativeContextMessage::Select)
                .with_svg_icon(CONTEXT_ICON_SVG)
                .with_shortcut_label("CTRL+C"),
        ]
    }
}

fn run_native_context_menu(nested: bool, opening_change: bool) {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let selected = Rc::new(Cell::new(0));
    let engine = RutterEngine::<NativeContextApp>::with_shared_font_system(
        NativeContextState {
            selected: selected.clone(),
            entries: if opening_change {
                vec![
                    ContextMenuEntry::disabled("Locked"),
                    ContextMenuEntry::separator(),
                    ContextMenuEntry::disabled_submenu(
                        "More",
                        vec![ContextMenuEntry::disabled("Old")],
                    ),
                ]
            } else {
                native_context_entries(nested)
            },
            opening_entries: opening_change.then(|| native_context_entries(nested)),
        },
        Rc::new(RefCell::new(FontSystem::new())),
        SurfaceConfig::default(),
    )
    .unwrap();
    builder
        .build()
        .unwrap()
        .run_app(NativeContextTest {
            runner: RutterRunner::with_engine(engine),
            selected: selected.clone(),
            opened: false,
            pressed_popup: false,
            nested,
            hovered_levels: 0,
            keyboard_checked: false,
            opening_change,
            deadline: Instant::now() + Duration::from_secs(20),
        })
        .unwrap();
    assert_eq!(selected.get(), 1);
}

fn assert_native_popup_fits_parent<A: AppLogic>(runner: &RutterRunner<A>, popup_id: WindowId) {
    let (connection, _) = x11rb::connect(None).unwrap();
    let parent = runner.engine.window.as_ref().unwrap();
    let root = connection
        .query_tree(parent.id().into_raw() as u32)
        .unwrap()
        .reply()
        .unwrap()
        .root;
    let position = |id| {
        connection
            .translate_coordinates(id, root, 0, 0)
            .unwrap()
            .reply()
            .unwrap()
    };
    let parent_origin = position(parent.id().into_raw() as u32);
    let popup_origin = position(popup_id.into_raw() as u32);
    let popup = runner.native_menu.as_ref().unwrap().backend.window();
    let relative = popup.outer_position().unwrap();
    assert_eq!(
        popup_origin.dst_x as i32,
        parent_origin.dst_x as i32 + relative.x
    );
    assert_eq!(
        popup_origin.dst_y as i32,
        parent_origin.dst_y as i32 + relative.y
    );
    assert!(
        relative.x >= 0 && relative.y >= 0,
        "popup must not overflow the parent top/left"
    );
    let (parent_size, popup_size) = (parent.surface_size(), popup.surface_size());
    assert!(
        relative.x as u32 + popup_size.width <= parent_size.width,
        "menu extends past the right edge of the parent"
    );
    assert!(
        relative.y as u32 + popup_size.height <= parent_size.height,
        "menu extends past the bottom edge of the parent"
    );
}

struct WaylandDropdownPositionTest {
    runner: RutterRunner<NativeDropdownApp>,
    backend: BackendType,
    selected: Rc<Cell<u32>>,
    opened: bool,
    hovered_submenu: bool,
    first_frame_at: Option<Instant>,
    deadline: Instant,
}

fn assert_wayland_initial_popup_geometry(event_loop: &dyn ActiveEventLoop, parent: &dyn Window) {
    let parent_handle = parent.window_handle().unwrap();
    // SAFETY: this probe is dropped before its live parent window.
    let attributes = unsafe {
        WindowAttributes::default()
            .with_window_type(WindowType::Popup)
            .with_parent_window(Some(parent_handle.as_raw()))
    }
    .with_active(false)
    .with_position(PhysicalPosition::new(230, 44))
    .with_surface_size(PhysicalSize::new(160, 72));
    let popup = event_loop.create_window(attributes).unwrap();
    assert_eq!(
        popup.surface_size(),
        PhysicalSize::new(160, 72),
        "initial popup size must not be replaced by its 1x1 anchor rectangle"
    );
    assert_eq!(
        popup.outer_position().unwrap(),
        PhysicalPosition::new(230, 44)
    );
}

impl ApplicationHandler for WaylandDropdownPositionTest {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner
            .resume_surface(
                event_loop,
                Some(
                    WindowAttributes::default()
                        .with_title("Wayland dropdown placement")
                        .with_surface_size(PhysicalSize::new(400, 220)),
                ),
                Some(self.backend),
            )
            .expect("create headless Wayland parent");
        assert_wayland_initial_popup_geometry(
            event_loop,
            self.runner.engine.window.as_ref().unwrap().as_ref(),
        );
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        self.runner.release_surface();
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        assert!(
            Instant::now() < self.deadline,
            "Wayland popup did not configure in time"
        );
        self.runner.new_events(event_loop, cause);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runner.about_to_wait(event_loop);
        if self.selected.get() == 1 {
            assert!(!self.runner.any_dropdown_menu_open());
            assert!(self.runner.native_menu.is_none());
            event_loop.exit();
            return;
        }
        if !self.opened
            && self
                .first_frame_at
                .is_some_and(|frame| frame.elapsed() >= Duration::from_millis(200))
        {
            self.opened = true;
            let parent = self.runner.active_window_id.unwrap();
            self.runner.window_event(
                event_loop,
                parent,
                WindowEvent::PointerButton {
                    device_id: None,
                    state: ElementState::Pressed,
                    position: PhysicalPosition::new(40.0, 20.0),
                    primary: true,
                    button: ButtonSource::Mouse(MouseButton::Left),
                    is_macos_activation_click: false,
                },
            );
        }
        if self.opened && self.runner.native_menus_unavailable {
            assert_eq!(self.backend, BackendType::CpuSoftbuffer);
            assert!(self.runner.native_menu.is_none());
            assert!(self.runner.any_dropdown_menu_open());
            let parent = self.runner.active_window_id.unwrap();
            let event = if !self.hovered_submenu {
                self.hovered_submenu = true;
                WindowEvent::PointerMoved {
                    device_id: None,
                    position: PhysicalPosition::new(48.0, 96.0),
                    primary: true,
                    source: winit::event::PointerSource::Mouse,
                }
            } else {
                WindowEvent::PointerButton {
                    device_id: None,
                    state: ElementState::Pressed,
                    position: PhysicalPosition::new(208.0, 96.0),
                    primary: true,
                    button: ButtonSource::Mouse(MouseButton::Left),
                    is_macos_activation_click: false,
                }
            };
            self.runner.window_event(event_loop, parent, event);
            return;
        }
        if let Some(popup) = self.runner.native_menu.as_ref() {
            if !popup.ready {
                return;
            }
            let window = popup.backend.window();
            let position = window
                .outer_position()
                .expect("configured Wayland popup position");
            assert!(
                (0..=16).contains(&position.x) && position.y == 44,
                "compositor must place popup near the trigger in parent content coordinates: {position:?}"
            );
            assert!(
                window.surface_size().width
                    <= self
                        .runner
                        .engine
                        .window
                        .as_ref()
                        .unwrap()
                        .surface_size()
                        .width
            );
            if !self.hovered_submenu {
                self.hovered_submenu = true;
                let id = window.id();
                self.runner.window_event(
                    event_loop,
                    id,
                    WindowEvent::PointerMoved {
                        device_id: None,
                        position: PhysicalPosition::new(40.0, 52.0),
                        primary: true,
                        source: winit::event::PointerSource::Mouse,
                    },
                );
            } else if window.surface_size().width >= 300 {
                let id = window.id();
                self.runner.window_event(
                    event_loop,
                    id,
                    WindowEvent::PointerButton {
                        device_id: None,
                        state: ElementState::Pressed,
                        position: PhysicalPosition::new(200.0, 52.0),
                        primary: true,
                        button: ButtonSource::Mouse(MouseButton::Left),
                        is_macos_activation_click: false,
                    },
                );
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            self.first_frame_at
                .filter(|_| !self.opened)
                .map(|frame| frame + Duration::from_millis(200))
                .unwrap_or(self.deadline),
        ));
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        let parent = self.runner.active_window_id.unwrap();
        self.runner.window_event(event_loop, id, event);
        assert!(self.runner.fatal_error.is_none());
        if id == parent && redraw {
            self.first_frame_at.get_or_insert_with(Instant::now);
        }
    }
}

#[test]
#[ignore = "requires a disposable Wayland compositor and an EGL driver with alpha support"]
fn native_wayland_dropdown_stays_at_parent_anchor_during_submenu_expansion() {
    run_wayland_dropdown_test(BackendType::OpenGl);
}

#[test]
#[ignore = "requires a disposable Wayland compositor with wl_shm support"]
fn wayland_cpu_dropdown_falls_back_to_overlay_and_selects_submenu() {
    run_wayland_dropdown_test(BackendType::CpuSoftbuffer);
}

fn run_wayland_dropdown_test(backend: BackendType) {
    let mut builder = EventLoop::builder();
    winit::platform::wayland::EventLoopBuilderExtWayland::with_wayland(&mut builder);
    winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    let selected = Rc::new(Cell::new(0));
    let engine = RutterEngine::<NativeDropdownApp>::with_shared_font_system(
        selected.clone(),
        Rc::new(RefCell::new(FontSystem::new())),
        SurfaceConfig::default(),
    )
    .unwrap();
    builder
        .build()
        .unwrap()
        .run_app(WaylandDropdownPositionTest {
            runner: RutterRunner::with_engine(engine),
            backend,
            selected: selected.clone(),
            opened: false,
            hovered_submenu: false,
            first_frame_at: None,
            deadline: Instant::now() + Duration::from_secs(20),
        })
        .unwrap();
    assert_eq!(selected.get(), 1);
}

#[test]
#[ignore = "requires a disposable X11 display, GL driver, and XTEST"]
fn native_mouse_selects_a_rendered_rutter_widget_with_accessibility_adapter() {
    let mut builder = EventLoop::builder();
    builder.with_x11();
    builder.with_any_thread(true);
    let clicks = Rc::new(Cell::new(0));
    let engine = RutterEngine::<NativeButtonApp>::with_shared_font_system(
        clicks.clone(),
        Rc::new(RefCell::new(FontSystem::new())),
        SurfaceConfig::default(),
    )
    .unwrap();
    builder
        .build()
        .unwrap()
        .run_app(NativeButtonTest {
            runner: RutterRunner::with_engine(engine),
            clicks: clicks.clone(),
            sent_click: false,
            received_press: false,
            recreated: false,
            deadline: Instant::now() + Duration::from_secs(20),
        })
        .unwrap();
    assert_eq!(clicks.get(), 2);
}
