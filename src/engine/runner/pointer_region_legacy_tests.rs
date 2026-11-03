//! Legacy literal and phase-order regressions share the captured runtime.
use arboard::Clipboard;
use cosmic_text::{Edit, FontSystem};
use skia_safe::{Color, Point};
use taffy::prelude::Style;

use super::*;
use crate::engine::{InputRuntime, PointerRegionRuntime, RutterEngine};
use crate::input_limits::{InputKind, InputLimits};
use crate::pointer::{DragBadge, DragPayload, DragPayloadKind, SelectedTextDrag};
use crate::widget::Widget;

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Pointer(PointerPhase),
    Source(DragPhase),
    Target(DragPhase, Option<u64>),
    Selected(String),
}

struct PointerApp;

impl AppLogic for PointerApp {
    type State = Vec<Message>;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Self::State {
        Vec::new()
    }

    fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
        let style = Style {
            size: taffy::prelude::Size::from_lengths(240.0, 40.0),
            flex_shrink: 0.0,
            ..Style::default()
        };
        Widget::Column {
            children: vec![
                Widget::pointer_region(
                    crate::WidgetId::manual(17).unwrap(),
                    Widget::Spacer {
                        style: style.clone(),
                    },
                    crate::PointerRegionConfig::new(pointer_message).with_drag_source(
                        crate::DragSource {
                            payload: DragPayload::new(DragPayloadKind::new(2), 7),
                            on_drag: source_message,
                        },
                    ),
                    style.clone(),
                ),
                Widget::pointer_region(
                    crate::WidgetId::manual(18).unwrap(),
                    Widget::Spacer {
                        style: style.clone(),
                    },
                    crate::PointerRegionConfig::new(pointer_message).with_drop_target(
                        crate::DropTarget {
                            accepted_kind: DragPayloadKind::new(2),
                            on_drag: target_message,
                        },
                    ),
                    style.clone(),
                ),
                Widget::text_input(
                    Message::Selected,
                    None,
                    style,
                    "",
                    "",
                    crate::InputState::Idle,
                    None,
                    false,
                )
                .with_id(19),
            ],
            style: Style::default(),
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        state.push(message);
    }

    fn selected_text_drag(_state: &Self::State, id: u64) -> Option<SelectedTextDrag<Message>> {
        (id == 19).then(|| SelectedTextDrag {
            source: crate::DragSource {
                payload: DragPayload::new(DragPayloadKind::new(2), 7),
                on_drag: source_message,
            },
            on_selected: Message::Selected,
            badge: DragBadge::new(Color::RED, Color::WHITE),
        })
    }
}

fn pointer_message(event: PointerEvent) -> Message {
    Message::Pointer(event.phase)
}
fn source_message(event: DragEvent) -> Message {
    Message::Source(event.phase)
}
fn target_message(event: DragEvent) -> Message {
    Message::Target(event.phase, event.target_id)
}

fn attach_window(runner: &mut RutterRunner<PointerApp>) {
    runner.engine.window = Some(std::rc::Rc::new(
        crate::engine::test_surface::MemoryWindow {
            size: winit::dpi::PhysicalSize::new(320, 200),
            scale: 1.0,
            popup: false,
            popup_state: None,
            input_state: None,
        },
    ));
    runner.engine.layout_dirty = true;
    runner.sync_pointer_region_layout().unwrap();
    runner.engine.last_mouse_pos = Point::new(4.0, 44.0);
}

fn register_capture_regions(runner: &mut RutterRunner<PointerApp>, target: &DropTarget<Message>) {
    let capture = runner.pointer_region_capture.as_ref().unwrap();
    runner.engine.runtime_caches.pointer_regions.insert(
        capture.id,
        PointerRegionRuntime {
            on_pointer: crate::ChangeCallback::from_function(pointer_message),
            on_hover: None,
            capture_on_press: true,
            drag_source: capture.drag_source.clone(),
            drop_target: None,
            drag_badge: None,
        },
    );
    for id in [18, 19] {
        runner.engine.runtime_caches.pointer_regions.insert(
            id,
            PointerRegionRuntime {
                on_pointer: crate::ChangeCallback::from_function(pointer_message),
                on_hover: None,
                capture_on_press: false,
                drag_source: None,
                drop_target: Some(target.clone()),
                drag_badge: None,
            },
        );
    }
}

