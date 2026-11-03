//! Review regressions use the window-event boundary, not gesture dispatch helpers.
use super::*;
use crate::engine::test_surface::MemoryEventLoop;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ButtonSource, ElementState, MouseButton, PointerSource, WindowEvent};
use winit::window::WindowId;

fn event<A: AppLogic + 'static>(r: &mut RutterRunner<A>, event: WindowEvent) {
    r.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
}

fn button<A: AppLogic + 'static>(r: &mut RutterRunner<A>, x: f64, y: f64, state: ElementState) {
    event(
        r,
        WindowEvent::PointerButton {
            device_id: None,
            state,
            primary: true,
            position: PhysicalPosition::new(x, y),
            button: ButtonSource::Mouse(MouseButton::Left),
            is_macos_activation_click: false,
        },
    );
}

fn motion<A: AppLogic + 'static>(r: &mut RutterRunner<A>, x: f64, y: f64) {
    event(
        r,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(x, y),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
}

fn event_runner() -> RutterRunner<PointerApp> {
    let mut r = runner();
    r.active_window_id = Some(WindowId::from_raw(1));
    r.engine.scale_factor = 2.0;
    r.engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(640, 320),
        scale: 2.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    dirty(&mut r);
    r
}

fn arm_event_selection(r: &mut RutterRunner<PointerApp>) {
    arm_selection(r);
    r.pending_selected_text_drag = None;
    button(r, 40.0, 192.0, ElementState::Pressed);
    assert!(r.pending_selected_text_drag.is_some());
}

#[test]
fn pointer_region_review_events_refresh_coordinates_and_fatal_publication() {
    let mut r = event_runner();
    button(&mut r, 8.0, 8.0, ElementState::Pressed);
    dirty(&mut r);
    motion(&mut r, 8.0, 88.0);
    dirty(&mut r);
    button(&mut r, 8.0, 88.0, ElementState::Released);
    assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
    let state = &r.engine.app_state;
    assert!(
        state
            .messages
            .contains(&Message(4, Event::Source(DragPhase::Dropped)))
    );
    let pointer = state.pointer_events.last().unwrap();
    assert_eq!(pointer.phase, PointerPhase::Released);
    assert_eq!(pointer.position, LogicalPointerPosition::new(4.0, 44.0));
    let drop = state.drag_events.last().unwrap();
    assert_eq!(
        (drop.source_id, drop.target_id, drop.payload),
        (17, Some(18), payload(7))
    );
    assert_eq!(drop.position, pointer.position);

    let mut r = event_runner();
    r.engine.app_state.trigger = Some((Event::Pointer(PointerPhase::Pressed), Effect::DuplicateId));
    button(&mut r, 8.0, 8.0, ElementState::Pressed);
    assert!(r.fatal_error.is_some());
    assert!(r.pointer_region_capture.is_none());
    assert!(!events(&r).contains(&Event::Source(DragPhase::Started)));
}

#[test]
fn pointer_region_review_rehits_after_phase_geometry_changes() {
    for phase in [
        Event::Source(DragPhase::Moved),
        Event::Target(DragPhase::Entered),
        Event::Target(DragPhase::Moved),
        Event::Pointer(PointerPhase::Released),
    ] {
        for effect in [
            Effect::MoveTarget,
            Effect::ClipTarget,
            Effect::OccludeTarget,
        ] {
            let mut r = event_runner();
            button(&mut r, 8.0, 8.0, ElementState::Pressed);
            r.engine.app_state.trigger = Some((phase.clone(), effect));
            motion(&mut r, 8.0, 88.0);
            if phase != Event::Pointer(PointerPhase::Released) {
                let events = events(&r);
                let exit = events
                    .iter()
                    .position(|e| *e == Event::Target(DragPhase::Exited))
                    .unwrap();
                assert!(!events[exit + 1..].contains(&Event::Target(DragPhase::Moved)));
            }
            button(&mut r, 8.0, 88.0, ElementState::Released);
            assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
            assert!(!events(&r).contains(&Event::Target(DragPhase::Dropped)));
            let drop = r.engine.app_state.drag_events.last().unwrap();
            assert_eq!(drop.phase, DragPhase::Dropped);
            assert_eq!(drop.target_id, None);
        }
    }
}

