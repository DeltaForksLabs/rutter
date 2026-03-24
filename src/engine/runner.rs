// ============================================================
// Rutter Framework — engine/runner.rs
// Fase 2: undo/redo (Ctrl+Z/Y), seleção visual (Ctrl+A),
// suporte a mudança de DPI, snapshot periódico.
// ============================================================

use std::num::NonZeroU32;

use cosmic_text::{Action, Edit, Motion};
use skia_safe::Point;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowId,
};

use super::RutterEngine;
use crate::app::AppLogic;
use crate::render::hit_test::{HitResult, collect_input_ids, find_input_callbacks, hit_test};

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

pub struct RutterRunner<A: AppLogic> {
    engine: RutterEngine<A>,
    cursor_pos: Point,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub fn run() {
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Wait);
        let mut r = Self {
            engine: RutterEngine::new(),
            cursor_pos: Point::new(0.0, 0.0),
        };
        el.run_app(&mut r).unwrap();
    }
}

impl<A: AppLogic + 'static> ApplicationHandler for RutterRunner<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.engine.handle_resumed(el);
    }

    // FIX #2: tick do cursor + snapshot periódico de undo
    fn new_events(&mut self, el: &ActiveEventLoop, _: StartCause) {
        // Snapshot de undo por inatividade
        self.engine.maybe_snapshot();

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

            // HiDPI: monitor mudou
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.engine.update_scale(scale_factor);
                self.redraw();
            }

            WindowEvent::ModifiersChanged(m) => self.engine.modifiers = m,

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = Point::new(position.x as f32, position.y as f32);
                self.redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                let size = self.engine.window.as_ref().unwrap().inner_size();
                self.engine.ensure_layout(size);
                let wt = A::view(&mut self.engine.app_state);
                self.engine.focused_input_id = None;

                if let Some(hit) = hit_test(
                    &wt,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    self.cursor_pos,
                    Point::new(0.0, 0.0),
                ) {
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
                            // Limpar seleção ao clicar
                            if let Some(s) = self.engine.input_states.get_mut(&id) {
                                s.clear_selection();
                            }
                        }
                    }
                    self.redraw();
                }
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_key(&event.logical_key);
            }

            WindowEvent::RedrawRequested => {
                self.engine.redraw(self.cursor_pos);
            }

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

    fn handle_key(&mut self, key: &Key) {
        // Tab / Shift+Tab
        if let Key::Named(NamedKey::Tab) = key {
            self.handle_tab();
            return;
        }

        // Atalhos Ctrl
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

        // Salvar snapshot antes de mudar foco
        if let Some(prev_id) = self.engine.focused_input_id {
            if let Some(s) = self.engine.input_states.get_mut(&prev_id) {
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

        // Limpar seleção ao digitar
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            s.clear_selection();
        }

        let wt = A::view(&mut self.engine.app_state);
        let Some((on_change, _)) = find_input_callbacks(&wt, fid) else {
            return;
        };

        let mut dirty = false;
        let mut full_text = String::new();

        if let Some(istate) = self.engine.input_states.get_mut(&fid) {
            if let Some(action) = map_key(key) {
                let mut fs = self.engine.font_system.borrow_mut();
                istate.editor.action(&mut fs, action);
                dirty = true;
            } else if let Key::Named(NamedKey::Space) = key {
                istate.editor.insert_string(" ", None);
                // Snapshot ao completar palavra (espaço)
                let t = istate.text();
                istate.undo.push(t);
                dirty = true;
            } else if let Key::Character(t) = key {
                if !t.is_empty() {
                    istate.editor.insert_string(t.as_str(), None);
                    dirty = true;
                }
            }

            if dirty {
                // Atualizar scroll
                let visible_w = 260.0; // TODO: obter do layout real
                istate.update_scroll(visible_w);
                full_text = istate.text();
            }
        }

        if dirty {
            self.engine.cursor_blink.reset();
            self.engine.schedule_snapshot();

            let msg = on_change(full_text);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            self.engine.layout_dirty = true;
            self.redraw();
        }
    }

    fn do_copy(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        if let Some(s) = self.engine.input_states.get(&fid) {
            let _ = self.engine.clipboard.set_text(s.text());
        }
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
        if let Some((on_change, _)) = find_input_callbacks(&wt, fid) {
            let msg = on_change(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_select_all(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        let Some(s) = self.engine.input_states.get_mut(&fid) else {
            return;
        };
        let mut fs = self.engine.font_system.borrow_mut();
        s.select_all(&mut fs);
        // Copia automaticamente para o clipboard (comportamento padrão)
        let _ = self.engine.clipboard.set_text(s.text());
        drop(fs);
        self.redraw();
    }

    fn do_undo(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        let Some(s) = self.engine.input_states.get_mut(&fid) else {
            return;
        };
        let mut fs = self.engine.font_system.borrow_mut();
        s.undo(&mut fs);
        drop(fs);

        let full = s.text();
        let wt = A::view(&mut self.engine.app_state);
        if let Some((on_change, _)) = find_input_callbacks(&wt, fid) {
            let msg = on_change(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_redo(&mut self) {
        let Some(fid) = self.engine.focused_input_id else {
            return;
        };
        let Some(s) = self.engine.input_states.get_mut(&fid) else {
            return;
        };
        let mut fs = self.engine.font_system.borrow_mut();
        s.redo(&mut fs);
        drop(fs);

        let full = s.text();
        let wt = A::view(&mut self.engine.app_state);
        if let Some((on_change, _)) = find_input_callbacks(&wt, fid) {
            let msg = on_change(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }
}
