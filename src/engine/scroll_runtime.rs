//! Viewport geometry shared by retained popovers and focus reveal. Layout nodes
//! stay in document coordinates; this pass applies live scroll offsets without
//! rebuilding layout or changing widget paths.

use std::collections::{HashMap, HashSet};

use skia_safe::{Point, Rect};
use taffy::prelude::{NodeId, TaffyTree};

use super::widget_state::{
    WidgetState, normalize_virtual_grid_columns, virtual_grid_cell_left, virtual_grid_cell_width,
};
use crate::i18n::LayoutDirection;
use crate::layout::{
    RutterContext, SCROLLBAR_W, VIRTUAL_GRID_GAP, build_layout_tree, compute_layout_with_renderer,
};
use crate::render::RichTextRenderer;
use crate::render::hit_test::{
    dialog_card_rect, interactive_carousel_card_rect, modal_card_rect, popover_rect,
    virtual_item_physical_size,
};
use crate::widget::{
    KeyedVirtualItems, Widget, pop_interactive_virtual_item_path,
    push_interactive_virtual_item_path,
};
use crate::widgets::carousel::geometry::carousel_item_frames;
use crate::widgets::table_of_contents::layout_nodes;

#[derive(Debug, Default)]
pub(super) struct ViewportGeometry {
    pub pointer_regions: HashMap<u64, Rect>,
    pub sliders: HashMap<u64, Rect>,
    // Painting uses is_open. Keep logically open, clipped popovers separately so
    // they can reappear on scroll and still absorb outside clicks/Escape.
    pub hidden_popovers: HashSet<u64>,
    pub hidden_focus: HashSet<u64>,
    pub focused_input_rect: Option<Rect>,
    // Nearest live scroll ancestor; portaled branches do not bubble to the page.
    pub focused_scroll: Option<u64>,
}

pub(super) fn reconcile<Msg>(
    widget: &Widget<'_, Msg>,
    layout: (&TaffyTree<RutterContext>, NodeId, &RichTextRenderer),
    states: &mut HashMap<u64, WidgetState>,
    geometry: &mut ViewportGeometry,
    viewport: (f32, f32),
    focus: Option<u64>,
    reveal: bool,
) -> Vec<u64> {
    let (taffy, root, renderer) = layout;
    // Collapsed/hidden layout branches may have no child nodes at all. Start
    // suppressed, then restore only popovers reached through visible geometry.
    for (&id, state) in states.iter_mut() {
        if let Some(popover) = state.as_popover_mut()
            && popover.is_open
        {
            geometry.hidden_popovers.insert(id);
            popover.set_open(false);
        }
    }
    let mut pass = GeometryPass {
        taffy,
        renderer,
        states,
        geometry,
        viewport,
        focus,
        reveal,
        path: Vec::new(),
        changed_scrolls: Vec::new(),
    };
    pass.geometry.hidden_focus.clear();
    pass.geometry.pointer_regions.clear();
    pass.geometry.sliders.clear();
    pass.geometry.focused_input_rect = None;
    pass.geometry.focused_scroll = None;
    pass.visit(
        widget,
        root,
        Point::new(0.0, 0.0),
        Rect::from_xywh(0.0, 0.0, viewport.0, viewport.1),
        true,
    );
    let changed = pass.changed_scrolls;
    // Reveal is inside-out. Reconcile every anchor and input frame again after
    // the outermost ancestor moved, including siblings visited before the target.
    if !changed.is_empty() {
        reconcile(
            widget,
            (taffy, root, renderer),
            states,
            geometry,
            viewport,
            focus,
            false,
        );
    }
    changed
}

struct GeometryPass<'a> {
    taffy: &'a TaffyTree<RutterContext>,
    renderer: &'a RichTextRenderer,
    states: &'a mut HashMap<u64, WidgetState>,
    geometry: &'a mut ViewportGeometry,
    viewport: (f32, f32),
    focus: Option<u64>,
    reveal: bool,
    path: Vec<usize>,
    changed_scrolls: Vec<u64>,
}

