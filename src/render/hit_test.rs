// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — render/hit_test.rs
// ============================================================

use cosmic_text::FontSystem;
use skia_safe::{Contains, Point, Rect as SkiaRect};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use taffy::Direction;
use taffy::prelude::{NodeId, TaffyTree};

use crate::app::{ContextMenuTarget, ContextMenuVirtualItem};
use crate::engine::widget_state::{
    VirtualGridState, WidgetState, normalize_virtual_grid_columns, virtual_grid_cell_left,
    virtual_grid_cell_width, virtual_grid_row_count,
};
use crate::i18n::LayoutDirection;
use crate::layout::{
    RutterContext, SCROLLBAR_W, VIRTUAL_GRID_GAP, build_taffy_tree_with_direction, compute_layout,
};
use crate::render::RichTextRenderer;
use crate::render::counter::{CounterSegment, counter_segment_at};
use crate::widget::{
    CONTEXT_MENU_ITEM_H, CONTEXT_MENU_PAD_Y, CONTEXT_MENU_SEPARATOR_H,
    CONTEXT_MENU_VIEWPORT_MARGIN, ContextMenuEntry, DialogAction, DialogPosition, POPOVER_GAP,
    POPOVER_VIEWPORT_MARGIN, Widget, estimate_context_menu_height, estimate_context_menu_width,
    pop_interactive_virtual_item_path, push_interactive_virtual_item_path,
};
use crate::widgets::carousel::geometry::carousel_item_frames;
use crate::widgets::table::{
    TableCellTarget, TableHit, TableLayoutDirection, TableModel, TableOptions, TableSelection,
    TableState, allocate_column_widths, calculate_viewport, hit_test as hit_test_table,
    minimum_content_width,
};
use crate::widgets::table_of_contents::{entry_offset_y, entry_rects, layout_nodes, title_rect};
use winit::dpi::PhysicalSize;

const ACCORDION_HEADER_H: f32 = 44.0;
const MODAL_MAX_CARD_W: f32 = 480.0;
const MODAL_MIN_CARD_H: f32 = 200.0;
const DIALOG_CARD_W: f32 = 400.0;
const DIALOG_CARD_H: f32 = 200.0;
const DIALOG_VIEWPORT_MARGIN: f32 = 32.0;

pub enum HitResult<Msg> {
    Message {
        focus_id: Option<u64>,
        msg: Msg,
    },
    InputFocus {
        id: u64,
        local_x: f32,
        local_y: f32,
        width: f32,
        height: f32,
    },
    SliderPress {
        id: u64,
        cursor_x: f32,
        abs_track_x: f32,
        track_w: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    CounterAdjust {
        id: u64,
        increment: bool,
    },
    CounterFocus(u64),
    SelectToggle(u64),
    DropdownMenuToggle(u64),
    SelectOption {
        id: u64,
        index: usize,
    },
    SearchSuggestion {
        id: u64,
        /// Original position inside the suggestions item slice.
        index: usize,
    },
    ScrollFocus(u64),
    TabPress {
        id: u64,
        focus_id: u64,
        index: usize,
    },
    ModalDismiss(u64),
    VListSelect {
        id: u64,
        index: usize,
    },
    VGridSelect {
        id: u64,
        index: usize,
    },
    CarouselSelect {
        id: u64,
        index: usize,
    },
    TableOfContentsActivate {
        id: u64,
        index: usize,
        focus_id: u64,
        target_y: f32,
    },
    TableActivate {
        id: u64,
        target: TableCellTarget,
    },
    CustomPointer {
        id: u64,
        position: Point,
        bounds: (f32, f32),
        focuses_keyboard: bool,
    },
    PointerRegion(u64),
}

pub type InputChangeCallback<Msg> = fn(String) -> Msg;
pub type InputCallbacks<Msg> = (InputChangeCallback<Msg>, Option<Msg>);
pub type InputProperties<Msg> = (InputChangeCallback<Msg>, Option<Msg>, bool, bool);

#[derive(Debug, Clone, Copy)]
pub struct ScrollbarDragHit {
    pub id: u64,
    pub axis: ScrollbarAxis,
    pub reversed: bool,
    pub start_offset: f32,
    pub viewport_extent: f32,
    pub content_extent: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy)]
