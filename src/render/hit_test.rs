// ============================================================
// Rutter Framework — render/hit_test.rs
// ============================================================

use skia_safe::{Contains, Point, Rect as SkiaRect};
use std::collections::HashMap;
use taffy::prelude::{NodeId, TaffyTree};

use crate::engine::widget_state::{WidgetState, virtual_grid_row_count};
use crate::layout::{OPTION_HEIGHT, RutterContext, SCROLLBAR_W};
use crate::widget::{DialogAction, Widget};

const ACCORDION_HEADER_H: f32 = 44.0;

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
        Widget::Button { on_press, .. } => Some(HitResult::Message {
            focus_id: widget.keyboard_focus_id(path),
            msg: on_press.clone(),
        }),
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
            path.push(0);
            let child_hit =
                hit_test_impl(child, taffy, ids[0], mouse, abs_pos, widget_states, path);
            path.pop();
            if child_hit.is_some() {
                return child_hit;
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
            Some(HitResult::ModalDismiss(widget.resolved_id(path).unwrap()))
        }
        Widget::VirtualList {
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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            collect_input_ids_impl(child, ids, path);
            path.pop();
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
        Widget::VirtualList { .. } => out.push((widget.resolved_id(path).unwrap(), "vlist")),
        Widget::VirtualGrid { .. } => out.push((widget.resolved_id(path).unwrap(), "vgrid")),
        Widget::Modal { child, .. } => {
            out.push((widget.resolved_id(path).unwrap(), "modal"));
            path.push(0);
            collect_stateful_ids_impl(child, out, path);
            path.pop();
        }
        Widget::Dialog { child, .. }
        | Widget::Accordion { child, .. }
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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_input_props_impl(child, target_id, path);
            path.pop();
            result
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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_select_callback_impl(child, target_id, path);
            path.pop();
            result
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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_slider_callback_impl(child, target_id, path);
            path.pop();
            result
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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_vlist_props_impl(child, target_id, path);
            path.pop();
            result
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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            let result = find_toast_dismiss_msg_impl(child, target_id, path);
            path.pop();
            result
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
        Widget::VirtualList { .. } | Widget::VirtualGrid { .. } => {
            Some(widget.resolved_id(path).unwrap())
        }
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
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use taffy::prelude::Style;

    use super::{collect_input_ids, collect_stateful_ids};
    use crate::widget::{AUTO_ID, InputState, Widget};

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
}
