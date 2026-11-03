//! Captured gesture callbacks across real input-layout publication boundaries.
use super::*;
use crate::engine::test_surface::MemoryWindow;
use crate::*;
use arboard::Clipboard;
use cosmic_text::{Edit, FontSystem};
use std::{rc::Rc, sync::Arc};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

#[derive(Clone, Debug, PartialEq)]
enum Event {
    Pointer(PointerPhase),
    PointerDetails(PointerEvent),
    Source(DragPhase),
    SourceDetails(DragEvent),
    Target(DragPhase),
    Hover(u64, bool),
    Selected(String),
}

#[derive(Clone, Debug, PartialEq)]
struct Message(u64, Event);

#[derive(Clone, Copy)]
enum Effect {
    RemoveSource,
    DisableSource,
    RemoveDeclaration,
    ReplacePayload,
    RemoveTarget,
    DisableTarget,
    ReplaceKind,
    Password,
    RemoveSelectedDeclaration,
    DuplicateId,
    Refresh,
    MoveTarget,
    RestoreTarget,
    ClipTarget,
    OccludeTarget,
}

struct Model {
    epoch: u64,
    visible: bool,
    enabled: bool,
    declared: bool,
    target: bool,
    target_enabled: bool,
    kind: u64,
    payload: u64,
    password: bool,
    selected: bool,
    captured_selected: bool,
    legacy_selected: bool,
    input_visible: bool,
    input_enabled: bool,
    target_offset: f32,
    clip_target: bool,
    occlude_target: bool,
    oscillate_target: bool,
    move_on_enter: bool,
    restore_on_exit: bool,
    hover: bool,
    duplicate: bool,
    trigger: Option<(Event, Effect)>,
    messages: Vec<Message>,
    drag_events: Vec<DragEvent>,
    pointer_events: Vec<PointerEvent>,
    owner: Arc<()>,
}

// Deliberately non-Clone, independently moved into each owned callback.
struct Capture(u64, Arc<()>);
impl Capture {
    fn message(&self, event: Event) -> Message {
        let _owner = &self.1;
        Message(self.0, event)
    }
}

struct PointerApp;
fn bounds() -> Style {
    Style {
        size: Size::from_lengths(240.0, 40.0),
        flex_shrink: 0.0,
        ..Style::default()
    }
}
fn payload(value: u64) -> DragPayload {
    DragPayload::new(DragPayloadKind::new(2), value)
}

