// ============================================================
// Rutter Framework — render/hit_test.rs
// ============================================================

use skia_safe::{Contains, Point, Rect as SkiaRect};
use std::collections::HashMap;
use taffy::prelude::{NodeId, TaffyTree};

use crate::engine::widget_state::WidgetState;
use crate::layout::{OPTION_HEIGHT, RutterContext, SCROLLBAR_W};
use crate::widget::Widget;

const ACCORDION_HEADER_H: f32 = 44.0;

pub enum HitResult<Msg> {
    Message(Msg),
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
        index: usize,
    },
    ModalDismiss(u64),
    VListSelect {
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

pub fn hit_test<Msg: Clone>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    mouse: Point,
    abs: Point,
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<HitResult<Msg>> {
    let layout = taffy.layout(node_id).unwrap();
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::Button { on_press, .. } => Some(HitResult::Message(on_press.clone())),
        Widget::TextInput { id, .. }
        | Widget::TextArea { id, .. }
        | Widget::SearchBar { id, .. } => Some(HitResult::InputFocus {
            id: *id,
            local_x: mouse.x - abs_pos.x,
            local_y: mouse.y - abs_pos.y,
            width: layout.size.width,
            height: layout.size.height,
        }),
        Widget::Checkbox {
            checked, on_change, ..
        } => Some(HitResult::Message(on_change(!checked))),
        Widget::Switch {
            checked, on_change, ..
        } => Some(HitResult::Message(on_change(!checked))),
        Widget::Radio { on_select, .. } => Some(HitResult::Message(on_select())),
        Widget::Slider {
            id, min, max, step, ..
        } => {
            let pad = 16.0_f32;
            let track_x = abs_pos.x + pad;
            let track_w = layout.size.width - pad * 2.0;
            Some(HitResult::SliderPress {
                id: *id,
                cursor_x: mouse.x,
                abs_track_x: track_x,
                track_w,
                min: *min,
                max: *max,
                step: *step,
            })
        }
        Widget::Select { id, options, .. } => {
            let is_open = widget_states
                .get(id)
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
                return Some(HitResult::SelectToggle(*id));
            }
            if is_open {
                let rel_y = mouse.y - (abs_pos.y + closed_h);
                let idx = (rel_y / OPTION_HEIGHT).floor() as usize;
                let idx = idx.min(options.len().saturating_sub(1));
                return Some(HitResult::SelectOption {
                    id: *id,
                    index: idx,
                });
            }
            None
        }
        Widget::ScrollView { id, child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            if let Some(r) = hit_test(child, taffy, ids[0], mouse, abs_pos, widget_states) {
                return Some(r);
            }
            Some(HitResult::ScrollFocus(*id))
        }
        Widget::Tooltip { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            hit_test(child, taffy, ids[0], mouse, abs_pos, widget_states)
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
                return Some(HitResult::Message(on_toggle.clone()));
            }
            if *expanded {
                let ids = taffy.children(node_id).unwrap();
                if ids.is_empty() {
                    return None;
                }
                return hit_test(
                    child,
                    taffy,
                    ids[0],
                    mouse,
                    Point::new(abs_pos.x, abs_pos.y + ACCORDION_HEADER_H),
                    widget_states,
                );
            }
            None
        }
        Widget::TabBar { id, tabs, .. } => {
            if tabs.is_empty() {
                return None;
            }
            let tab_w = layout.size.width / tabs.len() as f32;
            let idx = ((mouse.x - abs_pos.x) / tab_w).floor() as usize;
            let idx = idx.min(tabs.len().saturating_sub(1));
            Some(HitResult::TabPress {
                id: *id,
                index: idx,
            })
        }
        Widget::Modal {
            id,
            visible,
            child,
            on_dismiss,
            ..
        } => {
            if !visible {
                return None;
            }
            let ids = taffy.children(node_id).unwrap();
            let child_hit = hit_test(child, taffy, ids[0], mouse, abs_pos, widget_states);
            if child_hit.is_some() {
                return child_hit;
            }
            if let Some(msg) = on_dismiss.clone() {
                Some(HitResult::Message(msg))
            } else {
                Some(HitResult::ModalDismiss(*id))
            }
        }
        Widget::Dialog {
            id,
            visible,
            on_confirm,
            on_cancel,
            ..
        } => {
            if !visible {
                return None;
            }
            let card_w = 400.0;
            let card_h = 200.0;
            let card_x = (layout.size.width - card_w) / 2.0;
            let card_y = (layout.size.height - card_h) / 2.0;
            let cancel_w = 100.0;
            let confirm_w = 100.0;
            let btn_h = 36.0;
            let cancel_rect = SkiaRect::from_xywh(
                abs_pos.x + card_x + card_w - 24.0 - confirm_w - 12.0 - cancel_w,
                abs_pos.y + card_y + card_h - 24.0 - btn_h,
                cancel_w,
                btn_h,
            );
            let confirm_rect = SkiaRect::from_xywh(
                abs_pos.x + card_x + card_w - 24.0 - confirm_w,
                abs_pos.y + card_y + card_h - 24.0 - btn_h,
                confirm_w,
                btn_h,
            );

            if confirm_rect.contains(mouse) {
                return Some(HitResult::Message(on_confirm.clone()));
            }
            if cancel_rect.contains(mouse) {
                return Some(HitResult::Message(on_cancel.clone()));
            }
            Some(HitResult::ModalDismiss(*id))
        }
        Widget::VirtualList {
            id,
            item_height,
            item_count,
            ..
        } => {
            let scroll_y = widget_states
                .get(id)
                .and_then(|s| s.as_vlist())
                .map(|v| v.scroll_y)
                .unwrap_or(0.0);
            let rel_y = mouse.y - abs_pos.y + scroll_y;
            let idx = (rel_y / item_height).floor() as usize;
            if idx < *item_count {
                Some(HitResult::VListSelect {
                    id: *id,
                    index: idx,
                })
            } else {
                None
            }
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).unwrap();
            for (i, child) in children.iter().enumerate().rev() {
                if let Some(r) = hit_test(child, taffy, ids[i], mouse, abs_pos, widget_states) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            hit_test(child, taffy, ids[0], mouse, abs_pos, widget_states)
        }
        _ => None,
    }
}

