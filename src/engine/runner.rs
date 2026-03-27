// ============================================================
// Rutter Framework — engine/runner.rs  (Fase 4 + fixes v6.2)
//
// Correções de warnings do v5:
//   • _mouse  (variável não usada → prefixada com _)
//   • _id     (loop variable não usada → prefixada com _)
//
// Novos eventos:
//   TabPress       → emite on_change(index) do TabBar
//   ModalDismiss   → fecha Modal sem mensagem
//   VListSelect    → emite on_select(index) da VirtualList
//   Toast timer    → verifica expiração a cada tick
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
use crate::layout::SCROLLBAR_W;
use crate::render::hit_test::{
    HitResult, collect_input_ids, find_input_callbacks, find_select_callback, find_slider_callback,
    hit_test,
};

// ── Helpers ───────────────────────────────────────────────────

fn map_key(key: &Key) -> Option<Action> {
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(Action::Motion(Motion::Left)),
        Key::Named(NamedKey::ArrowRight) => Some(Action::Motion(Motion::Right)),
        Key::Named(NamedKey::ArrowUp) => Some(Action::Motion(Motion::Up)),
        Key::Named(NamedKey::ArrowDown) => Some(Action::Motion(Motion::Down)),
        Key::Named(NamedKey::Home) => Some(Action::Motion(Motion::Home)),
        Key::Named(NamedKey::End) => Some(Action::Motion(Motion::End)),
        Key::Named(NamedKey::Backspace) => Some(Action::Backspace),
        Key::Named(NamedKey::Delete) => Some(Action::Delete),
        Key::Named(NamedKey::Enter) => Some(Action::Enter),
        _ => None,
    }
}

/// Arredonda valor para múltiplo de step dentro de [min, max].
pub fn snap_to_step(value: f32, min: f32, max: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return value.clamp(min, max);
    }
    let snapped = (((value - min) / step).round() * step + min).clamp(min, max);
    let decimals = (-step.log10().floor()).max(0.0) as i32;
    let factor = 10_f32.powi(decimals);
    (snapped * factor).round() / factor
}