impl AppLogic for PointerApp {
    type State = Model;
    type Message = Message;
    fn new(_: &mut FontSystem) -> Model {
        Model {
            epoch: 1,
            visible: true,
            enabled: true,
            declared: true,
            target: true,
            target_enabled: true,
            kind: 2,
            payload: 7,
            password: false,
            selected: true,
            captured_selected: true,
            legacy_selected: true,
            input_visible: true,
            input_enabled: true,
            target_offset: 0.0,
            clip_target: false,
            occlude_target: false,
            oscillate_target: false,
            move_on_enter: false,
            restore_on_exit: false,
            hover: false,
            duplicate: false,
            trigger: None,
            messages: Vec::new(),
            drag_events: Vec::new(),
            pointer_events: Vec::new(),
            owner: Arc::new(()),
        }
    }
    fn view<'a>(model: &'a mut Model) -> Widget<'a, Message> {
        let capture = || Capture(model.epoch, model.owner.clone());
        let pointer = capture();
        let source = capture();
        let hover = capture();
        let mut config = PointerRegionConfigWith::new(move |event| {
            pointer.message(Event::PointerDetails(event))
        })
        .with_pointer_capture();
        if model.declared {
            config = config
                .with_drag_source(DragSourceWith::new(payload(model.payload), move |event| {
                    source.message(Event::SourceDetails(event))
                }));
        }
        if model.hover {
            config = config.with_hover(move |inside| hover.message(Event::Hover(17, inside)));
        }
        let source = if model.visible {
            Widget::pointer_region_with(
                WidgetId::manual(17).unwrap(),
                Widget::Spacer { style: bounds() },
                config,
                bounds(),
            )
            .enabled(model.enabled)
        } else {
            Widget::Spacer { style: bounds() }
        };
        let target = capture();
        let hover = capture();
        let mut config =
            PointerRegionConfigWith::new(|_| Message(0, Event::Pointer(PointerPhase::Moved)))
                .with_drop_target(DropTargetWith::new(
                    DragPayloadKind::new(model.kind),
                    move |event| target.message(Event::Target(event.phase)),
                ));
        if model.hover {
            config = config.with_hover(move |inside| hover.message(Event::Hover(18, inside)));
        }
        let target = if model.target {
            let mut style = bounds();
            style.margin.left = LengthPercentageAuto::length(model.target_offset);
            Widget::pointer_region_with(
                WidgetId::manual(if model.duplicate { 17 } else { 18 }).unwrap(),
                Widget::Spacer { style: bounds() },
                config,
                style,
            )
            .enabled(model.target_enabled)
        } else {
            Widget::Spacer { style: bounds() }
        };
        let target = if model.clip_target {
            Widget::ScrollView {
                id: 30,
                child: Box::new(target),
                style: Style {
                    size: Size::from_lengths(2.0, 40.0),
                    flex_shrink: 0.0,
                    ..Style::default()
                },
            }
        } else {
            target
        };
        let input = if model.input_visible {
            Widget::text_input(
                |text| Message(0, Event::Selected(text)),
                None,
                bounds(),
                "",
                "",
                InputState::Idle,
                None,
                model.password,
            )
            .with_id(19)
            .enabled(model.input_enabled)
        } else {
            Widget::Spacer { style: bounds() }
        };
        let mut children = vec![source, target, input];
        if model.occlude_target {
            let mut style = bounds();
            style.position = Position::Absolute;
            style.inset.top = LengthPercentageAuto::length(40.0);
            children.push(Widget::pointer_region_with(
                WidgetId::manual(20).unwrap(),
                Widget::Spacer { style: bounds() },
                PointerRegionConfigWith::new(|_| Message(0, Event::Hover(20, false))),
                style,
            ));
        }
        Widget::Column {
            children,
            style: Style::default(),
        }
    }
    fn update(model: &mut Model, mut message: Message, _: &mut Clipboard) {
        match message.1 {
            Event::SourceDetails(event) => {
                model.drag_events.push(event);
                message.1 = Event::Source(event.phase);
            }
            Event::PointerDetails(event) => {
                model.pointer_events.push(event);
                message.1 = Event::Pointer(event.phase);
            }
            _ => {}
        }
        if model.oscillate_target || model.move_on_enter {
            match message.1 {
                Event::Target(DragPhase::Entered) => model.target_offset = 100.0,
                Event::Target(DragPhase::Exited) if model.oscillate_target => {
                    model.target_offset = 0.0
                }
                _ => {}
            }
        }
        if model.restore_on_exit && message.1 == Event::Target(DragPhase::Exited) {
            model.target_offset = 0.0;
            model.restore_on_exit = false;
            model.epoch += 1;
        }
        if model
            .trigger
            .as_ref()
            .is_some_and(|(event, _)| *event == message.1)
        {
            let (_, effect) = model.trigger.take().unwrap();
            apply(model, effect);
            model.epoch += 1;
        }
        model.messages.push(message);
    }
    fn selected_text_drag_with(model: &Model, id: u64) -> Option<SelectedTextDragWith<Message>> {
        if id != 19 || !model.selected || !model.captured_selected {
            return None;
        }
        let source = Capture(model.epoch, model.owner.clone());
        let selected = Capture(model.epoch, model.owner.clone());
        Some(SelectedTextDragWith::new(
            DragSourceWith::new(payload(model.payload), move |event| {
                source.message(Event::SourceDetails(event))
            }),
            move |text| selected.message(Event::Selected(text)),
            DragBadge::new(skia_safe::Color::RED, skia_safe::Color::WHITE),
        ))
    }
    fn selected_text_drag(model: &Model, id: u64) -> Option<crate::SelectedTextDrag<Message>> {
        // A captured declaration must take precedence over this legacy hook.
        if !model.selected || !model.legacy_selected || id != 19 {
            return None;
        }
        Some(crate::SelectedTextDrag {
            source: crate::DragSource {
                payload: payload(7),
                on_drag: |event| Message(99, Event::SourceDetails(event)),
            },
            on_selected: |text| Message(99, Event::Selected(text)),
            badge: DragBadge::new(skia_safe::Color::RED, skia_safe::Color::WHITE),
        })
    }
}

