// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::{Contains, Point};

use super::RutterRunner;
use crate::app::{AppLogic, LogicalPointerPosition};
use crate::pointer::{
    ActiveDragBadge, DragBadge, DragCancelReason, DragEvent, DragPhase,
    DragSourceWith as DragSource, DropTargetWith as DropTarget, PointerEvent, PointerModifiers,
    PointerPhase, SelectedTextDragWith,
};
use crate::render::hit_test::{HitResult, hit_test};
use crate::render::select_overlay::collector::{
    collect_open_dropdown_overlays, collect_open_search_overlays, collect_open_select_overlays,
};
use crate::widget::id::WidgetIdError;

type DragTarget<Msg> = (u64, DropTarget<Msg>);

// Application updates can change geometry again. Bound work even when callbacks
// oscillate or manufacture new IDs; unreported hover changes wait for next input.
const MAX_HOVER_UPDATES: usize = 64;
const MAX_TARGET_TRANSITIONS: usize = 16;

pub(super) struct PendingSelectedTextDrag<Msg> {
    id: u64,
    press: Point,
    local: Point,
    width: f32,
    height: f32,
    config: SelectedTextDragWith<Msg>,
    selected: String,
    badge: DragBadge,
}

enum CaptureOrigin<Msg> {
    PointerRegion(crate::ChangeCallback<PointerEvent, Msg>),
    SelectedTextInput,
}

impl<Msg> Clone for CaptureOrigin<Msg> {
    fn clone(&self) -> Self {
        match self {
            Self::PointerRegion(callback) => Self::PointerRegion(callback.clone()),
            Self::SelectedTextInput => Self::SelectedTextInput,
        }
    }
}

pub(super) struct PointerRegionCapture<Msg> {
    id: u64,
    origin: CaptureOrigin<Msg>,
    drag_source: Option<DragSource<Msg>>,
    target: Option<DragTarget<Msg>>,
}

