// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::Point;

use super::RutterRunner;
use crate::app::{AppLogic, LogicalPointerPosition};
use crate::pointer::{
    ActiveDragBadge, DragBadge, DragCancelReason, DragEvent, DragPhase, DragSource, DropTarget,
    PointerEvent, PointerModifiers, PointerPhase, SelectedTextDrag,
};
use crate::render::hit_test::{HitResult, hit_test};
use crate::render::select_overlay::collector::{
    collect_open_dropdown_overlays, collect_open_search_overlays, collect_open_select_overlays,
};
use crate::widget::id::WidgetIdError;

type DragTarget<Msg> = (u64, DropTarget<Msg>);

pub(super) struct PendingSelectedTextDrag<Msg> {
    id: u64,
    press: Point,
    local: Point,
    width: f32,
    height: f32,
    config: SelectedTextDrag<Msg>,
    selected: String,
    badge: DragBadge,
}

enum CaptureOrigin<Msg> {
    PointerRegion(fn(PointerEvent) -> Msg),
    SelectedTextInput,
}

impl<Msg> Copy for CaptureOrigin<Msg> {}

impl<Msg> Clone for CaptureOrigin<Msg> {
    fn clone(&self) -> Self {
        *self
    }
}

pub(super) struct PointerRegionCapture<Msg> {
    id: u64,
    origin: CaptureOrigin<Msg>,
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
                origin: CaptureOrigin::PointerRegion(runtime.on_pointer),
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

    pub(super) fn arm_selected_text_drag(
        &mut self,
        id: u64,
        press: Point,
        local: Point,
        width: f32,
        height: f32,
    ) -> bool {
        let Some((config, selected, badge)) =
            self.selected_text_drag_snapshot(id, local.x, local.y)
        else {
            return false;
        };
        self.pending_selected_text_drag = Some(PendingSelectedTextDrag {
            id,
            press,
            local,
            width,
            height,
            config,
            selected,
            badge,
        });
        true
    }

    /// Only movement beyond the click tolerance turns an armed selection into a drag.
    pub(super) fn advance_pending_selected_text_drag(&mut self) -> Result<bool, WidgetIdError> {
        let Some(pending) = self.pending_selected_text_drag.take() else {
            return Ok(false);
        };
        let cursor = Point::new(
            self.cursor_pos.x / self.engine.scale_factor,
            self.cursor_pos.y / self.engine.scale_factor,
        );
        if (cursor.x - pending.press.x).hypot(cursor.y - pending.press.y) < 5.0 {
            self.pending_selected_text_drag = Some(pending);
            return Ok(true);
        }
        if self.engine.focused_input_id() != Some(pending.id)
            || !self
                .engine
                .runtime_caches
                .inputs
                .get(&pending.id)
                .is_some_and(|input| !input.is_password)
            || self
                .engine
                .input_states
                .get(&pending.id)
                .is_none_or(|input| input.is_sensitive())
            || (self.engine.window.is_some() && self.pointer_region_overlay_blocks())
        {
            return Ok(true);
        }
        self.start_selected_text_drag(pending);
        self.dispatch_captured_pointer_region_move()
    }

    pub(super) fn release_pending_selected_text_drag(&mut self) -> bool {
        let Some(pending) = self.pending_selected_text_drag.take() else {
            return false;
        };
        if self.engine.focused_input_id() == Some(pending.id)
            && self.engine.runtime_caches.inputs.contains_key(&pending.id)
        {
            self.focused_input_rect = Some(skia_safe::Rect::from_xywh(
                pending.press.x - pending.local.x,
                pending.press.y - pending.local.y,
                pending.width,
                pending.height,
            ));
            self.focus_input_at(
                pending.id,
                pending.local.x,
                pending.local.y,
                pending.width,
                pending.height,
                false,
            );
        }
        true
    }

