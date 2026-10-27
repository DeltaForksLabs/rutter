//! Wayland 63.8: bounded transactions and placement before paint/input.

use super::*;
use fixture::{PopupState, SharedCell};
use std::sync::Arc;

pub(super) fn controlled_popup(
    runner: &mut RutterRunner<MenuApp>,
    asynchronous: bool,
    position_during_frame: Option<f64>,
) -> (Arc<PopupState>, Rc<Cell<usize>>) {
    let popup = runner.native_menu.as_mut().unwrap();
    let geometry = popup.configured.unwrap();
    let state = Arc::new(PopupState {
        geometry: SharedCell::new(geometry),
        asynchronous,
        requests: SharedCell::new(0),
    });
    let frames = Rc::new(Cell::new(1));
    popup.backend = Box::new(RasterPopup {
        window: Rc::new(MemoryWindow {
            size: geometry.surface_size,
            scale: geometry.scale_factor,
            popup: true,
            popup_state: Some(state.clone()),
        }),
        surface: skia_safe::surfaces::raster_n32_premul((
            geometry.surface_size.width as i32,
            geometry.surface_size.height as i32,
        ))
        .unwrap(),
        frames: frames.clone(),
        drops: Rc::new(Cell::new(0)),
        configure_on_frame: position_during_frame.map(|x| {
            let mut actual = geometry;
            actual.parent_position.x = x;
            (state.clone(), actual)
        }),
    });
    (state, frames)
}

fn acknowledge(state: &PopupState) {
    let mut geometry = state.geometry.get();
    geometry.configure_serial = geometry
        .configure_serial
        .map(|serial| serial.wrapping_add(1));
    // The compositor may adjust the requested position. Never assume the request
    // equals the configured rectangle, even on a synchronous backend.
    geometry.parent_position.x = -12.0;
    state.geometry.set(geometry);
}

#[test]
fn wayland63_8_placement_precedes_redraw_and_input_and_requests_coalesce() {
    for first in [FirstEvent::Redraw, FirstEvent::Button, FirstEvent::Pointer] {
        for scale in [1.0, 1.5, 2.0] {
            let OpenMenu { mut runner, .. } = open_menu(false, scale);
            let pointer = row_pointer(&runner);
            let (state, frames) = controlled_popup(&mut runner, true, None);
            runner.engine.app_state.owners.insert(0, 9);
            runner.invalidate_published_model();
            deliver(&mut runner, first, pointer);
            assert_eq!(state.requests.get(), 1);
            assert!(!runner.native_menu.as_ref().unwrap().ready);
            assert!(runner.engine.native_menu_layout.is_none());
            assert!(runner.engine.app_state.selected.is_empty());
            assert_eq!(frames.get(), 1);
            let deadline = runner.pending_native_menu_deadline().unwrap();
            for _ in 0..3 {
                runner
                    .sync_native_menu(&MemoryEventLoop::default())
                    .unwrap();
                assert!(!runner.present_native_menu_frame().unwrap());
                assert!(!runner.reconcile_native_menu_before_input().unwrap());
                assert_eq!(state.requests.get(), 1);
                assert_eq!(frames.get(), 1);
                assert_eq!(runner.pending_native_menu_deadline(), Some(deadline));
            }
            // A second desired placement waits for the first configure; it must
            // not authorize a frame at that intermediate rectangle.
            runner.engine.app_state.owner_width = 200.0;
            runner.invalidate_published_model();
            assert!(!runner.present_native_menu_frame().unwrap());
            assert_eq!(state.requests.get(), 1);
            acknowledge(&state);
            assert!(!runner.present_native_menu_frame().unwrap());
            assert_eq!(state.requests.get(), 2);
            assert!(runner.pending_native_menu_deadline().unwrap() >= deadline);
            assert_eq!(frames.get(), 1);
            assert!(!runner.present_native_menu_frame().unwrap());
            acknowledge(&state);
            assert!(runner.reconcile_native_menu_before_input().unwrap());
            assert_eq!(frames.get(), 2);
            assert_eq!(runner.pending_native_menu_deadline(), None);
            let popup = runner.native_menu.as_ref().unwrap();
            assert_eq!(popup.configured.unwrap().parent_position.x, -12.0);
            assert_eq!(
                runner.engine.native_menu_layout.as_ref().unwrap().1[0]
                    .rect
                    .left,
                -12.0
            );
            // Use the current configured body's coordinates after the parent moved.
            let pointer = row_pointer(&runner);
            deliver(&mut runner, FirstEvent::Button, pointer);
            assert_eq!(runner.engine.app_state.selected, [7]);
        }
    }
}