fn apply(model: &mut Model, effect: Effect) {
    match effect {
        Effect::RemoveSource => model.visible = false,
        Effect::DisableSource => model.enabled = false,
        Effect::RemoveDeclaration => model.declared = false,
        Effect::ReplacePayload => model.payload += 1,
        Effect::RemoveTarget => model.target = false,
        Effect::DisableTarget => model.target_enabled = false,
        Effect::ReplaceKind => model.kind = 3,
        Effect::Password => model.password = true,
        Effect::RemoveSelectedDeclaration => model.selected = false,
        Effect::DuplicateId => model.duplicate = true,
        Effect::Refresh => {}
        Effect::MoveTarget => model.target_offset = 100.0,
        Effect::RestoreTarget => model.target_offset = 0.0,
        Effect::ClipTarget => model.clip_target = true,
        Effect::OccludeTarget => model.occlude_target = true,
    }
}

#[path = "pointer_region_review_tests.rs"]
mod review_tests;

fn runner() -> RutterRunner<PointerApp> {
    let mut runner =
        RutterRunner::with_engine(crate::engine::RutterEngine::<PointerApp>::new().unwrap());
    runner.engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(320, 160),
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    runner
        .engine
        .try_ensure_layout_for_input(PhysicalSize::new(320, 160))
        .unwrap();
    runner.engine.last_mouse_pos = Point::new(4.0, 4.0);
    runner
}
fn dirty(runner: &mut RutterRunner<PointerApp>) {
    runner.engine.app_state.epoch += 1;
    runner.engine.layout_dirty = true;
}
fn over_target(runner: &mut RutterRunner<PointerApp>) {
    runner.engine.last_mouse_pos = Point::new(4.0, 44.0);
    assert!(runner.dispatch_captured_pointer_region_move().unwrap());
}
fn events(runner: &RutterRunner<PointerApp>) -> Vec<Event> {
    runner
        .engine
        .app_state
        .messages
        .iter()
        .map(|message| message.1.clone())
        .collect()
}
fn arm_selection(runner: &mut RutterRunner<PointerApp>) {
    runner.engine.focused_widget_id = Some(19);
    runner.engine.ensure_input_state(19);
    let mut fonts = runner.engine.font_system.borrow_mut();
    let input = runner.engine.input_states.get_mut(&19).unwrap();
    input.set_text(&mut fonts, "River and Forest");
    input.sync_layout(&mut fonts, 240.0, 16.0, false);
    input
        .editor
        .action(&mut fonts, cosmic_text::Action::DoubleClick { x: 4, y: 8 });
    input.sync_selection();
    drop(fonts);
    let theme = PointerApp::theme();
    let local = Point::new(theme.spacing * 2.0 + 4.0, theme.spacing + 8.0);
    assert!(runner.arm_selected_text_drag(19, local, local, 240.0, 40.0));
    runner.cursor_pos = Point::new(local.x + 8.0, local.y);
}

#[test]
fn pointer_region_captured_dirty_callbacks_refresh_for_move_exit_and_drop() {
    let mut r = runner();
    r.dispatch_pointer_region_press(17);
    dirty(&mut r);
    over_target(&mut r);
    assert!(
        r.engine
            .app_state
            .messages
            .iter()
            .filter(|m| m.1 == Event::Source(DragPhase::Moved)
                || m.1 == Event::Target(DragPhase::Moved))
            .all(|m| m.0 == 2)
    );
    dirty(&mut r);
    r.engine.last_mouse_pos = Point::new(4.0, 4.0);
    r.dispatch_captured_pointer_region_move().unwrap();
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(3, Event::Target(DragPhase::Exited)))
    );
    over_target(&mut r);
    dirty(&mut r);
    r.release_pointer_region_capture().unwrap();
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(4, Event::Pointer(PointerPhase::Released)))
    );
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(4, Event::Target(DragPhase::Dropped)))
    );
    assert!(r.pointer_region_capture.is_none());
    assert!(r.fatal_error.is_none());
}

#[test]
fn pointer_region_captured_source_replacement_and_eligibility_cancel() {
    for effect in [
        Effect::RemoveSource,
        Effect::DisableSource,
        Effect::RemoveDeclaration,
        Effect::ReplacePayload,
    ] {
        for release in [false, true] {
            let mut r = runner();
            r.dispatch_pointer_region_press(17);
            apply(&mut r.engine.app_state, effect);
            dirty(&mut r);
            if release {
                r.release_pointer_region_capture().unwrap();
            } else {
                over_target(&mut r);
            }
            assert!(events(&r).contains(&Event::Source(DragPhase::Cancelled(
                DragCancelReason::SourceRemoved
            ))));
            assert!(!events(&r).contains(&Event::Source(DragPhase::Dropped)));
            assert!(r.pointer_region_capture.is_none());
        }
    }
}