    /// An opted-in text input shares pointer-region drop routing without
    /// changing the editor's selection while the drag is active.
    fn start_selected_text_drag(&mut self, pending: PendingSelectedTextDrag<A::Message>) {
        let PendingSelectedTextDrag {
            id,
            config,
            selected,
            badge,
            ..
        } = pending;
        A::update(
            &mut self.engine.app_state,
            (config.on_selected)(selected),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
        self.pointer_region_capture = Some(PointerRegionCapture {
            id,
            origin: CaptureOrigin::SelectedTextInput,
            drag_source: Some(config.source),
            target: None,
        });
        self.engine.active_drag_badge = Some(ActiveDragBadge {
            appearance: badge,
            can_drop: false,
        });
        self.dispatch_drag_message(
            config.source.on_drag,
            self.drag_event(DragPhase::Started, id, config.source, None),
        );
    }

    fn selected_text_drag_snapshot(
        &mut self,
        id: u64,
        local_x: f32,
        local_y: f32,
    ) -> Option<(SelectedTextDrag<A::Message>, String, DragBadge)> {
        if self.engine.focused_input_id() != Some(id) {
            return None;
        }
        if self.engine.window.is_some() && self.pointer_region_overlay_blocks() {
            return None;
        }
        let config = A::selected_text_drag(&self.engine.app_state, id)?;
        let input = self.engine.runtime_caches.inputs.get(&id)?;
        if input.is_password {
            return None;
        }
        let theme = A::theme_for(&self.engine.app_state);
        if local_x < theme.spacing * 2.0 + input.leading_text_inset || local_y < theme.spacing {
            return None;
        }
        let selected = self.engine.input_states.get(&id).and_then(|state| {
            let x = super::mapped_input_pointer_x(
                local_x,
                theme.spacing * 2.0,
                input.leading_text_inset,
                state.scroll_x,
            );
            let y = (local_y - theme.spacing + state.scroll_y).max(0.0);
            state.selected_text_at(x, y, 64)
        })?;
        if selected.trim().is_empty() {
            return None;
        }
        let badge = config.badge.clone().with_text(selected.as_str()).ok()?;
        Some((config, selected, badge))
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
        if !self.captured_source_is_live(capture) {
            self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
            return Ok(true);
        }
        if let CaptureOrigin::PointerRegion(on_pointer) = capture.origin {
            self.dispatch_pointer_message(on_pointer, PointerPhase::Moved);
        }
        if let Some(source) = capture.drag_source {
            let target = self.matching_drop_target(&source)?;
            if !self.captured_source_is_live(capture) {
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
            if !self.captured_source_is_live(capture) {
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
            && !self.captured_source_is_live(capture)
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
        if let CaptureOrigin::PointerRegion(on_pointer) = capture.origin {
            self.dispatch_pointer_message(on_pointer, PointerPhase::Released);
        }
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
        if let CaptureOrigin::PointerRegion(on_pointer) = capture.origin {
            self.dispatch_pointer_message(on_pointer, PointerPhase::Cancelled);
        }
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

    fn captured_source_is_live(&self, capture: PointerRegionCapture<A::Message>) -> bool {
        match capture.origin {
            CaptureOrigin::SelectedTextInput => self
                .engine
                .runtime_caches
                .inputs
                .get(&capture.id)
                .is_some_and(|input| !input.is_password),
            CaptureOrigin::PointerRegion(_) => self
                .engine
                .runtime_caches
                .pointer_regions
                .contains_key(&capture.id),
        }
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
            Widget::Spacer {
                style: Style::default(),
            }
        }

        fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
            state.push(message);
        }

        fn selected_text_drag(_state: &Self::State, id: u64) -> Option<SelectedTextDrag<Message>> {
            (id == 19).then(|| SelectedTextDrag {
                source: DragSource {
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

    fn runner_with_selected_word() -> RutterRunner<PointerApp> {
        let engine = RutterEngine::<PointerApp>::new().unwrap();
        let mut runner = RutterRunner::with_engine(engine);
        runner.engine.runtime_caches.inputs.insert(
            19,
            InputRuntime {
                on_change: Message::Selected,
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
        assert_eq!(runner.pointer_region_capture.unwrap().id, 19);
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
            theme.spacing + 8.0,
        ));
        let target = DropTarget {
            accepted_kind: DragPayloadKind::new(2),
            on_drag: target_message,
        };
        runner.finish_pointer_region_capture(Some((18, target)));

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
            origin: CaptureOrigin::PointerRegion(pointer_message),
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
            origin: CaptureOrigin::PointerRegion(pointer_message),
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
            origin: CaptureOrigin::PointerRegion(pointer_message),
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