fn find_slider_params<Msg: Clone>(
    widget: &crate::widget::Widget<Msg>,
    id: u64,
) -> Option<(f32, f32, f32, f32)> {
    use crate::widget::Widget;
    match widget {
        Widget::Slider {
            id: wid,
            min,
            max,
            step,
            value,
            ..
        } if *wid == id => Some((*min, *max, *step, *value)),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_slider_params(c, id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Modal { child, .. } => find_slider_params(child, id),
        _ => None,
    }
}

pub fn find_tab_callback<Msg: Clone>(
    widget: &crate::widget::Widget<Msg>,
    target_id: u64,
) -> Option<fn(usize) -> Msg> {
    use crate::widget::Widget;
    match widget {
        Widget::TabBar { id, on_change, .. } if *id == target_id => Some(*on_change),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_tab_callback(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Modal { child, .. } => find_tab_callback(child, target_id),
        _ => None,
    }
}

pub fn find_vlist_callback<Msg: Clone>(
    widget: &crate::widget::Widget<Msg>,
    target_id: u64,
) -> Option<fn(usize) -> Msg> {
    use crate::widget::Widget;
    match widget {
        Widget::VirtualList { id, on_select, .. } if *id == target_id => Some(*on_select),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for c in children {
                if let Some(r) = find_vlist_callback(c, target_id) {
                    return Some(r);
                }
            }
            None
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Modal { child, .. } => find_vlist_callback(child, target_id),
        _ => None,
    }
}

// ── FIX-4: estado de arraste do scrollbar ────────────────────

/// Estado de drag do polegar da scrollbar.
#[derive(Debug, Clone)]
struct ScrollDrag {
    id: u64,
    start_y: f32,      // y do mouse no início do drag (coord física)
    start_offset: f32, // offset_y do scroll no início do drag
    viewport_h: f32,   // altura da viewport (para calcular proporção)
    content_h: f32,    // altura total do conteúdo
}

// ── Runner ───────────────────────────────────────────────────

pub struct RutterRunner<A: AppLogic> {
    engine: RutterEngine<A>,
    cursor_pos: Point,
    /// FIX-4: drag do polegar do scrollbar.
    scroll_drag: Option<ScrollDrag>,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub fn run() {
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Wait);
        let mut r = Self {
            engine: RutterEngine::new(),
            cursor_pos: Point::new(0.0, 0.0),
            scroll_drag: None,
        };
        el.run_app(&mut r).unwrap();
    }
}

impl<A: AppLogic + 'static> ApplicationHandler for RutterRunner<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.engine.handle_resumed(el);
    }

    fn new_events(&mut self, el: &ActiveEventLoop, _: StartCause) {
        self.engine.maybe_snapshot();

        let mut toast_expired = false;
        for ws in self.engine.widget_states.values() {
            if let Some(t) = ws.as_toast() {
                if t.is_expired() && t.visible {
                    toast_expired = true;
                    break;
                }
            }
        }
        if toast_expired {
            for ws in self.engine.widget_states.values_mut() {
                if let Some(t) = ws.as_toast_mut() {
                    if t.is_expired() {
                        t.visible = false;
                    }
                }
            }
            self.redraw();
        }

        if self.engine.has_animated {
            if self.engine.tick_animations() {
                self.redraw();
            }
            el.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + Duration::from_millis(16),
            ));
            return;
        }

        if self.engine.focused_input_id.is_some() {
            if self.engine.cursor_blink.tick() {
                self.redraw();
            }
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
                if let (Some(w), Some(h)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    if let Some(s) = self.engine.surface.as_mut() {
                        s.resize(w, h).unwrap();
                    }
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
                self.cursor_pos = Point::new(position.x as f32, position.y as f32);

                // FIX-4: drag do scrollbar — atualiza offset proporcional
                if let Some(drag) = &self.scroll_drag {
                    let id = drag.id;
                    let dy_px = self.cursor_pos.y - drag.start_y;
                    // Converter pixels de drag em offset de conteúdo
                    let scrollable = (drag.content_h - drag.viewport_h).max(1.0);
                    let track_h = drag.viewport_h
                        - (drag.viewport_h / drag.content_h * drag.viewport_h).max(20.0);
                    let ratio = scrollable / track_h.max(1.0);
                    let new_offset = (drag.start_offset + dy_px * ratio).clamp(0.0, scrollable);
                    if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.offset_y = new_offset;
                        }
                    }
                    self.redraw();
                    return;
                }

                if let Some(sid) = self.engine.drag_slider_id {
                    self.update_slider_drag(sid, self.cursor_pos.x);
                    return;
                }
                self.redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                // FIX-4: verificar se clicou na faixa do scrollbar
                // antes do hit_test normal, para iniciar drag.
                if self.try_begin_scroll_drag() {
                    return;
                }

                let size = self.engine.window.as_ref().unwrap().inner_size();
                self.engine.ensure_layout(size);
                let wt = A::view(&mut self.engine.app_state);
                self.engine.focused_input_id = None;

                let cursor = self.cursor_pos;
                if let Some(hit) = hit_test(
                    &wt,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    cursor,
                    Point::new(0.0, 0.0),
                    &self.engine.widget_states,
                ) {
                    self.close_all_selects();
                    match hit {
                        HitResult::Message(msg) => {
                            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                            self.engine.layout_dirty = true;
                        }
                        HitResult::InputFocus(id) => {
                            self.engine.focused_input_id = Some(id);
                            self.engine.ensure_input_state(id);
                            self.engine.cursor_blink.reset();
                            self.engine.layout_dirty = true;
                            if let Some(s) = self.engine.input_states.get_mut(&id) {
                                s.clear_selection();
                            }
                        }
                        HitResult::SliderPress {
                            id,
                            cursor_x,
                            abs_track_x,
                            track_w,
                            min,
                            max,
                            step,
                        } => {
                            self.engine.drag_slider_id = Some(id);
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(s) = ws.as_slider_mut() {
                                    s.dragging = true;
                                    s.track_abs_x = abs_track_x;
                                    s.track_width = track_w;
                                }
                            }
                            let norm = ((cursor_x - abs_track_x) / track_w).clamp(0.0, 1.0);
                            let val = snap_to_step(min + norm * (max - min), min, max, step);
                            let wt2 = A::view(&mut self.engine.app_state);
                            if let Some(cb) = find_slider_callback(&wt2, id) {
                                let msg = cb(val);
                                A::update(
                                    &mut self.engine.app_state,
                                    msg,
                                    &mut self.engine.clipboard,
                                );
                                self.engine.layout_dirty = true;
                            }
                        }
                        HitResult::SelectToggle(id) => {
                            let was_open = self
                                .engine
                                .widget_states
                                .get(&id)
                                .and_then(|s| s.as_select())
                                .map(|s| s.is_open)
                                .unwrap_or(false);
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(s) = ws.as_select_mut() {
                                    s.is_open = !was_open;
                                }
                            }
                            self.engine.layout_dirty = true;
                        }
                        HitResult::SelectOption { id, index } => {
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(s) = ws.as_select_mut() {
                                    s.is_open = false;
                                }
                            }
                            let wt2 = A::view(&mut self.engine.app_state);
                            if let Some(cb) = find_select_callback(&wt2, id) {
                                let msg = cb(index);
                                A::update(
                                    &mut self.engine.app_state,
                                    msg,
                                    &mut self.engine.clipboard,
                                );
                            }
                            self.engine.layout_dirty = true;
                        }
                        HitResult::ScrollFocus(id) => {
                            self.engine.active_scroll_id = Some(id);
                        }
                        HitResult::TabPress { id, index } => {
                            let wt2 = A::view(&mut self.engine.app_state);
                            if let Some(cb) = find_tab_callback(&wt2, id) {
                                let msg = cb(index);
                                A::update(
                                    &mut self.engine.app_state,
                                    msg,
                                    &mut self.engine.clipboard,
                                );
                            }
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                let size_ref = self
                                    .engine
                                    .window
                                    .as_ref()
                                    .map(|w| w.inner_size().width as f32 / self.engine.scale_factor)
                                    .unwrap_or(800.0);
                                if let Some(t) = ws.as_tab_mut() {
                                    t.set_active(index, size_ref / 4.0);
                                }
                            }
                            self.engine.layout_dirty = true;
                        }
                        HitResult::ModalDismiss(id) => {
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(m) = ws.as_modal_mut() {
                                    m.close();
                                }
                            }
                            self.engine.layout_dirty = true;
                        }
                        HitResult::VListSelect { id, index } => {
                            if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                if let Some(vl) = ws.as_vlist_mut() {
                                    vl.selected_row = Some(index);
                                }
                            }
                            let wt2 = A::view(&mut self.engine.app_state);
                            if let Some(cb) = find_vlist_callback(&wt2, id) {
                                let msg = cb(index);
                                A::update(
                                    &mut self.engine.app_state,
                                    msg,
                                    &mut self.engine.clipboard,
                                );
                            }
                            self.engine.layout_dirty = true;
                        }
                    }
                    self.redraw();
                } else {
                    self.close_all_selects();
                    self.redraw();
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                // FIX-4: termina drag do scrollbar
                if self.scroll_drag.take().is_some() {
                    self.redraw();
                }
                if let Some(sid) = self.engine.drag_slider_id.take() {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_slider_mut() {
                            s.dragging = false;
                        }
                    }
                    self.redraw();
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 40.0,
                    MouseScrollDelta::PixelDelta(p) => -p.y as f32,
                };
                let mut dirty = false;
                if let Some(sid) = self.engine.active_scroll_id {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(dy);
                            dirty = true;
                        }
                    }
                }
                for ws in self.engine.widget_states.values_mut() {
                    if let Some(vl) = ws.as_vlist_mut() {
                        vl.scroll_by(dy, 30.0, 1000);
                        dirty = true;
                        break;
                    }
                }
                if dirty {
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

impl<A: AppLogic + 'static> RutterRunner<A> {
    fn redraw(&self) {
        if let Some(w) = self.engine.window.as_ref() {
            w.request_redraw();
        }
    }

    fn close_all_selects(&mut self) {
        for ws in self.engine.widget_states.values_mut() {
            if let Some(s) = ws.as_select_mut() {
                s.is_open = false;
            }
        }
        self.engine.layout_dirty = true;
    }

    fn update_slider_drag(&mut self, sid: u64, cursor_x: f32) {
        let Some((min, max, step, _)) = ({
            let wt = A::view(&mut self.engine.app_state);
            find_slider_params(&wt, sid)
        }) else {
            return;
        };
        let (abs_x, track_w) = {
            let ws = self.engine.widget_states.get(&sid);
            let s = ws.and_then(|w| w.as_slider());
            (
                s.map(|s| s.track_abs_x).unwrap_or(0.0),
                s.map(|s| s.track_width).unwrap_or(1.0),
            )
        };
        let norm = ((cursor_x - abs_x) / track_w).clamp(0.0, 1.0);
        let val = snap_to_step(min + norm * (max - min), min, max, step);
        let wt2 = A::view(&mut self.engine.app_state);
        if let Some(cb) = find_slider_callback(&wt2, sid) {
            let msg = cb(val);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            self.engine.layout_dirty = true;
            self.redraw();
        }
    }

    // ── FIX-4: iniciar drag do polegar do scrollbar ───────────

    /// Testa se o mouse está sobre a faixa de algum scrollbar ativo
    /// e inicia o drag se positivo. Retorna true se o drag foi iniciado.
    fn try_begin_scroll_drag(&mut self) -> bool {
        let cursor = self.cursor_pos;
        let scale = self.engine.scale_factor;
        let lx = cursor.x / scale;
        let ly = cursor.y / scale;

        // Buscar scroll com state e posição computada no layout
        for (id, ws) in &self.engine.widget_states {
            let Some(s) = ws.as_scroll() else { continue };
            if s.content_height <= s.viewport_h {
                continue;
            }

            // Verificar se o cursor está sobre a scrollbar deste widget.
            // Necessita da posição absoluta do widget — obtida do taffy layout.
            // Por ora, usamos uma heurística: se o cursor está no lado
            // direito da janela dentro de [viewport_h], inicia o drag.
            // (Fase 5 refinará com hit-test preciso via NodeId mapeado).
            let thumb_ratio = s.thumb_ratio();
            let thumb_h = (s.viewport_h * thumb_ratio).max(20.0);
            let thumb_y = s.thumb_y();

            // Faixa X aproximada: últimos SCROLLBAR_W + 4 px lógicos
            let win_w = self
                .engine
                .window
                .as_ref()
                .map(|w| w.inner_size().width as f32 / scale)
                .unwrap_or(800.0);
            let sb_x = win_w - SCROLLBAR_W - 4.0;

            if lx >= sb_x && ly >= thumb_y && ly <= thumb_y + thumb_h {
                self.scroll_drag = Some(ScrollDrag {
                    id: *id,
                    start_y: cursor.y,
                    start_offset: s.offset_y,
                    viewport_h: s.viewport_h,
                    content_h: s.content_height,
                });
                self.engine.active_scroll_id = Some(*id);
                return true;
            }
        }
        false
    }

    // ── Teclado ───────────────────────────────────────────────

    fn handle_key(&mut self, key: &Key) {
        // FIX-4: ArrowUp/Down scrollam o ScrollView focado
        if let Some(sid) = self.engine.active_scroll_id {
            match key {
                Key::Named(NamedKey::ArrowDown) => {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(40.0);
                            self.redraw();
                            return;
                        }
                    }
                }
                Key::Named(NamedKey::ArrowUp) => {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(-40.0);
                            self.redraw();
                            return;
                        }
                    }
                }
                Key::Named(NamedKey::PageDown) => {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(s.viewport_h * 0.9);
                            self.redraw();
                            return;
                        }
                    }
                }
                Key::Named(NamedKey::PageUp) => {
                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(-s.viewport_h * 0.9);
                            self.redraw();
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        if let Key::Named(NamedKey::Tab) = key {
            self.handle_tab();
            return;
        }
        if let Key::Named(NamedKey::Escape) = key {
            self.close_all_selects();
            self.engine.active_scroll_id = None;
            self.redraw();
            return;
        }
        if self.engine.modifiers.state().control_key() {
            if let Key::Character(ch) = key {
                match ch.to_lowercase().as_str() {
                    "c" => {
                        self.do_copy();
                        return;
                    }
                    "v" => {
                        self.do_paste();
                        return;
                    }
                    "a" => {
                        self.do_select_all();
                        return;
                    }
                    "z" => {
                        self.do_undo();
                        return;
                    }
                    "y" => {
                        self.do_redo();
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.handle_text_input(key);
    }

    fn handle_tab(&mut self) {
        let wt = A::view(&mut self.engine.app_state);
        let mut ids = vec![];
        collect_input_ids(&wt, &mut ids);
        if ids.is_empty() {
            return;
        }
        let shift = self.engine.modifiers.state().shift_key();
        let next = match self.engine.focused_input_id {
            Some(cur) => {
                let n = ids.len();
                let i = ids.iter().position(|&x| x == cur).unwrap_or(0);
                ids[if shift { (i + n - 1) % n } else { (i + 1) % n }]
            }
            None => ids[0],
        };
        if let Some(p) = self.engine.focused_input_id {
            if let Some(s) = self.engine.input_states.get_mut(&p) {
                s.snapshot();
            }
        }
        self.engine.focused_input_id = Some(next);
        self.engine.ensure_input_state(next);
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn handle_text_input(&mut self, key: &Key) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };

        let is_shift = self.engine.modifiers.state().shift_key();
        let is_arrow = matches!(
            key,
            Key::Named(NamedKey::ArrowLeft)
                | Key::Named(NamedKey::ArrowRight)
                | Key::Named(NamedKey::ArrowUp)
                | Key::Named(NamedKey::ArrowDown)
        );
        let is_del_key = matches!(
            key,
            Key::Named(NamedKey::Backspace) | Key::Named(NamedKey::Delete)
        );

        let wt = A::view(&mut self.engine.app_state);
        let Some((on_change, _)) = find_input_callbacks(&wt, fid) else {
            return;
        };

        let mut dirty = false;
        let mut full = String::new();

        if let Some(ist) = self.engine.input_states.get_mut(&fid) {
            // ── FIX-3b: Backspace/Delete apaga seleção ───────
            if is_del_key {
                let mut fs = self.engine.font_system.borrow_mut();
                if ist.delete_selection(&mut fs) {
                    drop(fs);
                    // Não processa o Backspace/Delete char-a-char depois de apagar seleção
                    let msg = on_change(ist.text());
                    A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                    self.engine.cursor_blink.reset();
                    self.engine.schedule_snapshot();
                    self.engine.layout_dirty = true;
                    self.redraw();
                    return;
                }
                drop(fs);
            }

            // ── FIX-3c: Shift+Seta estende a seleção ─────────
            if is_arrow && is_shift {
                let before = ist.cursor_byte_index();
                // Estabelece âncora se ainda não existe
                if ist.selection_anchor.is_none() {
                    ist.selection_anchor = Some(before);
                }
                // Move o cursor com cosmic-text
                if let Some(action) = map_key(key) {
                    let mut fs = self.engine.font_system.borrow_mut();
                    ist.editor.action(&mut fs, action);
                    drop(fs);
                }
                let after = ist.cursor_byte_index();
                let anchor = ist.selection_anchor.unwrap();
                ist.selection = Some(crate::input_state::TextSelection {
                    start: anchor,
                    end: after,
                });
                let vw = 260.0;
                ist.update_scroll(vw);
                dirty = true;
                full = ist.text();
            } else if is_arrow && !is_shift {
                // Seta sem Shift: limpa seleção e move cursor
                ist.clear_selection();
                if let Some(action) = map_key(key) {
                    let mut fs = self.engine.font_system.borrow_mut();
                    ist.editor.action(&mut fs, action);
                    drop(fs);
                }
                let vw = 260.0;
                ist.update_scroll(vw);
                dirty = true;
                full = ist.text();
            } else {
                // Demais teclas: limpa seleção, processa ação/char
                ist.clear_selection();
                if let Some(action) = map_key(key) {
                    let mut fs = self.engine.font_system.borrow_mut();
                    ist.editor.action(&mut fs, action);
                    drop(fs);
                    dirty = true;
                } else if let Key::Named(NamedKey::Space) = key {
                    ist.editor.insert_string(" ", None);
                    let t = ist.text();
                    ist.undo.push(t);
                    dirty = true;
                } else if let Key::Character(t) = key {
                    if !t.is_empty() {
                        ist.editor.insert_string(t.as_str(), None);
                        dirty = true;
                    }
                }
                if dirty {
                    let vw = 260.0;
                    ist.update_scroll(vw);
                    full = ist.text();
                }
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

    // ── FIX-3a: Copy — copia texto selecionado ou tudo ────────

    fn do_copy(&mut self) {
        if let Some(fid) = self.engine.focused_input_id {
            if let Some(s) = self.engine.input_states.get(&fid) {
                // Copiar somente a seleção se houver, senão o texto todo
                let text_to_copy = s
                    .selection
                    .filter(|sel| !sel.is_empty())
                    .map(|sel| {
                        let (a, b) = sel.normalized();
                        let full = s.text();
                        let b = b.min(full.len());
                        full[a..b].to_string()
                    })
                    .unwrap_or_else(|| s.text());

                let _ = self.engine.clipboard.set_text(text_to_copy);
                // FIX-3e: toast de cópia (descomentável pelo usuário)
                // self.show_copy_toast();
            }
        }
        self.redraw();
    }

    /// FIX-3e: Toast opcional ao copiar.
    /// Para ativar: descomente a chamada em do_copy() e defina
    /// COPY_TOAST_ID como o id do seu Widget::Toast.
    #[allow(dead_code)]
    fn show_copy_toast(&mut self) {
        const COPY_TOAST_ID: u64 = 91;
        use crate::engine::widget_state::{ToastState, WidgetState};
        self.engine
            .widget_states
            .insert(COPY_TOAST_ID, WidgetState::Toast(ToastState::new(2000)));
        self.redraw();
    }

    fn do_paste(&mut self) {
        let Ok(txt) = self.engine.clipboard.get_text() else {
            return;
        };
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        let Some(s) = self.engine.input_states.get_mut(&fid) else {
            return;
        };
        s.editor.insert_string(&txt, None);
        s.snapshot();
        let full = s.text();
        let wt = A::view(&mut self.engine.app_state);
        if let Some((cb, _)) = find_input_callbacks(&wt, fid) {
            A::update(
                &mut self.engine.app_state,
                cb(full),
                &mut self.engine.clipboard,
            );
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    /// FIX-3a: select_all NÃO chama clipboard.set_text — isso é
    /// responsabilidade exclusiva de do_copy().
    fn do_select_all(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.select_all(&mut fs);
            // REMOVIDO: let _ = self.engine.clipboard.set_text(s.text());
            // Isso causava o duplo-copy (aqui + do_copy()) e enviava
            // "textotexto" a alguns gerenciadores de clipboard no Linux.
        }
        self.redraw();
    }

    fn do_undo(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.undo(&mut fs);
            drop(fs);
            let full = s.text();
            let wt = A::view(&mut self.engine.app_state);
            if let Some((cb, _)) = find_input_callbacks(&wt, fid) {
                A::update(
                    &mut self.engine.app_state,
                    cb(full),
                    &mut self.engine.clipboard,
                );
            }
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_redo(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.redo(&mut fs);
            drop(fs);
            let full = s.text();
            let wt = A::view(&mut self.engine.app_state);
            if let Some((cb, _)) = find_input_callbacks(&wt, fid) {
                A::update(
                    &mut self.engine.app_state,
                    cb(full),
                    &mut self.engine.clipboard,
                );
            }
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }
}

// ── Testes ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_no_step() {
        assert!((snap_to_step(1.5, 0.0, 1.0, 0.0) - 1.0).abs() < f32::EPSILON);
    }
    #[test]
    fn snap_step_1_dn() {
        assert!((snap_to_step(2.3, 0.0, 10.0, 1.0) - 2.0).abs() < 0.001);
    }
    #[test]
    fn snap_step_1_up() {
        assert!((snap_to_step(2.7, 0.0, 10.0, 1.0) - 3.0).abs() < 0.001);
    }
    #[test]
    fn snap_step_05() {
        assert!((snap_to_step(2.3, 0.0, 10.0, 0.5) - 2.5).abs() < 0.001);
    }
    #[test]
    fn snap_clamp_low() {
        assert!((snap_to_step(-5.0, 0.0, 10.0, 1.0) - 0.0).abs() < 0.001);
    }
    #[test]
    fn snap_clamp_hi() {
        assert!((snap_to_step(15.0, 0.0, 10.0, 1.0) - 10.0).abs() < 0.001);
    }
}
