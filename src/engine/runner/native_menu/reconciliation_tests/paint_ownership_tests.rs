//! Paint ownership survives pending transactions without authorizing stale input.

use super::transaction_tests::controlled_popup;
use super::*;
use crate::engine::{BackendType, GraphicsError};

struct FailingFrame {
    window: Rc<dyn winit::window::Window>,
    surface: skia_safe::Surface,
}

impl GraphicsBackend for FailingFrame {
    fn backend_type(&self) -> BackendType {
        BackendType::CpuSoftbuffer
    }
    fn begin_frame(&mut self) -> Result<&skia_safe::Canvas, GraphicsError> {
        Ok(self.surface.canvas())
    }
    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        Err(GraphicsError::Frame {
            backend: self.backend_type(),
            reason: "controlled popup frame failure".into(),
        })
    }
    fn resize(&mut self, _: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        panic!("fixed configure must not resize");
    }
    fn window(&self) -> &Rc<dyn winit::window::Window> {
        &self.window
    }
}

#[test]
fn native_paint_owner_frame_failure_restores_fallback_before_and_after_first_frame() {
    for presented in [false, true] {
        let OpenMenu { mut runner, .. } = if presented {
            open_menu(false, 1.0)
        } else {
            pending_menu(false, 1.0)
        };
        assert_eq!(runner.native_menu_paint_owner().is_some(), presented);
        let popup = runner.native_menu.as_mut().unwrap();
        popup.backend = Box::new(FailingFrame {
            window: popup.backend.window().clone(),
            surface: skia_safe::surfaces::raster_n32_premul((160, 80)).unwrap(),
        });
        assert!(!runner.present_native_menu_frame().unwrap());
        assert!(runner.native_menu.is_none());
        assert_eq!(runner.native_menu_paint_owner(), None);
        assert!(runner.engine.native_menu_layout.is_none());
        assert!(runner.dropdown_is_open(7));
        assert!(runner.native_menus_unavailable);
        assert_eq!(runner.engine.app_state.failures.borrow().len(), 1);
    }
}

#[test]
fn initial_async_pending_does_not_acquire_native_paint() {
    let OpenMenu { mut runner, .. } = pending_menu(false, 1.0);
    assert!(runner.resolve_native_menu().unwrap());
    controlled_popup(&mut runner, true, None);
    runner.engine.app_state.owners.insert(0, 9);
    runner.invalidate_published_model();
    assert!(!runner.present_native_menu_frame().unwrap());
    assert!(runner.engine.native_menu_layout.is_none());
    assert_eq!(runner.native_menu_paint_owner(), None);
    assert!(runner.dropdown_is_open(7));
}

#[test]
fn native_paint_owner_survives_async_pending_without_resolved_layout() {
    for context in [false, true] {
        let OpenMenu { mut runner, .. } = open_menu(context, 1.0);
        let kind = runner.native_menu.as_ref().unwrap().kind;
        let (_, frames) = controlled_popup(&mut runner, true, None);
        // Move the dropdown opener or the context anchor to force a transaction.
        if context {
            runner
                .engine
                .widget_states
                .get_mut(&7)
                .unwrap()
                .as_context_menu_mut()
                .unwrap()
                .open_at(80.0, 60.0);
        } else {
            runner.engine.app_state.owners.insert(0, 9);
        }
        runner.invalidate_published_model();
        for _ in 0..3 {
            assert!(!runner.present_native_menu_frame().unwrap());
            assert!(!runner.native_menu.as_ref().unwrap().ready);
            assert!(runner.engine.native_menu_layout.is_none());
            assert_eq!(runner.native_menu_paint_owner(), Some(kind));
            assert!(!runner.reconcile_native_menu_before_input().unwrap());
            assert_eq!(
                runner
                    .native_cursor_targets(Point::new(40.0, 80.0))
                    .unwrap(),
                (None, None)
            );
            assert_eq!(frames.get(), 1);
        }
        assert!(runner.engine.app_state.selected.is_empty());
    }
}

#[test]
fn native_paint_owner_releases_on_pending_failure_timeout_and_retirement() {
    for release in ["failure", "timeout", "retirement", "replacement"] {
        let OpenMenu { mut runner, .. } = open_menu(false, 1.0);
        let kind = runner.native_menu_paint_owner().unwrap();
        controlled_popup(&mut runner, true, None);
        runner.engine.app_state.owners.insert(0, 9);
        runner.invalidate_published_model();
        assert!(!runner.resolve_native_menu().unwrap());
        assert_eq!(runner.native_menu_paint_owner(), Some(kind));
        match release {
            "failure" => runner
                .disable_native_menus(kind, "controlled failure".into())
                .unwrap(),
            "timeout" => {
                runner.native_menu.as_mut().unwrap().pending_deadline = Some(Instant::now());
                assert!(!runner.present_native_menu_frame().unwrap());
            }
            "retirement" => runner.retire_native_menu(),
            "replacement" => {
                runner.engine.app_state.entry_label = "Replacement";
                runner.invalidate_published_model();
                assert!(!runner.resolve_native_menu().unwrap());
            }
            _ => unreachable!(),
        }
        assert!(runner.native_menu.is_none());
        assert_eq!(runner.native_menu_paint_owner(), None);
        assert!(runner.engine.native_menu_layout.is_none());
        if matches!(release, "failure" | "timeout") {
            assert!(runner.dropdown_is_open(7));
        }
    }
}

#[test]
fn native_paint_owner_requires_first_successful_matching_frame() {
    let OpenMenu {
        mut runner, frames, ..
    } = pending_menu(false, 1.0);
    assert_eq!(frames.get(), 0);
    assert!(runner.resolve_native_menu().unwrap());
    assert_eq!(runner.native_menu_paint_owner(), None);
    // end_frame succeeds, but the configure changes during presentation.
    controlled_popup(&mut runner, false, Some(-12.0));
    assert!(!runner.present_native_menu_frame().unwrap());
    assert_eq!(runner.native_menu_paint_owner(), None);
    assert!(!runner.native_menu.as_ref().unwrap().ready);
    assert!(runner.present_native_menu_frame().unwrap());
    assert!(runner.native_menu_paint_owner().is_some());
}