#[test]
fn pointer_region_captured_earlier_phase_updates_retire_later_dispatch() {
    for phase in [
        Event::Pointer(PointerPhase::Pressed),
        Event::Pointer(PointerPhase::Released),
        Event::Target(DragPhase::Entered),
    ] {
        for effect in [
            Effect::RemoveSource,
            Effect::DisableSource,
            Effect::ReplacePayload,
        ] {
            let mut r = runner();
            r.engine.app_state.trigger = Some((phase.clone(), effect));
            r.dispatch_pointer_region_press(17);
            if r.pointer_region_capture.is_some() {
                over_target(&mut r);
                r.release_pointer_region_capture().unwrap();
            }
            assert!(!events(&r).contains(&Event::Source(DragPhase::Dropped)));
            if phase == Event::Pointer(PointerPhase::Pressed) {
                assert!(!events(&r).contains(&Event::Source(DragPhase::Started)));
            }
            assert!(r.pointer_region_capture.is_none());
            assert!(r.fatal_error.is_none());
        }
    }
}

#[test]
fn pointer_region_captured_target_eligibility_and_phase_removal_prevent_drop() {
    for phase in [
        None,
        Some(Event::Target(DragPhase::Entered)),
        Some(Event::Pointer(PointerPhase::Released)),
        Some(Event::Source(DragPhase::Dropped)),
    ] {
        for effect in [
            Effect::RemoveTarget,
            Effect::DisableTarget,
            Effect::ReplaceKind,
        ] {
            let mut r = runner();
            r.dispatch_pointer_region_press(17);
            if let Some(phase) = phase.clone() {
                r.engine.app_state.trigger = Some((phase, effect));
            }
            over_target(&mut r);
            if phase.is_none() {
                apply(&mut r.engine.app_state, effect);
                dirty(&mut r);
            }
            r.release_pointer_region_capture().unwrap();
            assert!(!events(&r).contains(&Event::Target(DragPhase::Dropped)));
            assert!(r.pointer_region_capture.is_none());
        }
    }
}

#[test]
fn pointer_region_captured_first_hover_publishes_and_does_not_dispatch_removed_peer() {
    let mut r = runner();
    r.engine.app_state.hover = true;
    dirty(&mut r);
    r.refresh_pointer_region_hover(false).unwrap();
    assert!(events(&r).contains(&Event::Hover(17, true)));
    r.engine.app_state.trigger = Some((Event::Hover(17, false), Effect::RemoveTarget));
    r.engine.last_mouse_pos = Point::new(4.0, 44.0);
    r.refresh_pointer_region_hover(false).unwrap();
    assert!(!events(&r).contains(&Event::Hover(18, true)));
}

#[test]
fn pointer_region_captured_selected_snapshot_refresh_and_secret_transitions() {
    let mut r = runner();
    arm_selection(&mut r);
    {
        let mut fonts = r.engine.font_system.borrow_mut();
        r.engine
            .input_states
            .get_mut(&19)
            .unwrap()
            .set_text(&mut fonts, "Changed after arm");
    }
    dirty(&mut r);
    r.advance_pending_selected_text_drag().unwrap();
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(2, Event::Selected("River".into())))
    );
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(2, Event::Source(DragPhase::Started)))
    );
    assert!(
        !r.engine
            .app_state
            .messages
            .iter()
            .any(|message| message.0 == 99)
    );
    r.engine.app_state.password = true;
    dirty(&mut r);
    // Password inputs are a distinct native ID family: same-ID replacement is
    // rejected by publication before any continuation can expose the selection.
    assert!(r.dispatch_captured_pointer_region_move().is_err());
    assert!(r.fatal_error.is_some());
    assert!(r.pointer_region_capture.is_none());

    for effect in [
        Effect::Password,
        Effect::ReplacePayload,
        Effect::RemoveSelectedDeclaration,
    ] {
        let mut r = runner();
        arm_selection(&mut r);
        apply(&mut r.engine.app_state, effect);
        dirty(&mut r);
        let result = r.advance_pending_selected_text_drag();
        if matches!(effect, Effect::Password) {
            assert!(result.is_err());
        } else {
            result.unwrap();
        }
        assert!(r.engine.app_state.messages.is_empty());
    }
    for effect in [
        Effect::Password,
        Effect::RemoveSelectedDeclaration,
        Effect::ReplacePayload,
    ] {
        let mut r = runner();
        arm_selection(&mut r);
        r.engine.app_state.trigger = Some((Event::Selected("River".into()), effect));
        r.advance_pending_selected_text_drag().unwrap();
        assert!(!events(&r).contains(&Event::Source(DragPhase::Started)));
        assert!(r.pointer_region_capture.is_none());
        assert_eq!(r.fatal_error.is_some(), matches!(effect, Effect::Password));
    }
}

