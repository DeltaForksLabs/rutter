// ============================================================
// Rutter Framework — engine/runner.rs  (Fase 3)
//
// Novidades:
//   • Mouse drag → Slider
//   • Scroll wheel → ScrollView
//   • Click em Select → toggle/option
//   • WaitUntil de 16ms quando has_animated = true
// ============================================================

use std::num::NonZeroU32;
use std::time::Duration;

use cosmic_text::{Action, Edit, Motion};
use skia_safe::Point;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowId,
};

use super::RutterEngine;
use crate::app::AppLogic;
use crate::render::hit_test::{
    HitResult, collect_input_ids, find_input_callbacks,
    find_select_callback, find_slider_callback, hit_test,
};

fn map_key(key: &Key) -> Option<Action> {
    match key {
        Key::Named(NamedKey::ArrowLeft)  => Some(Action::Motion(Motion::Left)),
        Key::Named(NamedKey::ArrowRight) => Some(Action::Motion(Motion::Right)),
        Key::Named(NamedKey::ArrowUp)    => Some(Action::Motion(Motion::Up)),
        Key::Named(NamedKey::ArrowDown)  => Some(Action::Motion(Motion::Down)),
        Key::Named(NamedKey::Home)       => Some(Action::Motion(Motion::Home)),
        Key::Named(NamedKey::End)        => Some(Action::Motion(Motion::End)),
        Key::Named(NamedKey::Backspace)  => Some(Action::Backspace),
        Key::Named(NamedKey::Delete)     => Some(Action::Delete),
        Key::Named(NamedKey::Enter)      => Some(Action::Enter),
        _ => None,
    }
}

pub struct RutterRunner<A: AppLogic> {
    engine:     RutterEngine<A>,
    cursor_pos: Point,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub fn run() {
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Wait);
        let mut r = Self { engine: RutterEngine::new(), cursor_pos: Point::new(0.0, 0.0) };
        el.run_app(&mut r).unwrap();
    }
}

