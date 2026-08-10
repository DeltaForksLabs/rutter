// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Point, Rect as SkiaRect};
use taffy::prelude::{NodeId, TaffyTree};

use super::super::ACCORDION_HEADER_H;
use super::super::hit_test::{modal_card_rect, popover_rect};
use crate::engine::widget_state::{PopoverState, WidgetState};
use crate::layout::RutterContext;
use crate::widget::Widget;
use crate::widgets::dropdown_menu::{DropdownMenuEntry, DropdownMenuState};

#[derive(Clone, Copy)]
pub(crate) struct SelectOverlay<'a> {
    pub(crate) id: u64,
    pub(crate) options: &'a [&'a str],
    pub(crate) selected_index: usize,
    pub(crate) hovered_option: Option<usize>,
    pub(crate) anchor: SkiaRect,
    owner: OverlayOwner,
}

pub(crate) struct DropdownOverlay<'a, Msg> {
    pub(crate) id: u64,
    pub(crate) entries: &'a [DropdownMenuEntry<'a, Msg>],
    pub(crate) anchor: SkiaRect,
    pub(crate) visible_anchor: SkiaRect,
    pub(crate) state: DropdownMenuState,
    pub(crate) owner: OverlayOwner,
}

#[derive(Clone, Copy)]
pub(crate) struct DropdownTrigger {
    pub(crate) id: u64,
    pub(crate) anchor: SkiaRect,
    pub(crate) visible_anchor: SkiaRect,
    owner: OverlayOwner,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OverlayOwner {
    phase: u8,
    order: u64,
}

pub(crate) fn collect_open_select_overlays<'a, Msg>(
    widget: &Widget<'a, Msg>,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget_states: &HashMap<u64, WidgetState>,
    viewport: (f32, f32),
) -> Vec<SelectOverlay<'a>> {
    let mut collector = SelectOverlayCollector::new(taffy, widget_states, viewport);
    collector.visit(widget, root, Point::new(0.0, 0.0));
    collector.finish_selects()
}

pub(crate) fn collect_open_dropdown_overlays<'widget, 'entry, Msg>(
    widget: &'widget Widget<'entry, Msg>,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget_states: &HashMap<u64, WidgetState>,
    viewport: (f32, f32),
) -> Vec<DropdownOverlay<'widget, Msg>>
where
    'entry: 'widget,
{
    let mut collector = SelectOverlayCollector::new(taffy, widget_states, viewport);
    collector.visit(widget, root, Point::new(0.0, 0.0));
    collector.finish_dropdowns()
}

pub(crate) fn collect_dropdown_triggers<'widget, 'entry, Msg>(
    widget: &'widget Widget<'entry, Msg>,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget_states: &HashMap<u64, WidgetState>,
    viewport: (f32, f32),
) -> Vec<DropdownTrigger>
where
    'entry: 'widget,
{
    let mut collector = SelectOverlayCollector::new(taffy, widget_states, viewport);
    collector.visit(widget, root, Point::new(0.0, 0.0));
    collector.finish_dropdown_triggers()
}

struct SelectOverlayCollector<'tree, 'widget, 'entry, Msg> {
    taffy: &'tree TaffyTree<RutterContext>,
    widget_states: &'tree HashMap<u64, WidgetState>,
    viewport: (f32, f32),
    overlays: Vec<SelectOverlay<'entry>>,
    dropdowns: Vec<DropdownOverlay<'widget, Msg>>,
    dropdown_triggers: Vec<DropdownTrigger>,
    path: Vec<usize>,
    clip: Option<SkiaRect>,
    active_owner: OverlayOwner,
    top_owner: OverlayOwner,
    next_order: u64,
}

