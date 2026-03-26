// ============================================================
// Rutter Framework — render/hit_test.rs  (Fase 3)
//
// HitResult estendido com:
//   SliderPress   — inicia drag de Slider
//   SelectToggle  — abre/fecha dropdown
//   SelectOption  — escolhe uma opção no dropdown aberto
//   ScrollWheel   — tratado diretamente no runner
//   CheckboxToggle, SwitchToggle, RadioSelect — mensagens diretas
// ============================================================

use skia_safe::{Contains, Point, Rect as SkiaRect};
use taffy::prelude::{NodeId, TaffyTree};

use crate::engine::widget_state::WidgetState;
use crate::layout::{RutterContext, OPTION_HEIGHT};
use crate::widget::Widget;
use std::collections::HashMap;

// ── Resultado ────────────────────────────────────────────────

pub enum HitResult<Msg> {
    /// Widget emitiu mensagem (Button, Checkbox, Switch, Radio).
    Message(Msg),
    /// TextInput foi clicado → dar foco.
    InputFocus(u64),
    /// Slider foi clicado/pressionado → iniciar drag.
    SliderPress {
        id:        u64,
        cursor_x:  f32, // posição absoluta do cursor
        abs_track_x: f32, // posição X absoluta do início do track
        track_w:   f32, // largura do track
        min:       f32,
        max:       f32,
        step:      f32,
    },
    /// Select clicado (header) → toggle open.
    SelectToggle(u64),
    /// Select option clicada quando aberto.
    SelectOption { id: u64, index: usize, on_change: fn(usize) -> () },
    /// ScrollView clicado → habilitar scroll wheel nele.
    ScrollFocus(u64),
}

// ── Hit test ─────────────────────────────────────────────────

pub fn hit_test<Msg: Clone>(
    widget:        &Widget<Msg>,
    taffy:         &TaffyTree<RutterContext>,
    node_id:       NodeId,
    mouse:         Point,
    abs:           Point,
    widget_states: &HashMap<u64, WidgetState>,
) -> Option<HitResult<Msg>> {
    let layout  = taffy.layout(node_id).unwrap();
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let rect    = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, layout.size.width, layout.size.height);
    if !rect.contains(mouse) { return None; }

    match widget {
        Widget::Button { on_press, .. } =>
            Some(HitResult::Message(on_press.clone())),

        Widget::TextInput { id, .. } =>
            Some(HitResult::InputFocus(*id)),

        Widget::Checkbox { checked, on_change, .. } =>
            Some(HitResult::Message(on_change(!checked))),

        Widget::Switch { checked, on_change, .. } =>
            Some(HitResult::Message(on_change(!checked))),

        Widget::Radio { on_select, .. } =>
            Some(HitResult::Message(on_select())),

        Widget::Slider { id, min, max, step, .. } => {
            // Track ocupa a faixa central, com 16px de padding de cada lado
            let pad      = 16.0_f32;
            let track_x  = abs_pos.x + pad;
            let track_w  = layout.size.width - pad * 2.0;
            Some(HitResult::SliderPress {
                id:          *id,
                cursor_x:    mouse.x,
                abs_track_x: track_x,
                track_w,
                min:         *min,
                max:         *max,
                step:        *step,
            })
        }

        Widget::Select { id, options, on_change, selected_index, .. } => {
            let is_open = widget_states.get(id)
                .and_then(|s| s.as_select())
                .map(|s| s.is_open)
                .unwrap_or(false);

            let closed_h = layout.size.height
                - if is_open { options.len() as f32 * OPTION_HEIGHT } else { 0.0 };

            // Click no header → toggle
            if mouse.y < abs_pos.y + closed_h {
                return Some(HitResult::SelectToggle(*id));
            }

            // Click em uma opção quando aberto
            if is_open {
                let rel_y    = mouse.y - (abs_pos.y + closed_h);
                let idx      = (rel_y / OPTION_HEIGHT).floor() as usize;
                let idx      = idx.min(options.len().saturating_sub(1));
                let on_ch    = *on_change;
                let _ = on_ch; // suprime warning
                return Some(HitResult::SelectOption {
                    id:        *id,
                    index:     idx,
                    on_change: |_| {}, // closure placeholder — runner usa on_change real
                });
            }
            None
        }

        Widget::ScrollView { id, child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            // Verificar se clicou no filho (dentro do scroll)
            if let Some(r) = hit_test(child, taffy, ids[0], mouse, abs_pos, widget_states) {
                return Some(r);
            }
            Some(HitResult::ScrollFocus(*id))
        }

        Widget::Tooltip { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            hit_test(child, taffy, ids[0], mouse, abs_pos, widget_states)
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
            for child in children { collect_input_ids(child, ids); }
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } => collect_input_ids(child, ids),
        _ => {}
    }
}