impl<A: AppLogic + 'static> ApplicationHandler for RutterRunner<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.engine.handle_resumed(el);
    }

    fn new_events(&mut self, el: &ActiveEventLoop, _: StartCause) {
        self.engine.maybe_snapshot();

        // Animações: tick a 60fps
        if self.engine.has_animated() {
            if self.engine.tick_animations() {
                self.redraw();
            }
            el.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + Duration::from_millis(16),
            ));
            return;
        }

        // Cursor piscante: 500ms
        if self.engine.focused_input_id.is_some() {
            if self.engine.cursor_blink.tick() { self.redraw(); }
            el.set_control_flow(ControlFlow::WaitUntil(
                self.engine.cursor_blink.next_tick_at(),
            ));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),

            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                    if let Some(s) = self.engine.surface.as_mut() { s.resize(w, h).unwrap(); }
                    self.engine.layout_dirty = true;
                    self.redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.engine.update_scale(scale_factor);
                self.redraw();
            }

            WindowEvent::ModifiersChanged(m) => self.engine.modifiers = m,

            WindowEvent::CursorMoved { position, .. } => {
                let prev = self.cursor_pos;
                self.cursor_pos = Point::new(position.x as f32, position.y as f32);

                // Drag ativo de Slider
                if let Some(sid) = self.engine.drag_slider_id {
                    self.update_slider_drag(sid, self.cursor_pos.x);
                    return;
                }

                // Hover em opcoes de Select — atualiza hovered_option
                self.update_select_hover();

                // Feedback hover/cursor
                if (self.cursor_pos.x - prev.x).abs() > 0.5
                    || (self.cursor_pos.y - prev.y).abs() > 0.5
                {
                    self.redraw();
                }
            }

            // ── Botão esquerdo PRESSED ──────────────────────────
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                let size = self.engine.window.as_ref().unwrap().inner_size();
                self.engine.ensure_layout(size);
                let wt = A::view(&mut self.engine.app_state);

                // Fechar qualquer select aberto ao clicar fora
                let cursor = self.cursor_pos;

                self.engine.focused_input_id = None;

                if let Some(hit) = hit_test(
                    &wt, &self.engine.taffy, self.engine.last_root_node,
                    cursor, Point::new(0.0, 0.0), &self.engine.widget_states,
                ) {
                    match hit {
                        HitResult::Message(msg) => {
                            // Fechar selects abertos
                            self.close_all_selects();
                            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                            self.engine.layout_dirty = true;
                        }
                        HitResult::InputFocus(id) => {
                            self.close_all_selects();
                            self.engine.focused_input_id = Some(id);
                            self.engine.ensure_input_state(id);
                            self.engine.cursor_blink.reset();
                            self.engine.layout_dirty = true;
                            if let Some(s) = self.engine.input_states.get_mut(&id) {
                                s.clear_selection();
                            }
                        }
                        HitResult::SliderPress { id, cursor_x, abs_track_x, track_w, min, max, step } => {
                            self.close_all_selects();
                            self.engine.drag_slider_id = Some(id);
                            // Calcular valor inicial a partir da posição do clique
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(s) = ws.as_slider_mut() {
                                    s.dragging          = true;
                                    s.drag_start_cursor = cursor_x;
                                    s.track_abs_x       = abs_track_x;
                                    s.track_width       = track_w;
                                }
                            }
                            // Emitir valor imediato
                            let norm = ((cursor_x - abs_track_x) / track_w).clamp(0.0, 1.0);
                            let raw  = min + norm * (max - min);
                            let val  = snap_to_step(raw, min, max, step);
                            let wt2  = A::view(&mut self.engine.app_state);
                            if let Some(cb) = find_slider_callback(&wt2, id) {
                                let msg = cb(val);
                                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                                self.engine.layout_dirty = true;
                            }
                        }
                        HitResult::SelectToggle(id) => {
                            let currently_open = self.engine.widget_states.get(&id)
                                .and_then(|s| s.as_select()).map(|s| s.is_open).unwrap_or(false);
                            // Fechar outros selects
                            self.close_all_selects();
                            // Toggling este
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(s) = ws.as_select_mut() {
                                    s.is_open = !currently_open;
                                }
                            }
                            self.engine.layout_dirty = true;
                        }
                        HitResult::SelectOption { id, index, .. } => {
                            // Fechar dropdown
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(s) = ws.as_select_mut() { s.is_open = false; }
                            }
                            // Emitir mudança
                            let wt2 = A::view(&mut self.engine.app_state);
                            if let Some(cb) = find_select_callback(&wt2, id) {
                                let msg = cb(index);
                                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                            }
                            self.engine.layout_dirty = true;
                        }
                        HitResult::ScrollFocus(id) => {
                            self.close_all_selects();
                            self.engine.active_scroll_id = Some(id);
                        }
                    }
                    self.redraw();
                } else {
                    // Clicou fora de tudo
                    self.close_all_selects();
                    self.redraw();
                }
            }

            // ── Botão esquerdo RELEASED ─────────────────────────
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if let Some(sid) = self.engine.drag_slider_id.take() {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_slider_mut() { s.dragging = false; }
                    }
                    self.redraw();
                }
            }

            // ── Scroll wheel ────────────────────────────────────
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y)   => -y * 40.0,
                    MouseScrollDelta::PixelDelta(pos)   => -pos.y as f32,
                };
                if let Some(sid) = self.engine.active_scroll_id {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() { s.scroll_by(dy); }
                    }
                    self.redraw();
                }
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_key(&event.logical_key);
            }

            WindowEvent::RedrawRequested => self.engine.redraw(self.cursor_pos),

            _ => {}
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