#[test]
fn pointer_region_review_exit_rehits_and_second_release_enter_cannot_leave_stale_metadata() {
    let mut r = event_runner();
    button(&mut r, 8.0, 8.0, ElementState::Pressed);
    motion(&mut r, 8.0, 88.0);
    r.engine.app_state.restore_on_exit = true;
    r.engine.app_state.trigger = Some((Event::Source(DragPhase::Moved), Effect::MoveTarget));
    motion(&mut r, 8.0, 88.0);
    let messages = &r.engine.app_state.messages;
    let exit = messages
        .iter()
        .rposition(|m| m.1 == Event::Target(DragPhase::Exited))
        .unwrap();
    assert_eq!(
        messages[exit + 1],
        Message(4, Event::Target(DragPhase::Entered))
    );
    assert_eq!(
        messages[exit + 2],
        Message(4, Event::Target(DragPhase::Moved))
    );

    let mut r = event_runner();
    r.engine.app_state.target_offset = 100.0;
    r.engine.app_state.move_on_enter = true;
    button(&mut r, 8.0, 8.0, ElementState::Pressed);
    r.engine.app_state.trigger = Some((
        Event::Pointer(PointerPhase::Released),
        Effect::RestoreTarget,
    ));
    button(&mut r, 8.0, 88.0, ElementState::Released);
    assert!(events(&r).contains(&Event::Target(DragPhase::Entered)));
    assert!(events(&r).contains(&Event::Target(DragPhase::Exited)));
    assert_eq!(
        r.engine.app_state.drag_events.last().unwrap().target_id,
        None
    );
    assert!(!events(&r).contains(&Event::Target(DragPhase::Dropped)));
    assert!(r.fatal_error.is_none());
}

#[test]
fn pointer_region_review_target_oscillation_is_bounded_and_terminal_update_can_veto_drop() {
    let mut r = event_runner();
    r.engine.app_state.oscillate_target = true;
    button(&mut r, 8.0, 8.0, ElementState::Pressed);
    motion(&mut r, 8.0, 88.0);
    let notifications = events(&r)
        .iter()
        .filter(|event| matches!(event, Event::Target(_)))
        .count();
    assert!(notifications > 1 && notifications <= 3 * (MAX_TARGET_TRANSITIONS + 1));
    assert!(r.pointer_region_capture.as_ref().unwrap().target.is_none());
    button(&mut r, 8.0, 88.0, ElementState::Released);
    assert!(r.pointer_region_capture.is_none());
    assert_eq!(
        r.engine.app_state.drag_events.last().unwrap().target_id,
        None
    );
    assert!(!events(&r).contains(&Event::Target(DragPhase::Dropped)));

    let mut r = event_runner();
    button(&mut r, 8.0, 8.0, ElementState::Pressed);
    motion(&mut r, 8.0, 88.0);
    r.engine.app_state.trigger = Some((Event::Source(DragPhase::Dropped), Effect::MoveTarget));
    button(&mut r, 8.0, 88.0, ElementState::Released);
    // Source terminal metadata describes its candidate before update, not an
    // atomic commit promise: its update is allowed to veto target delivery.
    assert_eq!(
        r.engine.app_state.drag_events.last().unwrap().target_id,
        Some(18)
    );
    assert!(!events(&r).contains(&Event::Target(DragPhase::Dropped)));
    assert!(r.fatal_error.is_none());
}

#[test]
fn pointer_region_review_selected_input_retirement_and_independent_legacy_fallback() {
    for legacy in [false, true] {
        for active in [false, true] {
            for disable in [false, true] {
                let mut r = event_runner();
                r.engine.app_state.captured_selected = !legacy;
                r.engine.app_state.legacy_selected = legacy;
                r.engine
                    .try_ensure_layout_for_input(PhysicalSize::new(640, 320))
                    .unwrap();
                arm_event_selection(&mut r);
                // Exercise pending/captured continuation through real motion.
                if active {
                    motion(&mut r, 56.0, 192.0);
                    assert!(r.pointer_region_capture.is_some());
                }
                if disable {
                    r.engine.app_state.input_enabled = false;
                } else {
                    r.engine.app_state.input_visible = false;
                }
                dirty(&mut r);
                motion(&mut r, 64.0, 192.0);
                assert!(r.pointer_region_capture.is_none());
                assert!(r.pending_selected_text_drag.is_none());
                assert!(r.fatal_error.is_none(), "{:?}", r.fatal_error);
                if active {
                    assert!(events(&r).contains(&Event::Source(DragPhase::Cancelled(
                        DragCancelReason::SourceRemoved
                    ))));
                } else {
                    assert!(!events(&r).iter().any(|e| matches!(e, Event::Selected(_))));
                }
            }
        }
        let mut r = event_runner();
        r.engine.app_state.captured_selected = !legacy;
        r.engine.app_state.legacy_selected = legacy;
        r.engine
            .try_ensure_layout_for_input(PhysicalSize::new(640, 320))
            .unwrap();
        arm_event_selection(&mut r);
        motion(&mut r, 56.0, 192.0);
        assert!(r.engine.app_state.messages.contains(&Message(
            if legacy { 99 } else { 2 },
            Event::Selected("River".into())
        )));
    }
}