fn runner_with_selected_word() -> RutterRunner<PointerApp> {
    let engine = RutterEngine::<PointerApp>::new().unwrap();
    let mut runner = RutterRunner::with_engine(engine);
    runner.engine.runtime_caches.inputs.insert(
        19,
        InputRuntime {
            model_value: None,
            on_change: crate::ChangeCallback::from_function(Message::Selected),
            on_submit: None,
            is_password: false,
            is_multiline: false,
            leading_text_inset: 0.0,
            visible_w: 240.0,
            visible_h: 30.0,
            limits: InputLimits::for_kind(InputKind::TextInput),
        },
    );
    runner.engine.focused_widget_id = Some(19);
    runner.engine.ensure_input_state(19);
    let mut fs = runner.engine.font_system.borrow_mut();
    let input = runner.engine.input_states.get_mut(&19).unwrap();
    input.set_text(&mut fs, "River and Forest");
    input.sync_layout(&mut fs, 240.0, 16.0, false);
    input
        .editor
        .action(&mut fs, cosmic_text::Action::DoubleClick { x: 4, y: 8 });
    input.sync_selection();
    drop(fs);
    runner
}

fn arm_word_drag(runner: &mut RutterRunner<PointerApp>, x: f32, y: f32) -> bool {
    runner.arm_selected_text_drag(19, Point::new(x, y), Point::new(x, y), 240.0, 30.0)
}

fn begin_word_drag(runner: &mut RutterRunner<PointerApp>, x: f32, y: f32) -> bool {
    if !arm_word_drag(runner, x, y) {
        return false;
    }
    let pending = runner.pending_selected_text_drag.take().unwrap();
    runner.start_selected_text_drag(pending);
    true
}

#[test]
fn clicking_a_selection_collapses_it_at_the_press_without_starting_drag() {
    let mut runner = runner_with_selected_word();
    let theme = PointerApp::theme();
    let x = theme.spacing * 2.0 + 20.0;
    let y = theme.spacing + 8.0;
    let expected_cursor = runner.engine.input_states[&19]
        .editor
        .with_buffer(|buffer| buffer.hit(20.0, 8.0).unwrap());
    assert!(arm_word_drag(&mut runner, x, y));
    runner.cursor_pos = Point::new(x + 2.0, y);
    assert!(runner.advance_pending_selected_text_drag().unwrap());
    assert!(runner.engine.app_state.is_empty());
    assert!(runner.pointer_region_capture.is_none());
    assert!(runner.release_pending_selected_text_drag());
    let input = &runner.engine.input_states[&19];
    assert_eq!(input.editor.cursor().index, expected_cursor.index);
    assert_eq!(input.selection, None);
    assert!(runner.engine.app_state.is_empty());
    assert!(runner.engine.active_drag_badge.is_none());
}

#[test]
fn moving_outside_click_tolerance_starts_selected_text_drag_once() {
    let mut runner = runner_with_selected_word();
    let theme = PointerApp::theme();
    let x = theme.spacing * 2.0 + 4.0;
    let y = theme.spacing + 8.0;
    assert!(arm_word_drag(&mut runner, x, y));
    assert!(runner.engine.app_state.is_empty());
    runner.cursor_pos = Point::new(x + 6.0, y);
    assert!(runner.advance_pending_selected_text_drag().unwrap());
    assert!(runner.pending_selected_text_drag.is_none());
    assert_eq!(
        runner.engine.app_state[0],
        Message::Selected("River".into())
    );
    assert_eq!(
        runner.engine.app_state[1],
        Message::Source(DragPhase::Started)
    );
    assert!(runner.pointer_region_capture.is_some());
    assert!(!runner.release_pending_selected_text_drag());
    assert!(runner.engine.input_states[&19].selection.is_some());
}