/// Coleta IDs de todos os widgets com estado (Slider, ScrollView, Select, Spinner).
pub fn collect_stateful_ids<Msg>(widget: &Widget<Msg>, out: &mut Vec<(u64, &'static str)>) {
    match widget {
        Widget::Slider    { id, .. } => out.push((*id, "slider")),
        Widget::ScrollView{ id, .. } => out.push((*id, "scroll")),
        Widget::Select    { id, .. } => out.push((*id, "select")),
        Widget::Spinner   { id, .. } => out.push((*id, "anim")),
        Widget::ProgressBar { indeterminate: true, .. } => {
            // ProgressBar indeterminada não tem ID estável — usamos ID fixo derivado
            // da posição; para Fase 3 usamos um ID sintético baseado no hash do node
            // Esta é uma limitação conhecida; Fase 4 adiciona IDs a ProgressBar.
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children { collect_stateful_ids(child, out); }
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. } => {
            collect_stateful_ids(child, out);
        }
        Widget::ScrollView { child, id, .. } => {
            out.push((*id, "scroll"));
            collect_stateful_ids(child, out);
        }
        _ => {}
    }
}

pub fn find_input_callbacks<Msg: Clone>(
    widget:    &Widget<Msg>,
    target_id: u64,
) -> Option<(fn(String) -> Msg, Option<Msg>)> {
    match widget {
        Widget::TextInput { id, on_change, on_submit, .. } if *id == target_id =>
            Some((*on_change, on_submit.clone())),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                if let Some(r) = find_input_callbacks(child, target_id) { return Some(r); }
            }
            None
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } =>
            find_input_callbacks(child, target_id),
        _ => None,
    }
}

/// Encontra o `on_change` de um Select pelo ID.
pub fn find_select_callback<Msg: Clone>(
    widget:    &Widget<Msg>,
    target_id: u64,
) -> Option<fn(usize) -> Msg> {
    match widget {
        Widget::Select { id, on_change, .. } if *id == target_id => Some(*on_change),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                if let Some(r) = find_select_callback(child, target_id) { return Some(r); }
            }
            None
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } =>
            find_select_callback(child, target_id),
        _ => None,
    }
}

/// Encontra o `on_change` de um Slider pelo ID.
pub fn find_slider_callback<Msg: Clone>(
    widget:    &Widget<Msg>,
    target_id: u64,
) -> Option<fn(f32) -> Msg> {
    match widget {
        Widget::Slider { id, on_change, .. } if *id == target_id => Some(*on_change),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                if let Some(r) = find_slider_callback(child, target_id) { return Some(r); }
            }
            None
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } =>
            find_slider_callback(child, target_id),
        _ => None,
    }
}

// ── Testes ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::Style;
    use crate::widget::{InputState, Orientation};

    #[derive(Debug, Clone, PartialEq)]
    enum M { A, B(String), F(bool), Slide(i32), Sel(usize) }

    fn states() -> HashMap<u64, WidgetState> { HashMap::new() }

    #[test]
    fn collect_inputs_across_new_widgets() {
        let w: Widget<M> = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::TextInput { id:1, on_change:|s|M::B(s), on_submit:None, style:Style::default(),
                    label:"",placeholder:"",state:InputState::Idle,error_msg:None,is_password:false },
                Widget::Slider    { id:5, value:0.5, min:0.0, max:1.0, step:0.1, on_change:|_|M::A, style:Style::default(), label:"" },
                Widget::TextInput { id:2, on_change:|s|M::B(s), on_submit:None, style:Style::default(),
                    label:"",placeholder:"",state:InputState::Idle,error_msg:None,is_password:false },
            ],
        };
        let mut ids = vec![];
        collect_input_ids(&w, &mut ids);
        assert_eq!(ids, vec![1, 2], "Slider não deve aparecer em input_ids");
    }

    #[test]
    fn collect_stateful_finds_slider() {
        let w: Widget<M> = Widget::Slider { id:42, value:0.0, min:0.0, max:10.0, step:1.0, on_change:|_|M::A, style:Style::default(), label:"" };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, kind)| *id == 42 && *kind == "slider"));
    }

    #[test]
    fn collect_stateful_finds_select() {
        let w: Widget<M> = Widget::Select { id:7, options:&["a","b"], selected_index:0,
            on_change:|i|M::Sel(i), style:Style::default(), label:"", placeholder:"" };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, kind)| *id == 7 && *kind == "select"));
    }

    #[test]
    fn collect_stateful_finds_spinner() {
        let w: Widget<M> = Widget::Spinner { id:3, style:Style::default() };
        let mut out = vec![];
        collect_stateful_ids(&w, &mut out);
        assert!(out.iter().any(|(id, kind)| *id == 3 && *kind == "anim"));
    }

    #[test]
    fn find_select_callback_found() {
        let w: Widget<M> = Widget::Select { id:9, options:&["x"], selected_index:0,
            on_change:|i|M::Sel(i), style:Style::default(), label:"", placeholder:"" };
        let cb = find_select_callback(&w, 9);
        assert!(cb.is_some());
    }

    #[test]
    fn find_select_callback_missing() {
        let w: Widget<M> = Widget::Select { id:9, options:&["x"], selected_index:0,
            on_change:|i|M::Sel(i), style:Style::default(), label:"", placeholder:"" };
        assert!(find_select_callback(&w, 99).is_none());
    }

    #[test]
    fn find_slider_callback_found() {
        let w: Widget<M> = Widget::Slider { id:3, value:0.5, min:0.0, max:1.0, step:0.1,
            on_change:|_|M::A, style:Style::default(), label:"" };
        assert!(find_slider_callback(&w, 3).is_some());
    }

    #[test]
    fn find_slider_callback_missing() {
        let w: Widget<M> = Widget::Slider { id:3, value:0.5, min:0.0, max:1.0, step:0.1,
            on_change:|_|M::A, style:Style::default(), label:"" };
        assert!(find_slider_callback(&w, 99).is_none());
    }
}
