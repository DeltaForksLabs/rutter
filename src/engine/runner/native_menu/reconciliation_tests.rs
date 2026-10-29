//! Runner event paths with an in-memory configure and raster popup; parent
//! redraws are deliberately never delivered after publication. As with other
//! runner unit tests, engine construction requires an available clipboard.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use cosmic_text::FontSystem;
use taffy::prelude::{Dimension, Size, Style};
use winit::event::{MouseScrollDelta, TouchPhase};

use super::*;
use crate::engine::RutterEngine;
use crate::render::dropdown_menu_overlay::DropdownMenuOverlayHit;
use crate::widget::id::{WidgetIdError, WidgetIdSnapshot};
use crate::widget::{ContextMenuEntry, Widget};
use crate::widgets::dropdown_menu::DropdownMenuEntry;

use crate::engine::test_surface as fixture;
use fixture::{MemoryEventLoop, MemoryWindow, RasterPopup};

mod paint_ownership_tests;
mod transaction_tests;

struct MenuApp;

struct MenuModel {
    context: bool,
    owners: Vec<u64>,
    entries: HashMap<u64, Vec<ContextMenuEntry<'static, u64>>>,
    fallback: Vec<ContextMenuEntry<'static, u64>>,
    selected: Vec<u64>,
    incompatible: bool,
    unstable: bool,
    unstable_calls: usize,
    action_offset: u64,
    owner_width: f32,
    entry_label: &'static str,
    publish_on_failure: bool,
    failures: std::cell::RefCell<Vec<(bool, String)>>,
}

fn owner_style(width: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::length(width),
            height: Dimension::length(40.0),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

impl AppLogic for MenuApp {
    type State = MenuModel;
    type Message = u64;

    fn new(_: &mut FontSystem) -> MenuModel {
        MenuModel {
            context: true,
            owners: vec![7, 8],
            entries: (7..=9)
                .map(|id| (id, vec![ContextMenuEntry::item("Run", id)]))
                .collect(),
            fallback: vec![ContextMenuEntry::item("Run", 0)],
            selected: Vec::new(),
            incompatible: false,
            unstable: false,
            unstable_calls: 0,
            action_offset: 0,
            owner_width: 100.0,
            entry_label: "Run",
            publish_on_failure: false,
            failures: Default::default(),
        }
    }

    fn view<'a>(state: &'a mut MenuModel) -> Widget<'a, u64> {
        let mut owners = state.owners.clone();
        if state.unstable {
            // Two matching captures followed by two different captures defeats
            // unconditional recapture + validate, but not a clean-state guard.
            if (state.unstable_calls / 2).is_multiple_of(2) {
                owners.push(9);
            }
            state.unstable_calls += 1;
        }
        let children = owners
            .into_iter()
            .map(|id| {
                if state.incompatible && id == 7 {
                    return Widget::spinner(owner_style(state.owner_width)).with_id(id);
                }
                if state.context {
                    Widget::context_menu(
                        Widget::Spacer {
                            style: owner_style(state.owner_width),
                        },
                        state.entries.get(&id).unwrap_or(&state.fallback),
                        owner_style(state.owner_width),
                    )
                    .with_id(id)
                } else {
                    Widget::dropdown_menu(
                        "Actions",
                        vec![DropdownMenuEntry::item(
                            state.entry_label,
                            id + state.action_offset,
                        )],
                        owner_style(state.owner_width),
                    )
                    .with_id(id)
                }
            })
            .collect();
        Widget::Row {
            children,
            style: Style::default(),
        }
    }

    fn update(state: &mut MenuModel, message: u64, _: &mut arboard::Clipboard) {
        if message == 999 {
            state.owners.insert(0, 9);
        } else {
            state.selected.push(message);
        }
    }

    fn native_menu_unavailable(
        state: &MenuModel,
        failure: crate::app::NativeMenuFailure,
    ) -> Option<u64> {
        state
            .failures
            .borrow_mut()
            .push((failure.overlay_usable, failure.reason));
        state.publish_on_failure.then_some(999)
    }
}

struct OpenMenu {
    runner: RutterRunner<MenuApp>,
    frames: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
}