impl<A: AppLogic + 'static> RutterRunner<A> {
    fn redraw(&self) {
        if let Some(w) = self.engine.window.as_ref() { w.request_redraw(); }
    }

    fn close_all_selects(&mut self) {
        for ws in self.engine.widget_states.values_mut() {
            if let Some(s) = ws.as_select_mut() { s.is_open = false; }
        }
        self.engine.layout_dirty = true;
    }

    fn update_slider_drag(&mut self, sid: u64, cursor_x: f32) {
        let (min, max, step, val) = {
            let wt = A::view(&mut self.engine.app_state);
            // Encontrar parâmetros do Slider no widget tree
            find_slider_params(&wt, sid).unwrap_or((0.0, 1.0, 0.0, 0.0))
        };

        if let Some(ws) = self.engine.widget_states.get(&sid) {
            if let Some(s) = ws.as_slider() {
                let norm = ((cursor_x - s.track_abs_x) / s.track_width).clamp(0.0, 1.0);
                let raw  = min + norm * (max - min);
                let snapped = snap_to_step(raw, min, max, step);
                let wt2 = A::view(&mut self.engine.app_state);
                if let Some(cb) = find_slider_callback(&wt2, sid) {
                    let msg = cb(snapped);
                    A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                    self.engine.layout_dirty = true;
                    self.redraw();
                }
            }
        }
        let _ = val;
    }

    fn update_select_hover(&mut self) {
        // Atualiza o hovered_option de qualquer Select aberto
        let mouse = Point::new(
            self.cursor_pos.x / self.engine.scale_factor,
            self.cursor_pos.y / self.engine.scale_factor,
        );
        for (id, ws) in self.engine.widget_states.iter_mut() {
            if let Some(s) = ws.as_select_mut() {
                if s.is_open {
                    // Heurística: calculamos com base na posição y do mouse
                    // relativa ao início das opções. Refinamento em Fase 4.
                    s.hovered_option = None; // reset; runner pode refinar
                }
            }
        }
    }

    fn handle_key(&mut self, key: &Key) {
        if let Key::Named(NamedKey::Tab) = key { self.handle_tab(); return; }
        if self.engine.modifiers.state().control_key() {
            if let Key::Character(ch) = key {
                match ch.to_lowercase().as_str() {
                    "c" => { self.do_copy();       return; }
                    "v" => { self.do_paste();      return; }
                    "a" => { self.do_select_all(); return; }
                    "z" => { self.do_undo();       return; }
                    "y" => { self.do_redo();       return; }
                    _   => {}
                }
            }
        }
        // Escape fecha selects abertos
        if let Key::Named(NamedKey::Escape) = key {
            self.close_all_selects(); self.redraw(); return;
        }
        self.handle_text_input(key);
    }

    fn handle_tab(&mut self) {
        let wt = A::view(&mut self.engine.app_state);
        let mut ids = vec![];
        collect_input_ids(&wt, &mut ids);
        if ids.is_empty() { return; }
        let shift = self.engine.modifiers.state().shift_key();
        let next  = match self.engine.focused_input_id {
            Some(cur) => {
                let n = ids.len();
                let i = ids.iter().position(|&x| x == cur).unwrap_or(0);
                ids[if shift { (i + n - 1) % n } else { (i + 1) % n }]
            }
            None => ids[0],
        };
        if let Some(prev) = self.engine.focused_input_id {
            if let Some(s) = self.engine.input_states.get_mut(&prev) { s.snapshot(); }
        }
        self.engine.focused_input_id = Some(next);
        self.engine.ensure_input_state(next);
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn handle_text_input(&mut self, key: &Key) {
        let Some(fid) = self.engine.focused_input_id else { return };
        if let Some(s) = self.engine.input_states.get_mut(&fid) { s.clear_selection(); }
        let wt = A::view(&mut self.engine.app_state);
        let Some((on_change, _)) = find_input_callbacks(&wt, fid) else { return };
        let mut dirty = false;
        let mut full  = String::new();
        if let Some(ist) = self.engine.input_states.get_mut(&fid) {
            if let Some(action) = map_key(key) {
                let mut fs = self.engine.font_system.borrow_mut();
                ist.editor.action(&mut fs, action); dirty = true;
            } else if let Key::Named(NamedKey::Space) = key {
                ist.editor.insert_string(" ", None);
                let t = ist.text(); ist.undo.push(t); dirty = true;
            } else if let Key::Character(t) = key {
                if !t.is_empty() { ist.editor.insert_string(t.as_str(), None); dirty = true; }
            }
            if dirty {
                let vw = 260.0; ist.update_scroll(vw); full = ist.text();
            }
        }
        if dirty {
            self.engine.cursor_blink.reset();
            self.engine.schedule_snapshot();
            let msg = on_change(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            self.engine.layout_dirty = true;
            self.redraw();
        }
    }

    fn do_copy(&mut self) {
        if let Some(fid) = self.engine.focused_input_id {
            if let Some(s) = self.engine.input_states.get(&fid) {
                let _ = self.engine.clipboard.set_text(s.text());
            }
        }
    }

    fn do_paste(&mut self) {
        let Ok(txt)  = self.engine.clipboard.get_text() else { return };
        let Some(fid)= self.engine.focused_input_id else { return };
        let Some(s)  = self.engine.input_states.get_mut(&fid) else { return };
        s.editor.insert_string(&txt, None); s.snapshot();
        let full = s.text();
        let wt   = A::view(&mut self.engine.app_state);
        if let Some((cb, _)) = find_input_callbacks(&wt, fid) {
            let msg = cb(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_select_all(&mut self) {
        let Some(fid) = self.engine.focused_input_id else { return };
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.select_all(&mut fs);
            let _ = self.engine.clipboard.set_text(s.text());
        }
        self.redraw();
    }

    fn do_undo(&mut self) {
        let Some(fid) = self.engine.focused_input_id else { return };
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut(); s.undo(&mut fs);
            drop(fs);
            let full = s.text();
            let wt   = A::view(&mut self.engine.app_state);
            if let Some((cb,_)) = find_input_callbacks(&wt, fid) {
                let msg = cb(full);
                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            }
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_redo(&mut self) {
        let Some(fid) = self.engine.focused_input_id else { return };
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut(); s.redo(&mut fs);
            drop(fs);
            let full = s.text();
            let wt   = A::view(&mut self.engine.app_state);
            if let Some((cb,_)) = find_input_callbacks(&wt, fid) {
                let msg = cb(full);
                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            }
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }
}

// ── Utilitários ───────────────────────────────────────────────

/// Arredonda `value` para o múltiplo de `step` mais próximo dentro de [min, max].
pub fn snap_to_step(value: f32, min: f32, max: f32, step: f32) -> f32 {
    if step <= 0.0 { return value.clamp(min, max); }
    let snapped = (((value - min) / step).round() * step + min).clamp(min, max);
    // Arredondar para evitar floating point noise
    let decimals = (-step.log10().floor()).max(0.0) as i32;
    let factor   = 10_f32.powi(decimals);
    (snapped * factor).round() / factor
}

fn find_slider_params<Msg: Clone>(widget: &crate::widget::Widget<Msg>, id: u64) -> Option<(f32, f32, f32, f32)> {
    use crate::widget::Widget;
    match widget {
        Widget::Slider { id: wid, min, max, step, value, .. } if *wid == id =>
            Some((*min, *max, *step, *value)),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children { if let Some(r) = find_slider_params(c, id) { return Some(r); } }
            None
        }
        Widget::Container { child, .. } | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } => find_slider_params(child, id),
        _ => None,
    }
}

// ── Testes ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_no_step_clamps() {
        assert!((snap_to_step(1.5, 0.0, 1.0, 0.0) - 1.0).abs() < f32::EPSILON);
        assert!((snap_to_step(-0.5, 0.0, 1.0, 0.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn snap_step_1_rounds_down() {
        assert!((snap_to_step(2.3, 0.0, 10.0, 1.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn snap_step_1_rounds_up() {
        assert!((snap_to_step(2.7, 0.0, 10.0, 1.0) - 3.0).abs() < 0.001);
    }

    #[test]
    fn snap_step_0_5() {
        assert!((snap_to_step(2.3, 0.0, 10.0, 0.5) - 2.5).abs() < 0.001);
        assert!((snap_to_step(2.1, 0.0, 10.0, 0.5) - 2.0).abs() < 0.001);
    }

    #[test]
    fn snap_step_10() {
        assert!((snap_to_step(14.0, 0.0, 100.0, 10.0) - 10.0).abs() < 0.001);
        assert!((snap_to_step(16.0, 0.0, 100.0, 10.0) - 20.0).abs() < 0.001);
    }

    #[test]
    fn snap_at_min() {
        assert!((snap_to_step(0.0, 0.0, 100.0, 5.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn snap_at_max() {
        assert!((snap_to_step(100.0, 0.0, 100.0, 5.0) - 100.0).abs() < 0.001);
    }

    #[test]
    fn snap_negative_min() {
        assert!((snap_to_step(-8.0, -10.0, 10.0, 5.0) - (-10.0)).abs() < 0.001);
        assert!((snap_to_step(-3.0, -10.0, 10.0, 5.0) - (-5.0)).abs() < 0.001);
    }

    #[test]
    fn snap_clamped_below_min() {
        assert!((snap_to_step(-5.0, 0.0, 10.0, 1.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn snap_clamped_above_max() {
        assert!((snap_to_step(15.0, 0.0, 10.0, 1.0) - 10.0).abs() < 0.001);
    }
}