pub fn collect_input_ids<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>) {
    match widget {
        Widget::TextInput { id, .. }
        | Widget::TextArea { id, .. }
        | Widget::SearchBar { id, .. } => ids.push(*id),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                collect_input_ids(c, ids);
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => collect_input_ids(child, ids),
        _ => {}
    }
}

pub fn collect_stateful_ids<Msg>(widget: &Widget<Msg>, out: &mut Vec<(u64, &'static str)>) {
    match widget {
        Widget::Slider { id, .. } => out.push((*id, "slider")),
        Widget::Select { id, .. } => out.push((*id, "select")),
        Widget::Spinner { id, .. } => out.push((*id, "anim")),
        Widget::ScrollView { id, child, .. } => {
            out.push((*id, "scroll"));
            collect_stateful_ids(child, out);
        }
        Widget::ProgressBar {
            id,
            indeterminate: true,
            ..
        } => out.push((*id, "anim")),
        Widget::TabBar { id, .. } => out.push((*id, "tab")),
        Widget::Toast { id, .. } => out.push((*id, "toast")),
        Widget::VirtualList { id, .. } => out.push((*id, "vlist")),
        Widget::Modal { id, child, .. } => {
            out.push((*id, "modal"));
            collect_stateful_ids(child, out);
        }
        Widget::Dialog { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Container { child, .. }
        | Widget::Tooltip { child, .. } => collect_stateful_ids(child, out),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                collect_stateful_ids(c, out);
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
    match widget {
        Widget::TextInput {
            id,
            on_change,
            on_submit,
            is_password,
            ..
        } if *id == target_id => Some((*on_change, on_submit.clone(), *is_password, false)),
        Widget::TextArea {
            id,
            on_change,
            on_submit,
            ..
        } if *id == target_id => Some((*on_change, on_submit.clone(), false, true)),
        Widget::SearchBar {
            id,
            on_change,
            on_submit,
            ..
        } if *id == target_id => Some((*on_change, on_submit.clone(), false, false)),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_input_props(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_input_props(child, target_id),
        _ => None,
    }
}

pub fn find_select_callback<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<fn(usize) -> Msg> {
    match widget {
        Widget::Select { id, on_change, .. } if *id == target_id => Some(*on_change),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_select_callback(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_select_callback(child, target_id),
        _ => None,
    }
}

pub fn find_slider_callback<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<fn(f32) -> Msg> {
    match widget {
        Widget::Slider { id, on_change, .. } if *id == target_id => Some(*on_change),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_slider_callback(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_slider_callback(child, target_id),
        _ => None,
    }
}

pub fn find_vlist_props<Msg>(widget: &Widget<Msg>, target_id: u64) -> Option<(f32, usize)> {
    match widget {
        Widget::VirtualList {
            id,
            item_height,
            item_count,
            ..
        } if *id == target_id => Some((*item_height, *item_count)),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_vlist_props(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_vlist_props(child, target_id),
        _ => None,
    }
}

pub fn find_toast_dismiss_msg<Msg: Clone>(widget: &Widget<Msg>, target_id: u64) -> Option<Msg> {
    match widget {
        Widget::Toast { id, on_dismiss, .. } if *id == target_id => on_dismiss.clone(),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_toast_dismiss_msg(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => find_toast_dismiss_msg(child, target_id),
        _ => None,
    }
}

pub fn find_input_geometry<Msg>(
    widget: &Widget<Msg>,
    taffy: &TaffyTree<RutterContext>,
    node_id: NodeId,
    target_id: u64,
) -> Option<InputGeometry> {
    match widget {
        Widget::TextInput { id, .. }
        | Widget::TextArea { id, .. }
        | Widget::SearchBar { id, .. }
            if *id == target_id =>
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
                if let Some(r) = find_input_geometry(child, taffy, ids[i], target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            find_input_geometry(child, taffy, ids[0], target_id)
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
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::ScrollView { id, child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            find_scroll_focus(child, taffy, ids[0], mouse, abs_pos).or(Some(*id))
        }
        Widget::VirtualList { id, .. } => Some(*id),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).ok()?;
            for (i, child) in children.iter().enumerate().rev() {
                if let Some(id) = find_scroll_focus(child, taffy, ids[i], mouse, abs_pos) {
                    return Some(id);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            find_scroll_focus(child, taffy, ids[0], mouse, abs_pos)
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
    let layout = taffy.layout(node_id).ok()?;
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) {
        return None;
    }

    match widget {
        Widget::ScrollView { id, child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if let Some(hit) =
                find_scrollbar_drag_hit(child, taffy, ids[0], mouse, abs_pos, widget_states)
            {
                return Some(hit);
            }
            let s = widget_states.get(id)?.as_scroll()?;
            if s.content_height <= s.viewport_h {
                return None;
            }
            let thumb_h = (layout.size.height * s.thumb_ratio()).max(20.0);
            let thumb_y = abs_pos.y + s.thumb_y();
            let sb_x = abs_pos.x + layout.size.width - SCROLLBAR_W - 2.0;
            let thumb_rect = SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h);
            if thumb_rect.contains(mouse) {
                return Some(ScrollbarDragHit {
                    id: *id,
                    start_offset: s.offset_y,
                    viewport_h: s.viewport_h.max(layout.size.height),
                    content_h: s.content_height,
                });
            }
            None
        }
        Widget::VirtualList {
            id,
            item_count,
            item_height,
            ..
        } => {
            let s = widget_states.get(id)?.as_vlist()?;
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
                    id: *id,
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
                if let Some(hit) =
                    find_scrollbar_drag_hit(child, taffy, ids[i], mouse, abs_pos, widget_states)
                {
                    return Some(hit);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            let ids = taffy.children(node_id).ok()?;
            if ids.is_empty() {
                return None;
            }
            find_scrollbar_drag_hit(child, taffy, ids[0], mouse, abs_pos, widget_states)
        }
        _ => None,
    }
}