#[test]
fn selected_text_drag_snapshots_the_word_without_swallowing_other_input_clicks() {
    let mut runner = runner_with_selected_word();
    let theme = PointerApp::theme();
    let x = theme.spacing * 2.0 + 4.0;
    let y = theme.spacing + 8.0;
    assert!(!arm_word_drag(&mut runner, x + 170.0, y));
    assert!(!arm_word_drag(&mut runner, x - 5.0, y));
    assert!(!arm_word_drag(&mut runner, x, y - 9.0));
    assert!(runner.engine.app_state.is_empty());
    assert!(begin_word_drag(&mut runner, x, y));
    assert_eq!(
        runner.engine.app_state,
        vec![
            Message::Selected("River".into()),
            Message::Source(DragPhase::Started)
        ]
    );
    assert_eq!(runner.pointer_region_capture.as_ref().unwrap().id, 19);
    assert!(runner.engine.active_drag_badge.is_some());
    assert_eq!(
        runner.engine.input_states.get(&19).unwrap().text(),
        "River and Forest"
    );
    assert!(
        !runner
            .engine
            .input_states
            .get(&19)
            .unwrap()
            .selection
            .unwrap()
            .is_empty()
    );
    runner.cancel_pointer_region_capture(DragCancelReason::FocusLost);
    assert_eq!(
        runner.engine.app_state.last(),
        Some(&Message::Source(DragPhase::Cancelled(
            DragCancelReason::FocusLost
        )))
    );
    assert!(runner.engine.active_drag_badge.is_none());
}

#[test]
fn selected_text_drag_never_starts_for_passwords_or_removed_sources() {
    let mut runner = runner_with_selected_word();
    let theme = PointerApp::theme();
    let x = theme.spacing * 2.0 + 4.0;
    let y = theme.spacing + 8.0;
    runner
        .engine
        .runtime_caches
        .inputs
        .get_mut(&19)
        .unwrap()
        .is_password = true;
    assert!(!arm_word_drag(&mut runner, x, y));
    runner
        .engine
        .runtime_caches
        .inputs
        .get_mut(&19)
        .unwrap()
        .is_password = false;
    runner
        .engine
        .input_states
        .get_mut(&19)
        .unwrap()
        .set_sensitive(true);
    assert!(!arm_word_drag(&mut runner, x, y));
    let mut runner = runner_with_selected_word();
    assert!(begin_word_drag(&mut runner, x, y));
    runner.engine.runtime_caches.inputs.remove(&19);
    assert!(runner.dispatch_captured_pointer_region_move().unwrap());
    assert!(runner.pointer_region_capture.is_none());
    assert_eq!(
        runner.engine.app_state.last(),
        Some(&Message::Source(DragPhase::Cancelled(
            DragCancelReason::SourceRemoved
        )))
    );
}

#[test]
fn selected_text_uses_the_existing_target_drop_lifecycle() {
    let mut runner = runner_with_selected_word();
    let theme = PointerApp::theme();
    assert!(begin_word_drag(
        &mut runner,
        theme.spacing * 2.0 + 4.0,
        theme.spacing + 8.0
    ));
    attach_window(&mut runner);
    runner.finish_pointer_region_capture();
    assert_eq!(
        runner.engine.app_state,
        vec![
            Message::Selected("River".into()),
            Message::Source(DragPhase::Started),
            Message::Target(DragPhase::Entered, Some(18)),
            Message::Source(DragPhase::Dropped),
            Message::Target(DragPhase::Dropped, Some(18)),
        ]
    );
}