impl<'tree, 'widget, 'entry: 'widget, Msg> SelectOverlayCollector<'tree, 'widget, 'entry, Msg> {
    fn new(
        taffy: &'tree TaffyTree<RutterContext>,
        widget_states: &'tree HashMap<u64, WidgetState>,
        viewport: (f32, f32),
    ) -> Self {
        Self {
            taffy,
            widget_states,
            viewport,
            overlays: Vec::new(),
            dropdowns: Vec::new(),
            dropdown_triggers: Vec::new(),
            path: Vec::new(),
            clip: Some(SkiaRect::from_xywh(0.0, 0.0, viewport.0, viewport.1)),
            active_owner: OverlayOwner::default(),
            top_owner: OverlayOwner::default(),
            next_order: 0,
        }
    }

    fn finish_selects(mut self) -> Vec<SelectOverlay<'entry>> {
        self.overlays
            .retain(|overlay| overlay.owner == self.top_owner);
        self.overlays
    }

    fn finish_dropdowns(mut self) -> Vec<DropdownOverlay<'widget, Msg>> {
        self.dropdowns
            .retain(|overlay| overlay.owner == self.top_owner);
        self.dropdowns
    }

    fn finish_dropdown_triggers(mut self) -> Vec<DropdownTrigger> {
        self.dropdown_triggers
            .retain(|trigger| trigger.owner == self.top_owner);
        self.dropdown_triggers
    }

    fn visit(&mut self, widget: &'widget Widget<'entry, Msg>, node: NodeId, parent: Point) {
        let Some((absolute, size)) = self.node_frame(node, parent) else {
            return;
        };
        match widget {
            Widget::Select {
                options,
                selected_index,
                ..
            } => self.capture_select(widget, options, *selected_index, absolute, size),
            Widget::DropdownMenu { entries, .. } => {
                self.capture_dropdown(widget, entries, absolute, size)
            }
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                self.visit_children(children, node, absolute)
            }
            Widget::ScrollView { child, .. } => {
                self.visit_scroll(widget, child, node, absolute, size)
            }
            Widget::Accordion {
                expanded, child, ..
            } => self.visit_accordion(*expanded, child, node, absolute),
            Widget::Modal { visible, child, .. } => {
                self.visit_modal(*visible, child, node, absolute, size)
            }
            Widget::Dialog { visible, .. } => self.register_blocking_overlay(*visible, 1),
            Widget::Popover {
                anchor, content, ..
            } => self.visit_popover(widget, anchor, content, node, absolute),
            Widget::Container { child, .. }
            | Widget::Tooltip { child, .. }
            | Widget::ContextMenu { child, .. }
            | Widget::ButtonContent { child, .. } => self.visit_first(child, node, absolute, 0),
            _ => {}
        }
    }

    fn node_frame(&self, node: NodeId, parent: Point) -> Option<(Point, (f32, f32))> {
        let layout = self.taffy.layout(node).ok()?;
        let absolute = Point::new(parent.x + layout.location.x, parent.y + layout.location.y);
        Some((absolute, (layout.size.width, layout.size.height)))
    }

    fn capture_select(
        &mut self,
        widget: &Widget<'entry, Msg>,
        options: &'entry [&'entry str],
        selected_index: usize,
        absolute: Point,
        size: (f32, f32),
    ) {
        let id = widget.resolved_id(&self.path).unwrap();
        let Some(state) = self.widget_states.get(&id).and_then(WidgetState::as_select) else {
            return;
        };
        let anchor = SkiaRect::from_xywh(absolute.x, absolute.y, size.0, size.1);
        if !state.is_open || self.clip.is_some_and(|clip| !rects_overlap(anchor, clip)) {
            return;
        }
        self.overlays.push(SelectOverlay {
            id,
            options,
            selected_index,
            hovered_option: state.hovered_option,
            anchor,
            owner: self.active_owner,
        });
    }

    fn capture_dropdown(
        &mut self,
        widget: &Widget<'entry, Msg>,
        entries: &'widget [DropdownMenuEntry<'entry, Msg>],
        absolute: Point,
        size: (f32, f32),
    ) {
        let id = widget.resolved_id(&self.path).unwrap();
        let anchor = SkiaRect::from_xywh(absolute.x, absolute.y, size.0, size.1);
        if self.clip.is_some_and(|clip| !rects_overlap(anchor, clip)) {
            return;
        }
        let visible_anchor = self
            .clip
            .map_or(anchor, |clip| intersect_rect(anchor, clip));
        self.dropdown_triggers.push(DropdownTrigger {
            id,
            anchor,
            visible_anchor,
            owner: self.active_owner,
        });
        let Some(WidgetState::DropdownMenu(state)) = self.widget_states.get(&id) else {
            return;
        };
        if !state.is_open() {
            return;
        }
        self.dropdowns.push(DropdownOverlay {
            id,
            entries,
            anchor,
            visible_anchor,
            state: state.clone(),
            owner: self.active_owner,
        });
    }

    fn visit_children(
        &mut self,
        children: &'widget [Widget<'entry, Msg>],
        node: NodeId,
        absolute: Point,
    ) {
        let Ok(nodes) = self.taffy.children(node) else {
            return;
        };
        for (index, child) in children.iter().enumerate() {
            if let Some(child_node) = nodes.get(index).copied() {
                self.visit_child(child, child_node, absolute, index);
            }
        }
    }

    fn visit_child(
        &mut self,
        child: &'widget Widget<'entry, Msg>,
        child_node: NodeId,
        parent: Point,
        path_index: usize,
    ) {
        self.path.push(path_index);
        self.visit(child, child_node, parent);
        self.path.pop();
    }

    fn visit_first(
        &mut self,
        child: &'widget Widget<'entry, Msg>,
        node: NodeId,
        parent: Point,
        path_index: usize,
    ) {
        let child_node = self
            .taffy
            .children(node)
            .ok()
            .and_then(|ids| ids.first().copied());
        if let Some(child_node) = child_node {
            self.visit_child(child, child_node, parent, path_index);
        }
    }

    fn visit_scroll(
        &mut self,
        widget: &Widget<'entry, Msg>,
        child: &'widget Widget<'entry, Msg>,
        node: NodeId,
        absolute: Point,
        size: (f32, f32),
    ) {
        let id = widget.resolved_id(&self.path).unwrap();
        let offset = self
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_scroll)
            .map(|state| state.offset_y)
            .unwrap_or(0.0);
        let previous_clip = self.clip;
        let viewport = SkiaRect::from_xywh(absolute.x, absolute.y, size.0, size.1);
        self.clip = intersect_optional(previous_clip, viewport);
        self.visit_first(child, node, Point::new(absolute.x, absolute.y - offset), 0);
        self.clip = previous_clip;
    }

    fn visit_accordion(
        &mut self,
        expanded: bool,
        child: &'widget Widget<'entry, Msg>,
        node: NodeId,
        absolute: Point,
    ) {
        if !expanded {
            return;
        }
        let parent = Point::new(absolute.x, absolute.y + ACCORDION_HEADER_H);
        self.visit_first(child, node, parent, 0);
    }

    fn visit_modal(
        &mut self,
        visible: bool,
        child: &'widget Widget<'entry, Msg>,
        node: NodeId,
        absolute: Point,
        size: (f32, f32),
    ) {
        if !visible {
            return;
        }
        let child_node = self
            .taffy
            .children(node)
            .ok()
            .and_then(|ids| ids.first().copied());
        let Some(child_node) = child_node else {
            return;
        };
        let Ok(child_layout) = self.taffy.layout(child_node) else {
            return;
        };
        let card = modal_card_rect(child_layout.size.height, size);
        let parent = Point::new(absolute.x + card.left, absolute.y + card.top);
        self.visit_overlay_child(child, child_node, parent, 0, 1);
    }

    fn visit_popover(
        &mut self,
        widget: &Widget<'entry, Msg>,
        anchor: &'widget Widget<'entry, Msg>,
        content: &'widget Widget<'entry, Msg>,
        node: NodeId,
        absolute: Point,
    ) {
        let Ok(nodes) = self.taffy.children(node) else {
            return;
        };
        if let Some(anchor_node) = nodes.first().copied() {
            self.visit_child(anchor, anchor_node, absolute, 0);
        }
        let id = widget.resolved_id(&self.path).unwrap();
        let state = self
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_popover)
            .cloned();
        if let Some(state) = state.filter(|state| state.is_open) {
            self.visit_popover_content(content, &nodes, &state);
        }
    }

    fn visit_popover_content(
        &mut self,
        content: &'widget Widget<'entry, Msg>,
        nodes: &[NodeId],
        state: &PopoverState,
    ) {
        let Some((popup_node, content_node)) = self.popover_nodes(nodes) else {
            return;
        };
        let Ok(layout) = self.taffy.layout(popup_node) else {
            return;
        };
        let anchor = SkiaRect::from_xywh(
            state.anchor_x,
            state.anchor_y,
            state.anchor_w,
            state.anchor_h,
        );
        let popup = popover_rect(
            anchor,
            (layout.size.width, layout.size.height),
            self.viewport,
        );
        let previous_clip = self.clip;
        self.clip = intersect_optional(previous_clip, popup);
        self.visit_overlay_child(
            content,
            content_node,
            Point::new(popup.left, popup.top),
            1,
            2,
        );
        self.clip = previous_clip;
    }

    fn popover_nodes(&self, nodes: &[NodeId]) -> Option<(NodeId, NodeId)> {
        let popup_node = nodes.get(1).copied()?;
        let content_node = self.taffy.children(popup_node).ok()?.first().copied()?;
        Some((popup_node, content_node))
    }

    fn visit_overlay_child(
        &mut self,
        child: &'widget Widget<'entry, Msg>,
        child_node: NodeId,
        parent: Point,
        path_index: usize,
        minimum_phase: u8,
    ) {
        let previous_owner = self.active_owner;
        self.active_owner = self.register_overlay(minimum_phase);
        self.visit_child(child, child_node, parent, path_index);
        self.active_owner = previous_owner;
    }

    fn register_blocking_overlay(&mut self, visible: bool, minimum_phase: u8) {
        if visible {
            self.register_overlay(minimum_phase);
        }
    }

    fn register_overlay(&mut self, minimum_phase: u8) -> OverlayOwner {
        self.next_order = self.next_order.saturating_add(1);
        let owner = OverlayOwner {
            phase: self.active_owner.phase.max(minimum_phase),
            order: self.next_order,
        };
        self.top_owner = self.top_owner.max(owner);
        owner
    }
}

fn rects_overlap(left: SkiaRect, right: SkiaRect) -> bool {
    left.left < right.right
        && left.right > right.left
        && left.top < right.bottom
        && left.bottom > right.top
}

fn intersect_optional(existing: Option<SkiaRect>, next: SkiaRect) -> Option<SkiaRect> {
    let Some(existing) = existing else {
        return Some(next);
    };
    Some(intersect_rect(existing, next))
}

fn intersect_rect(existing: SkiaRect, next: SkiaRect) -> SkiaRect {
    SkiaRect::new(
        existing.left.max(next.left),
        existing.top.max(next.top),
        existing.right.min(next.right),
        existing.bottom.min(next.bottom),
    )
}