impl GeometryPass<'_> {
    fn visit<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        node: NodeId,
        parent: Point,
        clip: Rect,
        visible: bool,
    ) -> Option<Rect> {
        let layout = *self.taffy.layout(node).ok()?;
        let absolute = Point::new(parent.x + layout.location.x, parent.y + layout.location.y);
        let frame = Rect::from_xywh(
            absolute.x,
            absolute.y,
            layout.size.width,
            layout.size.height,
        );
        let mut target = self.capture_focus(widget, frame, clip, visible);
        match widget {
            Widget::Disabled { child } => {
                self.visit(child, node, parent, clip, false);
            }
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                let nodes = self.taffy.children(node).ok()?;
                for (index, (child, node)) in children.iter().zip(nodes).enumerate() {
                    target = self
                        .child(child, node, absolute, clip, visible, index)
                        .or(target);
                }
            }
            Widget::InteractiveVirtualListContent { .. }
            | Widget::InteractiveVirtualGridContent { .. }
            | Widget::InteractiveCarouselView { .. } => {
                target = self
                    .virtual_collection(widget, node, frame, clip, visible)
                    .or(target);
            }
            Widget::ScrollView { child, .. } => {
                let id = widget.resolved_id(&self.path)?;
                let offset = self.offset(id);
                let child_node = self.first(node)?;
                let child_target = self.child(
                    child,
                    child_node,
                    Point::new(absolute.x, absolute.y - offset),
                    intersection(clip, frame),
                    visible,
                    0,
                );
                target = self.reveal_scroll(id, frame, child_target).or(target);
            }
            Widget::TableOfContents { child, .. } => {
                let nodes = layout_nodes(self.taffy, node)?;
                let viewport = self.taffy.layout(nodes.viewport).ok()?;
                let frame = Rect::from_xywh(
                    absolute.x + viewport.location.x,
                    absolute.y + viewport.location.y,
                    viewport.size.width,
                    viewport.size.height,
                );
                let id = widget.resolved_id(&self.path)?;
                let child_target = self.child(
                    child,
                    nodes.content,
                    Point::new(frame.left, frame.top - self.offset(id)),
                    intersection(clip, frame),
                    visible,
                    0,
                );
                target = self.reveal_scroll(id, frame, child_target).or(target);
            }
            Widget::Popover {
                anchor, content, ..
            } => {
                let nodes = self.taffy.children(node).ok()?;
                let anchor_node = *nodes.first()?;
                let anchor_layout = self.taffy.layout(anchor_node).ok()?;
                let anchor_frame = Rect::from_xywh(
                    absolute.x + anchor_layout.location.x,
                    absolute.y + anchor_layout.location.y,
                    anchor_layout.size.width,
                    anchor_layout.size.height,
                );
                let id = widget.resolved_id(&self.path)?;
                let was_hidden = self.geometry.hidden_popovers.remove(&id);
                let state = self
                    .states
                    .get_mut(&id)
                    .and_then(WidgetState::as_popover_mut)?;
                let open = state.is_open || was_hidden;
                state.set_anchor_rect(
                    anchor_frame.left,
                    anchor_frame.top,
                    anchor_frame.width(),
                    anchor_frame.height(),
                );
                let shown = open && visible && !intersection(clip, anchor_frame).is_empty();
                state.set_open(shown);
                if open && !shown {
                    self.geometry.hidden_popovers.insert(id);
                }
                target = self
                    .child(anchor, anchor_node, absolute, clip, visible, 0)
                    .or(target);
                if let Some(popup_node) = nodes.get(1).copied() {
                    let popup = self.taffy.layout(popup_node).ok()?;
                    let rect = popover_rect(
                        anchor_frame,
                        (popup.size.width, popup.size.height),
                        self.viewport,
                    );
                    if let Some(content_node) = self.first(popup_node) {
                        // Portaled content owns a new coordinate/scroll scope.
                        self.child(
                            content,
                            content_node,
                            Point::new(rect.left, rect.top),
                            rect,
                            shown,
                            1,
                        );
                    }
                }
            }
            Widget::Modal {
                visible: shown,
                child,
                ..
            }
            | Widget::Dialog {
                visible: shown,
                child,
                ..
            } => {
                let child_node = self.first(node)?;
                let child_layout = self.taffy.layout(child_node).ok()?;
                let card = match widget {
                    Widget::Dialog { position, .. } => {
                        dialog_card_rect(*position, (frame.width(), frame.height()))
                    }
                    _ => modal_card_rect(child_layout.size.height, (frame.width(), frame.height())),
                };
                let card = card.with_offset((absolute.x, absolute.y));
                self.child(
                    child,
                    child_node,
                    Point::new(card.left, card.top),
                    card,
                    visible && *shown,
                    0,
                );
            }
            Widget::Accordion {
                child, expanded, ..
            } => {
                target = self
                    .child(
                        child,
                        self.first(node)?,
                        absolute,
                        clip,
                        visible && *expanded,
                        0,
                    )
                    .or(target);
            }
            Widget::Container { child, radius, .. } => {
                let clip = if *radius > 0.0 {
                    intersection(clip, frame)
                } else {
                    clip
                };
                target = self
                    .child(child, self.first(node)?, absolute, clip, visible, 0)
                    .or(target);
            }
            Widget::PointerRegion { child, config, .. } => {
                if visible && config.on_hover.is_some() {
                    self.geometry
                        .pointer_regions
                        .insert(widget.resolved_id(&self.path)?, intersection(clip, frame));
                }
                target = self
                    .child(child, self.first(node)?, absolute, clip, visible, 0)
                    .or(target);
            }
            Widget::Tooltip { child, .. } | Widget::ContextMenu { child, .. } => {
                target = self
                    .child(child, self.first(node)?, absolute, clip, visible, 0)
                    .or(target);
            }
            Widget::TabBar { tabs, .. } => {
                for index in 0..tabs.len() {
                    if let Some(id) = widget.tab_focus_id(&self.path, index) {
                        if !visible {
                            self.geometry.hidden_focus.insert(id);
                        }
                        if visible && Some(id) == self.focus {
                            target = Some(frame);
                        }
                    }
                }
            }
            _ => {}
        }
        target
    }

    fn virtual_collection<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        node: NodeId,
        frame: Rect,
        clip: Rect,
        visible: bool,
    ) -> Option<Rect> {
        let id = widget.resolved_id(&self.path)?;
        let direction = match self.taffy.style(node).ok()?.direction {
            taffy::Direction::Ltr => LayoutDirection::Ltr,
            taffy::Direction::Rtl => LayoutDirection::Rtl,
        };
        let (items, frames): (_, Vec<_>) = match widget {
            Widget::InteractiveVirtualListContent {
                items, item_height, ..
            } => {
                let state = self.states.get(&id)?.as_vlist()?;
                let (first, last) = state.visible_range(*item_height, items.len());
                let frames = (first..last)
                    .map(|index| {
                        (
                            index,
                            Rect::from_xywh(
                                frame.left,
                                frame.top + index as f32 * item_height - state.scroll_y,
                                (frame.width() - SCROLLBAR_W - 4.0).max(0.0),
                                *item_height,
                            ),
                        )
                    })
                    .collect();
                (items, frames)
            }
            Widget::InteractiveVirtualGridContent {
                items,
                item_height,
                columns,
                ..
            } => {
                let state = self.states.get(&id)?.as_vgrid()?;
                let (first, last) = state.visible_row_range(*item_height, items.len(), *columns);
                let columns = normalize_virtual_grid_columns(*columns);
                let frames = (first * columns..(last * columns).min(items.len()))
                    .map(|index| {
                        (
                            index,
                            Rect::from_xywh(
                                frame.left
                                    + virtual_grid_cell_left(
                                        index % columns,
                                        frame.width(),
                                        columns,
                                    ),
                                frame.top + (index / columns) as f32 * item_height - state.scroll_y
                                    + VIRTUAL_GRID_GAP * 0.5,
                                virtual_grid_cell_width(frame.width(), columns),
                                (*item_height - VIRTUAL_GRID_GAP).max(12.0),
                            ),
                        )
                    })
                    .collect();
                (items, frames)
            }
            Widget::InteractiveCarouselView { items, config, .. } => {
                let state = self.states.get(&id)?.as_carousel()?;
                let frames = carousel_item_frames(
                    config,
                    state.position,
                    frame.width(),
                    items.len(),
                    direction,
                )
                .into_iter()
                .map(|item| {
                    (
                        item.index,
                        interactive_carousel_card_rect(item, frame.height())
                            .with_offset((frame.left, frame.top)),
                    )
                })
                .collect();
                (items, frames)
            }
            _ => return None,
        };
        let clip = intersection(clip, frame);
        let mut target = None;
        for (index, frame) in frames {
            target = self
                .virtual_item(items, index, frame, clip, visible, direction)
                .or(target);
        }
        target
    }

    #[allow(clippy::too_many_arguments)]
    fn virtual_item<Msg>(
        &mut self,
        items: &KeyedVirtualItems<'_, Msg>,
        index: usize,
        frame: Rect,
        clip: Rect,
        visible: bool,
        direction: LayoutDirection,
    ) -> Option<Rect> {
        let key = items.key_at(index)?;
        let item = items.build_item(index)?;
        // Virtual children have no nodes in the retained tree. Rebuild the same
        // item-local layout used by painting, then apply its live viewport origin
        // and keyed identity scope; press-time track geometry is never reused.
        let mut taffy = TaffyTree::new();
        let root = build_layout_tree(&mut taffy, &item, direction);
        compute_layout_with_renderer(
            &mut taffy,
            root,
            virtual_item_physical_size((frame.width(), frame.height())),
            self.renderer,
        );
        push_interactive_virtual_item_path(&mut self.path, key);
        let mut pass = GeometryPass {
            taffy: &taffy,
            renderer: self.renderer,
            states: self.states,
            geometry: self.geometry,
            viewport: self.viewport,
            focus: self.focus,
            reveal: self.reveal,
            path: self.path.clone(),
            changed_scrolls: Vec::new(),
        };
        let clip = intersection(clip, frame);
        let target = pass.visit(
            &item,
            root,
            Point::new(frame.left, frame.top),
            clip,
            visible && !clip.is_empty(),
        );
        self.changed_scrolls.extend(pass.changed_scrolls);
        pop_interactive_virtual_item_path(&mut self.path);
        target.map(|target| intersection(target, clip))
    }

    fn capture_focus<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        frame: Rect,
        clip: Rect,
        visible: bool,
    ) -> Option<Rect> {
        let id = widget.keyboard_focus_id(&self.path)?;
        if visible
            && matches!(widget, Widget::Slider { .. } | Widget::SliderWith { .. })
            && !intersection(frame, clip).is_empty()
        {
            // Clipping gates capture, but must not shorten the value-mapping track.
            self.geometry.sliders.insert(id, frame);
        }
        if !visible {
            self.geometry.hidden_focus.insert(id);
            return None;
        }
        if Some(id) != self.focus {
            return None;
        }
        if matches!(
            widget,
            Widget::TextInput { .. }
                | Widget::TextInputWith { .. }
                | Widget::TextArea { .. }
                | Widget::TextAreaWith { .. }
                | Widget::SearchBar { .. }
                | Widget::ComboBox { .. }
        ) {
            let body = crate::layout::field::FieldInsets::for_widget(widget)
                .body((frame.width(), frame.height()));
            self.geometry.focused_input_rect = Some(body.with_offset((frame.left, frame.top)));
        }
        Some(frame)
    }

    fn first(&self, node: NodeId) -> Option<NodeId> {
        self.taffy.children(node).ok()?.first().copied()
    }

    fn child<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        node: NodeId,
        parent: Point,
        clip: Rect,
        visible: bool,
        index: usize,
    ) -> Option<Rect> {
        self.path.push(index);
        let target = self.visit(widget, node, parent, clip, visible);
        self.path.pop();
        target
    }

    fn offset(&self, id: u64) -> f32 {
        self.states
            .get(&id)
            .and_then(WidgetState::as_scroll)
            .map_or(0.0, |state| state.offset_y)
    }

    fn reveal_scroll(&mut self, id: u64, viewport: Rect, target: Option<Rect>) -> Option<Rect> {
        let mut target = target?;
        self.geometry.focused_scroll.get_or_insert(id);
        if self.reveal {
            let state = self
                .states
                .get_mut(&id)
                .and_then(WidgetState::as_scroll_mut)?;
            let delta = if target.top < viewport.top || target.height() > viewport.height() {
                target.top - viewport.top
            } else {
                (target.bottom - viewport.bottom).max(0.0)
            };
            let previous = state.offset_y;
            state.scroll_by(delta);
            let moved = state.offset_y - previous;
            if moved != 0.0 {
                self.changed_scrolls.push(id);
            }
            target.offset((0.0, -moved));
        }
        // Only the visible portion inside the inner viewport needs revealing
        // in an outer scroll ancestor (also handles oversized controls).
        Some(intersection(target, viewport))
    }
}

fn intersection(left: Rect, right: Rect) -> Rect {
    Rect::new(
        left.left.max(right.left),
        left.top.max(right.top),
        left.right.min(right.right),
        left.bottom.min(right.bottom),
    )
}