#[test]
fn pointer_region_captured_phase_callbacks_refresh_before_the_next_phase() {
    for (trigger, expected) in [
        (
            Event::Pointer(PointerPhase::Pressed),
            Event::Source(DragPhase::Started),
        ),
        (
            Event::Target(DragPhase::Entered),
            Event::Source(DragPhase::Moved),
        ),
        (
            Event::Pointer(PointerPhase::Released),
            Event::Source(DragPhase::Dropped),
        ),
        (
            Event::Source(DragPhase::Dropped),
            Event::Target(DragPhase::Dropped),
        ),
    ] {
        let mut r = runner();
        r.engine.app_state.trigger = Some((trigger, Effect::Refresh));
        r.dispatch_pointer_region_press(17);
        over_target(&mut r);
        r.release_pointer_region_capture().unwrap();
        assert!(r.engine.app_state.messages.contains(&Message(2, expected)));
    }
    let mut r = runner();
    arm_selection(&mut r);
    r.engine.app_state.trigger = Some((Event::Selected("River".into()), Effect::Refresh));
    r.advance_pending_selected_text_drag().unwrap();
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(2, Event::Source(DragPhase::Started)))
    );
    dirty(&mut r);
    over_target(&mut r);
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(3, Event::Source(DragPhase::Moved)))
    );
    dirty(&mut r);
    r.release_pointer_region_capture().unwrap();
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(4, Event::Source(DragPhase::Dropped)))
    );
    assert!(
        r.engine
            .app_state
            .messages
            .contains(&Message(4, Event::Target(DragPhase::Dropped)))
    );
}

#[test]
fn pointer_region_captured_selected_sensitive_state_revokes_active_drag() {
    let mut r = runner();
    arm_selection(&mut r);
    r.advance_pending_selected_text_drag().unwrap();
    r.engine
        .input_states
        .get_mut(&19)
        .unwrap()
        .set_sensitive(true);
    r.dispatch_captured_pointer_region_move().unwrap();
    assert!(r.pointer_region_capture.is_none());
    assert!(events(&r).contains(&Event::Source(DragPhase::Cancelled(
        DragCancelReason::SourceRemoved
    ))));
}

#[test]
fn pointer_region_captured_owner_release_and_publication_failure() {
    let mut r = runner();
    let owner = Arc::downgrade(&r.engine.app_state.owner);
    r.dispatch_pointer_region_press(17);
    r.engine.app_state.owner = Arc::new(());
    dirty(&mut r);
    r.release_pointer_region_capture().unwrap();
    assert!(owner.upgrade().is_none());
    assert!(r.pointer_region_capture.is_none());

    let mut r = runner();
    r.engine.app_state.trigger = Some((Event::Pointer(PointerPhase::Pressed), Effect::DuplicateId));
    r.dispatch_pointer_region_press(17);
    assert!(r.fatal_error.is_some());
    assert!(r.pointer_region_capture.is_none());
    assert!(!events(&r).contains(&Event::Source(DragPhase::Started)));
}

#[test]
fn pointer_region_captured_selected_declaration_removal_cancels_without_legacy_fallback() {
    let mut r = runner();
    arm_selection(&mut r);
    r.advance_pending_selected_text_drag().unwrap();
    apply(&mut r.engine.app_state, Effect::RemoveSelectedDeclaration);
    dirty(&mut r);
    r.dispatch_captured_pointer_region_move().unwrap();
    assert!(r.pointer_region_capture.is_none());
    assert!(events(&r).contains(&Event::Source(DragPhase::Cancelled(
        DragCancelReason::SourceRemoved
    ))));
}
