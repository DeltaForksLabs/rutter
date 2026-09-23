// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::Point;

use super::RutterRunner;
use crate::app::{AppLogic, LogicalPointerPosition};
use crate::pointer::{
    ActiveDragBadge, DragCancelReason, DragEvent, DragPhase, DragSource, DropTarget, PointerEvent,
    PointerModifiers, PointerPhase,
};
use crate::render::hit_test::{HitResult, hit_test};
use crate::render::select_overlay::collector::{
    collect_open_dropdown_overlays, collect_open_search_overlays, collect_open_select_overlays,
};
use crate::widget::id::WidgetIdError;

type DragTarget<Msg> = (u64, DropTarget<Msg>);

pub(super) struct PointerRegionCapture<Msg> {
    id: u64,
    on_pointer: fn(PointerEvent) -> Msg,
    drag_source: Option<DragSource<Msg>>,
    target: Option<DragTarget<Msg>>,
}

impl<Msg> Copy for PointerRegionCapture<Msg> {}

impl<Msg> Clone for PointerRegionCapture<Msg> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn dispatch_pointer_region_press(&mut self, id: u64) {
        let Some(runtime) = self.engine.runtime_caches.pointer_regions.get(&id).cloned() else {
            return;
        };
        if runtime.capture_on_press || runtime.drag_source.is_some() {
            self.pointer_region_capture = Some(PointerRegionCapture {
                id,
                on_pointer: runtime.on_pointer,
                drag_source: runtime.drag_source,
                target: None,
            });
            self.engine.active_drag_badge =
                runtime
                    .drag_source
                    .and(runtime.drag_badge)
                    .map(|appearance| ActiveDragBadge {
                        appearance,
                        can_drop: false,
                    });
        }
        self.dispatch_pointer_message(runtime.on_pointer, PointerPhase::Pressed);
        if let Some(source) = runtime.drag_source {
            self.dispatch_drag_message(
                source.on_drag,
                self.drag_event(DragPhase::Started, id, source, None),
            );
        }
    }

    pub(super) fn dispatch_captured_pointer_region_move(&mut self) -> Result<bool, WidgetIdError> {
        let Some(capture) = self.pointer_region_capture else {
            return Ok(false);
        };
        self.sync_pointer_region_layout()?;
        if self.engine.window.is_some() && self.pointer_region_overlay_blocks() {
            self.cancel_pointer_region_capture(DragCancelReason::BlockingOverlay);
            return Ok(true);
        }
        if !self.pointer_region_is_live(capture.id) {
            self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
            return Ok(true);
        }
        self.dispatch_pointer_message(capture.on_pointer, PointerPhase::Moved);
        if let Some(source) = capture.drag_source {
            let target = self.matching_drop_target(&source)?;
            if !self.pointer_region_is_live(capture.id) {
                self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
                return Ok(true);
            }
            self.transition_drop_target(target);
            self.dispatch_drag_message(
                source.on_drag,
                self.drag_event(
                    DragPhase::Moved,
                    capture.id,
                    source,
                    target.map(|(id, _)| id),
                ),
            );
            if let Some((id, target)) = target {
                self.dispatch_drag_message(
                    target.on_drag,
                    self.drag_event(DragPhase::Moved, capture.id, source, Some(id)),
                );
            }
        }
        Ok(true)
    }

    pub(super) fn release_pointer_region_capture(&mut self) -> Result<bool, WidgetIdError> {
        if self.pointer_region_capture.is_none() {
            return Ok(false);
        }
        self.sync_pointer_region_layout()?;
        if let Some(capture) = self.pointer_region_capture {
            if !self.pointer_region_is_live(capture.id) {
                self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
                return Ok(true);
            }
            if self.pointer_region_overlay_blocks() {
                self.cancel_pointer_region_capture(DragCancelReason::BlockingOverlay);
                return Ok(true);
            }
        }
        let target = match self
            .pointer_region_capture
            .and_then(|capture| capture.drag_source)
        {
            Some(source) => self.matching_drop_target(&source)?,
            None => None,
        };
        if let Some(capture) = self.pointer_region_capture
            && !self.pointer_region_is_live(capture.id)
        {
            self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
            return Ok(true);
        }
        self.finish_pointer_region_capture(target);
        Ok(true)
    }

    fn finish_pointer_region_capture(&mut self, target: Option<DragTarget<A::Message>>) {
        let Some(capture) = self.pointer_region_capture else {
            return;
        };
        self.transition_drop_target(target);
        self.pointer_region_capture = None;
        self.engine.active_drag_badge = None;
        self.dispatch_pointer_message(capture.on_pointer, PointerPhase::Released);
        let Some(source) = capture.drag_source else {
            return;
        };
        self.dispatch_drag_message(
            source.on_drag,
            self.drag_event(
                DragPhase::Dropped,
                capture.id,
                source,
                target.map(|(id, _)| id),
            ),
        );
        if let Some((id, target)) = target {
            self.dispatch_drag_message(
                target.on_drag,
                self.drag_event(DragPhase::Dropped, capture.id, source, Some(id)),
            );
        }
    }

    pub(crate) fn cancel_pointer_region_capture(&mut self, reason: DragCancelReason) {
        let Some(capture) = self.pointer_region_capture.take() else {
            return;
        };
        self.engine.active_drag_badge = None;
        self.dispatch_pointer_message(capture.on_pointer, PointerPhase::Cancelled);
        if let Some(source) = capture.drag_source {
            if let Some((id, target)) = capture.target {
                self.dispatch_drag_message(
                    target.on_drag,
                    self.drag_event(DragPhase::Cancelled(reason), capture.id, source, Some(id)),
                );
            }
            self.dispatch_drag_message(
                source.on_drag,
                self.drag_event(DragPhase::Cancelled(reason), capture.id, source, None),
            );
        }
    }

    fn transition_drop_target(&mut self, next: Option<DragTarget<A::Message>>) {
        let Some(mut capture) = self.pointer_region_capture else {
            return;
        };
        let Some(source) = capture.drag_source else {
            return;
        };
        let previous = capture.target;
        if previous.map(|(id, _)| id) == next.map(|(id, _)| id) {
            return;
        }
        capture.target = next;
        self.pointer_region_capture = Some(capture);
        if let Some(badge) = &mut self.engine.active_drag_badge {
            badge.can_drop = next.is_some();
        }
        if let Some((id, target)) = previous {
            self.dispatch_drag_message(
                target.on_drag,
                self.drag_event(DragPhase::Exited, capture.id, source, Some(id)),
            );
        }
        if let Some((id, target)) = next {
            self.dispatch_drag_message(
                target.on_drag,
                self.drag_event(DragPhase::Entered, capture.id, source, Some(id)),
            );
        }
    }

    pub(super) fn pointer_region_overlay_blocks(&mut self) -> bool {
        let Some(viewport) = self.pointer_region_logical_viewport() else {
            return true;
        };
        let overlay_open = self.engine.any_context_menu_open() || self.engine.any_popover_open();
        let widget = A::view(&mut self.engine.app_state);
        let select_open = !collect_open_select_overlays(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            &self.engine.widget_states,
            viewport,
        )
        .is_empty();
        let dropdown_open = !collect_open_dropdown_overlays(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            &self.engine.widget_states,
            viewport,
        )
        .is_empty();
        let search_open = !collect_open_search_overlays(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            &self.engine.widget_states,
            &self.engine.input_states,
            self.engine.focused_widget_id,
            viewport,
        )
        .is_empty();
        super::has_visible_blocking_overlay(&widget)
            || overlay_open
            || select_open
            || dropdown_open
            || search_open
    }

    fn pointer_region_is_live(&self, id: u64) -> bool {
        self.engine.runtime_caches.pointer_regions.contains_key(&id)
    }

    fn sync_pointer_region_layout(&mut self) -> Result<(), WidgetIdError> {
        let Some(size) = self
            .engine
            .window
            .as_ref()
            .map(|window| window.inner_size())
        else {
            return Ok(());
        };
        self.engine.try_ensure_widget_states()?;
        self.engine.try_ensure_layout(size)
    }

    fn dispatch_pointer_message(
        &mut self,
        callback: fn(PointerEvent) -> A::Message,
        phase: PointerPhase,
    ) {
        let message = callback(self.pointer_event(phase));
        A::update(
            &mut self.engine.app_state,
            message,
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
    }

    fn dispatch_drag_message(&mut self, callback: fn(DragEvent) -> A::Message, event: DragEvent) {
        let message = callback(event);
        A::update(
            &mut self.engine.app_state,
            message,
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
    }

    fn pointer_event(&self, phase: PointerPhase) -> PointerEvent {
        PointerEvent {
            phase,
            position: self.logical_pointer_position(),
            modifiers: self.pointer_modifiers(),
        }
    }

    fn drag_event(
        &self,
        phase: DragPhase,
        source_id: u64,
        source: DragSource<A::Message>,
        target_id: Option<u64>,
    ) -> DragEvent {
        DragEvent {
            phase,
            payload: source.payload,
            source_id,
            target_id,
            position: self.logical_pointer_position(),
            modifiers: self.pointer_modifiers(),
        }
    }

    fn logical_pointer_position(&self) -> LogicalPointerPosition {
        LogicalPointerPosition::new(self.engine.last_mouse_pos.x, self.engine.last_mouse_pos.y)
    }

    fn pointer_region_logical_viewport(&self) -> Option<(f32, f32)> {
        let size = self.engine.window.as_ref()?.inner_size();
        let scale = self.engine.scale_factor;
        Some((size.width as f32 / scale, size.height as f32 / scale))
    }

    fn pointer_modifiers(&self) -> PointerModifiers {
        let modifiers = self.engine.modifiers.state();
        PointerModifiers::new(
            modifiers.shift_key(),
            modifiers.control_key(),
            modifiers.alt_key(),
            modifiers.super_key(),
        )
    }

    fn matching_drop_target(
        &mut self,
        source: &DragSource<A::Message>,
    ) -> Result<Option<DragTarget<A::Message>>, WidgetIdError> {
        let Some(id) = self.pointer_region_at_cursor()? else {
            return Ok(None);
        };
        let Some(target) = self
            .engine
            .runtime_caches
            .pointer_regions
            .get(&id)
            .and_then(|region| region.drop_target)
        else {
            return Ok(None);
        };
        Ok(drag_source_matches_target(source, target).then_some((id, target)))
    }

    fn pointer_region_at_cursor(&mut self) -> Result<Option<u64>, WidgetIdError> {
        if self.engine.window.is_none() {
            return Ok(None);
        }
        self.sync_pointer_region_layout()?;
        if self.pointer_region_overlay_blocks() {
            return Ok(None);
        }
        let widget = A::view(&mut self.engine.app_state);
        let cursor = Point::new(self.engine.last_mouse_pos.x, self.engine.last_mouse_pos.y);
        let hit = hit_test(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            cursor,
            Point::new(0.0, 0.0),
            &self.engine.widget_states,
        );
        Ok(match hit {
            Some(HitResult::PointerRegion(id)) => Some(id),
            _ => None,
        })
    }
}

fn drag_source_matches_target<Msg>(source: &DragSource<Msg>, target: DropTarget<Msg>) -> bool {
    source.payload.kind() == target.accepted_kind
}

#[cfg(test)]
mod tests {
    use arboard::Clipboard;
    use cosmic_text::FontSystem;
    use skia_safe::{Color, Point};
    use taffy::prelude::Style;

    use super::*;
    use crate::engine::{PointerRegionRuntime, RutterEngine};
    use crate::pointer::{DragBadge, DragPayload, DragPayloadKind};
    use crate::widget::Widget;

    #[derive(Clone, Debug, PartialEq)]
    enum Message {
        Pointer(PointerPhase),
        Source(DragPhase),
        Target(DragPhase, Option<u64>),
    }

    struct PointerApp;

    impl AppLogic for PointerApp {
        type State = Vec<Message>;
        type Message = Message;

        fn new(_: &mut FontSystem) -> Self::State {
            Vec::new()
        }

        fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Style::default(),
            }
        }

        fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
            state.push(message);
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

    #[test]
    fn capture_delivers_pointer_and_drag_phases_in_order() {
        let engine = RutterEngine::<PointerApp>::new().unwrap();
        let mut runner = RutterRunner::with_engine(engine);
        runner.engine.last_mouse_pos = Point::new(12.0, 18.0);
        let source = DragSource {
            payload: DragPayload::new(DragPayloadKind::new(4), 88),
            on_drag: source_message,
        };
        runner.engine.runtime_caches.pointer_regions.insert(
            17,
            PointerRegionRuntime {
                on_pointer: pointer_message,
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
            on_drag: source_message,
        };
        let matching = DropTarget {
            accepted_kind: DragPayloadKind::new(2),
            on_drag: source_message,
        };
        let mismatched = DropTarget {
            accepted_kind: DragPayloadKind::new(3),
            on_drag: source_message,
        };

        assert!(drag_source_matches_target(&source, matching));
        assert!(!drag_source_matches_target(&source, mismatched));
    }

    #[test]
    fn changing_drop_targets_exits_before_entering_and_cancels_current_target() {
        let engine = RutterEngine::<PointerApp>::new().unwrap();
        let mut runner = RutterRunner::with_engine(engine);
        let source = DragSource {
            payload: DragPayload::new(DragPayloadKind::new(2), 7),
            on_drag: source_message,
        };
        let target = DropTarget {
            accepted_kind: source.payload.kind(),
            on_drag: target_message,
        };
        runner.pointer_region_capture = Some(PointerRegionCapture {
            id: 17,
            on_pointer: pointer_message,
            drag_source: Some(source),
            target: None,
        });

        runner.transition_drop_target(Some((18, target)));
        runner.transition_drop_target(Some((19, target)));
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
            on_drag: source_message,
        };
        let target = DropTarget {
            accepted_kind: source.payload.kind(),
            on_drag: target_message,
        };
        runner.pointer_region_capture = Some(PointerRegionCapture {
            id: 17,
            on_pointer: pointer_message,
            drag_source: Some(source),
            target: None,
        });

        runner.finish_pointer_region_capture(Some((18, target)));

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
            on_pointer: pointer_message,
            drag_source: Some(DragSource {
                payload: DragPayload::new(DragPayloadKind::new(2), 7),
                on_drag: source_message,
            }),
            target: None,
        });

        assert!(runner.dispatch_captured_pointer_region_move().unwrap());
        assert_eq!(
            runner.engine.app_state,
            vec![
                Message::Pointer(PointerPhase::Cancelled),
                Message::Source(DragPhase::Cancelled(DragCancelReason::SourceRemoved)),
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
            on_drag: source_message,
        };
        let appearance = DragBadge::new(Color::RED, Color::WHITE);
        runner.engine.runtime_caches.pointer_regions.insert(
            17,
            PointerRegionRuntime {
                on_pointer: pointer_message,
                capture_on_press: false,
                drag_source: Some(source),
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
            on_drag: target_message,
        };
        runner.transition_drop_target(Some((18, target)));
        assert!(runner.engine.active_drag_badge.as_ref().unwrap().can_drop);
        runner.transition_drop_target(None);
        assert!(!runner.engine.active_drag_badge.as_ref().unwrap().can_drop);
        runner.finish_pointer_region_capture(Some((18, target)));
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
}
