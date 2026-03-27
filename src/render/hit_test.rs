// ============================================================
// Rutter Framework — render/hit_test.rs  (Fase 4)
//
// Correções de warnings do v5:
//   • selected_index agora usada no match de Select
//   • padrão duplicado de ScrollView consolidado
//   • _mouse e _id prefixados onde não usados
//
// Novos handlers:
//   TabPress       — clique em aba
//   ModalDismiss   — clique no backdrop
//   VListSelect    — clique em item da VirtualList
// ============================================================

use skia_safe::{Contains, Point, Rect as SkiaRect};
use std::collections::HashMap;
use taffy::prelude::{NodeId, TaffyTree};

use crate::engine::widget_state::WidgetState;
use crate::layout::{OPTION_HEIGHT, RutterContext};
use crate::widget::Widget;

// ── Resultado ────────────────────────────────────────────────

pub enum HitResult<Msg> {
    Message(Msg),
    InputFocus(u64),
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

// ── Hit test ─────────────────────────────────────────────────

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

        Widget::TextInput { id, .. } => Some(HitResult::InputFocus(*id)),

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

        // FIX WARNING: selected_index agora prefixado com _ (não usada aqui,
        // é lida do AppState pelo runner).
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

        // FIX WARNING: padrão duplicado de ScrollView consolidado em um único arm.
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

        // ── Fase 4 ────────────────────────────────────────────
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
            // Clicou no backdrop
            if let Some(msg) = on_dismiss.clone() {
                Some(HitResult::Message(msg))
            } else {
                Some(HitResult::ModalDismiss(*id))
            }
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

// ── Coleta de IDs ─────────────────────────────────────────────

pub fn collect_input_ids<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>) {
    match widget {
        Widget::TextInput { id, .. } => ids.push(*id),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                collect_input_ids(c, ids);
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Modal { child, .. } => {
            collect_input_ids(child, ids);
        }
        _ => {}
    }
}

/// Coleta IDs de widgets com estado interno.
pub fn collect_stateful_ids<Msg>(widget: &Widget<Msg>, out: &mut Vec<(u64, &'static str)>) {
    match widget {
        Widget::Slider { id, .. } => out.push((*id, "slider")),
        Widget::Select { id, .. } => out.push((*id, "select")),
        Widget::Spinner { id, .. } => out.push((*id, "anim")),
        // FIX WARNING: ScrollView tratado em único arm (sem duplicação)
        Widget::ScrollView { id, child, .. } => {
            out.push((*id, "scroll"));
            collect_stateful_ids(child, out);
        }
        Widget::ProgressBar {
            id,
            indeterminate: true,
            ..
        } => {
            out.push((*id, "anim"));
        }
        // Fase 4
        Widget::TabBar { id, .. } => out.push((*id, "tab")),
        Widget::Toast { id, .. } => out.push((*id, "toast")),
        Widget::VirtualList { id, .. } => out.push((*id, "vlist")),
        Widget::Modal { id, child, .. } => {
            out.push((*id, "modal"));
            collect_stateful_ids(child, out);
        }

        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                collect_stateful_ids(c, out);
            }
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. } => {
            collect_stateful_ids(child, out);
        }
        _ => {}
    }
}