#[test]
fn capture_delivers_pointer_and_drag_phases_in_order() {
    let engine = RutterEngine::<PointerApp>::new().unwrap();
    let mut runner = RutterRunner::with_engine(engine);
    runner.engine.last_mouse_pos = Point::new(12.0, 18.0);
    let source = DragSource {
        payload: DragPayload::new(DragPayloadKind::new(4), 88),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    runner.engine.runtime_caches.pointer_regions.insert(
        17,
        PointerRegionRuntime {
            on_hover: None,
            on_pointer: crate::ChangeCallback::from_function(pointer_message),
            capture_on_press: true,
            drag_source: Some(source),
            drop_target: None,
            drag_badge: None,
        },
    );
    runner.dispatch_pointer_region_press(17);
    assert!(runner.dispatch_captured_pointer_region_move().unwrap());
    runner.cancel_pointer_region_capture(DragCancelReason::FocusLost);
    assert_eq!(
        runner.engine.app_state,
        vec![
            Message::Pointer(PointerPhase::Pressed),
            Message::Source(DragPhase::Started),
            Message::Pointer(PointerPhase::Moved),
            Message::Source(DragPhase::Moved),
            Message::Pointer(PointerPhase::Cancelled),
            Message::Source(DragPhase::Cancelled(DragCancelReason::FocusLost)),
        ]
    );
}

#[test]
fn drop_target_requires_the_declared_payload_kind() {
    let source = DragSource {
        payload: DragPayload::new(DragPayloadKind::new(2), 7),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    let matching = DropTarget {
        accepted_kind: DragPayloadKind::new(2),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    let mismatched = DropTarget {
        accepted_kind: DragPayloadKind::new(3),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    assert!(drag_source_matches_target(&source, &matching));
    assert!(!drag_source_matches_target(&source, &mismatched));
}

#[test]
fn changing_drop_targets_exits_before_entering_and_cancels_current_target() {
    let engine = RutterEngine::<PointerApp>::new().unwrap();
    let mut runner = RutterRunner::with_engine(engine);
    let source = DragSource {
        payload: DragPayload::new(DragPayloadKind::new(2), 7),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    let target = DropTarget {
        accepted_kind: source.payload.kind(),
        on_drag: crate::ChangeCallback::from_function(target_message),
    };
    runner.pointer_region_capture = Some(PointerRegionCapture {
        id: 17,
        origin: CaptureOrigin::PointerRegion(crate::ChangeCallback::from_function(pointer_message)),
        drag_source: Some(source),
        target: None,
    });
    register_capture_regions(&mut runner, &target);
    runner.transition_drop_target(Some((18, target.clone())));
    runner.transition_drop_target(Some((19, target.clone())));
    runner.transition_drop_target(None);
    runner.transition_drop_target(Some((19, target)));
    runner.cancel_pointer_region_capture(DragCancelReason::FocusLost);
    assert_eq!(
        runner.engine.app_state,
        vec![
            Message::Target(DragPhase::Entered, Some(18)),
            Message::Target(DragPhase::Exited, Some(18)),
            Message::Target(DragPhase::Entered, Some(19)),
            Message::Target(DragPhase::Exited, Some(19)),
            Message::Target(DragPhase::Entered, Some(19)),
            Message::Pointer(PointerPhase::Cancelled),
            Message::Target(DragPhase::Cancelled(DragCancelReason::FocusLost), Some(19)),
            Message::Source(DragPhase::Cancelled(DragCancelReason::FocusLost)),
        ]
    );
    assert!(runner.pointer_region_capture.is_none());
}

#[test]
fn release_delivers_a_matching_drop_to_source_and_target_once() {
    let engine = RutterEngine::<PointerApp>::new().unwrap();
    let mut runner = RutterRunner::with_engine(engine);
    let source = DragSource {
        payload: DragPayload::new(DragPayloadKind::new(2), 7),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    runner.pointer_region_capture = Some(PointerRegionCapture {
        id: 17,
        origin: CaptureOrigin::PointerRegion(crate::ChangeCallback::from_function(pointer_message)),
        drag_source: Some(source),
        target: None,
    });
    attach_window(&mut runner);
    runner.finish_pointer_region_capture();
    assert_eq!(
        runner.engine.app_state,
        vec![
            Message::Target(DragPhase::Entered, Some(18)),
            Message::Pointer(PointerPhase::Released),
            Message::Source(DragPhase::Dropped),
            Message::Target(DragPhase::Dropped, Some(18)),
        ]
    );
    assert!(runner.pointer_region_capture.is_none());
}

#[test]
fn a_removed_drag_source_is_cancelled_without_a_drop() {
    let engine = RutterEngine::<PointerApp>::new().unwrap();
    let mut runner = RutterRunner::with_engine(engine);
    runner.pointer_region_capture = Some(PointerRegionCapture {
        id: 17,
        origin: CaptureOrigin::PointerRegion(crate::ChangeCallback::from_function(pointer_message)),
        drag_source: Some(DragSource {
            payload: DragPayload::new(DragPayloadKind::new(2), 7),
            on_drag: crate::ChangeCallback::from_function(source_message),
        }),
        target: None,
    });
    assert!(runner.dispatch_captured_pointer_region_move().unwrap());
    assert_eq!(
        runner.engine.app_state,
        vec![
            Message::Pointer(PointerPhase::Cancelled),
            Message::Source(DragPhase::Cancelled(DragCancelReason::SourceRemoved))
        ]
    );
    assert!(runner.pointer_region_capture.is_none());
}

#[test]
fn badge_is_only_active_for_opted_in_drag_and_tracks_matching_targets() {
    let engine = RutterEngine::<PointerApp>::new().unwrap();
    let mut runner = RutterRunner::with_engine(engine);
    let source = DragSource {
        payload: DragPayload::new(DragPayloadKind::new(2), 7),
        on_drag: crate::ChangeCallback::from_function(source_message),
    };
    let appearance = DragBadge::new(Color::RED, Color::WHITE);
    runner.engine.runtime_caches.pointer_regions.insert(
        17,
        PointerRegionRuntime {
            on_hover: None,
            on_pointer: crate::ChangeCallback::from_function(pointer_message),
            capture_on_press: false,
            drag_source: Some(source.clone()),
            drop_target: None,
            drag_badge: Some(appearance),
        },
    );
    runner.dispatch_pointer_region_press(17);
    let active = runner.engine.active_drag_badge.as_ref().unwrap();
    assert_eq!(active.appearance.background, Color::RED);
    assert!(!active.can_drop);
    let target = DropTarget {
        accepted_kind: source.payload.kind(),
        on_drag: crate::ChangeCallback::from_function(target_message),
    };
    register_capture_regions(&mut runner, &target);
    runner.transition_drop_target(Some((18, target.clone())));
    assert!(runner.engine.active_drag_badge.as_ref().unwrap().can_drop);
    runner.transition_drop_target(None);
    assert!(!runner.engine.active_drag_badge.as_ref().unwrap().can_drop);
    runner.finish_pointer_region_capture();
    assert!(runner.engine.active_drag_badge.is_none());
    runner.dispatch_pointer_region_press(17);
    runner.cancel_pointer_region_capture(DragCancelReason::FocusLost);
    assert!(runner.engine.active_drag_badge.is_none());
    runner
        .engine
        .runtime_caches
        .pointer_regions
        .get_mut(&17)
        .unwrap()
        .drag_badge = None;
    runner.dispatch_pointer_region_press(17);
    assert!(runner.engine.active_drag_badge.is_none());
    runner.cancel_pointer_region_capture(DragCancelReason::FocusLost);
    let region = runner
        .engine
        .runtime_caches
        .pointer_regions
        .get_mut(&17)
        .unwrap();
    region.drag_source = None;
    region.capture_on_press = true;
    region.drag_badge = Some(DragBadge::new(Color::RED, Color::WHITE));
    runner.dispatch_pointer_region_press(17);
    assert!(runner.engine.active_drag_badge.is_none());
}