impl<Msg> Clone for PointerRegionCapture<Msg> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            origin: self.origin.clone(),
            drag_source: self.drag_source.clone(),
            target: self.target.clone(),
        }
    }
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn refresh_pointer_region_hover(&mut self, left: bool) -> Result<(), WidgetIdError> {
        self.sync_pointer_region_layout()?;
        if self.pointer_region_hovered.is_empty()
            && !self
                .engine
                .runtime_caches
                .pointer_regions
                .values()
                .any(|region| region.on_hover.is_some())
        {
            return Ok(());
        }
        let point = Point::new(self.engine.last_mouse_pos.x, self.engine.last_mouse_pos.y);
        let mut transitions = std::collections::HashMap::<u64, usize>::new();
        for _ in 0..MAX_HOVER_UPDATES {
            self.sync_pointer_region_layout()?;
            let blocked = left || self.pointer_region_overlay_blocks();
            let regions = &self.engine.runtime_caches.pointer_regions;
            self.pointer_region_hovered.retain(|id| {
                regions
                    .get(id)
                    .is_some_and(|region| region.on_hover.is_some())
            });
            let hovered: std::collections::HashSet<_> = self
                .engine
                .runtime_caches
                .viewport_geometry
                .pointer_regions
                .iter()
                .filter_map(|(&id, bounds)| {
                    (!blocked
                        && bounds.contains(point)
                        && regions
                            .get(&id)
                            .is_some_and(|region| region.on_hover.is_some()))
                    .then_some(id)
                })
                .collect();
            let Some(id) = self
                .pointer_region_hovered
                .symmetric_difference(&hovered)
                .filter(|id| transitions.get(id).copied().unwrap_or(0) < 2)
                .copied()
                .min()
            else {
                break;
            };
            let inside = hovered.contains(&id);
            let callback = regions[&id].on_hover.as_ref().unwrap().clone();
            *transitions.entry(id).or_default() += 1;
            if inside {
                self.pointer_region_hovered.insert(id);
            } else {
                self.pointer_region_hovered.remove(&id);
            }
            A::update(
                &mut self.engine.app_state,
                callback.call(inside),
                &mut self.engine.clipboard,
            );
            self.engine.layout_dirty = true;
            self.redraw();
        }
        // Keep membership as last notified, not silently equal to geometry: an
        // exhausted ID can reconcile on the next move even at the same position.
        self.sync_pointer_region_layout()?;
        Ok(())
    }
    pub(super) fn dispatch_pointer_region_press(&mut self, id: u64) {
        if !self.publish_pointer_continuation() {
            return;
        }
        let Some(runtime) = self.engine.runtime_caches.pointer_regions.get(&id).cloned() else {
            return;
        };
        if runtime.capture_on_press || runtime.drag_source.is_some() {
            self.pointer_region_capture = Some(PointerRegionCapture {
                id,
                origin: CaptureOrigin::PointerRegion(runtime.on_pointer.clone()),
                drag_source: runtime.drag_source.clone(),
                target: None,
            });
            self.engine.active_drag_badge = runtime
                .drag_source
                .as_ref()
                .and(runtime.drag_badge)
                .map(|appearance| ActiveDragBadge {
                    appearance,
                    can_drop: false,
                });
        }
        self.dispatch_pointer_message(runtime.on_pointer, PointerPhase::Pressed);
        if !self.publish_pointer_continuation() {
            return;
        }
        if let Some(capture) = self.refresh_pointer_capture()
            && let Some(source) = capture.drag_source
        {
            self.dispatch_drag_message(
                source.on_drag.clone(),
                self.drag_event(DragPhase::Started, id, &source, None),
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
        self.sync_pointer_region_layout()?;
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
            || self
                .engine
                .runtime_caches
                .inputs
                .get(&pending.id)
                .is_none_or(|input| input.is_password)
            || self
                .engine
                .input_states
                .get(&pending.id)
                .is_none_or(|input| input.is_sensitive())
            || (self.engine.window.is_some() && self.pointer_region_overlay_blocks())
        {
            return Ok(true);
        }
        let Some(config) = self.current_selected_text_drag(pending.id) else {
            return Ok(true);
        };
        if config.source.payload != pending.config.source.payload {
            return Ok(true);
        }
        let badge = config.badge.clone().with_text(pending.selected.as_str());
        let Ok(badge) = badge else {
            return Ok(true);
        };
        self.start_selected_text_drag(PendingSelectedTextDrag {
            config,
            badge,
            ..pending
        });
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
            config.on_selected.call(selected),
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
        if !self.publish_pointer_continuation() {
            return;
        }
        if let Some(capture) = self.refresh_pointer_capture()
            && let Some(source) = capture.drag_source
        {
            self.dispatch_drag_message(
                source.on_drag.clone(),
                self.drag_event(DragPhase::Started, id, &source, None),
            );
        }
    }

    fn selected_text_drag_snapshot(
        &mut self,
        id: u64,
        local_x: f32,
        local_y: f32,
    ) -> Option<(SelectedTextDragWith<A::Message>, String, DragBadge)> {
        if self.engine.focused_input_id() != Some(id) {
            return None;
        }
        if self.engine.window.is_some() && self.pointer_region_overlay_blocks() {
            return None;
        }
        let config = self.current_selected_text_drag(id)?;
        let input = self.engine.runtime_caches.inputs.get(&id)?;
        if input.is_password {
            return None;
        }
        let theme = A::theme_for(&self.engine.app_state);
        if local_x < theme.spacing * 2.0 + input.leading_text_inset
            || local_x > theme.spacing * 2.0 + input.leading_text_inset + input.visible_w
            || local_y < theme.spacing
        {
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
        if self.pointer_region_capture.is_none() {
            return Ok(false);
        }
        self.sync_pointer_region_layout()?;
        if self.engine.window.is_some() && self.pointer_region_overlay_blocks() {
            self.cancel_pointer_region_capture(DragCancelReason::BlockingOverlay);
            return Ok(true);
        }
        let Some(capture) = self.refresh_pointer_capture() else {
            return Ok(true);
        };
        if let CaptureOrigin::PointerRegion(on_pointer) = capture.origin {
            self.dispatch_pointer_message(on_pointer, PointerPhase::Moved);
        }
        self.sync_pointer_region_layout()?;
        if self
            .refresh_pointer_capture()
            .is_some_and(|capture| capture.drag_source.is_some())
        {
            let Some(capture) = self.reconcile_drop_target()? else {
                return Ok(true);
            };
            let source = capture.drag_source.as_ref().unwrap();
            self.dispatch_drag_message(
                source.on_drag.clone(),
                self.drag_event(
                    DragPhase::Moved,
                    capture.id,
                    source,
                    capture.target.as_ref().map(|(id, _)| *id),
                ),
            );
            let Some(capture) = self.reconcile_drop_target()? else {
                return Ok(true);
            };
            let source = capture.drag_source.as_ref().unwrap();
            if let Some((id, target)) = capture.target {
                self.dispatch_drag_message(
                    target.on_drag,
                    self.drag_event(DragPhase::Moved, capture.id, source, Some(id)),
                );
            }
            self.reconcile_drop_target()?;
        }
        Ok(true)
    }

    pub(super) fn release_pointer_region_capture(&mut self) -> Result<bool, WidgetIdError> {
        if self.pointer_region_capture.is_none() {
            return Ok(false);
        }
        self.sync_pointer_region_layout()?;
        if self.refresh_pointer_capture().is_some() && self.pointer_region_overlay_blocks() {
            self.cancel_pointer_region_capture(DragCancelReason::BlockingOverlay);
            return Ok(true);
        }
        self.finish_pointer_region_capture();
        Ok(true)
    }

    fn finish_pointer_region_capture(&mut self) {
        if !self.publish_pointer_continuation() {
            return;
        }
        let Some(capture) = self.reconciled_pointer_continuation() else {
            return;
        };
        if let CaptureOrigin::PointerRegion(on_pointer) = capture.origin {
            self.dispatch_pointer_message(on_pointer, PointerPhase::Released);
        }
        if !self.publish_pointer_continuation() {
            return;
        }
        let Some(capture) = self.refresh_pointer_capture() else {
            return;
        };
        if capture.drag_source.is_none() {
            self.pointer_region_capture = None;
            self.engine.active_drag_badge = None;
            return;
        }
        // Reconcile through all Entered/Exited updates before fixing terminal
        // metadata. An unstable target is exited and omitted for this release.
        let Some(capture) = self.reconciled_pointer_continuation() else {
            return;
        };
        let Some(source) = capture.drag_source.as_ref() else {
            return;
        };
        let target = capture.target.clone();
        self.pointer_region_capture = None;
        self.engine.active_drag_badge = None;
        self.dispatch_drag_message(
            source.on_drag.clone(),
            self.drag_event(
                DragPhase::Dropped,
                capture.id,
                source,
                target.as_ref().map(|(id, _)| *id),
            ),
        );
        if !self.publish_pointer_continuation() {
            return;
        }
        let live_target = match self.matching_drop_target(source) {
            Ok(target) => target,
            Err(error) => {
                self.fatal_error.get_or_insert_with(|| error.into());
                return;
            }
        };
        // Dropped is terminal for the source. Its update can retire either
        // owner; do not send a second terminal event or drop through old metadata.
        if self.gesture_source_is_eligible(&capture)
            && let Some((id, target)) = target
            && let Some((live_id, live)) = live_target
            && live_id == id
            && live.accepted_kind == target.accepted_kind
        {
            self.dispatch_drag_message(
                live.on_drag,
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
                    self.drag_event(DragPhase::Cancelled(reason), capture.id, &source, Some(id)),
                );
            }
            self.dispatch_drag_message(
                source.on_drag.clone(),
                self.drag_event(DragPhase::Cancelled(reason), capture.id, &source, None),
            );
        }
    }

    fn transition_drop_target(&mut self, next: Option<DragTarget<A::Message>>) {
        let Some(mut capture) = self.pointer_region_capture.clone() else {
            return;
        };
        let Some(source) = capture.drag_source.clone() else {
            return;
        };
        let previous = capture.target;
        if previous.as_ref().map(|(id, _)| id) == next.as_ref().map(|(id, _)| id) {
            capture.target = next;
            self.pointer_region_capture = Some(capture);
            return;
        }
        capture.target = None;
        self.pointer_region_capture = Some(capture.clone());
        if let Some(badge) = &mut self.engine.active_drag_badge {
            badge.can_drop = next.is_some();
        }
        if let Some((id, target)) = previous {
            self.dispatch_drag_message(
                target.on_drag,
                self.drag_event(DragPhase::Exited, capture.id, &source, Some(id)),
            );
        }
        if !self.publish_pointer_continuation() {
            return;
        }
        let next = if self.engine.window.is_some() {
            match self.matching_drop_target(&source) {
                Ok(target) => target.filter(|_| next.is_some()),
                Err(error) => {
                    self.fatal_error.get_or_insert_with(|| error.into());
                    return;
                }
            }
        } else {
            next
        };
        if let Some((id, target)) = next {
            let Some(mut capture) = self.refresh_pointer_capture() else {
                return;
            };
            let Some(target) = self
                .engine
                .runtime_caches
                .pointer_regions
                .get(&id)
                .and_then(|region| region.drop_target.clone())
                .filter(|live| live.accepted_kind == target.accepted_kind)
            else {
                return;
            };
            capture.target = Some((id, target.clone()));
            self.pointer_region_capture = Some(capture.clone());
            if let Some(badge) = &mut self.engine.active_drag_badge {
                badge.can_drop = true;
            }
            self.dispatch_drag_message(
                target.on_drag,
                self.drag_event(DragPhase::Entered, capture.id, &source, Some(id)),
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

    fn current_selected_text_drag(&self, id: u64) -> Option<SelectedTextDragWith<A::Message>> {
        A::selected_text_drag_with(&self.engine.app_state, id)
            .or_else(|| A::selected_text_drag(&self.engine.app_state, id).map(Into::into))
    }

    fn gesture_source_is_eligible(&self, capture: &PointerRegionCapture<A::Message>) -> bool {
        let current = match &capture.origin {
            CaptureOrigin::PointerRegion(_) => self
                .engine
                .runtime_caches
                .pointer_regions
                .get(&capture.id)
                .and_then(|region| region.drag_source.as_ref())
                .map(|source| source.payload),
            CaptureOrigin::SelectedTextInput => {
                if self
                    .engine
                    .runtime_caches
                    .inputs
                    .get(&capture.id)
                    .is_none_or(|input| input.is_password)
                    || self
                        .engine
                        .input_states
                        .get(&capture.id)
                        .is_none_or(|input| input.is_sensitive())
                {
                    return false;
                }
                self.current_selected_text_drag(capture.id)
                    .map(|config| config.source.payload)
            }
        };
        current == capture.drag_source.as_ref().map(|source| source.payload)
    }

    /// Gesture identity/payload are immutable. Only live continuation callbacks
    /// refresh; retired owners are retained solely for terminal teardown.
    fn refresh_pointer_capture(&mut self) -> Option<PointerRegionCapture<A::Message>> {
        let mut capture = self.pointer_region_capture.clone()?;
        if self.engine.window.is_some() && self.pointer_region_overlay_blocks() {
            self.cancel_pointer_region_capture(DragCancelReason::BlockingOverlay);
            return None;
        }
        let source = match &capture.origin {
            CaptureOrigin::SelectedTextInput => {
                let eligible = self
                    .engine
                    .runtime_caches
                    .inputs
                    .get(&capture.id)
                    .is_some_and(|input| !input.is_password)
                    && self
                        .engine
                        .input_states
                        .get(&capture.id)
                        .is_some_and(|input| !input.is_sensitive());
                if !eligible {
                    self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
                    return None;
                }
                self.current_selected_text_drag(capture.id)
                    .map(|config| config.source)
            }
            CaptureOrigin::PointerRegion(_) => {
                let Some(region) = self.engine.runtime_caches.pointer_regions.get(&capture.id)
                else {
                    self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
                    return None;
                };
                if !region.capture_on_press && region.drag_source.is_none() {
                    self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
                    return None;
                }
                capture.origin = CaptureOrigin::PointerRegion(region.on_pointer.clone());
                region.drag_source.clone()
            }
        };
        if capture.drag_source.as_ref().map(|source| source.payload)
            != source.as_ref().map(|source| source.payload)
        {
            self.cancel_pointer_region_capture(DragCancelReason::SourceRemoved);
            return None;
        }
        capture.drag_source = source;
        if let Some((id, old)) = capture.target.take() {
            let current = self
                .engine
                .runtime_caches
                .pointer_regions
                .get(&id)
                .and_then(|region| region.drop_target.clone());
            if let Some(target) = current.filter(|target| target.accepted_kind == old.accepted_kind)
            {
                capture.target = Some((id, target));
            } else {
                // Reconciliation will deliver the retired owner's terminal Exit.
                capture.target = Some((id, old));
            }
        }
        self.pointer_region_capture = Some(capture.clone());
        if let Some(badge) = &mut self.engine.active_drag_badge {
            badge.can_drop = capture.target.is_some();
        }
        Some(capture)
    }

    /// Re-hit after every transition's update, including exits which may move a
    /// different target under the stationary pointer. At most 16 notifications
    /// plus a balancing Exit: oscillation leaves no active target for this phase.
    fn reconcile_drop_target(
        &mut self,
    ) -> Result<Option<PointerRegionCapture<A::Message>>, WidgetIdError> {
        for _ in 0..MAX_TARGET_TRANSITIONS {
            self.sync_pointer_region_layout()?;
            let Some(mut capture) = self.refresh_pointer_capture() else {
                return Ok(None);
            };
            let Some(source) = capture.drag_source.as_ref() else {
                return Ok(Some(capture));
            };
            let next = self.matching_drop_target(source)?;
            let key = |target: &Option<DragTarget<A::Message>>| {
                target
                    .as_ref()
                    .map(|(id, target)| (*id, target.accepted_kind))
            };
            if key(&capture.target) == key(&next) {
                capture.target = next;
                self.pointer_region_capture = Some(capture.clone());
                return Ok(Some(capture));
            }
            // Exit before entering, and re-hit in the next iteration rather
            // than entering the candidate chosen before the Exit callback.
            if capture.target.is_some() {
                self.transition_drop_target(None);
            } else {
                self.transition_drop_target(next);
            }
        }
        self.transition_drop_target(None);
        self.sync_pointer_region_layout()?;
        Ok(self.refresh_pointer_capture())
    }

    fn reconciled_pointer_continuation(&mut self) -> Option<PointerRegionCapture<A::Message>> {
        match self.reconcile_drop_target() {
            Ok(capture) => capture,
            Err(error) => {
                self.fatal_error.get_or_insert_with(|| error.into());
                None
            }
        }
    }

    fn publish_pointer_continuation(&mut self) -> bool {
        if self.fatal_error.is_some() {
            return false;
        }
        if let Err(error) = self.sync_pointer_region_layout() {
            self.fatal_error.get_or_insert_with(|| error.into());
            self.pointer_region_capture = None;
            self.engine.active_drag_badge = None;
            return false;
        }
        true
    }

    fn sync_pointer_region_layout(&mut self) -> Result<(), WidgetIdError> {
        let Some(size) = self
            .engine
            .window
            .as_ref()
            .map(|window| window.surface_size())
        else {
            return Ok(());
        };
        if let Err(error) = self.engine.try_ensure_layout_for_input(size) {
            self.fatal_error.get_or_insert_with(|| error.clone().into());
            self.pointer_region_capture = None;
            self.pending_selected_text_drag = None;
            self.engine.active_drag_badge = None;
            return Err(error);
        }
        Ok(())
    }

    fn dispatch_pointer_message(
        &mut self,
        callback: crate::ChangeCallback<PointerEvent, A::Message>,
        phase: PointerPhase,
    ) {
        let message = callback.call(self.pointer_event(phase));
        A::update(
            &mut self.engine.app_state,
            message,
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
    }

    fn dispatch_drag_message(
        &mut self,
        callback: crate::ChangeCallback<DragEvent, A::Message>,
        event: DragEvent,
    ) {
        let message = callback.call(event);
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
        source: &DragSource<A::Message>,
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
        let size = self.engine.window.as_ref()?.surface_size();
        let scale = self.engine.scale_factor;
        Some((size.width as f32 / scale, size.height as f32 / scale))
    }

    fn pointer_modifiers(&self) -> PointerModifiers {
        let modifiers = self.engine.modifiers.state();
        PointerModifiers::new(
            modifiers.shift_key(),
            modifiers.control_key(),
            modifiers.alt_key(),
            modifiers.meta_key(),
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
            .and_then(|region| region.drop_target.clone())
        else {
            return Ok(None);
        };
        Ok(drag_source_matches_target(source, &target).then_some((id, target)))
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

fn drag_source_matches_target<Msg>(source: &DragSource<Msg>, target: &DropTarget<Msg>) -> bool {
    source.payload.kind() == target.accepted_kind
}

#[cfg(test)]
#[path = "pointer_region_captured_tests.rs"]
mod captured_tests;

#[cfg(test)]
#[path = "pointer_region_legacy_tests.rs"]
mod tests;