fn open_menu(context: bool, scale: f64) -> OpenMenu {
    let mut menu = pending_menu(context, scale);
    assert!(menu.runner.present_native_menu_frame().unwrap());
    assert_eq!(menu.frames.get(), 1);
    menu
}

fn pending_menu(context: bool, scale: f64) -> OpenMenu {
    let mut engine = RutterEngine::<MenuApp>::new().expect("runner test clipboard service");
    engine.app_state.context = context;
    engine.scale_factor = scale as f32;
    engine.window = Some(Rc::new(MemoryWindow {
        size: winit::dpi::LogicalSize::new(800, 500).to_physical(scale),
        scale,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.reconcile_native_menu_model().unwrap();
    if context {
        runner
            .engine
            .widget_states
            .get_mut(&7)
            .unwrap()
            .as_context_menu_mut()
            .unwrap()
            .open_at(20.0, 60.0);
        runner.engine.layout_dirty = true;
    } else {
        runner.open_dropdown_menu(7, Some(vec![0]));
    }
    runner.reconcile_native_menu_model().unwrap();
    let NativeMenuRequest::Menu(placement, surfaces) = runner.requested_native_menu().unwrap()
    else {
        panic!("fixture opener must request a native menu");
    };
    let size = winit::dpi::LogicalSize::new(160, 80).to_physical(scale);
    let frames = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    runner.native_menu = Some(NativeMenuSurface {
        backend: Box::new(RasterPopup {
            window: Rc::new(MemoryWindow {
                size,
                scale,
                popup: true,
                popup_state: None,
                input_state: None,
            }),
            surface: skia_safe::surfaces::raster_n32_premul((
                size.width as i32,
                size.height as i32,
            ))
            .unwrap(),
            frames: frames.clone(),
            drops: drops.clone(),
            configure_on_frame: None,
        }),
        kind: placement.kind,
        ready: false,
        owns_paint: false,
        parent_layout_revision: runner.engine.layout_revision,
        pending_deadline: Some(Instant::now() + Duration::from_secs(60)),
        placement,
        desired_placement: placement,
        requested_origin: Point::new(
            placement.position.x as f32 / scale as f32,
            placement.position.y as f32 / scale as f32,
        ),
        requested_surfaces: surfaces,
        awaiting_configure: false,
        configure_serial_before_request: None,
        configured: None,
        allocated_size: size,
        last_pointer: None,
    });
    OpenMenu {
        runner,
        frames,
        drops,
    }
}

#[derive(Clone, Copy, Debug)]
enum FirstEvent {
    Pointer,
    Button,
    Wheel,
    Configure,
    Move,
    Redraw,
    Cursor,
    Present,
}

fn row_pointer(runner: &RutterRunner<MenuApp>) -> PhysicalPosition<f64> {
    let (_, surfaces) = runner.engine.native_menu_layout.as_ref().unwrap();
    let rect = surfaces[0].rect;
    let scale = runner
        .native_menu
        .as_ref()
        .unwrap()
        .configured
        .unwrap()
        .scale_factor;
    PhysicalPosition::new(f64::from(rect.center_x() - 20.0) * scale, 20.0 * scale)
}

fn button(position: PhysicalPosition<f64>) -> WindowEvent {
    WindowEvent::PointerButton {
        device_id: None,
        state: ElementState::Pressed,
        position,
        primary: true,
        button: ButtonSource::Mouse(MouseButton::Left),
        is_macos_activation_click: false,
    }
}

fn deliver(runner: &mut RutterRunner<MenuApp>, first: FirstEvent, position: PhysicalPosition<f64>) {
    let event_loop = MemoryEventLoop::default();
    let event = match first {
        FirstEvent::Pointer => WindowEvent::PointerMoved {
            device_id: None,
            position,
            primary: true,
            source: PointerSource::Mouse,
        },
        FirstEvent::Button => button(position),
        FirstEvent::Wheel => WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::LineDelta(0.0, -1.0),
            phase: TouchPhase::Moved,
        },
        FirstEvent::Configure => {
            WindowEvent::SurfaceResized(runner.native_menu.as_ref().unwrap().allocated_size)
        }
        FirstEvent::Move => WindowEvent::Moved(PhysicalPosition::new(20, 60)),
        FirstEvent::Redraw => WindowEvent::RedrawRequested,
        FirstEvent::Cursor => {
            let point = popup_logical_pointer(
                position,
                runner.native_menu.as_ref().unwrap().configured.unwrap(),
            );
            runner.native_cursor_targets(point).unwrap();
            return;
        }
        FirstEvent::Present => {
            runner.present_native_menu_frame().unwrap();
            return;
        }
    };
    runner.native_menu_event(&event_loop, event);
    assert!(
        runner.fatal_error.is_none(),
        "{first:?}: {:?}",
        runner.fatal_error
    );
    assert!(!event_loop.exiting());
}

fn assert_current_snapshot(runner: &mut RutterRunner<MenuApp>) {
    let widget = MenuApp::view(&mut runner.engine.app_state);
    let current = WidgetIdSnapshot::capture(&widget).unwrap();
    runner
        .engine
        .widget_id_snapshot
        .as_ref()
        .unwrap()
        .validate_reconstruction(&current)
        .unwrap();
}

#[test]
fn wayland63_8_ready_popup_invalidation_does_not_reuse_expired_deadline() {
    for context in [true, false] {
        let OpenMenu {
            mut runner, frames, ..
        } = open_menu(context, 1.0);
        // A completed frame retires its budget even when the old deadline has
        // elapsed by the next publication (no wall-clock sleep required).
        assert_eq!(runner.pending_native_menu_deadline(), None);
        runner.invalidate_published_model();
        assert!(runner.present_native_menu_frame().unwrap());
        assert_eq!(frames.get(), 2);
        assert!(!runner.native_menus_unavailable);
    }
}

#[test]
fn native_menu_publication_insert_move_and_remove_before_parent_redraw() {
    for context in [true, false] {
        for first in [
            FirstEvent::Pointer,
            FirstEvent::Button,
            FirstEvent::Wheel,
            FirstEvent::Configure,
            FirstEvent::Move,
            FirstEvent::Redraw,
            FirstEvent::Cursor,
            FirstEvent::Present,
        ] {
            for owners in [vec![9, 7, 8], vec![8, 7], vec![7]] {
                let OpenMenu {
                    mut runner, frames, ..
                } = open_menu(context, 1.0);
                let pointer = row_pointer(&runner);
                let before = runner.engine.widget_id_snapshot.clone().unwrap();
                runner.engine.app_state.owners = owners;
                runner.invalidate_published_model();
                assert!(runner.engine.layout_dirty);
                deliver(&mut runner, first, pointer);
                assert_current_snapshot(&mut runner);
                assert_ne!(runner.engine.widget_id_snapshot.as_ref(), Some(&before));
                assert_eq!(
                    runner.engine.widget_states.len(),
                    runner.engine.app_state.owners.len()
                );
                let children = runner
                    .engine
                    .taffy
                    .children(runner.engine.last_root_node)
                    .unwrap();
                assert_eq!(children.len(), runner.engine.app_state.owners.len());
                let index = runner
                    .engine
                    .app_state
                    .owners
                    .iter()
                    .position(|id| *id == 7)
                    .unwrap();
                let layout = runner.engine.taffy.layout(children[index]).unwrap();
                assert_eq!(layout.location.x, index as f32 * 100.0);
                if matches!(first, FirstEvent::Button) {
                    assert_eq!(runner.engine.app_state.selected, [7]);
                } else {
                    // Configure/move resolution can defer presentation, but
                    // input must present the reconciled model first.
                    assert!(runner.reconcile_native_menu_before_input().unwrap());
                    assert!(frames.get() >= 2);
                    assert!(matches!(
                        runner
                            .native_cursor_targets(Point::new(40.0, 80.0))
                            .unwrap()
                            .0,
                        Some(DropdownMenuOverlayHit::Entry { id: 7, .. })
                    ));
                    runner.native_menu_event(&MemoryEventLoop::default(), button(pointer));
                    assert_eq!(runner.engine.app_state.selected, [7]);
                }
            }
        }
    }
}

#[test]
fn native_menu_removed_opener_releases_surface_and_cannot_retarget_replacement() {
    for context in [true, false] {
        for first in [
            FirstEvent::Pointer,
            FirstEvent::Button,
            FirstEvent::Wheel,
            FirstEvent::Configure,
            FirstEvent::Move,
            FirstEvent::Redraw,
            FirstEvent::Cursor,
            FirstEvent::Present,
        ] {
            let OpenMenu {
                mut runner,
                drops,
                frames,
            } = open_menu(context, 1.0);
            let pointer = row_pointer(&runner);
            runner.engine.app_state.owners = vec![8]; // Different key now occupies slot zero.
            runner.invalidate_published_model();
            deliver(&mut runner, first, pointer);
            assert_current_snapshot(&mut runner);
            assert!(runner.native_menu.is_none());
            assert!(runner.engine.native_menu_layout.is_none());
            assert!(!runner.engine.widget_states.contains_key(&7));
            assert_eq!(drops.get(), 1);
            assert_eq!(frames.get(), 1);
            assert!(runner.engine.app_state.selected.is_empty());
            assert_eq!(
                runner
                    .native_cursor_targets(Point::new(40.0, 80.0))
                    .unwrap(),
                (None, None)
            );
            runner
                .press_native_menu(Point::new(40.0, 80.0), MouseButton::Left)
                .unwrap();
            assert!(runner.engine.app_state.selected.is_empty());
        }
    }
}

#[test]
fn native_menu_same_id_model_changes_refresh_actions_and_present_before_input_at_each_scale() {
    for context in [true, false] {
        for scale in [1.0, 1.5, 2.0] {
            let OpenMenu {
                mut runner, frames, ..
            } = open_menu(context, scale);
            let pointer = row_pointer(&runner);
            let before = runner.engine.widget_id_snapshot.clone();
            let previous_width = runner.native_menu.as_ref().unwrap().requested_surfaces[0]
                .rect
                .width();
            runner.engine.app_state.action_offset = 63;
            runner.engine.app_state.owner_width = 200.0;
            runner.engine.app_state.entries.insert(
                7,
                vec![ContextMenuEntry::item("Run", 70).with_shortcut_label("CTRL+SHIFT+ENTER")],
            );
            runner.invalidate_published_model();
            deliver(&mut runner, FirstEvent::Button, pointer);
            assert_eq!(runner.engine.widget_id_snapshot, before);
            assert_eq!(runner.engine.app_state.selected, [70]);
            assert_eq!(frames.get(), 2);
            if context {
                assert!(
                    runner.native_menu.as_ref().unwrap().requested_surfaces[0]
                        .rect
                        .width()
                        > previous_width
                );
            }
            let children = runner
                .engine
                .taffy
                .children(runner.engine.last_root_node)
                .unwrap();
            assert_eq!(
                runner.engine.taffy.layout(children[0]).unwrap().size.width,
                200.0
            );
        }
    }
}

#[test]
fn native_menu_unavailable_callback_publication_reconciles_without_a_parent_redraw() {
    for context in [true, false] {
        let OpenMenu {
            mut runner, drops, ..
        } = open_menu(context, 1.0);
        runner.engine.app_state.publish_on_failure = true;
        let kind = runner.native_menu.as_ref().unwrap().kind;
        runner
            .disable_native_menus(kind, "controlled native failure".into())
            .unwrap();
        assert_eq!(drops.get(), 1);
        assert!(runner.engine.layout_dirty);
        assert!(!runner.resolve_native_menu().unwrap());
        assert_current_snapshot(&mut runner);
        assert!(runner.engine.widget_states.contains_key(&9));
        assert!(runner.engine.native_menu_layout.is_none());
    }
}

#[test]
fn native_menu_changed_opener_topology_revokes_the_popup_before_selection() {
    for context in [true, false] {
        let OpenMenu {
            mut runner,
            frames,
            drops,
        } = open_menu(context, 1.0);
        let pointer = row_pointer(&runner);
        runner.engine.app_state.entry_label = "Replacement";
        runner
            .engine
            .app_state
            .entries
            .insert(7, vec![ContextMenuEntry::item("Replacement", 70)]);
        runner.invalidate_published_model();
        deliver(&mut runner, FirstEvent::Button, pointer);
        assert!(runner.native_menu.is_none());
        assert!(runner.engine.native_menu_layout.is_none());
        assert!(!runner.dropdown_is_open(7));
        assert!(runner.engine.app_state.selected.is_empty());
        assert_eq!(frames.get(), 1);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn native_menu_repaints_after_another_input_path_already_reconciled_the_parent() {
    for context in [true, false] {
        let OpenMenu {
            mut runner, frames, ..
        } = open_menu(context, 1.0);
        let pointer = row_pointer(&runner);
        runner.engine.app_state.action_offset = 63;
        runner
            .engine
            .app_state
            .entries
            .insert(7, vec![ContextMenuEntry::item("Run", 70)]);
        runner.invalidate_published_model();
        let size = runner.engine.window.as_ref().unwrap().surface_size();
        runner.engine.try_ensure_layout_for_input(size).unwrap();
        assert!(!runner.engine.layout_dirty);
        assert_ne!(
            runner.native_menu.as_ref().unwrap().parent_layout_revision,
            runner.engine.layout_revision
        );
        deliver(&mut runner, FirstEvent::Button, pointer);
        assert_eq!(frames.get(), 2);
        assert_eq!(runner.engine.app_state.selected, [70]);
    }
}

#[test]
fn native_menu_invalid_publications_still_fail_at_the_event_boundary() {
    for context in [true, false] {
        for invalid in 0..3 {
            let OpenMenu { mut runner, .. } = open_menu(context, 1.0);
            let before = runner.engine.widget_id_snapshot.clone();
            match invalid {
                0 => runner.engine.app_state.owners.push(7),
                1 => runner.engine.app_state.owners.push(u64::MAX),
                _ => runner.engine.app_state.incompatible = true,
            }
            runner.invalidate_published_model();
            let event_loop = MemoryEventLoop::default();
            runner.native_menu_event(&event_loop, WindowEvent::RedrawRequested);
            let Some(RutterRunError::WidgetId(error)) = &runner.fatal_error else {
                panic!("invalid model must remain a widget-ID error");
            };
            assert!(match invalid {
                0 => matches!(error, WidgetIdError::Duplicate { value: 7, .. }),
                1 => matches!(error, WidgetIdError::ReservedValue { value: u64::MAX }),
                _ => matches!(error, WidgetIdError::IncompatibleReuse { value: 7, .. }),
            });
            assert!(event_loop.exiting());
            assert_eq!(runner.engine.widget_id_snapshot, before);
        }
    }
}

#[test]
fn native_menu_unchanged_nondeterministic_view_is_not_accepted_as_another_publication() {
    for context in [true, false] {
        let OpenMenu { mut runner, .. } = open_menu(context, 1.0);
        runner.engine.app_state.unstable = true;
        runner.invalidate_published_model();
        // First two views agree and legitimately publish a new snapshot/layout.
        runner
            .engine
            .try_ensure_layout_for_input(runner.engine.window.as_ref().unwrap().surface_size())
            .unwrap();
        assert!(!runner.engine.layout_dirty);
        let before = runner.engine.widget_id_snapshot.clone();
        // Ordinary dropdown pointer preparation also calls this method. It
        // must not replace the accepted snapshot before the native hit guard.
        assert!(matches!(
            runner.engine.try_ensure_widget_states(),
            Err(WidgetIdError::InconsistentTree { .. })
        ));
        assert_eq!(runner.engine.widget_id_snapshot, before);
        let event_loop = MemoryEventLoop::default();
        runner.native_menu_event(&event_loop, WindowEvent::RedrawRequested);
        assert!(matches!(
            runner.fatal_error,
            Some(RutterRunError::WidgetId(
                WidgetIdError::InconsistentTree { .. }
            ))
        ));
        assert!(event_loop.exiting());
        assert_eq!(runner.engine.widget_id_snapshot, before);
    }
}
