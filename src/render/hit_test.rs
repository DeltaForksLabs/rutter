// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — render/hit_test.rs
// ============================================================

use skia_safe::{Contains, Point, Rect as SkiaRect};
use std::collections::HashMap;
use taffy::prelude::{NodeId, TaffyTree};

use crate::engine::widget_state::{WidgetState, virtual_grid_row_count};
use crate::layout::{OPTION_HEIGHT, RutterContext, SCROLLBAR_W};
use crate::widget::{
    CONTEXT_MENU_ITEM_H, CONTEXT_MENU_PAD_Y, CONTEXT_MENU_SEPARATOR_H,
    CONTEXT_MENU_VIEWPORT_MARGIN, ContextMenuEntry, DialogAction, DialogPosition, POPOVER_GAP,
    POPOVER_VIEWPORT_MARGIN, Widget, estimate_context_menu_height, estimate_context_menu_width,
};

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
    SelectToggle(u64),
    SelectOption {
        id: u64,
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
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollbarDragHit {
    pub id: u64,
    pub start_offset: f32,
    pub viewport_h: f32,
    pub content_h: f32,
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
    let width = (viewport_size.0 * 0.85).min(MODAL_MAX_CARD_W).max(1.0);
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
            if let Some(state) = menu_state {
                if state.is_open {
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
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } => {
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
            if let Some(popover) = popover {
                if popover.is_open {
                    *any_open = true;
                    if let Some(popup_node) = node_children.get(1).copied() {
                        if let Ok(popup_layout) = taffy.layout(popup_node) {
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
        Widget::Select { options, .. } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let is_open = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_select())
                .map(|s| s.is_open)
                .unwrap_or(false);
            let closed_h = layout.size.height
                - if is_open {
                    options.len() as f32 * OPTION_HEIGHT
                } else {
                    0.0
                };
            if mouse.y < abs_pos.y + closed_h {
                return Some(HitResult::SelectToggle(resolved_id));
            }
            if is_open {
                let rel_y = mouse.y - (abs_pos.y + closed_h);
                let idx = (rel_y / OPTION_HEIGHT).floor() as usize;
                let idx = idx.min(options.len().saturating_sub(1));
                return Some(HitResult::SelectOption {
                    id: resolved_id,
                    index: idx,
                });
            }
            None
        }
        Widget::ScrollView { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            path.push(0);
            let child_hit =
                hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            if let Some(result) = child_hit {
                return Some(result);
            }
            Some(HitResult::ScrollFocus(widget.resolved_id(path).unwrap()))
        }
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
            let Some(anchor_node) = ids.first().copied() else {
                return None;
            };
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
                let result = hit_test_impl(
                    child,
                    taffy,
                    ids[0],
                    mouse,
                    Point::new(abs_pos.x, abs_pos.y + ACCORDION_HEADER_H),
                    widget_states,
                    path,
                );
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
        Widget::VirtualList {
            item_height,
            item_count,
            ..
        }
        | Widget::VirtualListContent {
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
            let rel_y = mouse.y - abs_pos.y + scroll_y;
            let idx = (rel_y / item_height).floor() as usize;
            if idx < *item_count {
                Some(HitResult::VListSelect {
                    id: resolved_id,
                    index: idx,
                })
            } else {
                None
            }
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
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let fallback_state = crate::engine::widget_state::VirtualGridState {
                viewport_w: layout.size.width,
                viewport_h: layout.size.height,
                ..Default::default()
            };
            let grid_state = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_vgrid())
                .unwrap_or(&fallback_state);
            grid_state
                .index_at(
                    mouse.x - abs_pos.x,
                    mouse.y - abs_pos.y,
                    *item_height,
                    *item_count,
                    *columns,
                )
                .map(|index| HitResult::VGridSelect {
                    id: resolved_id,
                    index,
                })
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
        Widget::Container { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            path.push(0);
            let result = hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            result
        }
        _ => None,
    }
}

pub fn collect_input_ids<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>) {
    let mut path = Vec::new();
    collect_input_ids_impl(widget, ids, &mut path);
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

fn collect_stateful_ids_impl<Msg>(
    widget: &Widget<Msg>,
    out: &mut Vec<(u64, &'static str)>,
    path: &mut Vec<usize>,
) {
    match widget {
        Widget::Slider { .. } => out.push((widget.resolved_id(path).unwrap(), "slider")),
        Widget::Select { .. } => out.push((widget.resolved_id(path).unwrap(), "select")),
        Widget::Spinner { .. } => out.push((widget.resolved_id(path).unwrap(), "anim")),
        Widget::ScrollView { child, .. } => {
            out.push((widget.resolved_id(path).unwrap(), "scroll"));
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
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
        Widget::VirtualList { .. } | Widget::VirtualListContent { .. } => {
            out.push((widget.resolved_id(path).unwrap(), "vlist"))
        }
        Widget::VirtualGrid { .. } | Widget::VirtualGridContent { .. } => {
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
) -> Option<(fn(String) -> Msg, Option<Msg>)> {
    find_input_props(widget, target_id).map(|(cb, submit, _, _)| (cb, submit))
}

pub fn find_input_props<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<(fn(String) -> Msg, Option<Msg>, bool, bool)> {
    let mut path = Vec::new();
    find_input_props_impl(widget, target_id, &mut path)
}

fn find_input_props_impl<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
    path: &mut Vec<usize>,
) -> Option<(fn(String) -> Msg, Option<Msg>, bool, bool)> {
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
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
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
            if *open {
                if let Some(popup_node) = ids.get(1).copied() {
                    if let Some(content_node) = taffy
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
                }
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
) -> Option<u64> {
    let mut path = Vec::new();
    find_scroll_focus_impl(widget, taffy, node_id, mouse, abs, &mut path)
}

pub fn find_context_menu_target<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
) -> Option<u64> {
    let mut path = Vec::new();
    find_context_menu_target_impl(widget, taffy, node_id, mouse, abs, &mut path)
}

fn find_context_menu_target_impl<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    path: &mut Vec<usize>,
) -> Option<u64> {
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::ContextMenu { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if !ids.is_empty() {
                path.push(0);
                let hit = find_context_menu_target_impl(child, taffy, ids[0], mouse, abs_pos, path);
                path.pop();
                if hit.is_some() {
                    return hit;
                }
            }
            Some(widget.resolved_id(path).unwrap())
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let anchor_node = ids.first().copied()?;
            path.push(0);
            let hit =
                find_context_menu_target_impl(anchor, taffy, anchor_node, mouse, abs_pos, path);
            path.pop();
            hit
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate().rev() {
                path.push(i);
                let hit = find_context_menu_target_impl(child, taffy, ids[i], mouse, abs_pos, path);
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
            let hit = find_context_menu_target_impl(child, taffy, ids[0], mouse, abs_pos, path);
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
                Point::new(abs_pos.x, abs_pos.y + ACCORDION_HEADER_H),
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
            let hit = find_context_menu_target_impl(child, taffy, ids[0], mouse, abs_pos, path);
            path.pop();
            hit
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
            path.push(0);
            let result = find_scroll_focus_impl(child, taffy, ids[0], mouse, abs_pos, path);
            path.pop();
            result.or(Some(widget.resolved_id(path).unwrap()))
        }
        Widget::VirtualList { .. }
        | Widget::VirtualListContent { .. }
        | Widget::VirtualGrid { .. }
        | Widget::VirtualGridContent { .. } => Some(widget.resolved_id(path).unwrap()),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate().rev() {
                path.push(i);
                let result = find_scroll_focus_impl(child, taffy, ids[i], mouse, abs_pos, path);
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
            let result = find_scroll_focus_impl(child, taffy, ids[0], mouse, abs_pos, path);
            path.pop();
            result
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node_id).ok()?;
            let anchor_node = ids.first().copied()?;
            path.push(0);
            let result = find_scroll_focus_impl(anchor, taffy, anchor_node, mouse, abs_pos, path);
            path.pop();
            result
        }
        _ => None,
    }
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
            let thumb_y = abs_pos.y + s.thumb_y();
            let sb_x = abs_pos.x + layout.size.width - SCROLLBAR_W - 2.0;
            let thumb_rect = SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h);
            if thumb_rect.contains(mouse) {
                return Some(ScrollbarDragHit {
                    id: resolved_id,
                    start_offset: s.offset_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: s.content_height,
                });
            }
            None
        }
        Widget::VirtualList {
            item_count,
            item_height,
            ..
        }
        | Widget::VirtualListContent {
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
            let thumb_y = abs_pos.y + s.thumb_y(*item_height, *item_count);
            let sb_x = abs_pos.x + layout.size.width - SCROLLBAR_W - 2.0;
            let thumb_rect = SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h);
            if thumb_rect.contains(mouse) {
                return Some(ScrollbarDragHit {
                    id: resolved_id,
                    start_offset: s.scroll_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: total_h,
                });
            }
            None
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
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let s = widget_states.get(&resolved_id)?.as_vgrid()?;
            let total_h = virtual_grid_row_count(*item_count, *columns) as f32 * *item_height;
            if total_h <= s.viewport_h {
                return None;
            }
            let thumb_h =
                (layout.size.height * s.thumb_ratio(*item_height, *item_count, *columns)).max(20.0);
            let thumb_y = abs_pos.y + s.thumb_y(*item_height, *item_count, *columns);
            let sb_x = abs_pos.x + layout.size.width - SCROLLBAR_W - 2.0;
            let thumb_rect = SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h);
            if thumb_rect.contains(mouse) {
                return Some(ScrollbarDragHit {
                    id: resolved_id,
                    start_offset: s.scroll_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: total_h,
                });
            }
            None
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use skia_safe::Point;
    use taffy::prelude::{Dimension, Size, Style, TaffyTree};
    use winit::dpi::PhysicalSize;

    use super::{HitResult, collect_input_ids, collect_stateful_ids, dialog_card_rect, hit_test};
    use crate::layout::{build_taffy_tree, compute_layout};
    use crate::widget::{AUTO_ID, ButtonVariant, DialogPosition, InputState, Widget};

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Str(String),
        Usize(usize),
        Toggle,
    }

    fn text_msg(value: String) -> Msg {
        Msg::Str(value)
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