pub fn find_input_callbacks<Msg: Clone>(
    widget: &Widget<Msg>,
    target_id: u64,
) -> Option<(fn(String) -> Msg, Option<Msg>)> {
    match widget {
        Widget::TextInput {
            id,
            on_change,
            on_submit,
            ..
        } if *id == target_id => Some((*on_change, on_submit.clone())),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_input_callbacks(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Modal { child, .. } => find_input_callbacks(child, target_id),
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
        | Widget::Modal { child, .. } => find_select_callback(child, target_id),
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
        | Widget::Modal { child, .. } => find_slider_callback(child, target_id),
        _ => None,
    }
}

// ── Testes ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::{InputState, ToastKind};
    use taffy::prelude::Style;

    #[derive(Debug, Clone, PartialEq)]
    enum M {
        A,
        Str(String),
        Bool(bool),
        Usize(usize),
        Float(f32),
    }

    fn empty() -> HashMap<u64, WidgetState> {
        HashMap::new()
    }

    fn input(id: u64) -> Widget<'static, M> {
        Widget::TextInput {
            id,
            on_change: M::Str,
            on_submit: None,
            style: Style::default(),
            label: "",
            placeholder: "",
            state: InputState::Idle,
            error_msg: None,
            is_password: false,
        }
    }

    fn slider(id: u64) -> Widget<'static, M> {
        Widget::Slider {
            id,
            value: 0.5,
            min: 0.0,
            max: 1.0,
            step: 0.1,
            on_change: M::Float,
            style: Style::default(),
            label: "",
        }
    }

    // ── collect_input_ids ─────────────────────────────────────

    #[test]
    fn inputs_not_found_in_tabbar() {
        let w: Widget<M> = Widget::TabBar {
            id: 1,
            tabs: &["A", "B"],
            active: 0,
            on_change: M::Usize,
            style: Style::default(),
        };
        let mut ids = vec![];
        collect_input_ids(&w, &mut ids);
        assert!(ids.is_empty());
    }

    #[test]
    fn inputs_found_inside_modal() {
        let w: Widget<M> = Widget::Modal {
            id: 1,
            visible: true,
            on_dismiss: None,
            style: Style::default(),
            child: Box::new(input(77)),
        };
        let mut ids = vec![];
        collect_input_ids(&w, &mut ids);
        assert_eq!(ids, vec![77]);
    }

    #[test]
    fn collect_inputs_skips_virtual_list() {
        let w: Widget<M> = Widget::VirtualList {
            id: 3,
            item_height: 30.0,
            item_count: 100,
            items: &|_| None,
            on_select: M::Usize,
            style: Style::default(),
        };
        let mut ids = vec![];
        collect_input_ids(&w, &mut ids);
        assert!(ids.is_empty());
    }

    // ── collect_stateful_ids ─────────────────────────────────

    #[test]
    fn stateful_finds_tabbar() {
        let w: Widget<M> = Widget::TabBar {
            id: 5,
            tabs: &["X"],
            active: 0,
            on_change: M::Usize,
            style: Style::default(),
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, k)| *id == 5 && *k == "tab"));
    }

    #[test]
    fn stateful_finds_modal() {
        let w: Widget<M> = Widget::Modal {
            id: 7,
            visible: false,
            on_dismiss: None,
            style: Style::default(),
            child: Box::new(Widget::Spacer {
                style: Style::default(),
            }),
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, k)| *id == 7 && *k == "modal"));
    }

    #[test]
    fn stateful_finds_toast() {
        let w: Widget<M> = Widget::Toast {
            id: 9,
            message: "Hi",
            kind: ToastKind::Info,
            duration_ms: 3000,
            on_dismiss: None,
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, k)| *id == 9 && *k == "toast"));
    }

    #[test]
    fn stateful_finds_vlist() {
        let w: Widget<M> = Widget::VirtualList {
            id: 11,
            item_height: 30.0,
            item_count: 50,
            items: &|_| None,
            on_select: M::Usize,
            style: Style::default(),
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, k)| *id == 11 && *k == "vlist"));
    }

    #[test]
    fn stateful_progress_bar_indeterminate_registered() {
        let w: Widget<M> = Widget::ProgressBar {
            id: 20,
            value: 0.0,
            indeterminate: true,
            style: Style::default(),
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, k)| *id == 20 && *k == "anim"));
    }

    #[test]
    fn stateful_progress_bar_determinate_not_registered() {
        let w: Widget<M> = Widget::ProgressBar {
            id: 21,
            value: 0.5,
            indeterminate: false,
            style: Style::default(),
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.is_empty());
    }

    // ── Callbacks ────────────────────────────────────────────

    #[test]
    fn find_input_inside_modal() {
        let w: Widget<M> = Widget::Modal {
            id: 1,
            visible: true,
            on_dismiss: None,
            style: Style::default(),
            child: Box::new(input(42)),
        };
        assert!(find_input_callbacks(&w, 42).is_some());
        assert!(find_input_callbacks(&w, 99).is_none());
    }

    #[test]
    fn find_slider_inside_scroll() {
        let w: Widget<M> = Widget::ScrollView {
            id: 1,
            style: Style::default(),
            child: Box::new(slider(33)),
        };
        assert!(find_slider_callback(&w, 33).is_some());
    }

    #[test]
    fn no_duplicate_scroll_ids_in_collect() {
        // FIX: padrão duplicado do v5 causaria double-push do ID "scroll"
        let w: Widget<M> = Widget::ScrollView {
            id: 5,
            style: Style::default(),
            child: Box::new(Widget::Spacer {
                style: Style::default(),
            }),
        };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        let scroll_count = out
            .iter()
            .filter(|(id, k)| *id == 5 && *k == "scroll")
            .count();
        assert_eq!(scroll_count, 1, "ID de scroll não deve aparecer duas vezes");
    }
}