struct ScrollbarMetrics {
    current_offset: f32,
    viewport_h: f32,
    content_h: f32,
    thumb_y: f32,
    thumb_h: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct InputGeometry {
    pub width: f32,
    pub height: f32,
}

pub enum ContextMenuOverlayHit<Msg> {
    Item { id: u64, msg: Msg },
    Consume,
    Dismiss,
}

pub enum PopoverOverlayHit<Msg> {
    Content(HitResult<Msg>),
    Consume,
    Dismiss { id: u64, on_dismiss: Option<Msg> },
}

pub(crate) fn context_menu_rect<Msg>(
    entries: &[ContextMenuEntry<'_, Msg>],
    anchor: Point,
    viewport_size: (f32, f32),
    font_size: f32,
) -> SkiaRect {
    let width = estimate_context_menu_width(entries, font_size)
        .min((viewport_size.0 - CONTEXT_MENU_VIEWPORT_MARGIN * 2.0).max(1.0));
    let height = (estimate_context_menu_height(entries) + CONTEXT_MENU_PAD_Y * 2.0)
        .min((viewport_size.1 - CONTEXT_MENU_VIEWPORT_MARGIN * 2.0).max(1.0));
    let x = anchor.x.clamp(
        CONTEXT_MENU_VIEWPORT_MARGIN,
        (viewport_size.0 - width - CONTEXT_MENU_VIEWPORT_MARGIN).max(CONTEXT_MENU_VIEWPORT_MARGIN),
    );
    let y = anchor.y.clamp(
        CONTEXT_MENU_VIEWPORT_MARGIN,
        (viewport_size.1 - height - CONTEXT_MENU_VIEWPORT_MARGIN).max(CONTEXT_MENU_VIEWPORT_MARGIN),
    );
    SkiaRect::from_xywh(x, y, width, height)
}

pub(crate) fn popover_rect(
    anchor_rect: SkiaRect,
    popup_size: (f32, f32),
    viewport_size: (f32, f32),
) -> SkiaRect {
    let width = popup_size
        .0
        .min((viewport_size.0 - POPOVER_VIEWPORT_MARGIN * 2.0).max(1.0))
        .max(1.0);
    let height = popup_size
        .1
        .min((viewport_size.1 - POPOVER_VIEWPORT_MARGIN * 2.0).max(1.0))
        .max(1.0);
    let max_x = (viewport_size.0 - width - POPOVER_VIEWPORT_MARGIN).max(POPOVER_VIEWPORT_MARGIN);
    let x = anchor_rect.left.clamp(POPOVER_VIEWPORT_MARGIN, max_x);
    let below_y = anchor_rect.bottom + POPOVER_GAP;
    let above_y = anchor_rect.top - height - POPOVER_GAP;
    let y = if below_y + height + POPOVER_VIEWPORT_MARGIN <= viewport_size.1 {
        below_y
    } else {
        above_y
    }
    .clamp(
        POPOVER_VIEWPORT_MARGIN,
        (viewport_size.1 - height - POPOVER_VIEWPORT_MARGIN).max(POPOVER_VIEWPORT_MARGIN),
    );
    SkiaRect::from_xywh(x, y, width, height)
}

pub(crate) fn dialog_card_rect(position: DialogPosition, viewport_size: (f32, f32)) -> SkiaRect {
    let width = DIALOG_CARD_W
        .min((viewport_size.0 - DIALOG_VIEWPORT_MARGIN * 2.0).max(1.0))
        .max(1.0);
    let height = DIALOG_CARD_H
        .min((viewport_size.1 - DIALOG_VIEWPORT_MARGIN * 2.0).max(1.0))
        .max(1.0);
    let max_x = (viewport_size.0 - width - DIALOG_VIEWPORT_MARGIN)
        .max(DIALOG_VIEWPORT_MARGIN.min(viewport_size.0));
    let x =
        ((viewport_size.0 - width) / 2.0).clamp(DIALOG_VIEWPORT_MARGIN.min(viewport_size.0), max_x);
    let y = match position {
        DialogPosition::Top => DIALOG_VIEWPORT_MARGIN,
        DialogPosition::Center => (viewport_size.1 - height) / 2.0,
        DialogPosition::Bottom => viewport_size.1 - height - DIALOG_VIEWPORT_MARGIN,
    }
    .clamp(
        DIALOG_VIEWPORT_MARGIN.min(viewport_size.1),
        (viewport_size.1 - height - DIALOG_VIEWPORT_MARGIN)
            .max(DIALOG_VIEWPORT_MARGIN.min(viewport_size.1)),
    );
    SkiaRect::from_xywh(x, y, width, height)
}

pub(crate) fn modal_card_rect(content_height: f32, viewport_size: (f32, f32)) -> SkiaRect {
    let requested_width = viewport_size.0 * 0.85;
    let width = if requested_width.is_nan() {
        // Preserve the previous min/max fallback for malformed viewport widths.
        MODAL_MAX_CARD_W
    } else {
        requested_width.clamp(1.0, MODAL_MAX_CARD_W)
    };
    let height = content_height
        .max(MODAL_MIN_CARD_H)
        .min((viewport_size.1 * 0.9).max(1.0));
    let x = (viewport_size.0 - width) / 2.0;
    let y = (viewport_size.1 - height) / 2.0;
    SkiaRect::from_xywh(x, y, width, height)
}

pub fn hit_test_context_menu_overlay<Msg: Clone>(
    widget: &Widget<Msg>,
    mouse: Point,
    viewport_size: (f32, f32),
    widget_states: &HashMap<u64, WidgetState>,
    font_size: f32,
) -> Option<ContextMenuOverlayHit<Msg>> {
    let mut path = Vec::new();
    let mut any_open = false;
    let hit = hit_test_context_menu_overlay_impl(
        widget,
        mouse,
        viewport_size,
        widget_states,
        font_size,
        &mut path,
        &mut any_open,
    );
    hit.or_else(|| any_open.then_some(ContextMenuOverlayHit::Dismiss))
}

fn hit_test_context_menu_overlay_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    mouse: Point,
    viewport_size: (f32, f32),
    widget_states: &HashMap<u64, WidgetState>,
    font_size: f32,
    path: &mut Vec<usize>,
    any_open: &mut bool,
) -> Option<ContextMenuOverlayHit<Msg>> {
    match widget {
        Widget::ContextMenu { child, entries, .. } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let menu_state = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_context_menu());
            if let Some(state) = menu_state
                && state.is_open
            {
                *any_open = true;
                let rect = context_menu_rect(
                    entries,
                    Point::new(state.anchor_x, state.anchor_y),
                    viewport_size,
                    font_size,
                );
                if rect.contains(mouse) {
                    let mut y = rect.top + CONTEXT_MENU_PAD_Y;
                    for entry in entries.iter() {
                        let item_h = match entry {
                            ContextMenuEntry::Item { .. } => CONTEXT_MENU_ITEM_H,
                            ContextMenuEntry::Separator => CONTEXT_MENU_SEPARATOR_H,
                        };
                        let item_rect = SkiaRect::from_xywh(rect.left, y, rect.width(), item_h);
                        if item_rect.contains(mouse) {
                            return match entry {
                                ContextMenuEntry::Item {
                                    on_select: Some(msg),
                                    ..
                                } => Some(ContextMenuOverlayHit::Item {
                                    id: resolved_id,
                                    msg: msg.clone(),
                                }),
                                _ => Some(ContextMenuOverlayHit::Consume),
                            };
                        }
                        y += item_h;
                    }
                    return Some(ContextMenuOverlayHit::Consume);
                }
            }
            path.push(0);
            let hit = hit_test_context_menu_overlay_impl(
                child,
                mouse,
                viewport_size,
                widget_states,
                font_size,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate().rev() {
                path.push(index);
                let hit = hit_test_context_menu_overlay_impl(
                    child,
                    mouse,
                    viewport_size,
                    widget_states,
                    font_size,
                    path,
                    any_open,
                );
                path.pop();
                if hit.is_some() {
                    return hit;
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::PointerRegion { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. } => {
            path.push(0);
            let hit = hit_test_context_menu_overlay_impl(
                child,
                mouse,
                viewport_size,
                widget_states,
                font_size,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if !expanded {
                return None;
            }
            path.push(0);
            let hit = hit_test_context_menu_overlay_impl(
                child,
                mouse,
                viewport_size,
                widget_states,
                font_size,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        Widget::Modal { visible, child, .. } | Widget::Dialog { visible, child, .. } => {
            if !visible {
                return None;
            }
            path.push(0);
            let hit = hit_test_context_menu_overlay_impl(
                child,
                mouse,
                viewport_size,
                widget_states,
                font_size,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        _ => None,
    }
}

pub fn hit_test_popover_overlay<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    viewport_size: (f32, f32),
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<PopoverOverlayHit<Msg>> {
    let mut path = Vec::new();
    let mut any_open = false;
    let hit = hit_test_popover_overlay_impl(
        widget,
        taffy,
        node_id,
        mouse,
        Point::new(0.0, 0.0),
        viewport_size,
        widget_states,
        &mut path,
        &mut any_open,
    );
    hit.or_else(|| {
        any_open.then_some(PopoverOverlayHit::Dismiss {
            id: 0,
            on_dismiss: None,
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn hit_test_popover_overlay_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    viewport_size: (f32, f32),
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
    any_open: &mut bool,
) -> Option<PopoverOverlayHit<Msg>> {
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    match widget {
        Widget::Popover {
            anchor,
            content,
            on_dismiss,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let node_children = taffy.children(node_id).ok()?;
            let popover = widget_states
                .get(&resolved_id)
                .and_then(|state| state.as_popover());
            if let Some(popover) = popover
                && popover.is_open
            {
                *any_open = true;
                if let Some(popup_node) = node_children.get(1).copied()
                    && let Ok(popup_layout) = taffy.layout(popup_node)
                {
                    let anchor_rect = SkiaRect::from_xywh(
                        popover.anchor_x,
                        popover.anchor_y,
                        popover.anchor_w,
                        popover.anchor_h,
                    );
                    let rect = popover_rect(
                        anchor_rect,
                        (popup_layout.size.width, popup_layout.size.height),
                        viewport_size,
                    );
                    if rect.contains(mouse) {
                        if let Some(content_node) = taffy
                            .children(popup_node)
                            .ok()
                            .and_then(|ids| ids.first().copied())
                        {
                            path.push(1);
                            let nested = hit_test_popover_overlay_impl(
                                content,
                                taffy,
                                content_node,
                                mouse,
                                Point::new(rect.left, rect.top),
                                viewport_size,
                                widget_states,
                                path,
                                any_open,
                            );
                            if nested.is_some() {
                                path.pop();
                                return nested;
                            }
                            let content_hit = hit_test_impl(
                                content,
                                taffy,
                                content_node,
                                mouse,
                                Point::new(rect.left, rect.top),
                                widget_states,
                                path,
                            );
                            path.pop();
                            return Some(match content_hit {
                                Some(hit) => PopoverOverlayHit::Content(hit),
                                None => PopoverOverlayHit::Consume,
                            });
                        }
                        return Some(PopoverOverlayHit::Consume);
                    }
                }
            }

            if let Some(anchor_node) = node_children.first().copied() {
                path.push(0);
                let hit = hit_test_popover_overlay_impl(
                    anchor,
                    taffy,
                    anchor_node,
                    mouse,
                    abs_pos,
                    viewport_size,
                    widget_states,
                    path,
                    any_open,
                );
                path.pop();
                if hit.is_some() {
                    return hit;
                }
            }

            if popover.is_some_and(|state| state.is_open) {
                return Some(PopoverOverlayHit::Dismiss {
                    id: resolved_id,
                    on_dismiss: on_dismiss.clone(),
                });
            }
            None
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (index, child) in children.iter().enumerate().rev() {
                path.push(index);
                let hit = hit_test_popover_overlay_impl(
                    child,
                    taffy,
                    ids[index],
                    mouse,
                    abs_pos,
                    viewport_size,
                    widget_states,
                    path,
                    any_open,
                );
                path.pop();
                if hit.is_some() {
                    return hit;
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let child_node = ids.first().copied()?;
            path.push(0);
            let hit = hit_test_popover_overlay_impl(
                child,
                taffy,
                child_node,
                mouse,
                abs_pos,
                viewport_size,
                widget_states,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        Widget::TableOfContents { child, .. } => {
            let nodes = layout_nodes(taffy, node_id)?;
            let viewport = taffy.layout(nodes.viewport).ok()?;
            let table_id = widget.resolved_id(path)?;
            let offset_y = widget_states
                .get(&table_id)
                .and_then(WidgetState::as_scroll)
                .map(|state| state.offset_y)
                .unwrap_or(0.0);
            path.push(0);
            let hit = hit_test_popover_overlay_impl(
                child,
                taffy,
                nodes.content,
                mouse,
                Point::new(
                    abs_pos.x + viewport.location.x,
                    abs_pos.y + viewport.location.y - offset_y,
                ),
                viewport_size,
                widget_states,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if !expanded {
                return None;
            }
            let ids = taffy.children(node_id).ok()?;
            let child_node = ids.first().copied()?;
            path.push(0);
            let hit = hit_test_popover_overlay_impl(
                child,
                taffy,
                child_node,
                mouse,
                abs_pos,
                viewport_size,
                widget_states,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        Widget::Modal { visible, child, .. } | Widget::Dialog { visible, child, .. } => {
            if !visible {
                return None;
            }
            let ids = taffy.children(node_id).ok()?;
            let child_node = ids.first().copied()?;
            path.push(0);
            let hit = hit_test_popover_overlay_impl(
                child,
                taffy,
                child_node,
                mouse,
                abs_pos,
                viewport_size,
                widget_states,
                path,
                any_open,
            );
            path.pop();
            hit
        }
        _ => None,
    }
}

pub fn hit_test<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<HitResult<Msg>> {
    let mut path = Vec::new();
    hit_test_impl(widget, taffy, node_id, mouse, abs, widget_states, &mut path)
}

fn hit_test_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<HitResult<Msg>> {
    let layout = taffy.layout(node_id).unwrap();
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::Button { on_press, .. } | Widget::ButtonContent { on_press, .. } => {
            Some(HitResult::Message {
                focus_id: widget.keyboard_focus_id(path),
                msg: on_press.clone(),
            })
        }
        Widget::TextInput { .. } | Widget::TextArea { .. } | Widget::SearchBar { .. } => {
            Some(HitResult::InputFocus {
                id: widget.resolved_id(path).unwrap(),
                local_x: mouse.x - abs_pos.x,
                local_y: mouse.y - abs_pos.y,
                width: layout.size.width,
                height: layout.size.height,
            })
        }
        Widget::Checkbox {
            checked, on_change, ..
        } => Some(HitResult::Message {
            focus_id: widget.keyboard_focus_id(path),
            msg: on_change(!checked),
        }),
        Widget::Switch {
            checked, on_change, ..
        } => Some(HitResult::Message {
            focus_id: widget.keyboard_focus_id(path),
            msg: on_change(!checked),
        }),
        Widget::Radio { on_select, .. } => Some(HitResult::Message {
            focus_id: widget.keyboard_focus_id(path),
            msg: on_select(),
        }),
        Widget::Slider { min, max, step, .. } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let pad = 16.0_f32;
            let track_x = abs_pos.x + pad;
            let track_w = layout.size.width - pad * 2.0;
            Some(HitResult::SliderPress {
                id: resolved_id,
                cursor_x: mouse.x,
                abs_track_x: track_x,
                track_w,
                min: *min,
                max: *max,
                step: *step,
            })
        }
        Widget::Counter { .. } => {
            let id = widget.resolved_id(path).unwrap();
            let local = Point::new(mouse.x - abs_pos.x, mouse.y - abs_pos.y);
            match counter_segment_at((layout.size.width, layout.size.height), local) {
                CounterSegment::Decrement => Some(HitResult::CounterAdjust {
                    id,
                    increment: false,
                }),
                CounterSegment::Increment => Some(HitResult::CounterAdjust {
                    id,
                    increment: true,
                }),
                CounterSegment::Value => Some(HitResult::CounterFocus(id)),
            }
        }
        Widget::Select { .. } => Some(HitResult::SelectToggle(widget.resolved_id(path).unwrap())),
        Widget::DropdownMenu { .. } => Some(HitResult::DropdownMenuToggle(
            widget.resolved_id(path).unwrap(),
        )),
        Widget::Custom { widget: custom, .. } if custom.interaction().supports_pointer() => {
            Some(HitResult::CustomPointer {
                id: widget.resolved_id(path).unwrap(),
                position: Point::new(mouse.x - abs_pos.x, mouse.y - abs_pos.y),
                bounds: (layout.size.width, layout.size.height),
                focuses_keyboard: custom.interaction().supports_keyboard(),
            })
        }
        Widget::PointerRegion { .. } => {
            Some(HitResult::PointerRegion(widget.resolved_id(path).unwrap()))
        }
        Widget::ScrollView { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            let offset_y = widget_states
                .get(&widget.resolved_id(path).unwrap())
                .and_then(WidgetState::as_scroll)
                .map(|state| state.offset_y)
                .unwrap_or(0.0);
            path.push(0);
            let child_hit = hit_test_impl(
                child,
                taffy,
                ids[0],
                mouse,
                Point::new(abs_pos.x, abs_pos.y - offset_y),
                widget_states,
                path,
            );
            path.pop();
            if let Some(result) = child_hit {
                return Some(result);
            }
            Some(HitResult::ScrollFocus(widget.resolved_id(path).unwrap()))
        }
        Widget::TableOfContents { child, .. } => table_of_contents_hit(
            widget,
            child,
            taffy,
            node_id,
            mouse,
            abs_pos,
            widget_states,
            path,
        ),
        Widget::Table { model, options, .. } => table_hit(
            widget,
            model,
            options,
            taffy,
            node_id,
            mouse,
            abs_pos,
            widget_states,
            path,
        ),
        Widget::Tooltip { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            path.push(0);
            let result = hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            result
        }
        Widget::ContextMenu { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let result = hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            result
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node_id).unwrap();
            let anchor_node = ids.first().copied()?;
            path.push(0);
            let result = hit_test_impl(
                anchor,
                taffy,
                anchor_node,
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            result
        }
        Widget::Accordion {
            child,
            expanded,
            on_toggle,
            ..
        } => {
            let header_rect = SkiaRect::from_xywh(
                abs_pos.x,
                abs_pos.y,
                layout.size.width,
                ACCORDION_HEADER_H.min(layout.size.height),
            );
            if header_rect.contains(mouse) {
                return Some(HitResult::Message {
                    focus_id: widget.keyboard_focus_id(path),
                    msg: on_toggle.clone(),
                });
            }
            if *expanded {
                let ids = taffy.children(node_id).unwrap();
                if ids.is_empty() {
                    return None;
                }
                path.push(0);
                // The child node already includes the Accordion header padding in its Taffy location.
                let result =
                    hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
                path.pop();
                return result;
            }
            None
        }
        Widget::TabBar { tabs, .. } => {
            if tabs.is_empty() {
                return None;
            }
            let tab_w = layout.size.width / tabs.len() as f32;
            let idx = ((mouse.x - abs_pos.x) / tab_w).floor() as usize;
            let idx = idx.min(tabs.len().saturating_sub(1));
            Some(HitResult::TabPress {
                id: widget.resolved_id(path).unwrap(),
                focus_id: widget.tab_focus_id(path, idx).unwrap(),
                index: idx,
            })
        }
        Widget::Modal {
            visible,
            child,
            on_dismiss,
            ..
        } => {
            if !visible {
                return None;
            }
            let ids = taffy.children(node_id).unwrap();
            let child_layout = taffy.layout(ids[0]).unwrap();
            let card = modal_card_rect(
                child_layout.size.height,
                (layout.size.width, layout.size.height),
            );
            let abs_card = SkiaRect::from_xywh(
                abs_pos.x + card.left,
                abs_pos.y + card.top,
                card.width(),
                card.height(),
            );
            path.push(0);
            let child_hit = hit_test_impl(
                child,
                taffy,
                ids[0],
                mouse,
                Point::new(abs_card.left, abs_card.top),
                widget_states,
                path,
            );
            path.pop();
            if child_hit.is_some() {
                return child_hit;
            }
            if abs_card.contains(mouse) {
                return None;
            }
            if let Some(msg) = on_dismiss.clone() {
                Some(HitResult::Message {
                    focus_id: None,
                    msg,
                })
            } else {
                Some(HitResult::ModalDismiss(widget.resolved_id(path).unwrap()))
            }
        }
        Widget::Dialog {
            visible,
            on_confirm,
            on_cancel,
            on_dismiss,
            position,
            ..
        } => {
            if !visible {
                return None;
            }
            let card = dialog_card_rect(*position, (layout.size.width, layout.size.height));
            let cancel_w = 100.0;
            let confirm_w = 100.0;
            let btn_h = 36.0;
            let cancel_rect = SkiaRect::from_xywh(
                abs_pos.x + card.right - 24.0 - confirm_w - 12.0 - cancel_w,
                abs_pos.y + card.bottom - 24.0 - btn_h,
                cancel_w,
                btn_h,
            );
            let confirm_rect = SkiaRect::from_xywh(
                abs_pos.x + card.right - 24.0 - confirm_w,
                abs_pos.y + card.bottom - 24.0 - btn_h,
                confirm_w,
                btn_h,
            );

            if confirm_rect.contains(mouse) {
                return Some(HitResult::Message {
                    focus_id: widget.dialog_action_focus_id(path, DialogAction::Confirm),
                    msg: on_confirm.clone(),
                });
            }
            if cancel_rect.contains(mouse) {
                return Some(HitResult::Message {
                    focus_id: widget.dialog_action_focus_id(path, DialogAction::Cancel),
                    msg: on_cancel.clone(),
                });
            }
            let abs_card = SkiaRect::from_xywh(
                abs_pos.x + card.left,
                abs_pos.y + card.top,
                card.width(),
                card.height(),
            );
            if abs_card.contains(mouse) {
                return None;
            }
            if let Some(msg) = on_dismiss.clone() {
                Some(HitResult::Message {
                    focus_id: None,
                    msg,
                })
            } else {
                Some(HitResult::ModalDismiss(widget.resolved_id(path).unwrap()))
            }
        }
        Widget::CarouselView {
            item_count, config, ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let direction = carousel_node_direction(taffy, node_id);
            let mut fallback = crate::widgets::carousel::CarouselState::default();
            fallback.sync_viewport(layout.size.width, config, *item_count);
            let state = widget_states
                .get(&resolved_id)
                .and_then(WidgetState::as_carousel)
                .unwrap_or(&fallback);
            state
                .index_at(mouse.x - abs_pos.x, config, *item_count, direction)
                .map(|index| HitResult::CarouselSelect {
                    id: resolved_id,
                    index,
                })
        }
        Widget::InteractiveCarouselView { items, config, .. } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let direction = carousel_node_direction(taffy, node_id);
            let mut fallback = crate::widgets::carousel::CarouselState::default();
            fallback.sync_viewport(layout.size.width, config, items.len());
            let state = widget_states
                .get(&resolved_id)
                .and_then(WidgetState::as_carousel)
                .unwrap_or(&fallback);
            let frames = carousel_item_frames(
                config,
                state.position,
                layout.size.width,
                items.len(),
                direction,
            );
            let hit = frames.iter().find_map(|frame| {
                let rect = interactive_carousel_card_rect(*frame, layout.size.height);
                if !SkiaRect::from_xywh(
                    abs_pos.x + rect.left,
                    abs_pos.y + rect.top,
                    rect.width(),
                    rect.height(),
                )
                .contains(mouse)
                {
                    return None;
                }
                let item = items.build_item(frame.index)?;
                let key = items.key_at(frame.index)?;
                push_interactive_virtual_item_path(path, key);
                let result = hit_interactive_virtual_item(
                    &item,
                    (rect.width(), rect.height()),
                    Point::new(abs_pos.x + rect.left, abs_pos.y + rect.top),
                    direction,
                    mouse,
                    widget_states,
                    path,
                );
                pop_interactive_virtual_item_path(path);
                result
            });
            hit.or_else(|| {
                state
                    .index_at(mouse.x - abs_pos.x, config, items.len(), direction)
                    .map(|index| HitResult::CarouselSelect {
                        id: resolved_id,
                        index,
                    })
            })
        }
        Widget::VirtualList {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContent {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListWithSelection {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContentWithSelection {
            item_height,
            item_count,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let scroll_y = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_vlist())
                .map(|v| v.scroll_y)
                .unwrap_or(0.0);
            virtual_list_item_index_at(mouse.y - abs_pos.y, *item_height, *item_count, scroll_y)
                .map(|index| HitResult::VListSelect {
                    id: resolved_id,
                    index,
                })
        }
        Widget::InteractiveVirtualListContent {
            item_height, items, ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let scroll_y = widget_states
                .get(&resolved_id)
                .and_then(WidgetState::as_vlist)
                .map(|state| state.scroll_y)
                .unwrap_or(0.0);
            let index = virtual_list_item_index_at(
                mouse.y - abs_pos.y,
                *item_height,
                items.len(),
                scroll_y,
            );
            let child_hit = index.and_then(|index| {
                let item = items.build_item(index)?;
                let key = items.key_at(index)?;
                let origin =
                    Point::new(abs_pos.x, abs_pos.y + index as f32 * item_height - scroll_y);
                let size = (
                    (layout.size.width - SCROLLBAR_W - 4.0).max(0.0),
                    *item_height,
                );
                let bounds = SkiaRect::from_xywh(origin.x, origin.y, size.0, size.1);
                if !bounds.contains(mouse) {
                    return None;
                }
                push_interactive_virtual_item_path(path, key);
                let result = hit_interactive_virtual_item(
                    &item,
                    size,
                    origin,
                    carousel_node_direction(taffy, node_id),
                    mouse,
                    widget_states,
                    path,
                );
                pop_interactive_virtual_item_path(path);
                result
            });
            child_hit.or(index.map(|index| HitResult::VListSelect {
                id: resolved_id,
                index,
            }))
        }
        Widget::VirtualGrid {
            columns,
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualGridContent {
            columns,
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualGridWithSelection {
            columns,
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualGridContentWithSelection {
            columns,
            item_height,
            item_count,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let grid_state = widget_states
                .get(&resolved_id)
                .and_then(WidgetState::as_vgrid);
            virtual_grid_item_index_at(
                grid_state,
                Point::new(mouse.x - abs_pos.x, mouse.y - abs_pos.y),
                (layout.size.width, layout.size.height),
                *item_height,
                *item_count,
                *columns,
            )
            .map(|index| HitResult::VGridSelect {
                id: resolved_id,
                index,
            })
        }
        Widget::InteractiveVirtualGridContent {
            columns,
            item_height,
            items,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let grid_state = widget_states
                .get(&resolved_id)
                .and_then(WidgetState::as_vgrid);
            let index = virtual_grid_item_index_at(
                grid_state,
                Point::new(mouse.x - abs_pos.x, mouse.y - abs_pos.y),
                (layout.size.width, layout.size.height),
                *item_height,
                items.len(),
                *columns,
            );
            let child_hit = index.and_then(|index| {
                let item = items.build_item(index)?;
                let key = items.key_at(index)?;
                let scroll_y = grid_state.map(|state| state.scroll_y).unwrap_or(0.0);
                let columns = normalize_virtual_grid_columns(*columns);
                let row = index / columns;
                let cell_h = (*item_height - VIRTUAL_GRID_GAP).max(12.0);
                let origin = Point::new(
                    abs_pos.x + virtual_grid_cell_left(index % columns, layout.size.width, columns),
                    abs_pos.y + row as f32 * item_height - scroll_y + VIRTUAL_GRID_GAP * 0.5,
                );
                let size = (virtual_grid_cell_width(layout.size.width, columns), cell_h);
                let bounds = SkiaRect::from_xywh(origin.x, origin.y, size.0, size.1);
                if !bounds.contains(mouse) {
                    return None;
                }
                push_interactive_virtual_item_path(path, key);
                let result = hit_interactive_virtual_item(
                    &item,
                    size,
                    origin,
                    carousel_node_direction(taffy, node_id),
                    mouse,
                    widget_states,
                    path,
                );
                pop_interactive_virtual_item_path(path);
                result
            });
            child_hit.or(index.map(|index| HitResult::VGridSelect {
                id: resolved_id,
                index,
            }))
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).unwrap();
            for (i, child) in children.iter().enumerate().rev() {
                path.push(i);
                let result =
                    hit_test_impl(child, taffy, ids[i], mouse, abs_pos, widget_states, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, radius, .. } => {
            if !rounded_rect_contains(rect, *radius, mouse) {
                return None;
            }
            let ids = taffy.children(node_id).unwrap();
            path.push(0);
            let result = hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            result
        }
        _ => None,
    }
}

fn hit_interactive_virtual_item<Msg: Clone>(
    item: &Widget<Msg>,
    size: (f32, f32),
    origin: Point,
    direction: LayoutDirection,
    mouse: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<HitResult<Msg>> {
    let mut item_taffy = TaffyTree::new();
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let root = build_taffy_tree_with_direction(
        &mut item_taffy,
        item,
        fonts.clone(),
        widget_states,
        direction,
    );
    compute_layout(
        &mut item_taffy,
        root,
        virtual_item_physical_size(size),
        fonts,
        &RichTextRenderer::default(),
    );
    hit_test_impl(item, &item_taffy, root, mouse, origin, widget_states, path)
}

fn virtual_item_physical_size(size: (f32, f32)) -> PhysicalSize<u32> {
    PhysicalSize::new(size.0.max(1.0).ceil() as u32, size.1.max(1.0).ceil() as u32)
}

fn interactive_carousel_card_rect(
    frame: crate::widgets::carousel::geometry::CarouselItemFrame,
    viewport_height: f32,
) -> SkiaRect {
    const GAP: f32 = 8.0;
    let horizontal = (GAP * 0.5).min(frame.width * 0.2);
    let vertical = (GAP * 0.5).min(viewport_height * 0.2);
    SkiaRect::from_xywh(
        frame.x + horizontal,
        vertical,
        (frame.width - horizontal * 2.0).max(1.0),
        (viewport_height - vertical * 2.0).max(1.0),
    )
}

#[allow(clippy::too_many_arguments)]
fn table_hit<Msg>(
    widget: &Widget<Msg>,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs_pos: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &[usize],
) -> Option<HitResult<Msg>> {
    let table_id = widget.resolved_id(path)?;
    let layout = taffy.layout(node_id).ok()?;
    let state = widget_states.get(&table_id).and_then(WidgetState::as_table);
    let (viewport, widths, scroll, direction) = table_geometry(
        model,
        options,
        (layout.size.width, layout.size.height),
        state,
        table_node_direction(taffy, node_id),
    );
    let point = (mouse.x - abs_pos.x, mouse.y - abs_pos.y);
    match hit_test_table(
        point,
        &widths,
        model.rows().len(),
        viewport,
        scroll,
        direction,
        options.metrics(),
    )? {
        TableHit::Header { column } => table_header_hit(widget, model, options, path, column),
        TableHit::Cell { row, column } => table_cell_hit(widget, model, options, path, row, column),
        TableHit::HorizontalScrollbar | TableHit::VerticalScrollbar | TableHit::EmptyState => {
            Some(HitResult::ScrollFocus(table_id))
        }
    }
}

fn table_header_hit<Msg>(
    widget: &Widget<Msg>,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    path: &[usize],
    column: usize,
) -> Option<HitResult<Msg>> {
    let column = model.columns().get(column)?;
    if options.sorting().is_none() || !column.is_sortable() {
        return Some(HitResult::ScrollFocus(widget.resolved_id(path)?));
    }
    Some(HitResult::TableActivate {
        id: widget.resolved_id(path)?,
        target: TableCellTarget::header(column.key()),
    })
}

fn table_cell_hit<Msg>(
    widget: &Widget<Msg>,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    path: &[usize],
    row: usize,
    column: usize,
) -> Option<HitResult<Msg>> {
    let row = model.rows().get(row)?;
    let column = model.columns().get(column)?;
    if matches!(options.selection(), TableSelection::None) {
        return Some(HitResult::ScrollFocus(widget.resolved_id(path)?));
    }
    Some(HitResult::TableActivate {
        id: widget.resolved_id(path)?,
        target: TableCellTarget::body(row.key(), column.key()),
    })
}

fn table_geometry<Msg>(
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    size: (f32, f32),
    state: Option<&TableState>,
    direction: TableLayoutDirection,
) -> (
    crate::widgets::table::geometry::TableViewport,
    Vec<f32>,
    (f32, f32),
    TableLayoutDirection,
) {
    let viewport = calculate_viewport(
        size,
        options.metrics(),
        model.rows().len(),
        minimum_content_width(model.columns()),
    );
    let widths = allocate_column_widths(model.columns(), viewport.body.width);
    let scroll = state
        .map(|state| (state.scroll_x, state.scroll_y))
        .unwrap_or_default();
    (viewport, widths, scroll, direction)
}

fn table_node_direction(taffy: &TaffyTree<RutterContext>, node_id: NodeId) -> TableLayoutDirection {
    match taffy.style(node_id).map(|style| style.direction) {
        Ok(Direction::Rtl) => TableLayoutDirection::Rtl,
        _ => TableLayoutDirection::Ltr,
    }
}

#[allow(clippy::too_many_arguments)]
fn table_of_contents_hit<Msg: Clone>(
    widget: &Widget<Msg>,
    child: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    table_abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<HitResult<Msg>> {
    let table_id = widget.resolved_id(path)?;
    if let Some(hit) =
        table_of_contents_accordion_hit(widget, taffy, node_id, table_abs, path, mouse)
    {
        return Some(hit);
    }
    if let Some(index) = table_of_contents_entry_index(taffy, node_id, table_abs, mouse) {
        let target_y = layout_nodes(taffy, node_id)
            .and_then(|nodes| entry_offset_y(child, taffy, nodes.content, index))
            .unwrap_or(0.0);
        return Some(HitResult::TableOfContentsActivate {
            id: table_id,
            index,
            focus_id: widget.table_of_contents_entry_focus_id(path, index)?,
            target_y,
        });
    }
    table_of_contents_content_hit(
        child,
        taffy,
        node_id,
        mouse,
        table_abs,
        table_id,
        widget_states,
        path,
    )
}

fn table_of_contents_accordion_hit<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    table_abs: Point,
    path: &[usize],
    mouse: Point,
) -> Option<HitResult<Msg>> {
    let on_toggle = widget.table_of_contents_accordion()?.on_toggle().clone();
    let header = title_rect(taffy, node_id)?;
    if !table_entry_contains(header, table_abs, mouse) {
        return None;
    }
    Some(HitResult::Message {
        focus_id: widget.table_of_contents_accordion_focus_id(path),
        msg: on_toggle,
    })
}

fn table_of_contents_entry_index(
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    table_abs: Point,
    mouse: Point,
) -> Option<usize> {
    let nodes = layout_nodes(taffy, node_id)?;
    let navigation = taffy.layout(nodes.navigation).ok()?;
    let navigation_rect = SkiaRect::from_xywh(
        table_abs.x + navigation.location.x,
        table_abs.y + navigation.location.y,
        navigation.size.width,
        navigation.size.height,
    );
    if !navigation_rect.contains(mouse) {
        return None;
    }
    entry_rects(taffy, node_id)
        .into_iter()
        .position(|rect| table_entry_contains(rect, table_abs, mouse))
}

fn table_entry_contains(rect: SkiaRect, table_abs: Point, mouse: Point) -> bool {
    SkiaRect::from_xywh(
        rect.left + table_abs.x,
        rect.top + table_abs.y,
        rect.width(),
        rect.height(),
    )
    .contains(mouse)
}

#[allow(clippy::too_many_arguments)]
fn table_of_contents_content_hit<Msg: Clone>(
    child: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    table_abs: Point,
    table_id: u64,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<HitResult<Msg>> {
    let nodes = layout_nodes(taffy, node_id)?;
    let viewport = taffy.layout(nodes.viewport).ok()?;
    let viewport_abs = Point::new(
        table_abs.x + viewport.location.x,
        table_abs.y + viewport.location.y,
    );
    let viewport_rect = SkiaRect::from_xywh(
        viewport_abs.x,
        viewport_abs.y,
        viewport.size.width,
        viewport.size.height,
    );
    if !viewport_rect.contains(mouse) {
        return None;
    }
    let offset_y = widget_states
        .get(&table_id)
        .and_then(WidgetState::as_scroll)
        .map(|state| state.offset_y)
        .unwrap_or(0.0);
    path.push(0);
    let hit = hit_test_impl(
        child,
        taffy,
        nodes.content,
        mouse,
        Point::new(viewport_abs.x, viewport_abs.y - offset_y),
        widget_states,
        path,
    );
    path.pop();
    hit.or(Some(HitResult::ScrollFocus(table_id)))
}

fn carousel_node_direction(taffy: &TaffyTree<RutterContext>, node_id: NodeId) -> LayoutDirection {
    match taffy.style(node_id).map(|style| style.direction) {
        Ok(Direction::Rtl) => LayoutDirection::Rtl,
        _ => LayoutDirection::Ltr,
    }
}

fn rounded_rect_contains(rect: SkiaRect, radius: f32, point: Point) -> bool {
    if !rect.contains(point) {
        return false;
    }
    if !radius.is_finite() || radius <= 0.0 {
        return true;
    }
    let radius = radius.min(rect.width().min(rect.height()) * 0.5);
    let x = point.x - rect.left;
    let y = point.y - rect.top;
    if (radius..=rect.width() - radius).contains(&x)
        || (radius..=rect.height() - radius).contains(&y)
    {
        return true;
    }
    let center_x = if x < radius {
        radius
    } else {
        rect.width() - radius
    };
    let center_y = if y < radius {
        radius
    } else {
        rect.height() - radius
    };
    (x - center_x).powi(2) + (y - center_y).powi(2) <= radius.powi(2)
}

pub fn collect_input_ids<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>) {
    let mut path = Vec::new();
    collect_input_ids_impl(widget, ids, &mut path);
}

pub(crate) fn collect_input_ids_at_path<Msg>(
    widget: &Widget<Msg>,
    ids: &mut Vec<u64>,
    path: &mut Vec<usize>,
) {
    collect_input_ids_impl(widget, ids, path);
}

fn collect_input_ids_impl<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>, path: &mut Vec<usize>) {
    match widget {
        Widget::TextInput { .. } | Widget::TextArea { .. } | Widget::SearchBar { .. } => {
            ids.push(widget.resolved_id(path).unwrap());
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_input_ids_impl(child, ids, path);
                path.pop();
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            collect_input_ids_impl(child, ids, path);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            collect_input_ids_impl(anchor, ids, path);
            path.pop();
            if *open {
                path.push(1);
                collect_input_ids_impl(content, ids, path);
                path.pop();
            }
        }
        _ => {}
    }
}

pub fn collect_stateful_ids<Msg>(widget: &Widget<Msg>, out: &mut Vec<(u64, &'static str)>) {
    let mut path = Vec::new();
    collect_stateful_ids_impl(widget, out, &mut path);
}

pub(crate) fn collect_stateful_ids_at_path<Msg>(
    widget: &Widget<Msg>,
    ids: &mut Vec<(u64, &'static str)>,
    path: &mut Vec<usize>,
) {
    collect_stateful_ids_impl(widget, ids, path);
}

fn collect_stateful_ids_impl<Msg>(
    widget: &Widget<Msg>,
    out: &mut Vec<(u64, &'static str)>,
    path: &mut Vec<usize>,
) {
    match widget {
        Widget::Slider { .. } => out.push((widget.resolved_id(path).unwrap(), "slider")),
        Widget::Select { .. } => out.push((widget.resolved_id(path).unwrap(), "select")),
        Widget::SearchBar {
            suggestions: Some(_),
            ..
        } => {
            // Only bars with an attached suggestion source own runtime state;
            // the plain input needs none.
            out.push((widget.resolved_id(path).unwrap(), "search"))
        }
        Widget::DropdownMenu { .. } => {
            out.push((widget.resolved_id(path).unwrap(), "dropdown_menu"))
        }
        Widget::Spinner { .. } => out.push((widget.resolved_id(path).unwrap(), "anim")),
        Widget::ScrollView { child, .. } => {
            out.push((widget.resolved_id(path).unwrap(), "scroll"));
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
        Widget::TableOfContents { child, .. } => {
            let table_id = widget.resolved_id(path).unwrap();
            out.push((table_id, "scroll"));
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
        Widget::Table { .. } => out.push((widget.resolved_id(path).unwrap(), "table")),
        Widget::ProgressBar {
            indeterminate: true,
            ..
        } => out.push((widget.resolved_id(path).unwrap(), "anim")),
        Widget::TabBar { .. } => out.push((widget.resolved_id(path).unwrap(), "tab")),
        Widget::Toast { .. } => out.push((widget.resolved_id(path).unwrap(), "toast")),
        Widget::ContextMenu { child, .. } => {
            out.push((widget.resolved_id(path).unwrap(), "context_menu"));
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            out.push((widget.resolved_id(path).unwrap(), "popover"));
            path.push(0);
            collect_stateful_ids_impl(anchor, out, path);
            path.pop();
            if *open {
                path.push(1);
                collect_stateful_ids_impl(content, out, path);
                path.pop();
            }
        }
        Widget::VirtualList { .. }
        | Widget::VirtualListContent { .. }
        | Widget::InteractiveVirtualListContent { .. }
        | Widget::VirtualListWithSelection { .. }
        | Widget::VirtualListContentWithSelection { .. } => {
            out.push((widget.resolved_id(path).unwrap(), "vlist"))
        }
        Widget::CarouselView { .. } | Widget::InteractiveCarouselView { .. } => {
            out.push((widget.resolved_id(path).unwrap(), "carousel"))
        }
        Widget::VirtualGrid { .. }
        | Widget::VirtualGridContent { .. }
        | Widget::InteractiveVirtualGridContent { .. }
        | Widget::VirtualGridWithSelection { .. }
        | Widget::VirtualGridContentWithSelection { .. } => {
            out.push((widget.resolved_id(path).unwrap(), "vgrid"))
        }
        Widget::Modal { child, .. } => {
            out.push((widget.resolved_id(path).unwrap(), "modal"));
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
        Widget::Dialog { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::Container { child, .. }
        | Widget::PointerRegion { child, .. }
        | Widget::Tooltip { child, .. } => {
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_stateful_ids_impl(child, out, path);
                path.pop();
            }
        }
        _ => {}
    }
}

pub fn find_input_callbacks<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<InputCallbacks<Msg>> {
    find_input_props(widget, target_id).map(|(cb, submit, _, _)| (cb, submit))
}

pub fn find_input_props<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<InputProperties<Msg>> {
    let mut path = Vec::new();
    find_input_props_impl(widget, target_id, &mut path)
}

fn find_input_props_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<InputProperties<Msg>> {
    match widget {
        Widget::TextInput {
            on_change,
            on_submit,
            is_password,
            ..
        } if widget.resolved_id(path) == Some(target_id) => {
            Some((*on_change, on_submit.clone(), *is_password, false))
        }
        Widget::TextArea {
            on_change,
            on_submit,
            ..
        } if widget.resolved_id(path) == Some(target_id) => {
            Some((*on_change, on_submit.clone(), false, true))
        }
        Widget::SearchBar {
            on_change,
            on_submit,
            ..
        } if widget.resolved_id(path) == Some(target_id) => {
            Some((*on_change, on_submit.clone(), false, false))
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                let result = find_input_props_impl(child, target_id, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::PointerRegion { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_input_props_impl(child, target_id, path);
            path.pop();
            result
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            let result = find_input_props_impl(anchor, target_id, path);
            path.pop();
            if result.is_some() {
                return result;
            }
            if *open {
                path.push(1);
                let result = find_input_props_impl(content, target_id, path);
                path.pop();
                result
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn find_select_callback<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<fn(usize) -> Msg> {
    let mut path = Vec::new();
    find_select_callback_impl(widget, target_id, &mut path)
}

fn find_select_callback_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<fn(usize) -> Msg> {
    match widget {
        Widget::Select { on_change, .. } if widget.resolved_id(path) == Some(target_id) => {
            Some(*on_change)
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                let result = find_select_callback_impl(child, target_id, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_select_callback_impl(child, target_id, path);
            path.pop();
            result
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            let result = find_select_callback_impl(anchor, target_id, path);
            path.pop();
            if result.is_some() {
                return result;
            }
            if *open {
                path.push(1);
                let result = find_select_callback_impl(content, target_id, path);
                path.pop();
                result
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn find_slider_callback<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<fn(f32) -> Msg> {
    let mut path = Vec::new();
    find_slider_callback_impl(widget, target_id, &mut path)
}

fn find_slider_callback_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<fn(f32) -> Msg> {
    match widget {
        Widget::Slider { on_change, .. } if widget.resolved_id(path) == Some(target_id) => {
            Some(*on_change)
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                let result = find_slider_callback_impl(child, target_id, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_slider_callback_impl(child, target_id, path);
            path.pop();
            result
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            let result = find_slider_callback_impl(anchor, target_id, path);
            path.pop();
            if result.is_some() {
                return result;
            }
            if *open {
                path.push(1);
                let result = find_slider_callback_impl(content, target_id, path);
                path.pop();
                result
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn find_vlist_props<Msg>(widget: &Widget<Msg>, target_id: u64) -> Option<(f32, usize)> {
    let mut path = Vec::new();
    find_vlist_props_impl(widget, target_id, &mut path)
}

fn find_vlist_props_impl<Msg>(
    widget: &Widget<Msg>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<(f32, usize)> {
    match widget {
        Widget::VirtualList {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContent {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListWithSelection {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContentWithSelection {
            item_height,
            item_count,
            ..
        } if widget.resolved_id(path) == Some(target_id) => Some((*item_height, *item_count)),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                let result = find_vlist_props_impl(child, target_id, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_vlist_props_impl(child, target_id, path);
            path.pop();
            result
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            let result = find_vlist_props_impl(anchor, target_id, path);
            path.pop();
            if result.is_some() {
                return result;
            }
            if *open {
                path.push(1);
                let result = find_vlist_props_impl(content, target_id, path);
                path.pop();
                result
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn find_toast_dismiss_msg<Msg: Clone>(widget: &Widget<Msg>, target_id: u64) -> Option<Msg> {
    let mut path = Vec::new();
    find_toast_dismiss_msg_impl(widget, target_id, &mut path)
}

fn find_toast_dismiss_msg_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<Msg> {
    match widget {
        Widget::Toast { on_dismiss, .. } if widget.resolved_id(path) == Some(target_id) => {
            on_dismiss.clone()
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                let result = find_toast_dismiss_msg_impl(child, target_id, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::TableOfContents { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_toast_dismiss_msg_impl(child, target_id, path);
            path.pop();
            result
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            let result = find_toast_dismiss_msg_impl(anchor, target_id, path);
            path.pop();
            if result.is_some() {
                return result;
            }
            if *open {
                path.push(1);
                let result = find_toast_dismiss_msg_impl(content, target_id, path);
                path.pop();
                result
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn find_input_geometry<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    target_id: u64,
) -> Option<InputGeometry> {
    let mut path = Vec::new();
    find_input_geometry_impl(widget, taffy, node_id, target_id, &mut path)
}

fn find_input_geometry_impl<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<InputGeometry> {
    match widget {
        Widget::TextInput { .. } | Widget::TextArea { .. } | Widget::SearchBar { .. }
            if widget.resolved_id(path) == Some(target_id) =>
        {
            let layout = taffy.layout(node_id).ok()?;
            Some(InputGeometry {
                width: layout.size.width,
                height: layout.size.height,
            })
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate() {
                path.push(i);
                let result = find_input_geometry_impl(child, taffy, ids[i], target_id, path);
                path.pop();
                if let Some(r) = result {
                    return Some(r);
                }
            }
            None
        }
        Widget::TableOfContents { child, .. } => {
            let nodes = layout_nodes(taffy, node_id)?;
            path.push(0);
            let result = find_input_geometry_impl(child, taffy, nodes.content, target_id, path);
            path.pop();
            result
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let result = find_input_geometry_impl(child, taffy, ids[0], target_id, path);
            path.pop();
            result
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            let ids = taffy.children(node_id).ok()?;
            if let Some(anchor_node) = ids.first().copied() {
                path.push(0);
                let result = find_input_geometry_impl(anchor, taffy, anchor_node, target_id, path);
                path.pop();
                if result.is_some() {
                    return result;
                }
            }
            if *open
                && let Some(popup_node) = ids.get(1).copied()
                && let Some(content_node) = taffy
                    .children(popup_node)
                    .ok()
                    .and_then(|ids| ids.first().copied())
            {
                path.push(1);
                let result =
                    find_input_geometry_impl(content, taffy, content_node, target_id, path);
                path.pop();
                return result;
            }
            None
        }
        _ => None,
    }
}

pub fn find_scroll_focus<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<u64> {
    let mut path = Vec::new();
    find_scroll_focus_impl(widget, taffy, node_id, mouse, abs, widget_states, &mut path)
}

fn virtual_list_item_index_at(
    local_y: f32,
    item_height: f32,
    item_count: usize,
    scroll_y: f32,
) -> Option<usize> {
    if item_height <= 0.0 || item_count == 0 {
        return None;
    }
    let index = ((local_y + scroll_y) / item_height).floor() as usize;
    (index < item_count).then_some(index)
}

fn virtual_grid_item_index_at(
    state: Option<&VirtualGridState>,
    local_mouse: Point,
    viewport_size: (f32, f32),
    item_height: f32,
    item_count: usize,
    columns: usize,
) -> Option<usize> {
    let fallback = VirtualGridState {
        viewport_w: viewport_size.0,
        viewport_h: viewport_size.1,
        ..Default::default()
    };
    state.unwrap_or(&fallback).index_at(
        local_mouse.x,
        local_mouse.y,
        item_height,
        item_count,
        columns,
    )
}

enum ContextMenuTargetSearchResult {
    Menu(ContextMenuTarget),
    VirtualItem(ContextMenuVirtualItem),
}

pub fn find_context_menu_target<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
) -> Option<u64> {
    let widget_states = HashMap::new();
    find_context_menu_target_with_metadata(widget, taffy, node_id, mouse, abs, &widget_states)
        .map(ContextMenuTarget::id)
}

pub(crate) fn find_context_menu_target_with_metadata<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<ContextMenuTarget> {
    let mut path = Vec::new();
    match find_context_menu_target_impl(
        widget,
        taffy,
        node_id,
        mouse,
        abs,
        widget_states,
        &mut path,
    )? {
        ContextMenuTargetSearchResult::Menu(target) => Some(target),
        ContextMenuTargetSearchResult::VirtualItem(_) => None,
    }
}

fn context_menu_target_from_child(
    menu_id: u64,
    child_result: Option<ContextMenuTargetSearchResult>,
) -> ContextMenuTargetSearchResult {
    match child_result {
        Some(ContextMenuTargetSearchResult::Menu(target)) => {
            ContextMenuTargetSearchResult::Menu(target)
        }
        Some(ContextMenuTargetSearchResult::VirtualItem(item)) => {
            ContextMenuTargetSearchResult::Menu(ContextMenuTarget::from_virtual_item(menu_id, item))
        }
        None => ContextMenuTargetSearchResult::Menu(ContextMenuTarget::from_resolved_id(menu_id)),
    }
}

fn find_context_menu_target_impl<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<ContextMenuTargetSearchResult> {
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::ContextMenu { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let menu_id = widget.resolved_id(path).unwrap();
            let mut child_result = None;
            if !ids.is_empty() {
                path.push(0);
                child_result = find_context_menu_target_impl(
                    child,
                    taffy,
                    ids[0],
                    mouse,
                    abs_pos,
                    widget_states,
                    path,
                );
                path.pop();
            }
            Some(context_menu_target_from_child(menu_id, child_result))
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let anchor_node = ids.first().copied()?;
            path.push(0);
            let hit = find_context_menu_target_impl(
                anchor,
                taffy,
                anchor_node,
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            hit
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate().rev() {
                path.push(i);
                let hit = find_context_menu_target_impl(
                    child,
                    taffy,
                    ids[i],
                    mouse,
                    abs_pos,
                    widget_states,
                    path,
                );
                path.pop();
                if hit.is_some() {
                    return hit;
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let hit = find_context_menu_target_impl(
                child,
                taffy,
                ids[0],
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            hit
        }
        Widget::TableOfContents { child, .. } => {
            let table_id = widget.resolved_id(path)?;
            let nodes = layout_nodes(taffy, node_id)?;
            let viewport = taffy.layout(nodes.viewport).ok()?;
            let viewport_abs = Point::new(
                abs_pos.x + viewport.location.x,
                abs_pos.y + viewport.location.y,
            );
            if !SkiaRect::from_xywh(
                viewport_abs.x,
                viewport_abs.y,
                viewport.size.width,
                viewport.size.height,
            )
            .contains(mouse)
            {
                return None;
            }
            let offset_y = widget_states
                .get(&table_id)
                .and_then(WidgetState::as_scroll)
                .map(|state| state.offset_y)
                .unwrap_or(0.0);
            path.push(0);
            let hit = find_context_menu_target_impl(
                child,
                taffy,
                nodes.content,
                mouse,
                Point::new(viewport_abs.x, viewport_abs.y - offset_y),
                widget_states,
                path,
            );
            path.pop();
            hit
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if !expanded {
                return None;
            }
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let hit = find_context_menu_target_impl(
                child,
                taffy,
                ids[0],
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            hit
        }
        Widget::Modal { visible, child, .. } | Widget::Dialog { visible, child, .. } => {
            if !visible {
                return None;
            }
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let hit = find_context_menu_target_impl(
                child,
                taffy,
                ids[0],
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            hit
        }
        Widget::VirtualList {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContent {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListWithSelection {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContentWithSelection {
            item_height,
            item_count,
            ..
        } => {
            let collection_id = widget.resolved_id(path).unwrap();
            let scroll_y = widget_states
                .get(&collection_id)
                .and_then(WidgetState::as_vlist)
                .map(|state| state.scroll_y)
                .unwrap_or(0.0);
            virtual_list_item_index_at(mouse.y - abs_pos.y, *item_height, *item_count, scroll_y)
                .map(|index| {
                    ContextMenuTargetSearchResult::VirtualItem(ContextMenuVirtualItem::List {
                        collection_id,
                        index,
                    })
                })
        }
        Widget::VirtualGrid {
            columns,
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualGridContent {
            columns,
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualGridWithSelection {
            columns,
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualGridContentWithSelection {
            columns,
            item_height,
            item_count,
            ..
        } => {
            let collection_id = widget.resolved_id(path).unwrap();
            let grid_state = widget_states
                .get(&collection_id)
                .and_then(WidgetState::as_vgrid);
            virtual_grid_item_index_at(
                grid_state,
                Point::new(mouse.x - abs_pos.x, mouse.y - abs_pos.y),
                (layout.size.width, layout.size.height),
                *item_height,
                *item_count,
                *columns,
            )
            .map(|index| {
                ContextMenuTargetSearchResult::VirtualItem(ContextMenuVirtualItem::Grid {
                    collection_id,
                    index,
                })
            })
        }
        _ => None,
    }
}

fn find_scroll_focus_impl<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<u64> {
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::ScrollView { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let resolved_id = widget.resolved_id(path).unwrap();
            let offset_y = widget_states
                .get(&resolved_id)
                .and_then(WidgetState::as_scroll)
                .map(|state| state.offset_y)
                .unwrap_or(0.0);
            path.push(0);
            let child_abs = Point::new(abs_pos.x, abs_pos.y - offset_y);
            let result =
                find_scroll_focus_impl(child, taffy, ids[0], mouse, child_abs, widget_states, path);
            path.pop();
            result.or(Some(resolved_id))
        }
        Widget::TableOfContents { child, .. } => table_of_contents_scroll_focus(
            widget,
            child,
            taffy,
            node_id,
            mouse,
            abs_pos,
            widget_states,
            path,
        ),
        Widget::Table { .. } => Some(widget.resolved_id(path).unwrap()),
        Widget::VirtualList { .. }
        | Widget::VirtualListContent { .. }
        | Widget::VirtualListWithSelection { .. }
        | Widget::VirtualListContentWithSelection { .. }
        | Widget::VirtualGrid { .. }
        | Widget::VirtualGridContent { .. }
        | Widget::VirtualGridWithSelection { .. }
        | Widget::VirtualGridContentWithSelection { .. }
        | Widget::CarouselView { .. } => Some(widget.resolved_id(path).unwrap()),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate().rev() {
                path.push(i);
                let result = find_scroll_focus_impl(
                    child,
                    taffy,
                    ids[i],
                    mouse,
                    abs_pos,
                    widget_states,
                    path,
                );
                path.pop();
                if let Some(id) = result {
                    return Some(id);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let result =
                find_scroll_focus_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            result
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let anchor_node = ids.first().copied()?;
            path.push(0);
            let result = find_scroll_focus_impl(
                anchor,
                taffy,
                anchor_node,
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            result
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn table_of_contents_scroll_focus<Msg>(
    widget: &Widget<Msg>,
    child: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    table_abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<u64> {
    let table_id = widget.resolved_id(path)?;
    let nodes = layout_nodes(taffy, node_id)?;
    let navigation = taffy.layout(nodes.navigation).ok()?;
    let navigation_abs = Point::new(
        table_abs.x + navigation.location.x,
        table_abs.y + navigation.location.y,
    );
    if SkiaRect::from_xywh(
        navigation_abs.x,
        navigation_abs.y,
        navigation.size.width,
        navigation.size.height,
    )
    .contains(mouse)
    {
        return Some(table_id);
    }
    let viewport = taffy.layout(nodes.viewport).ok()?;
    let viewport_abs = Point::new(
        table_abs.x + viewport.location.x,
        table_abs.y + viewport.location.y,
    );
    if !SkiaRect::from_xywh(
        viewport_abs.x,
        viewport_abs.y,
        viewport.size.width,
        viewport.size.height,
    )
    .contains(mouse)
    {
        return None;
    }
    let offset_y = widget_states
        .get(&table_id)
        .and_then(WidgetState::as_scroll)
        .map(|state| state.offset_y)
        .unwrap_or(0.0);
    path.push(0);
    let result = find_scroll_focus_impl(
        child,
        taffy,
        nodes.content,
        mouse,
        Point::new(viewport_abs.x, viewport_abs.y - offset_y),
        widget_states,
        path,
    );
    path.pop();
    result.or(Some(table_id))
}

pub fn find_scrollbar_drag_hit<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<ScrollbarDragHit> {
    let mut path = Vec::new();
    find_scrollbar_drag_hit_impl(widget, taffy, node_id, mouse, abs, widget_states, &mut path)
}

fn scrollbar_drag_hit(
    id: u64,
    abs_pos: Point,
    size: (f32, f32),
    metrics: ScrollbarMetrics,
    mouse: Point,
) -> Option<ScrollbarDragHit> {
    let track = scrollbar_track_rect(abs_pos, size);
    let thumb = SkiaRect::from_xywh(
        track.left,
        abs_pos.y + metrics.thumb_y,
        SCROLLBAR_W,
        metrics.thumb_h,
    );
    let start_offset = scrollbar_press_offset(track, thumb, metrics, mouse)?;
    Some(ScrollbarDragHit {
        id,
        axis: ScrollbarAxis::Vertical,
        reversed: false,
        start_offset,
        viewport_extent: metrics.viewport_h,
        content_extent: metrics.content_h,
    })
}

fn scrollbar_track_rect(abs_pos: Point, size: (f32, f32)) -> SkiaRect {
    SkiaRect::from_xywh(
        abs_pos.x + size.0 - SCROLLBAR_W - 2.0,
        abs_pos.y,
        SCROLLBAR_W,
        size.1,
    )
}

fn scrollbar_press_offset(
    track: SkiaRect,
    thumb: SkiaRect,
    metrics: ScrollbarMetrics,
    mouse: Point,
) -> Option<f32> {
    if !track.contains(mouse) {
        return None;
    }
    if thumb.contains(mouse) {
        return Some(metrics.current_offset);
    }
    Some(scrollbar_track_offset(track, thumb, metrics, mouse.y))
}

fn scrollbar_track_offset(
    track: SkiaRect,
    thumb: SkiaRect,
    metrics: ScrollbarMetrics,
    pointer_y: f32,
) -> f32 {
    let travel = (track.height() - thumb.height()).max(0.0);
    let scrollable = (metrics.content_h - metrics.viewport_h).max(0.0);
    if travel <= f32::EPSILON || scrollable <= f32::EPSILON {
        return 0.0;
    }
    // Centering the thumb makes the click identify the viewport's destination
    // while allowing the same pointer press to continue as a smooth drag.
    let thumb_top = (pointer_y - track.top - thumb.height() * 0.5).clamp(0.0, travel);
    thumb_top / travel * scrollable
}

fn find_scrollbar_drag_hit_impl<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<ScrollbarDragHit> {
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::ScrollView { child, .. } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let ids = taffy.children(node_id).ok()?;
            path.push(0);
            let child_hit = find_scrollbar_drag_hit_impl(
                child,
                taffy,
                ids[0],
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            if let Some(hit) = child_hit {
                return Some(hit);
            }
            let s = widget_states.get(&resolved_id)?.as_scroll()?;
            if s.content_height <= s.viewport_h {
                return None;
            }
            let thumb_h = (layout.size.height * s.thumb_ratio()).max(20.0);
            scrollbar_drag_hit(
                resolved_id,
                abs_pos,
                (layout.size.width, layout.size.height),
                ScrollbarMetrics {
                    current_offset: s.offset_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: s.content_height,
                    thumb_y: s.thumb_y(),
                    thumb_h,
                },
                mouse,
            )
        }
        Widget::TableOfContents { child, .. } => table_of_contents_scrollbar_hit(
            widget,
            child,
            taffy,
            node_id,
            mouse,
            abs_pos,
            widget_states,
            path,
        ),
        Widget::Table { model, options, .. } => table_scrollbar_hit(
            widget,
            model,
            options,
            taffy,
            node_id,
            mouse,
            abs_pos,
            widget_states,
            path,
        ),
        Widget::VirtualList {
            item_count,
            item_height,
            ..
        }
        | Widget::VirtualListContent {
            item_count,
            item_height,
            ..
        }
        | Widget::VirtualListWithSelection {
            item_count,
            item_height,
            ..
        }
        | Widget::VirtualListContentWithSelection {
            item_count,
            item_height,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let s = widget_states.get(&resolved_id)?.as_vlist()?;
            let total_h = *item_count as f32 * *item_height;
            if total_h <= s.viewport_h {
                return None;
            }
            let thumb_h = (layout.size.height * s.thumb_ratio(*item_height, *item_count)).max(20.0);
            scrollbar_drag_hit(
                resolved_id,
                abs_pos,
                (layout.size.width, layout.size.height),
                ScrollbarMetrics {
                    current_offset: s.scroll_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: total_h,
                    thumb_y: s.thumb_y(*item_height, *item_count),
                    thumb_h,
                },
                mouse,
            )
        }
        Widget::VirtualGrid {
            item_count,
            item_height,
            columns,
            ..
        }
        | Widget::VirtualGridContent {
            item_count,
            item_height,
            columns,
            ..
        }
        | Widget::VirtualGridWithSelection {
            item_count,
            item_height,
            columns,
            ..
        }
        | Widget::VirtualGridContentWithSelection {
            item_count,
            item_height,
            columns,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let s = widget_states.get(&resolved_id)?.as_vgrid()?;
            let total_h = virtual_grid_row_count(*item_count, *columns) as f32 * *item_height;
            if total_h <= s.viewport_h {
                return None;
            }
            let thumb_h =
                (layout.size.height * s.thumb_ratio(*item_height, *item_count, *columns)).max(20.0);
            scrollbar_drag_hit(
                resolved_id,
                abs_pos,
                (layout.size.width, layout.size.height),
                ScrollbarMetrics {
                    current_offset: s.scroll_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: total_h,
                    thumb_y: s.thumb_y(*item_height, *item_count, *columns),
                    thumb_h,
                },
                mouse,
            )
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate().rev() {
                path.push(i);
                let result = find_scrollbar_drag_hit_impl(
                    child,
                    taffy,
                    ids[i],
                    mouse,
                    abs_pos,
                    widget_states,
                    path,
                );
                path.pop();
                if let Some(hit) = result {
                    return Some(hit);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            path.push(0);
            let result = find_scrollbar_drag_hit_impl(
                child,
                taffy,
                ids[0],
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            result
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let anchor_node = ids.first().copied()?;
            path.push(0);
            let result = find_scrollbar_drag_hit_impl(
                anchor,
                taffy,
                anchor_node,
                mouse,
                abs_pos,
                widget_states,
                path,
            );
            path.pop();
            result
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn table_scrollbar_hit<Msg>(
    widget: &Widget<Msg>,
    model: &TableModel<'_>,
    options: &TableOptions<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs_pos: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &[usize],
) -> Option<ScrollbarDragHit> {
    let table_id = widget.resolved_id(path)?;
    let layout = taffy.layout(node_id).ok()?;
    let state = widget_states.get(&table_id)?.as_table()?;
    let (viewport, _, scroll, direction) = table_geometry(
        model,
        options,
        (layout.size.width, layout.size.height),
        Some(state),
        table_node_direction(taffy, node_id),
    );
    let local = (mouse.x - abs_pos.x, mouse.y - abs_pos.y);
    let thumbs = state.thumbs(direction);
    if let (Some(track), Some(thumb)) = (viewport.horizontal_track, thumbs.horizontal)
        && track.contains(local.0, local.1)
    {
        return Some(table_axis_drag_hit(
            table_id,
            ScrollbarAxis::Horizontal,
            direction == TableLayoutDirection::Rtl,
            track,
            thumb,
            local.0,
            scroll.0,
            viewport.content_width,
        ));
    }
    let (track, thumb) = (viewport.vertical_track?, thumbs.vertical?);
    track.contains(local.0, local.1).then(|| {
        table_axis_drag_hit(
            table_id,
            ScrollbarAxis::Vertical,
            false,
            track,
            thumb,
            local.1,
            scroll.1,
            viewport.content_height,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn table_axis_drag_hit(
    id: u64,
    axis: ScrollbarAxis,
    reversed: bool,
    track: crate::widgets::table::geometry::TableRect,
    thumb: crate::widgets::table::geometry::TableRect,
    pointer: f32,
    current_offset: f32,
    content_extent: f32,
) -> ScrollbarDragHit {
    let horizontal = axis == ScrollbarAxis::Horizontal;
    let track_start = if horizontal { track.x } else { track.y };
    let track_extent = if horizontal {
        track.width
    } else {
        track.height
    };
    let thumb_start = if horizontal { thumb.x } else { thumb.y };
    let thumb_extent = if horizontal {
        thumb.width
    } else {
        thumb.height
    };
    let pointer_in_thumb = pointer >= thumb_start && pointer <= thumb_start + thumb_extent;
    let start_offset = if pointer_in_thumb {
        current_offset
    } else {
        table_track_offset(
            pointer,
            track_start,
            track_extent,
            thumb_extent,
            content_extent,
            reversed,
        )
    };
    ScrollbarDragHit {
        id,
        axis,
        reversed,
        start_offset,
        viewport_extent: track_extent,
        content_extent,
    }
}

fn table_track_offset(
    pointer: f32,
    track_start: f32,
    track_extent: f32,
    thumb_extent: f32,
    content_extent: f32,
    reversed: bool,
) -> f32 {
    let travel = (track_extent - thumb_extent).max(0.0);
    let thumb_start = (pointer - track_start - thumb_extent * 0.5).clamp(0.0, travel);
    let max_offset = (content_extent - track_extent).max(0.0);
    let visual = if travel > 0.0 {
        thumb_start / travel * max_offset
    } else {
        0.0
    };
    if reversed {
        max_offset - visual
    } else {
        visual
    }
}

#[allow(clippy::too_many_arguments)]
fn table_of_contents_scrollbar_hit<Msg>(
    widget: &Widget<Msg>,
    child: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    table_abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<ScrollbarDragHit> {
    let table_id = widget.resolved_id(path)?;
    let nodes = layout_nodes(taffy, node_id)?;
    let viewport = taffy.layout(nodes.viewport).ok()?;
    let viewport_abs = Point::new(
        table_abs.x + viewport.location.x,
        table_abs.y + viewport.location.y,
    );
    if !SkiaRect::from_xywh(
        viewport_abs.x,
        viewport_abs.y,
        viewport.size.width,
        viewport.size.height,
    )
    .contains(mouse)
    {
        return None;
    }
    let state = widget_states.get(&table_id)?.as_scroll()?;
    if let Some(hit) = table_of_contents_nested_scrollbar_hit(
        child,
        taffy,
        nodes.content,
        mouse,
        viewport_abs,
        state.offset_y,
        widget_states,
        path,
    ) {
        return Some(hit);
    }
    if state.content_height <= state.viewport_h {
        return None;
    }
    table_of_contents_scrollbar(table_id, viewport_abs, viewport, state, mouse)
}

#[allow(clippy::too_many_arguments)]
fn table_of_contents_nested_scrollbar_hit<Msg>(
    child: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    content_node: NodeId,
    mouse: Point,
    viewport_abs: Point,
    offset_y: f32,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
) -> Option<ScrollbarDragHit> {
    path.push(0);
    let hit = find_scrollbar_drag_hit_impl(
        child,
        taffy,
        content_node,
        mouse,
        Point::new(viewport_abs.x, viewport_abs.y - offset_y),
        widget_states,
        path,
    );
    path.pop();
    hit
}

fn table_of_contents_scrollbar(
    table_id: u64,
    viewport_abs: Point,
    viewport: &taffy::tree::Layout,
    state: &crate::engine::widget_state::ScrollState,
    mouse: Point,
) -> Option<ScrollbarDragHit> {
    let thumb_h = (viewport.size.height * state.thumb_ratio()).max(20.0);
    scrollbar_drag_hit(
        table_id,
        viewport_abs,
        (viewport.size.width, viewport.size.height),
        ScrollbarMetrics {
            current_offset: state.offset_y,
            viewport_h: state.viewport_h.max(viewport.size.height),
            content_h: state.content_height,
            thumb_y: state.thumb_y(),
            thumb_h,
        },
        mouse,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use skia_safe::{Point, Rect as SkiaRect};
    use taffy::prelude::{AlignItems, Dimension, Size, Style, TaffyTree};
    use winit::dpi::PhysicalSize;

    use super::{
        HitResult, ScrollbarMetrics, collect_input_ids, collect_stateful_ids, dialog_card_rect,
        find_context_menu_target, find_context_menu_target_with_metadata, find_scroll_focus,
        find_scrollbar_drag_hit, hit_test, rounded_rect_contains, scrollbar_drag_hit,
    };
    use crate::app::ContextMenuVirtualItem;
    use crate::engine::widget_state::{
        ScrollState, VirtualGridState, VirtualListState, WidgetState,
    };
    use crate::layout::{build_taffy_tree, compute_layout};
    use crate::widget::{
        AUTO_ID, ButtonVariant, ContextMenuEntry, CustomInteraction, CustomPaintContext,
        CustomWidgetV1, DialogPosition, InputState, Widget,
    };
    use crate::widgets::carousel::CarouselState;
    use crate::widgets::table_of_contents::HeadingLevel;
    use crate::{KeyedVirtualItems, PointerEvent, PointerRegionConfig, VirtualItemKey, WidgetId};

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Str(String),
        Usize(usize),
        Toggle,
    }

    fn text_msg(value: String) -> Msg {
        Msg::Str(value)
    }

    struct PointerCustom;

    impl CustomWidgetV1<Msg> for PointerCustom {
        fn paint(&self, _: CustomPaintContext<'_>) {}

        fn interaction(&self) -> CustomInteraction {
            CustomInteraction::PointerAndKeyboard
        }
    }

    struct VisualCustom;

    impl CustomWidgetV1<Msg> for VisualCustom {
        fn paint(&self, _: CustomPaintContext<'_>) {}
    }

    #[test]
    fn rounded_container_hit_region_excludes_clipped_corners() {
        let rect = SkiaRect::from_xywh(0.0, 0.0, 20.0, 20.0);

        assert!(!rounded_rect_contains(rect, 8.0, Point::new(0.0, 0.0)));
        assert!(rounded_rect_contains(rect, 8.0, Point::new(10.0, 10.0)));
    }

    #[test]
    fn custom_pointer_hit_uses_container_layout_coordinates() {
        let widget = Widget::Container {
            child: Box::new(Widget::custom(
                WidgetId::manual(301).unwrap(),
                PointerCustom,
                fixed_size_style(60.0, 30.0),
            )),
            style: fixed_size_style(100.0, 60.0),
            color: None,
            radius: 0.0,
        };
        let states = HashMap::new();
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(100, 60));

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(20.0, 10.0),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(
            hit,
            Some(HitResult::CustomPointer {
                id: 301,
                focuses_keyboard: true,
                ..
            })
        ));
    }

    #[test]
    fn visual_only_custom_node_cannot_claim_pointer_input() {
        let widget = Widget::custom(
            WidgetId::manual(302).unwrap(),
            VisualCustom,
            fixed_size_style(60.0, 30.0),
        );
        let states = HashMap::new();
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(60, 30));

        assert!(
            hit_test(
                &widget,
                &taffy,
                root,
                Point::new(20.0, 10.0),
                Point::new(0.0, 0.0),
                &states,
            )
            .is_none()
        );
    }

    fn pointer_message(_: PointerEvent) -> Msg {
        Msg::Toggle
    }

    #[test]
    fn pointer_region_claims_primary_hits_before_wrapped_content() {
        let widget = Widget::pointer_region(
            WidgetId::manual(303).unwrap(),
            sized_button(),
            PointerRegionConfig::new(pointer_message).with_pointer_capture(),
            fixed_size_style(100.0, 40.0),
        );
        let states = HashMap::new();
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(100, 40));

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(20.0, 10.0),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(hit, Some(HitResult::PointerRegion(303))));
    }

    #[test]
    fn scroll_view_hits_the_visible_region_at_the_painted_offset() {
        let source = |id| {
            Widget::pointer_region(
                WidgetId::manual(id).unwrap(),
                Widget::Spacer {
                    style: fixed_size_style(100.0, 40.0),
                },
                PointerRegionConfig::new(pointer_message),
                fixed_size_style(100.0, 40.0),
            )
        };
        let widget = Widget::ScrollView {
            id: 305,
            child: Box::new(Widget::Column {
                children: vec![source(306), source(307)],
                style: fixed_size_style(100.0, 80.0),
            }),
            style: Style {
                align_items: Some(AlignItems::FlexStart),
                ..fixed_size_style(100.0, 40.0)
            },
        };
        let mut states = HashMap::from([(
            305,
            WidgetState::Scroll(ScrollState {
                offset_y: 0.0,
                content_height: 80.0,
                viewport_h: 40.0,
            }),
        )]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(100, 40));
        let point = Point::new(20.0, 20.0);
        assert!(matches!(
            hit_test(&widget, &taffy, root, point, Point::default(), &states),
            Some(HitResult::PointerRegion(306))
        ));
        states
            .get_mut(&305)
            .and_then(WidgetState::as_scroll_mut)
            .unwrap()
            .offset_y = 40.0;
        assert!(matches!(
            hit_test(&widget, &taffy, root, point, Point::default(), &states),
            Some(HitResult::PointerRegion(307))
        ));
        assert!(
            hit_test(
                &widget,
                &taffy,
                root,
                Point::new(20.0, 41.0),
                Point::default(),
                &states
            )
            .is_none()
        );
    }

    fn usize_msg(value: usize) -> Msg {
        Msg::Usize(value)
    }

    fn sized_button() -> Widget<'static, Msg> {
        Widget::Button {
            text: "OK",
            on_press: Msg::Toggle,
            style: Style {
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::length(40.0),
                },
                ..Style::default()
            },
            color: None,
            variant: ButtonVariant::Primary,
        }
    }

    fn rich_button() -> Widget<'static, Msg> {
        Widget::button_content(
            "OK",
            Widget::Text {
                content: "OK".into(),
                style: Style::default(),
                color: None,
                size: 14.0,
            },
            Msg::Toggle,
            Style {
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::length(40.0),
                },
                ..Style::default()
            },
            None,
            ButtonVariant::Primary,
        )
    }

    fn interactive_row<'a>(index: usize) -> Option<Widget<'a, Msg>> {
        (index == 0).then(|| Widget::Row {
            children: vec![
                Widget::Button {
                    text: "Run",
                    on_press: Msg::Usize(9),
                    style: fixed_size_style(30.0, 40.0),
                    color: None,
                    variant: ButtonVariant::Primary,
                },
                Widget::Switch {
                    checked: false,
                    on_change: |_| Msg::Toggle,
                    style: fixed_size_style(30.0, 40.0),
                },
                Widget::Spacer {
                    style: fixed_size_style(40.0, 40.0),
                },
            ],
            style: fixed_size_style(100.0, 40.0),
        })
    }

    #[test]
    fn interactive_virtual_rows_route_child_actions_before_row_selection() {
        let keys = [VirtualItemKey::new(71).unwrap()];
        let items = KeyedVirtualItems::try_new(&keys, &interactive_row).unwrap();
        let widget = Widget::interactive_virtual_list_content(
            40.0,
            items,
            usize_msg,
            fixed_size_style(100.0, 40.0),
        )
        .with_id(700);
        let mut states = HashMap::new();
        states.insert(
            700,
            WidgetState::VList(VirtualListState {
                viewport_h: 40.0,
                ..Default::default()
            }),
        );
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(100, 40));

        let button_hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(10.0, 20.0),
            Point::new(0.0, 0.0),
            &states,
        );
        let switch_hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(45.0, 20.0),
            Point::new(0.0, 0.0),
            &states,
        );
        let row_hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(90.0, 20.0),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(
            button_hit,
            Some(HitResult::Message {
                msg: Msg::Usize(9),
                ..
            })
        ));
        assert!(matches!(
            switch_hit,
            Some(HitResult::Message {
                msg: Msg::Toggle,
                ..
            })
        ));
        assert!(matches!(
            row_hit,
            Some(HitResult::VListSelect { id: 700, index: 0 })
        ));
    }

    #[test]
    fn keyed_virtual_child_focus_identity_survives_item_reordering() {
        let key = VirtualItemKey::new(99).unwrap();
        let mut first_path = vec![0];
        super::push_interactive_virtual_item_path(&mut first_path, key);
        let first = sized_button().keyboard_focus_id(&first_path);
        let mut reordered_path = vec![0];
        super::push_interactive_virtual_item_path(&mut reordered_path, key);
        let reordered = sized_button().keyboard_focus_id(&reordered_path);

        assert_eq!(first, reordered);
    }

    #[test]
    fn interactive_grid_and_carousel_route_visible_child_actions() {
        let keys = [VirtualItemKey::new(72).unwrap()];
        let grid_items = KeyedVirtualItems::try_new(&keys, &interactive_row).unwrap();
        let grid = Widget::interactive_virtual_grid_content(
            1,
            40.0,
            grid_items,
            usize_msg,
            fixed_size_style(100.0, 40.0),
        )
        .with_id(701);
        let grid_states = HashMap::from([(
            701,
            WidgetState::VGrid(VirtualGridState {
                viewport_w: 100.0,
                viewport_h: 40.0,
                ..Default::default()
            }),
        )]);
        let (grid_taffy, grid_root) = test_layout(&grid, &grid_states, PhysicalSize::new(100, 40));
        let grid_hit = hit_test(
            &grid,
            &grid_taffy,
            grid_root,
            Point::new(20.0, 20.0),
            Point::new(0.0, 0.0),
            &grid_states,
        );

        let carousel_items = KeyedVirtualItems::try_new(&keys, &interactive_row).unwrap();
        let carousel = Widget::interactive_carousel_view(
            carousel_items,
            usize_msg,
            crate::CarouselConfig::uncontained(100.0).unwrap(),
            fixed_size_style(100.0, 48.0),
        )
        .with_id(702);
        let carousel_states =
            HashMap::from([(702, WidgetState::Carousel(CarouselState::default()))]);
        let (carousel_taffy, carousel_root) =
            test_layout(&carousel, &carousel_states, PhysicalSize::new(100, 48));
        let carousel_hit = hit_test(
            &carousel,
            &carousel_taffy,
            carousel_root,
            Point::new(20.0, 20.0),
            Point::new(0.0, 0.0),
            &carousel_states,
        );

        assert!(matches!(
            grid_hit,
            Some(HitResult::Message {
                msg: Msg::Usize(9),
                ..
            })
        ));
        assert!(matches!(
            carousel_hit,
            Some(HitResult::Message {
                msg: Msg::Usize(9),
                ..
            })
        ));
    }

    fn expanded_accordion_with_body_button() -> Widget<'static, Msg> {
        Widget::Accordion {
            id: 64,
            title: "Details",
            expanded: true,
            on_toggle: Msg::Toggle,
            child: Box::new(Widget::Button {
                text: "Body action",
                on_press: Msg::Usize(42),
                style: fixed_size_style(100.0, 40.0),
                color: None,
                variant: ButtonVariant::Primary,
            }),
            style: Style {
                size: Size {
                    width: Dimension::length(100.0),
                    height: Dimension::auto(),
                },
                ..Style::default()
            },
        }
    }

    #[test]
    fn auto_input_ids_are_stable_and_distinct_by_path() {
        let widget = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::text_input(
                    text_msg,
                    None,
                    Style::default(),
                    "Name",
                    "type",
                    InputState::Idle,
                    None,
                    false,
                ),
                Widget::Row {
                    style: Style::default(),
                    children: vec![
                        Widget::text_input(
                            text_msg,
                            None,
                            Style::default(),
                            "Email",
                            "mail",
                            InputState::Idle,
                            None,
                            false,
                        ),
                        Widget::text_input(
                            text_msg,
                            None,
                            Style::default(),
                            "Search",
                            "query",
                            InputState::Idle,
                            None,
                            false,
                        ),
                    ],
                },
            ],
        };

        let mut first = Vec::new();
        let mut second = Vec::new();
        collect_input_ids(&widget, &mut first);
        collect_input_ids(&widget, &mut second);

        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert!(first.iter().all(|id| *id != AUTO_ID));
        assert_eq!(first.iter().copied().collect::<HashSet<_>>().len(), 3);
    }

    #[test]
    fn manual_id_override_wins_over_generated_path_id() {
        let widget = Widget::text_input(
            text_msg,
            None,
            Style::default(),
            "Override",
            "",
            InputState::Idle,
            None,
            false,
        )
        .with_id(77);

        let mut ids = Vec::new();
        collect_input_ids(&widget, &mut ids);

        assert_eq!(ids, vec![77]);
    }

    #[test]
    fn auto_stateful_ids_include_widget_kind() {
        let slider = Widget::slider(25.0, 0.0, 100.0, 1.0, |_| Msg::Toggle, Style::default(), "");
        let select = Widget::select(&["A", "B"], 0, usize_msg, Style::default(), "", "");

        let mut slider_ids = Vec::new();
        let mut select_ids = Vec::new();
        collect_stateful_ids(&slider, &mut slider_ids);
        collect_stateful_ids(&select, &mut select_ids);

        assert_eq!(slider_ids.len(), 1);
        assert_eq!(select_ids.len(), 1);
        assert_ne!(slider_ids[0].0, select_ids[0].0);
        assert_eq!(slider_ids[0].1, "slider");
        assert_eq!(select_ids[0].1, "select");
    }

    #[test]
    fn scroll_focus_applies_parent_scroll_offset_to_nested_carousel() {
        let carousel = Widget::carousel_view(
            20,
            |_| None,
            usize_msg,
            crate::CarouselConfig::uncontained(100.0).unwrap(),
            fixed_size_style(300.0, 100.0),
        )
        .with_id(62);
        let content = Widget::Column {
            children: vec![
                Widget::Spacer {
                    style: fixed_size_style(300.0, 200.0),
                },
                carousel,
            ],
            style: Style {
                flex_shrink: 0.0,
                ..fixed_size_style(300.0, 300.0)
            },
        };
        let widget = Widget::scroll_view(content, fixed_size_style(300.0, 100.0)).with_id(61);
        let states = HashMap::from([
            (
                61,
                WidgetState::Scroll(ScrollState {
                    offset_y: 200.0,
                    content_height: 300.0,
                    viewport_h: 100.0,
                }),
            ),
            (62, WidgetState::Carousel(CarouselState::default())),
        ]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(300, 100));

        let focus = find_scroll_focus(
            &widget,
            &taffy,
            root,
            Point::new(50.0, 50.0),
            Point::new(0.0, 0.0),
            &states,
        );
        assert_eq!(focus, Some(62));
    }

    #[test]
    fn context_menu_target_captures_scrolled_virtual_list_content_item() {
        let entries = [ContextMenuEntry::item("Open", Msg::Toggle)];
        let collection = Widget::virtual_list_content(
            20.0,
            10,
            &virtual_widget_item,
            usize_msg,
            fixed_size_style(160.0, 80.0),
        )
        .with_id(71);
        let widget =
            Widget::context_menu(collection, &entries, fixed_size_style(160.0, 80.0)).with_id(72);
        let states = HashMap::from([(
            71,
            WidgetState::VList(VirtualListState {
                scroll_y: 40.0,
                viewport_h: 80.0,
                ..VirtualListState::default()
            }),
        )]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(160, 80));

        let target = find_context_menu_target_with_metadata(
            &widget,
            &taffy,
            root,
            Point::new(20.0, 10.0),
            Point::new(0.0, 0.0),
            &states,
        )
        .expect("a press at (20, 10) must target the virtual-list context menu");

        assert_eq!(target.id(), 72);
        assert_eq!(
            target.virtual_item(),
            Some(ContextMenuVirtualItem::List {
                collection_id: 71,
                index: 2,
            })
        );
        assert_eq!(
            find_context_menu_target(
                &widget,
                &taffy,
                root,
                Point::new(20.0, 10.0),
                Point::new(0.0, 0.0),
            ),
            Some(72)
        );
    }

    #[test]
    fn context_menu_target_captures_scrolled_virtual_grid_content_item() {
        let entries = [ContextMenuEntry::item("Open", Msg::Toggle)];
        let collection = Widget::virtual_grid_content(
            2,
            30.0,
            10,
            &virtual_widget_item,
            usize_msg,
            fixed_size_style(160.0, 90.0),
        )
        .with_id(81);
        let widget =
            Widget::context_menu(collection, &entries, fixed_size_style(160.0, 90.0)).with_id(82);
        let states = HashMap::from([(
            81,
            WidgetState::VGrid(VirtualGridState {
                scroll_y: 60.0,
                viewport_w: 160.0,
                viewport_h: 90.0,
                ..VirtualGridState::default()
            }),
        )]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(160, 90));

        let target = find_context_menu_target_with_metadata(
            &widget,
            &taffy,
            root,
            Point::new(100.0, 15.0),
            Point::new(0.0, 0.0),
            &states,
        )
        .expect("a press at (100, 15) must target the virtual-grid context menu");

        assert_eq!(target.id(), 82);
        assert_eq!(
            target.virtual_item(),
            Some(ContextMenuVirtualItem::Grid {
                collection_id: 81,
                index: 5,
            })
        );
    }

    #[test]
    fn scrollbar_track_click_maps_all_scrollable_widgets_to_the_clicked_position() {
        let (scroll_view, scroll_view_states) = scrollable_view();
        let (virtual_list, virtual_list_states) = scrollable_list();
        let (virtual_grid, virtual_grid_states) = scrollable_grid();

        assert_eq!(
            bottom_scrollbar_track_hit(&scroll_view, &scroll_view_states).start_offset,
            900.0
        );
        assert_eq!(
            bottom_scrollbar_track_hit(&virtual_list, &virtual_list_states).start_offset,
            1900.0
        );
        assert_eq!(
            bottom_scrollbar_track_hit(&virtual_grid, &virtual_grid_states).start_offset,
            1900.0
        );
    }

    #[test]
    fn scrollbar_thumb_click_keeps_its_existing_offset_for_dragging() {
        let hit = scrollbar_drag_hit(
            7,
            Point::new(0.0, 0.0),
            (100.0, 100.0),
            ScrollbarMetrics {
                current_offset: 300.0,
                viewport_h: 100.0,
                content_h: 1000.0,
                thumb_y: 20.0,
                thumb_h: 20.0,
            },
            Point::new(94.0, 25.0),
        )
        .expect("a click on scrollbar thumb at (94, 25) must start dragging");

        assert_eq!(hit.start_offset, 300.0);
    }

    #[test]
    fn scrollbar_track_click_maps_an_intermediate_position_to_scroll_offset() {
        let hit = scrollbar_drag_hit(
            7,
            Point::new(0.0, 0.0),
            (100.0, 100.0),
            ScrollbarMetrics {
                current_offset: 0.0,
                viewport_h: 100.0,
                content_h: 1000.0,
                thumb_y: 0.0,
                thumb_h: 20.0,
            },
            Point::new(94.0, 50.0),
        )
        .expect("a scrollbar track click at (94, 50) must return a scroll hit");

        assert_eq!(hit.start_offset, 450.0);
    }

    fn bottom_scrollbar_track_hit(
        widget: &Widget<'_, Msg>,
        states: &HashMap<u64, WidgetState>,
    ) -> super::ScrollbarDragHit {
        let (taffy, root) = test_layout(widget, states, PhysicalSize::new(100, 100));
        find_scrollbar_drag_hit(
            widget,
            &taffy,
            root,
            Point::new(94.0, 90.0),
            Point::new(0.0, 0.0),
            states,
        )
        .expect("a scrollbar track click at (94, 90) must return a scroll hit")
    }

    fn scrollable_view() -> (Widget<'static, Msg>, HashMap<u64, WidgetState>) {
        let content = Widget::Spacer {
            style: fixed_size_style(100.0, 1000.0),
        };
        let widget = Widget::scroll_view(content, fixed_size_style(100.0, 100.0)).with_id(11);
        let states = HashMap::from([(
            11,
            WidgetState::Scroll(ScrollState {
                offset_y: 0.0,
                content_height: 1000.0,
                viewport_h: 100.0,
            }),
        )]);
        (widget, states)
    }

    fn scrollable_list() -> (Widget<'static, Msg>, HashMap<u64, WidgetState>) {
        let widget = Widget::virtual_list(
            20.0,
            100,
            &virtual_text_item,
            usize_msg,
            fixed_size_style(100.0, 100.0),
        )
        .with_id(12);
        let states = HashMap::from([(
            12,
            WidgetState::VList(VirtualListState {
                viewport_h: 100.0,
                ..VirtualListState::default()
            }),
        )]);
        (widget, states)
    }

    fn scrollable_grid() -> (Widget<'static, Msg>, HashMap<u64, WidgetState>) {
        let widget = Widget::virtual_grid(
            4,
            20.0,
            400,
            &virtual_text_item,
            usize_msg,
            fixed_size_style(100.0, 100.0),
        )
        .with_id(13);
        let states = HashMap::from([(
            13,
            WidgetState::VGrid(VirtualGridState {
                viewport_w: 100.0,
                viewport_h: 100.0,
                ..VirtualGridState::default()
            }),
        )]);
        (widget, states)
    }

    fn virtual_text_item(_: usize) -> Option<String> {
        Some(String::from("item"))
    }

    fn virtual_widget_item<'a>(_: usize) -> Option<Widget<'a, Msg>> {
        Some(Widget::Spacer {
            style: Style::default(),
        })
    }

    fn fixed_size_style(width: f32, height: f32) -> Style {
        Style {
            size: Size {
                width: Dimension::length(width),
                height: Dimension::length(height),
            },
            ..Style::default()
        }
    }

    fn test_layout<Msg>(
        widget: &Widget<'_, Msg>,
        states: &HashMap<u64, WidgetState>,
        size: PhysicalSize<u32>,
    ) -> (TaffyTree<crate::layout::RutterContext>, taffy::NodeId) {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new()));
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), states);
        compute_layout(
            &mut taffy,
            root,
            size,
            fonts,
            &crate::render::RichTextRenderer::default(),
        );
        (taffy, root)
    }

    #[test]
    fn table_of_contents_link_hit_resolves_a_heading_scroll_target() {
        let document: Widget<'_, Msg> = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::heading(HeadingLevel::H1, "Overview", fixed_size_style(320.0, 36.0)),
                Widget::Spacer {
                    style: fixed_size_style(320.0, 96.0),
                },
                Widget::heading(
                    HeadingLevel::H2,
                    "Installation",
                    fixed_size_style(320.0, 32.0),
                ),
            ],
        };
        let widget =
            Widget::table_of_contents("Contents", document, fixed_size_style(320.0, 220.0))
                .with_id(71);
        let states = HashMap::from([(71, WidgetState::Scroll(ScrollState::default()))]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(320, 220));
        let link = crate::widgets::table_of_contents::entry_rects(&taffy, root)[1];

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(link.center_x(), link.center_y()),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(
            hit,
            Some(HitResult::TableOfContentsActivate {
                id: 71,
                index: 1,
                target_y,
                ..
            }) if target_y > 0.0
        ));
    }

    #[test]
    fn table_of_contents_finds_nested_scrollbar_when_its_document_fits() {
        let nested: Widget<'_, Msg> = Widget::scroll_view(
            Widget::Spacer {
                style: fixed_size_style(300.0, 300.0),
            },
            fixed_size_style(300.0, 100.0),
        )
        .with_id(82);
        let widget = Widget::table_of_contents("Contents", nested, fixed_size_style(300.0, 300.0))
            .with_id(81);
        let states = HashMap::from([
            (
                81,
                WidgetState::Scroll(ScrollState {
                    offset_y: 0.0,
                    content_height: 100.0,
                    viewport_h: 200.0,
                }),
            ),
            (
                82,
                WidgetState::Scroll(ScrollState {
                    offset_y: 0.0,
                    content_height: 300.0,
                    viewport_h: 100.0,
                }),
            ),
        ]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(300, 300));
        let nodes = crate::widgets::table_of_contents::layout_nodes(&taffy, root).unwrap();
        let viewport = taffy.layout(nodes.viewport).unwrap();
        let content = taffy.layout(nodes.content).unwrap();
        let nested_node = taffy.children(nodes.content).unwrap()[0];
        let nested_layout = taffy.layout(nested_node).unwrap();
        let mouse = Point::new(
            viewport.location.x
                + content.location.x
                + nested_layout.location.x
                + nested_layout.size.width
                - crate::layout::SCROLLBAR_W
                - 1.0,
            viewport.location.y + content.location.y + nested_layout.location.y + 5.0,
        );

        let hit =
            find_scrollbar_drag_hit(&widget, &taffy, root, mouse, Point::new(0.0, 0.0), &states);

        assert!(matches!(hit, Some(hit) if hit.id == 82));
    }

    #[test]
    fn table_of_contents_navigation_has_no_independent_scroll_state() {
        let document: Widget<'_, Msg> = Widget::Column {
            style: Style::default(),
            children: (0..16)
                .map(|index| -> Widget<'_, Msg> {
                    Widget::heading(
                        HeadingLevel::H2,
                        format!("Section {index}"),
                        fixed_size_style(320.0, 28.0),
                    )
                })
                .collect(),
        };
        let options = crate::TableOfContentsOptions::new(2).unwrap();
        let widget = Widget::table_of_contents_with_options(
            "Contents",
            document,
            fixed_size_style(320.0, 220.0),
            options,
        )
        .with_id(83);
        let layout_states = HashMap::from([(83, WidgetState::Scroll(ScrollState::default()))]);
        let (taffy, root) = test_layout(&widget, &layout_states, PhysicalSize::new(320, 220));
        let nodes = crate::widgets::table_of_contents::layout_nodes(&taffy, root).unwrap();
        let navigation = taffy.layout(nodes.navigation).unwrap();
        let entry = crate::widgets::table_of_contents::entry_rects(&taffy, root)[15];
        let states = HashMap::from([(83, WidgetState::Scroll(ScrollState::default()))]);
        let navigation_mouse = Point::new(
            navigation.location.x + 8.0,
            navigation.location.y + navigation.size.height * 0.5,
        );
        let link_mouse = Point::new(entry.center_x(), entry.center_y());
        let mut stateful = Vec::new();
        collect_stateful_ids(&widget, &mut stateful);

        assert_eq!(
            find_scroll_focus(
                &widget,
                &taffy,
                root,
                navigation_mouse,
                Point::new(0.0, 0.0),
                &states,
            ),
            Some(83)
        );
        assert_eq!(stateful, vec![(83, "scroll")]);
        assert!(entry.bottom <= taffy.layout(root).unwrap().size.height);
        assert!(matches!(
            hit_test(
                &widget,
                &taffy,
                root,
                link_mouse,
                Point::new(0.0, 0.0),
                &states,
            ),
            Some(HitResult::TableOfContentsActivate { index: 15, .. })
        ));
    }

    #[test]
    fn collapsed_table_of_contents_accordion_only_activates_its_header() {
        let options =
            crate::TableOfContentsOptions::default().with_accordion_state(false, Msg::Toggle);
        let widget = Widget::table_of_contents_with_options(
            "Contents",
            Widget::heading(HeadingLevel::H2, "Overview", fixed_size_style(320.0, 32.0)),
            fixed_size_style(320.0, 220.0),
            options,
        )
        .with_id(84);
        let states = HashMap::from([(84, WidgetState::Scroll(ScrollState::default()))]);
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(320, 220));
        let header = crate::widgets::table_of_contents::title_rect(&taffy, root).unwrap();

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(header.center_x(), header.center_y()),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(crate::widgets::table_of_contents::entry_rects(&taffy, root).is_empty());
        assert!(matches!(
            hit,
            Some(HitResult::Message {
                focus_id: Some(_),
                msg: Msg::Toggle,
            })
        ));
    }

    #[test]
    fn modal_hit_test_uses_centered_card_origin() {
        let widget = Widget::Modal {
            id: 10,
            visible: true,
            child: Box::new(sized_button()),
            on_dismiss: Some(Msg::Usize(99)),
            style: Style::default(),
        };
        let states = HashMap::new();
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(
            &mut taffy,
            &widget,
            std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new())),
            &states,
        );
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(400, 300),
            std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new())),
            &crate::render::RichTextRenderer::default(),
        );

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(70.0, 60.0),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(
            hit,
            Some(HitResult::Message {
                msg: Msg::Toggle,
                ..
            })
        ));
    }

    #[test]
    fn button_content_hit_test_returns_button_message() {
        let widget = rich_button();
        let states = HashMap::new();
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(
            &mut taffy,
            &widget,
            std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new())),
            &states,
        );
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(120, 60),
            std::rc::Rc::new(std::cell::RefCell::new(cosmic_text::FontSystem::new())),
            &crate::render::RichTextRenderer::default(),
        );

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(8.0, 8.0),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(
            hit,
            Some(HitResult::Message {
                msg: Msg::Toggle,
                ..
            })
        ));
    }

    #[test]
    fn expanded_accordion_body_hit_uses_its_taffy_header_offset_once() {
        let widget = expanded_accordion_with_body_button();
        let states = HashMap::new();
        let (taffy, root) = test_layout(&widget, &states, PhysicalSize::new(100, 100));

        let hit = hit_test(
            &widget,
            &taffy,
            root,
            Point::new(50.0, 64.0),
            Point::new(0.0, 0.0),
            &states,
        );

        assert!(matches!(
            hit,
            Some(HitResult::Message {
                msg: Msg::Usize(42),
                ..
            })
        ));
    }

    #[test]
    fn popover_registers_stateful_id_and_open_content_inputs() {
        let popover = Widget::popover(
            true,
            Widget::Button {
                text: "Open",
                on_press: Msg::Toggle,
                style: Style::default(),
                color: None,
                variant: crate::widget::ButtonVariant::Primary,
            },
            Widget::text_input(
                text_msg,
                None,
                Style::default(),
                "Filter",
                "",
                InputState::Idle,
                None,
                false,
            )
            .with_id(88),
            Some(Msg::Toggle),
            Style::default(),
            Style::default(),
        )
        .with_id(99);

        let mut stateful = Vec::new();
        let mut inputs = Vec::new();
        collect_stateful_ids(&popover, &mut stateful);
        collect_input_ids(&popover, &mut inputs);

        assert!(
            stateful
                .iter()
                .any(|(id, kind)| *id == 99 && *kind == "popover")
        );
        assert_eq!(inputs, vec![88]);
    }

    #[test]
    fn closed_popover_skips_content_inputs() {
        let popover = Widget::popover(
            false,
            Widget::Spacer {
                style: Style::default(),
            },
            Widget::text_input(
                text_msg,
                None,
                Style::default(),
                "Hidden",
                "",
                InputState::Idle,
                None,
                false,
            )
            .with_id(88),
            None,
            Style::default(),
            Style::default(),
        );

        let mut inputs = Vec::new();
        collect_input_ids(&popover, &mut inputs);

        assert!(inputs.is_empty());
    }

    #[test]
    fn dialog_card_rect_respects_vertical_position() {
        let viewport = (800.0, 600.0);
        let top = dialog_card_rect(DialogPosition::Top, viewport);
        let center = dialog_card_rect(DialogPosition::Center, viewport);
        let bottom = dialog_card_rect(DialogPosition::Bottom, viewport);

        assert!(top.top < center.top);
        assert!(center.top < bottom.top);
        assert!((center.top - 200.0).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
#[path = "table_hit_tests.rs"]
mod table_hit_tests;