#[test]
fn wayland63_8_pending_expiry_is_bounded_and_reports_phase_once() {
    for awaiting in [false, true] {
        let OpenMenu { mut runner, .. } = open_menu(false, 1.0);
        let popup = runner.native_menu.as_mut().unwrap();
        popup.begin_pending();
        popup.awaiting_configure = awaiting;
        popup.pending_deadline = Some(Instant::now());
        assert!(!runner.present_native_menu_frame().unwrap());
        assert!(runner.native_menus_unavailable);
        assert!(runner.dropdown_is_open(7)); // usable overlay remains interactive
        assert!(!runner.resolve_native_menu().unwrap());
        runner
            .sync_native_menu(&MemoryEventLoop::default())
            .unwrap();
        let failures = runner.engine.app_state.failures.borrow();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].0);
        let phase = if awaiting {
            "configure"
        } else {
            "presentation"
        };
        assert_eq!(failures[0].1, format!("popup {phase} timed out"));
    }
}

#[test]
fn wayland63_8_timeout_dismisses_unusable_overlay_and_reopen_remains_observable() {
    let OpenMenu { mut runner, .. } = open_menu(false, 1.0);
    runner.engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(30, 20),
        scale: 1.0,
        popup: false,
        popup_state: None,
    }));
    let popup = runner.native_menu.as_mut().unwrap();
    popup.begin_pending();
    popup.pending_deadline = Some(Instant::now());
    runner.invalidate_published_model();
    assert!(!runner.present_native_menu_frame().unwrap());
    assert!(!runner.dropdown_is_open(7));
    assert!(runner.native_menu.is_none());
    assert_eq!(runner.pending_native_menu_deadline(), None);
    assert_eq!(runner.engine.app_state.failures.borrow().len(), 1);
    assert!(!runner.engine.app_state.failures.borrow()[0].0);
    runner.open_dropdown_menu(7, Some(vec![0]));
    runner
        .sync_native_menu(&MemoryEventLoop::default())
        .unwrap();
    assert!(!runner.dropdown_is_open(7));
    assert_eq!(runner.engine.app_state.failures.borrow().len(), 2);
    assert!(!runner.engine.app_state.failures.borrow()[1].0);
}

#[test]
fn wayland63_8_content_invalidation_does_not_extend_a_pending_frame() {
    let OpenMenu {
        mut runner, frames, ..
    } = open_menu(true, 1.0);
    runner.invalidate_published_model();
    assert!(runner.resolve_native_menu().unwrap());
    let deadline = runner.pending_native_menu_deadline().unwrap();
    for _ in 0..3 {
        runner.invalidate_published_model();
        assert!(runner.resolve_native_menu().unwrap());
        assert_eq!(runner.pending_native_menu_deadline(), Some(deadline));
    }
    assert!(runner.present_native_menu_frame().unwrap());
    assert_eq!(frames.get(), 2);
    assert_eq!(runner.pending_native_menu_deadline(), None);
    runner.invalidate_published_model();
    assert!(runner.resolve_native_menu().unwrap());
    assert!(runner.pending_native_menu_deadline().unwrap() >= deadline);
}

#[test]
fn wayland63_8_synchronous_placement_rereads_configured_geometry() {
    let OpenMenu { mut runner, .. } = open_menu(false, 1.0);
    let (state, frames) = controlled_popup(&mut runner, false, None);
    acknowledge(&state);
    runner.engine.app_state.owners.insert(0, 9);
    runner.invalidate_published_model();
    assert!(runner.reconcile_native_menu_before_input().unwrap());
    assert_eq!(state.requests.get(), 1);
    assert_eq!(
        runner
            .native_menu
            .as_ref()
            .unwrap()
            .configured
            .unwrap()
            .parent_position
            .x,
        -12.0
    );
    assert_eq!(runner.pending_native_menu_deadline(), None);
    assert_eq!(frames.get(), 2);
}

#[test]
fn wayland63_8_configure_during_presentation_requires_a_fresh_matching_frame() {
    let OpenMenu { mut runner, .. } = open_menu(false, 1.0);
    let (_, frames) = controlled_popup(&mut runner, false, Some(-12.0));
    assert!(!runner.present_native_menu_frame().unwrap());
    assert_eq!(frames.get(), 2);
    assert!(!runner.native_menu.as_ref().unwrap().ready);
    assert!(runner.pending_native_menu_deadline().is_some());
    assert!(runner.reconcile_native_menu_before_input().unwrap());
    assert_eq!(frames.get(), 3);
    assert!(runner.native_menu.as_ref().unwrap().ready);
    assert_eq!(runner.pending_native_menu_deadline(), None);
}
