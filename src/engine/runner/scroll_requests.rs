//! Additional request 63.7/63.8 ownership, clipping and scale regressions.
use super::*;

fn move_pointer(runner: &mut RutterRunner<ScrollApp>, x: f64, y: f64) {
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(x, y),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
}

#[test]
fn task63_thumb_drag_is_logical_at_each_scale_and_preserves_outer_offset() {
    for scale in [1.0, 1.5, 2.0] {
        let mut runner = runner(Model {
            nested: true,
            ..Model::default()
        });
        // Keep the logical viewport identical across scale factors.
        runner.engine.window = Some(Rc::new(MemoryWindow {
            size: PhysicalSize::new((240.0 * scale) as u32, (120.0 * scale) as u32),
            scale,
            popup: false,
            popup_state: None,
        }));
        runner.engine.update_scale(scale);
        publish(&mut runner);
        scroll(&mut runner, 10, 150.0);
        click(&mut runner, 170.0 * scale, 15.0 * scale);
        assert_eq!(runner.scroll_drag.as_ref().map(|drag| drag.id), Some(11));
        let state = runner.engine.widget_states[&11].as_scroll().unwrap();
        let travel = state.viewport_h - (state.viewport_h * state.thumb_ratio()).max(20.0);
        let expected = 10.0 / travel * (state.content_height - state.viewport_h);
        move_pointer(&mut runner, 170.0 * scale, 25.0 * scale);
        assert!(
            (offset(&runner, 11) - expected).abs() < 0.001,
            "scale {scale}"
        );
        assert_eq!(offset(&runner, 10), 150.0);
    }
}

#[test]
fn task63_keyboard_ignores_hovered_inner_when_focus_is_in_outer() {
    let mut runner = runner(Model {
        nested: true,
        outer_button: true,
        ..Model::default()
    });
    click(&mut runner, 40.0, 10.0);
    publish(&mut runner);
    runner.engine.active_scroll_id = Some(11);
    key(&mut runner, NamedKey::ArrowDown, false);
    assert_eq!(offset(&runner, 10), 40.0);
    assert_eq!(offset(&runner, 11), 0.0);
}

#[test]
fn task63_nested_button_keys_choose_inner_and_inputs_keep_navigation() {
    let mut runner = runner(Model {
        nested: true,
        ..Model::default()
    });
    key(&mut runner, NamedKey::Tab, false);
    let outer = offset(&runner, 10);
    runner.engine.active_scroll_id = Some(10);
    key(&mut runner, NamedKey::ArrowDown, false);
    assert_eq!(offset(&runner, 11), 40.0);
    assert_eq!(offset(&runner, 10), outer);
    key(&mut runner, NamedKey::Tab, false);
    assert_eq!(runner.engine.focused_widget_id, Some(2));
    let before = (offset(&runner, 10), offset(&runner, 11));
    for navigation in [
        NamedKey::ArrowDown,
        NamedKey::PageDown,
        NamedKey::Home,
        NamedKey::End,
    ] {
        key(&mut runner, navigation, false);
        assert_eq!((offset(&runner, 10), offset(&runner, 11)), before);
    }
}

#[test]
fn task63_portaled_button_does_not_scroll_the_underlying_document() {
    for overlay_scroll in [false, true] {
        let mut runner = runner(Model {
            popover: true,
            open: true,
            overlay_scroll,
            ..Model::default()
        });
        scroll(&mut runner, 10, 180.0);
        key(&mut runner, NamedKey::Tab, false);
        key(&mut runner, NamedKey::Tab, false);
        assert_eq!(
            runner.engine.focused_widget_id,
            Some(button_focus(&runner, 4))
        );
        runner.engine.active_scroll_id = Some(10);
        let before = offset(&runner, 10);
        key(&mut runner, NamedKey::PageDown, false);
        assert_eq!(offset(&runner, 10), before);
        if overlay_scroll {
            assert!(offset(&runner, 12) > 0.0);
        }
    }
}