#[derive(Default)]
struct HoverModel {
    enabled: bool,
    child_inside: bool,
    oscillate: bool,
    epoch: u64,
    messages: Vec<(u64, u64, bool)>,
}
struct HoverApp;
impl AppLogic for HoverApp {
    type State = HoverModel;
    type Message = (u64, u64, bool);
    fn new(_: &mut FontSystem) -> HoverModel {
        HoverModel::default()
    }
    fn view<'a>(model: &'a mut HoverModel) -> Widget<'a, Self::Message> {
        let config = |id| {
            let epoch = model.epoch;
            let config = PointerRegionConfigWith::new(|_| (0, 0, false));
            if model.enabled {
                config.with_hover(move |inside| (epoch, id, inside))
            } else {
                config
            }
        };
        let child = Widget::pointer_region_with(
            WidgetId::manual(11).unwrap(),
            Widget::Spacer { style: bounds() },
            config(11),
            Style {
                size: Size::from_lengths(100.0, 40.0),
                position: Position::Absolute,
                inset: Rect {
                    top: LengthPercentageAuto::length(if model.child_inside {
                        20.0
                    } else {
                        120.0
                    }),
                    ..Rect::auto()
                },
                ..Style::default()
            },
        );
        Widget::pointer_region_with(
            WidgetId::manual(10).unwrap(),
            child,
            config(10),
            Style {
                size: Size::from_lengths(240.0, 100.0),
                ..Style::default()
            },
        )
    }
    fn update(model: &mut HoverModel, message: Self::Message, _: &mut Clipboard) {
        model.messages.push(message);
        model.epoch += 1;
        match (message.1, message.2) {
            (10, inside) => model.child_inside = inside,
            (11, inside) if model.oscillate => model.child_inside = !inside,
            _ => {}
        }
    }
}

fn hover_runner() -> RutterRunner<HoverApp> {
    let mut r = RutterRunner::with_engine(crate::engine::RutterEngine::<HoverApp>::new().unwrap());
    r.engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(320, 160),
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    r.active_window_id = Some(WindowId::from_raw(1));
    r.engine
        .try_ensure_layout_for_input(PhysicalSize::new(320, 160))
        .unwrap();
    r.engine.app_state.enabled = true;
    r.engine.layout_dirty = true;
    r
}

#[test]
fn pointer_region_review_stationary_hover_discovers_child_and_refreshes_exit_epochs() {
    let mut r = hover_runner();
    motion(&mut r, 24.0, 24.0);
    assert_eq!(
        r.engine.app_state.messages,
        vec![(0, 10, true), (1, 11, true)]
    );
    assert!(r.pointer_region_hovered.contains(&11));
    motion(&mut r, 300.0, 150.0);
    assert_eq!(
        r.engine.app_state.messages[2..],
        [(2, 10, false), (3, 11, false)]
    );
    assert!(r.pointer_region_hovered.is_empty());
    assert!(r.fatal_error.is_none());
}

#[test]
fn pointer_region_review_hover_oscillation_defers_only_unreported_changes_to_next_motion() {
    let mut r = hover_runner();
    r.engine.app_state.oscillate = true;
    motion(&mut r, 24.0, 24.0);
    assert_eq!(
        r.engine.app_state.messages,
        vec![(0, 10, true), (1, 11, true), (2, 11, false)]
    );
    assert!(!r.pointer_region_hovered.contains(&11));
    assert!(r.engine.app_state.child_inside);
    r.engine.app_state.oscillate = false;
    r.engine.layout_dirty = true;
    motion(&mut r, 24.0, 24.0);
    assert_eq!(r.engine.app_state.messages.last(), Some(&(3, 11, true)));
    assert!(r.pointer_region_hovered.contains(&11));
    assert!(r.fatal_error.is_none());
}

#[path = "pointer_region_wrapper_tests.rs"]
mod wrapper_tests;