#[test]
fn task63_clipped_inner_track_is_ignored_and_outer_track_is_not_translated() {
    let mut runner = runner(Model {
        nested: true,
        ..Model::default()
    });
    // The inner track starts below the outer viewport, even though it exists in layout.
    click(&mut runner, 170.0, 85.0);
    assert!(runner.scroll_drag.is_none());
    scroll(&mut runner, 10, 150.0);
    click(&mut runner, 190.0, 85.0);
    assert_eq!(runner.scroll_drag.as_ref().map(|drag| drag.id), Some(10));
    assert_eq!(offset(&runner, 11), 0.0);
}

#[test]
fn task63_virtual_collection_track_jump_and_drag_apply_both_ancestor_offsets() {
    for collection in [1, 2] {
        let mut runner = runner(Model {
            nested: true,
            collection,
            ..Model::default()
        });
        scroll(&mut runner, 10, 150.0);
        scroll(&mut runner, 11, 60.0);
        click(&mut runner, 150.0, 90.0);
        assert_eq!(runner.scroll_drag.as_ref().map(|drag| drag.id), Some(13));
        let offset = |runner: &RutterRunner<ScrollApp>| {
            let state = &runner.engine.widget_states[&13];
            if collection == 1 {
                state.as_vlist().unwrap().scroll_y
            } else {
                state.as_vgrid().unwrap().scroll_y
            }
        };
        let jumped = offset(&runner);
        assert!(jumped > 0.0);
        move_pointer(&mut runner, 150.0, 80.0);
        assert!(offset(&runner) < jumped);
        assert_eq!(super::offset(&runner, 10), 150.0);
        assert_eq!(super::offset(&runner, 11), 60.0);
    }
}

#[test]
fn task63_virtual_navigation_retains_selection_priority_over_scroll_ancestors() {
    for collection in [1, 2] {
        let mut runner = runner(Model {
            nested: true,
            collection,
            ..Model::default()
        });
        key(&mut runner, NamedKey::Tab, false);
        assert_eq!(runner.engine.focused_widget_id, Some(13));
        let before = (offset(&runner, 10), offset(&runner, 11));
        key(&mut runner, NamedKey::ArrowDown, false);
        let state = &runner.engine.widget_states[&13];
        if collection == 1 {
            assert!(state.as_vlist().unwrap().selected_row.is_some());
        } else {
            assert!(state.as_vgrid().unwrap().selected_item.is_some());
        }
        assert_eq!((offset(&runner, 10), offset(&runner, 11)), before);
    }
}

#[test]
fn task63_popup_button_over_underlying_track_does_not_begin_document_drag() {
    let mut runner = runner(Model {
        popover: true,
        open: true,
        ..Model::default()
    });
    scroll(&mut runner, 10, 180.0);
    click(&mut runner, 190.0, 60.0);
    assert!(runner.scroll_drag.is_none());
    assert_eq!(runner.engine.app_state.messages, vec![4]);
    assert_eq!(offset(&runner, 10), 180.0);
}

#[test]
fn task63_unfocused_viewport_keeps_pointer_navigation_and_line_wheel_units() {
    for scale in [1.0, 1.5, 2.0] {
        let mut runner = runner(Model::default());
        runner.engine.update_scale(scale);
        publish(&mut runner);
        runner.update_cursor_position(PhysicalPosition::new(40.0 * scale, 8.0 * scale));
        runner.window_event(
            &MemoryEventLoop::default(),
            WindowId::from_raw(1),
            WindowEvent::MouseWheel {
                device_id: None,
                delta: MouseScrollDelta::LineDelta(0.0, -1.0),
                phase: winit::event::TouchPhase::Moved,
            },
        );
        assert_eq!(offset(&runner, 10), 40.0);
        assert!(runner.engine.focused_widget_id.is_none());
        key(&mut runner, NamedKey::ArrowUp, false);
        assert_eq!(offset(&runner, 10), 0.0);
        key(&mut runner, NamedKey::End, false);
        assert!(offset(&runner, 10) > 0.0);
        key(&mut runner, NamedKey::Home, false);
        assert_eq!(offset(&runner, 10), 0.0);
    }
}
