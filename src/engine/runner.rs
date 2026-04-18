// ============================================================
// Rutter Framework — engine/runner.rs
// ============================================================

use std::{collections::HashMap, num::NonZeroU32, time::Duration};

use cosmic_text::{Action, Edit, Motion};
use skia_safe::{Point, Rect as SkiaRect};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowId,
};

use super::RutterEngine;
use crate::app::AppLogic;
use crate::engine::widget_state::WidgetState;
use crate::render::hit_test::{HitResult, find_scroll_focus, find_scrollbar_drag_hit, hit_test};

fn is_bidi_override_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn skip_ansi_escape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            while let Some(ch) = chars.next() {
                if ('@'..='~').contains(&ch) {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            let mut saw_escape = false;
            while let Some(ch) = chars.next() {
                if ch == '\u{0007}' {
                    break;
                }
                if saw_escape && ch == '\\' {
                    break;
                }
                saw_escape = ch == '\u{001B}';
            }
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
}

fn sanitize_clipboard_text(text: &str, allow_newlines: bool) -> String {
    let mut sanitized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{001B}' {
            skip_ansi_escape(&mut chars);
            continue;
        }

        if is_bidi_override_char(ch) {
            continue;
        }

        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if allow_newlines {
                    sanitized.push('\n');
                } else {
                    sanitized.push(' ');
                }
            }
            '\n' => {
                if allow_newlines {
                    sanitized.push('\n');
                } else {
                    sanitized.push(' ');
                }
            }
            '\t' => sanitized.push(' '),
            _ if ch.is_control() => {}
            _ => sanitized.push(ch),
        }
    }

    sanitized
}

fn map_key(key: &Key, ctrl: bool) -> Option<Action> {
    match key {
        Key::Named(NamedKey::ArrowLeft) if ctrl => Some(Action::Motion(Motion::Left)),
        Key::Named(NamedKey::ArrowRight) if ctrl => Some(Action::Motion(Motion::Right)),
        Key::Named(NamedKey::ArrowUp) if ctrl => Some(Action::Motion(Motion::ParagraphStart)),
        Key::Named(NamedKey::ArrowDown) if ctrl => Some(Action::Motion(Motion::ParagraphEnd)),
        Key::Named(NamedKey::Home) if ctrl => Some(Action::Motion(Motion::BufferStart)),
        Key::Named(NamedKey::End) if ctrl => Some(Action::Motion(Motion::BufferEnd)),
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

fn is_activation_key(key: &Key) -> bool {
    matches!(key, Key::Named(NamedKey::Enter)) || matches!(key, Key::Character(ch) if ch == " ")
}

fn collect_toast_runtime_state(widget_states: &HashMap<u64, WidgetState>) -> (Vec<u64>, bool) {
    let mut expired_toasts = Vec::new();
    let mut has_active_timed_toasts = false;

    for (id, ws) in widget_states {
        let Some(toast) = ws.as_toast() else {
            continue;
        };
        if !toast.visible {
            continue;
        }
        if toast.is_expired() {
            expired_toasts.push(*id);
        } else if toast.duration_ms > 0 {
            has_active_timed_toasts = true;
        }
    }

    (expired_toasts, has_active_timed_toasts)
}

pub fn snap_to_step(value: f32, min: f32, max: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return value.clamp(min, max);
    }
    let snapped = (((value - min) / step).round() * step + min).clamp(min, max);
    let decimals = (-step.log10().floor()).max(0.0) as i32;
    let factor = 10_f32.powi(decimals);
    (snapped * factor).round() / factor
}

#[derive(Debug, Clone)]
struct ScrollDrag {
    id: u64,
    start_y: f32,
    start_offset: f32,
    viewport_h: f32,
    content_h: f32,
}

pub struct RutterRunner<A: AppLogic> {
    engine: RutterEngine<A>,
    cursor_pos: Point,
    scroll_drag: Option<ScrollDrag>,
    mouse_down: bool,
    last_click_time: std::time::Instant,
    last_click_pos: Point,
    focused_input_rect: Option<SkiaRect>,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub fn run() {
        let el = EventLoop::new().unwrap();
        el.set_control_flow(ControlFlow::Wait);
        let mut r = Self {
            engine: RutterEngine::new(),
            cursor_pos: Point::new(0.0, 0.0),
            scroll_drag: None,
            mouse_down: false,
            last_click_time: std::time::Instant::now(),
            last_click_pos: Point::new(0.0, 0.0),
            focused_input_rect: None,
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

        let (expired_toasts, has_active_timed_toasts) =
            collect_toast_runtime_state(&self.engine.widget_states);
        if !expired_toasts.is_empty() {
            for id in &expired_toasts {
                if let Some(ws) = self.engine.widget_states.get_mut(id) {
                    if let Some(t) = ws.as_toast_mut() {
                        t.visible = false;
                    }
                }
                if let Some(msg) = self.engine.runtime_caches.toast_dismiss.get(id).cloned() {
                    A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                }
            }
            self.engine.layout_dirty = true;
            self.redraw();
        }

        let mut needs_frame_tick = false;
        if self.engine.has_animated && self.engine.tick_animations() {
            self.redraw();
            needs_frame_tick = true;
        } else if self.engine.has_animated {
            needs_frame_tick = true;
        }
        if has_active_timed_toasts {
            self.redraw();
            needs_frame_tick = true;
        }

        if needs_frame_tick {
            el.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + Duration::from_millis(16),
            ));
            return;
        }

        if self.engine.focused_input_id().is_some() {
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

                if let Some(drag) = &self.scroll_drag {
                    let id = drag.id;
                    let dy_px = self.cursor_pos.y - drag.start_y;
                    let scrollable = (drag.content_h - drag.viewport_h).max(1.0);
                    let track_h = drag.viewport_h
                        - (drag.viewport_h / drag.content_h * drag.viewport_h).max(20.0);
                    let ratio = scrollable / track_h.max(1.0);
                    let new_offset = (drag.start_offset + dy_px * ratio).clamp(0.0, scrollable);
                    if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.offset_y = new_offset;
                        } else if let Some(s) = ws.as_vlist_mut() {
                            s.scroll_y = new_offset;
                        }
                    }
                    self.redraw();
                    return;
                }

                if let Some(sid) = self.engine.drag_slider_id {
                    self.update_slider_drag(sid, self.cursor_pos.x);
                    return;
                }

                if self.mouse_down {
                    if let Some(fid) = self.engine.focused_input_id() {
                        if let Some(rect) = self.focused_input_rect {
                            let local_x = self.cursor_pos.x / self.engine.scale_factor - rect.left;
                            let local_y = self.cursor_pos.y / self.engine.scale_factor - rect.top;
                            let pad_x = A::theme().spacing * 2.0;
                            let pad_y = A::theme().spacing;
                            if let Some(ist) = self.engine.input_states.get_mut(&fid) {
                                let click_x = ((local_x - pad_x) + ist.scroll_x).max(0.0);
                                let click_y = ((local_y - pad_y) + ist.scroll_y).max(0.0);
                                let mut fs = self.engine.font_system.borrow_mut();
                                ist.editor.action(
                                    &mut fs,
                                    Action::Drag {
                                        x: click_x as i32,
                                        y: click_y as i32,
                                    },
                                );
                                ist.normalize_cursor();
                                ist.sync_selection();
                                self.redraw();
                            }
                        }
                    }
                }
                self.redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } if matches!(
                button,
                winit::event::MouseButton::Left | winit::event::MouseButton::Right
            ) =>
            {
                self.mouse_down = true;
                let now = std::time::Instant::now();
                let is_double = now.duration_since(self.last_click_time)
                    < Duration::from_millis(300)
                    && (self.cursor_pos.x - self.last_click_pos.x).abs() < 5.0
                    && (self.cursor_pos.y - self.last_click_pos.y).abs() < 5.0;
                self.last_click_time = now;
                self.last_click_pos = self.cursor_pos;

                let size = self.engine.window.as_ref().unwrap().inner_size();
                self.engine.ensure_layout(size);

                let cursor = Point::new(
                    self.cursor_pos.x / self.engine.scale_factor,
                    self.cursor_pos.y / self.engine.scale_factor,
                );
                let (scroll_drag_hit, active_scroll_id, hit) = {
                    let wt = A::view(&mut self.engine.app_state);
                    let scroll_drag_hit = if button == winit::event::MouseButton::Left {
                        find_scrollbar_drag_hit(
                            &wt,
                            &self.engine.taffy,
                            self.engine.last_root_node,
                            cursor,
                            Point::new(0.0, 0.0),
                            &self.engine.widget_states,
                        )
                    } else {
                        None
                    };
                    let active_scroll_id = find_scroll_focus(
                        &wt,
                        &self.engine.taffy,
                        self.engine.last_root_node,
                        cursor,
                        Point::new(0.0, 0.0),
                    );
                    let hit = hit_test(
                        &wt,
                        &self.engine.taffy,
                        self.engine.last_root_node,
                        cursor,
                        Point::new(0.0, 0.0),
                        &self.engine.widget_states,
                    );
                    (scroll_drag_hit, active_scroll_id, hit)
                };

                if button == winit::event::MouseButton::Left {
                    if let Some(hit) = scroll_drag_hit {
                        self.begin_scroll_drag(hit);
                        return;
                    }
                }

                self.engine.active_scroll_id = active_scroll_id;

                if let Some(hit) = hit {
                    self.close_all_selects();
                    match hit {
                        HitResult::Message { focus_id, msg } => {
                            if button == winit::event::MouseButton::Left {
                                self.focus_widget(focus_id);
                                A::update(
                                    &mut self.engine.app_state,
                                    msg,
                                    &mut self.engine.clipboard,
                                );
                                self.engine.layout_dirty = true;
                            }
                        }
                        HitResult::InputFocus {
                            id,
                            local_x,
                            local_y,
                            width,
                            height,
                        } => {
                            let rect_left = cursor.x - local_x;
                            let rect_top = cursor.y - local_y;
                            self.focused_input_rect =
                                Some(SkiaRect::from_xywh(rect_left, rect_top, width, height));
                            self.focus_input_at(id, local_x, local_y, width, height, is_double);
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
                            if button == winit::event::MouseButton::Left {
                                self.focus_widget(Some(id));
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
                                let cb = self
                                    .engine
                                    .runtime_caches
                                    .sliders
                                    .get(&id)
                                    .map(|s| s.on_change);
                                if let Some(cb) = cb {
                                    let msg = cb(val);
                                    A::update(
                                        &mut self.engine.app_state,
                                        msg,
                                        &mut self.engine.clipboard,
                                    );
                                    self.engine.layout_dirty = true;
                                }
                            }
                        }
                        HitResult::SelectToggle(id) => {
                            if button == winit::event::MouseButton::Left {
                                self.focus_widget(Some(id));
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
                        }
                        HitResult::SelectOption { id, index } => {
                            if button == winit::event::MouseButton::Left {
                                self.focus_widget(Some(id));
                                if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                    if let Some(s) = ws.as_select_mut() {
                                        s.is_open = false;
                                    }
                                }
                                let select = self.engine.runtime_caches.selects.get(&id).cloned();
                                if let Some(select) = select {
                                    let msg = (select.on_change)(index);
                                    A::update(
                                        &mut self.engine.app_state,
                                        msg,
                                        &mut self.engine.clipboard,
                                    );
                                }
                                self.engine.layout_dirty = true;
                            }
                        }
                        HitResult::ScrollFocus(id) => {
                            self.engine.active_scroll_id = Some(id);
                        }
                        HitResult::TabPress {
                            id,
                            focus_id,
                            index,
                        } => {
                            if button == winit::event::MouseButton::Left {
                                self.activate_tab_item(id, index, focus_id);
                            }
                        }
                        HitResult::ModalDismiss(id) => {
                            if button == winit::event::MouseButton::Left {
                                if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                    if let Some(m) = ws.as_modal_mut() {
                                        m.close();
                                    }
                                }
                                self.engine.layout_dirty = true;
                            }
                        }
                        HitResult::VListSelect { id, index } => {
                            if button == winit::event::MouseButton::Left {
                                self.focus_widget(Some(id));
                                if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                    if let Some(vl) = ws.as_vlist_mut() {
                                        vl.selected_row = Some(index);
                                    }
                                }
                                let cb = self
                                    .engine
                                    .runtime_caches
                                    .vlists
                                    .get(&id)
                                    .map(|v| v.on_select);
                                if let Some(cb) = cb {
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
                    }
                    self.redraw();
                } else {
                    self.focus_widget(None);
                    self.close_all_selects();
                    self.redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                self.mouse_down = false;
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
                    let vlist_props = self
                        .engine
                        .runtime_caches
                        .vlists
                        .get(&sid)
                        .map(|v| (v.item_height, v.item_count));

                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(dy);
                            dirty = true;
                        } else if let Some(s) = ws.as_vlist_mut() {
                            if let Some((ih, ic)) = vlist_props {
                                s.scroll_by(dy, ih, ic);
                                dirty = true;
                            }
                        }
                    }
                } else {
                    let mut vlist_to_scroll = None;
                    for (id, ws) in self.engine.widget_states.iter() {
                        if ws.as_vlist().is_some() {
                            if let Some(props) = self
                                .engine
                                .runtime_caches
                                .vlists
                                .get(id)
                                .map(|v| (v.item_height, v.item_count))
                            {
                                vlist_to_scroll = Some((*id, props));
                                break;
                            }
                        }
                    }
                    if let Some((id, (ih, ic))) = vlist_to_scroll {
                        if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                            if let Some(vl) = ws.as_vlist_mut() {
                                vl.scroll_by(dy, ih, ic);
                                dirty = true;
                            }
                        }
                    }
                }

                if dirty {
                    self.redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if !self.handle_text_commit(&event) {
                    self.handle_key(&event.logical_key);
                }
            }
            WindowEvent::RedrawRequested => self.engine.redraw(self.cursor_pos),
            _ => {}
        }
    }
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    fn handle_text_commit(&mut self, event: &KeyEvent) -> bool {
        if self.engine.focused_input_id().is_none() || self.engine.modifiers.state().control_key() {
            return false;
        }

        let Some(text) = event.text.as_ref() else {
            return false;
        };

        if text.is_empty() || text.chars().all(char::is_control) {
            return false;
        }

        self.insert_text_commit(text.as_str());
        true
    }

    fn insert_text_commit(&mut self, text: &str) {
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };

        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };

        let visible_w = input.visible_w;
        let visible_h = input.visible_h;
        let is_password = input.is_password;
        let is_multiline = input.is_multiline;
        let on_change = input.on_change;

        let mut full = None;

        if let Some(ist) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            ist.sync_layout(&mut fs, visible_w, A::theme().font_body, is_multiline);
            if !ist.delete_selection(&mut fs) {
                ist.clear_selection();
            }
            ist.editor.insert_string(text, None);
            ist.normalize_cursor();
            ist.update_scroll(
                &mut fs,
                visible_w,
                visible_h,
                A::theme().font_body,
                is_password,
                is_multiline,
            );
            full = Some(ist.text());
        }

        let Some(full) = full else {
            return;
        };

        self.engine.cursor_blink.reset();
        self.engine.schedule_snapshot();
        A::update(
            &mut self.engine.app_state,
            on_change(full),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn redraw(&self) {
        if let Some(w) = self.engine.window.as_ref() {
            w.request_redraw();
        }
    }

    fn focus_widget(&mut self, focus_id: Option<u64>) {
        if self.engine.focused_widget_id == focus_id {
            return;
        }

        if let Some(previous_input) = self.engine.focused_input_id() {
            if let Some(state) = self.engine.input_states.get_mut(&previous_input) {
                state.snapshot();
            }
        }

        self.engine.focused_widget_id = focus_id;
        if let Some(id) = focus_id {
            if self.engine.runtime_caches.inputs.contains_key(&id) {
                self.engine.ensure_input_state(id);
                self.engine.cursor_blink.reset();
            } else {
                self.focused_input_rect = None;
            }
        } else {
            self.focused_input_rect = None;
        }
        self.engine.layout_dirty = true;
    }

    fn activate_tab_item(&mut self, parent_id: u64, index: usize, focus_id: u64) {
        let tab = self.engine.runtime_caches.tabs.get(&parent_id).cloned();
        self.focus_widget(Some(focus_id));
        if let Some(tab) = tab {
            A::update(
                &mut self.engine.app_state,
                (tab.on_change)(index),
                &mut self.engine.clipboard,
            );
            if let Some(ws) = self.engine.widget_states.get_mut(&parent_id) {
                let size_ref = self
                    .engine
                    .window
                    .as_ref()
                    .map(|w| w.inner_size().width as f32 / self.engine.scale_factor)
                    .unwrap_or(800.0);
                if let Some(state) = ws.as_tab_mut() {
                    state.set_active(index, size_ref / tab.tab_count.max(1) as f32);
                }
            }
            self.engine.layout_dirty = true;
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
        let Some(slider) = self.engine.runtime_caches.sliders.get(&sid).cloned() else {
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
        let val = snap_to_step(
            slider.min + norm * (slider.max - slider.min),
            slider.min,
            slider.max,
            slider.step,
        );
        let msg = (slider.on_change)(val);
        A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn begin_scroll_drag(&mut self, hit: crate::render::hit_test::ScrollbarDragHit) {
        self.scroll_drag = Some(ScrollDrag {
            id: hit.id,
            start_y: self.cursor_pos.y,
            start_offset: hit.start_offset,
            viewport_h: hit.viewport_h,
            content_h: hit.content_h,
        });
        self.engine.active_scroll_id = Some(hit.id);
    }

    fn focus_input_at(
        &mut self,
        id: u64,
        local_x: f32,
        local_y: f32,
        width: f32,
        height: f32,
        is_double: bool,
    ) {
        self.focus_widget(Some(id));
        self.engine.ensure_input_state(id);

        let pad_x = A::theme().spacing * 2.0;
        let pad_y = A::theme().spacing;
        let input = self.engine.runtime_caches.inputs.get(&id).cloned();
        if let Some(ist) = self.engine.input_states.get_mut(&id) {
            let mut fs = self.engine.font_system.borrow_mut();
            if let Some(input) = input.as_ref() {
                ist.sync_layout(
                    &mut fs,
                    input.visible_w,
                    A::theme().font_body,
                    input.is_multiline,
                );
            } else {
                ist.set_metrics(&mut fs, A::theme().font_body);
            }
            ist.clear_selection();
            let click_x = ((local_x - pad_x) + ist.scroll_x).max(0.0);
            let click_y = ((local_y - pad_y) + ist.scroll_y).max(0.0);

            if is_double {
                ist.editor.action(
                    &mut fs,
                    Action::DoubleClick {
                        x: click_x as i32,
                        y: click_y as i32,
                    },
                );
            } else {
                ist.editor.action(
                    &mut fs,
                    Action::Click {
                        x: click_x as i32,
                        y: click_y as i32,
                    },
                );
            }
            ist.normalize_cursor();
            ist.sync_selection();

            let visible_w = input
                .as_ref()
                .map(|input| input.visible_w)
                .unwrap_or_else(|| (width - pad_x * 2.0).max(24.0));
            let visible_h = input
                .as_ref()
                .map(|input| input.visible_h)
                .unwrap_or_else(|| (height - pad_y * 2.0).max(18.0));
            let is_password = input
                .as_ref()
                .map(|input| input.is_password)
                .unwrap_or(false);
            let is_multiline = input
                .as_ref()
                .map(|input| input.is_multiline)
                .unwrap_or(false);
            ist.update_scroll(
                &mut fs,
                visible_w,
                visible_h,
                A::theme().font_body,
                is_password,
                is_multiline,
            );
        }
    }

    fn handle_focused_widget_key(&mut self, key: &Key) -> bool {
        let Some(fid) = self.engine.focused_widget_id else {
            return false;
        };
        if self.engine.runtime_caches.inputs.contains_key(&fid) {
            return false;
        }

        if let Some(msg) = self.engine.runtime_caches.buttons.get(&fid).cloned() {
            if is_activation_key(key) {
                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
            }
        }

        if let Some(toggle) = self.engine.runtime_caches.checkboxes.get(&fid).cloned() {
            if is_activation_key(key) {
                A::update(
                    &mut self.engine.app_state,
                    (toggle.on_change)(!toggle.checked),
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
            }
        }

        if let Some(toggle) = self.engine.runtime_caches.switches.get(&fid).cloned() {
            if is_activation_key(key) {
                A::update(
                    &mut self.engine.app_state,
                    (toggle.on_change)(!toggle.checked),
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
            }
        }

        if let Some(on_select) = self.engine.runtime_caches.radios.get(&fid).copied() {
            if is_activation_key(key) {
                A::update(
                    &mut self.engine.app_state,
                    on_select(),
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
            }
        }

        if let Some(msg) = self.engine.runtime_caches.accordions.get(&fid).cloned() {
            if is_activation_key(key) {
                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
            }
        }

        if let Some(slider) = self.engine.runtime_caches.sliders.get(&fid).cloned() {
            let next_value = match key {
                Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowDown) => {
                    Some(slider.value - slider.step.max(f32::EPSILON))
                }
                Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowUp) => {
                    Some(slider.value + slider.step.max(f32::EPSILON))
                }
                Key::Named(NamedKey::Home) => Some(slider.min),
                Key::Named(NamedKey::End) => Some(slider.max),
                Key::Named(NamedKey::PageUp) => Some(slider.value + slider.step.max(1.0) * 10.0),
                Key::Named(NamedKey::PageDown) => Some(slider.value - slider.step.max(1.0) * 10.0),
                _ => None,
            };
            if let Some(next_value) = next_value {
                let snapped = snap_to_step(next_value, slider.min, slider.max, slider.step);
                A::update(
                    &mut self.engine.app_state,
                    (slider.on_change)(snapped),
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
            }
        }

        if let Some(select) = self.engine.runtime_caches.selects.get(&fid).cloned() {
            let mut dirty = false;
            if is_activation_key(key) {
                if let Some(ws) = self.engine.widget_states.get_mut(&fid) {
                    if let Some(state) = ws.as_select_mut() {
                        state.is_open = !state.is_open;
                        dirty = true;
                    }
                }
                if dirty {
                    self.engine.layout_dirty = true;
                    self.redraw();
                    return true;
                }
            }

            if select.option_count > 0 {
                let next_index = match key {
                    Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowRight) => {
                        Some((select.selected_index + 1).min(select.option_count - 1))
                    }
                    Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowLeft) => {
                        Some(select.selected_index.saturating_sub(1))
                    }
                    Key::Named(NamedKey::Home) => Some(0),
                    Key::Named(NamedKey::End) => Some(select.option_count - 1),
                    _ => None,
                };
                if let Some(next_index) = next_index {
                    if let Some(ws) = self.engine.widget_states.get_mut(&fid) {
                        if let Some(state) = ws.as_select_mut() {
                            state.hovered_option = Some(next_index);
                        }
                    }
                    A::update(
                        &mut self.engine.app_state,
                        (select.on_change)(next_index),
                        &mut self.engine.clipboard,
                    );
                    self.engine.layout_dirty = true;
                    self.redraw();
                    return true;
                }
            }
        }

        if let Some(tab_item) = self.engine.runtime_caches.tab_items.get(&fid).cloned() {
            if let Some(tab) = self
                .engine
                .runtime_caches
                .tabs
                .get(&tab_item.parent_id)
                .cloned()
            {
                if tab.tab_count > 0 {
                    let next_index = match key {
                        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => {
                            Some(tab_item.index.saturating_sub(1))
                        }
                        Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown) => {
                            Some((tab_item.index + 1).min(tab.tab_count - 1))
                        }
                        Key::Named(NamedKey::Home) => Some(0),
                        Key::Named(NamedKey::End) => Some(tab.tab_count - 1),
                        _ if is_activation_key(key) => Some(tab_item.index),
                        _ => None,
                    };
                    if let Some(next_index) = next_index {
                        let next_focus_id = tab.focus_ids.get(next_index).copied().unwrap_or(fid);
                        self.activate_tab_item(tab_item.parent_id, next_index, next_focus_id);
                        self.redraw();
                        return true;
                    }
                }
            }
        }

        if let Some(vlist) = self.engine.runtime_caches.vlists.get(&fid).cloned() {
            match key {
                Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowUp) => {
                    if let Some(ws) = self.engine.widget_states.get_mut(&fid) {
                        if let Some(state) = ws.as_vlist_mut() {
                            let current = state.selected_row.unwrap_or(0);
                            let next = if matches!(key, Key::Named(NamedKey::ArrowDown)) {
                                (current + 1).min(vlist.item_count.saturating_sub(1))
                            } else {
                                current.saturating_sub(1)
                            };
                            state.selected_row = Some(next);
                            state.scroll_to_index(next, vlist.item_height, vlist.item_count);
                            A::update(
                                &mut self.engine.app_state,
                                (vlist.on_select)(next),
                                &mut self.engine.clipboard,
                            );
                            self.engine.layout_dirty = true;
                            self.redraw();
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }

        false
    }

    fn handle_key(&mut self, key: &Key) {
        if self.engine.focused_widget_id.is_none() {
            if let Some(sid) = self.engine.active_scroll_id {
                let vlist_props = self
                    .engine
                    .runtime_caches
                    .vlists
                    .get(&sid)
                    .map(|v| (v.item_height, v.item_count, v.on_select));
                match key {
                    Key::Named(NamedKey::ArrowDown) => {
                        if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                            if let Some(s) = ws.as_scroll_mut() {
                                s.scroll_by(40.0);
                                self.redraw();
                                return;
                            } else if let Some(s) = ws.as_vlist_mut() {
                                if let Some((ih, ic, cb)) = vlist_props {
                                    let new_sel = s.selected_row.unwrap_or(0) + 1;
                                    if new_sel < ic {
                                        s.selected_row = Some(new_sel);
                                        s.scroll_to_index(new_sel, ih, ic);
                                        A::update(
                                            &mut self.engine.app_state,
                                            cb(new_sel),
                                            &mut self.engine.clipboard,
                                        );
                                        self.engine.layout_dirty = true;
                                        self.redraw();
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                            if let Some(s) = ws.as_scroll_mut() {
                                s.scroll_by(-40.0);
                                self.redraw();
                                return;
                            } else if let Some(s) = ws.as_vlist_mut() {
                                if let Some((ih, ic, cb)) = vlist_props {
                                    let new_sel = s.selected_row.unwrap_or(0).saturating_sub(1);
                                    s.selected_row = Some(new_sel);
                                    s.scroll_to_index(new_sel, ih, ic);
                                    A::update(
                                        &mut self.engine.app_state,
                                        cb(new_sel),
                                        &mut self.engine.clipboard,
                                    );
                                    self.engine.layout_dirty = true;
                                    self.redraw();
                                    return;
                                }
                            }
                        }
                    }
                    Key::Named(NamedKey::PageDown) => {
                        if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                            if let Some(s) = ws.as_scroll_mut() {
                                s.scroll_by(s.viewport_h * 0.9);
                                self.redraw();
                                return;
                            } else if let Some(s) = ws.as_vlist_mut() {
                                if let Some((ih, ic, _)) = vlist_props {
                                    s.scroll_by(s.viewport_h * 0.9, ih, ic);
                                    self.redraw();
                                    return;
                                }
                            }
                        }
                    }
                    Key::Named(NamedKey::PageUp) => {
                        if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                            if let Some(s) = ws.as_scroll_mut() {
                                s.scroll_by(-s.viewport_h * 0.9);
                                self.redraw();
                                return;
                            } else if let Some(s) = ws.as_vlist_mut() {
                                if let Some((ih, ic, _)) = vlist_props {
                                    s.scroll_by(-s.viewport_h * 0.9, ih, ic);
                                    self.redraw();
                                    return;
                                }
                            }
                        }
                    }
                    _ => {}
                }
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
            if self.engine.focused_input_id().is_none() {
                return;
            }
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
        if self.engine.focused_input_id().is_some() {
            self.handle_text_input(key);
            return;
        }
        let _ = self.handle_focused_widget_key(key);
    }

    fn handle_tab(&mut self) {
        let ids = self.engine.runtime_caches.focus_order.clone();
        if ids.is_empty() {
            return;
        }
        let shift = self.engine.modifiers.state().shift_key();
        let next = match self.engine.focused_widget_id {
            Some(cur) => {
                let n = ids.len();
                let i = ids.iter().position(|&x| x == cur).unwrap_or(0);
                ids[if shift { (i + n - 1) % n } else { (i + 1) % n }]
            }
            None => ids[0],
        };
        self.focus_widget(Some(next));
        self.redraw();
    }

    fn handle_text_input(&mut self, key: &Key) {
        let Some(fid) = self.engine.focused_input_id() else {
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

        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };

        let on_change = input.on_change;
        let on_submit = input.on_submit.clone();
        let is_password = input.is_password;
        let is_multiline = input.is_multiline;
        let visible_w = input.visible_w;
        let visible_h = input.visible_h;

        let mut dirty = false;
        let mut full = String::new();
        let mut deleted_selection = None;
        let mut submit_msg = None;

        if let Some(ist) = self.engine.input_states.get_mut(&fid) {
            {
                let mut fs = self.engine.font_system.borrow_mut();
                ist.sync_layout(&mut fs, visible_w, A::theme().font_body, is_multiline);
            }

            if is_del_key {
                let mut fs = self.engine.font_system.borrow_mut();
                ist.normalize_cursor();
                if ist.delete_selection(&mut fs) {
                    ist.update_scroll(
                        &mut fs,
                        visible_w,
                        visible_h,
                        A::theme().font_body,
                        is_password,
                        is_multiline,
                    );
                    deleted_selection = Some(ist.text());
                }
            }

            if deleted_selection.is_none() {
                if is_arrow && is_shift {
                    let before = ist.cursor_byte_index();
                    if ist.selection_anchor.is_none() {
                        ist.selection_anchor = Some(before);
                    }
                    if let Some(action) = map_key(key, self.engine.modifiers.state().control_key())
                    {
                        let mut fs = self.engine.font_system.borrow_mut();
                        ist.normalize_cursor();
                        ist.editor.action(&mut fs, action);
                        ist.normalize_cursor();
                    }
                    let after = ist.cursor_byte_index();
                    let anchor = ist.selection_anchor.unwrap();
                    ist.selection = Some(crate::input_state::TextSelection {
                        start: anchor,
                        end: after,
                    });
                    dirty = true;
                } else if is_arrow {
                    ist.clear_selection();
                    if let Some(action) = map_key(key, self.engine.modifiers.state().control_key())
                    {
                        let mut fs = self.engine.font_system.borrow_mut();
                        ist.normalize_cursor();
                        ist.editor.action(&mut fs, action);
                        ist.normalize_cursor();
                    }
                    dirty = true;
                } else {
                    ist.clear_selection();
                    if let Key::Named(NamedKey::Enter) = key {
                        if is_multiline {
                            let mut fs = self.engine.font_system.borrow_mut();
                            ist.editor.action(&mut fs, Action::Enter);
                            ist.normalize_cursor();
                            dirty = true;
                        } else {
                            submit_msg = on_submit.clone();
                        }
                    } else if let Some(action) =
                        map_key(key, self.engine.modifiers.state().control_key())
                    {
                        let mut fs = self.engine.font_system.borrow_mut();
                        ist.normalize_cursor();
                        ist.editor.action(&mut fs, action);
                        ist.normalize_cursor();
                        dirty = true;
                    }
                }

                if dirty {
                    let mut fs = self.engine.font_system.borrow_mut();
                    ist.update_scroll(
                        &mut fs,
                        visible_w,
                        visible_h,
                        A::theme().font_body,
                        is_password,
                        is_multiline,
                    );
                    full = ist.text();
                }
            }
        }

        if let Some(full) = deleted_selection {
            let msg = on_change(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            self.engine.cursor_blink.reset();
            self.engine.schedule_snapshot();
            self.engine.layout_dirty = true;
            self.redraw();
            return;
        }

        if let Some(msg) = submit_msg {
            self.engine.cursor_blink.reset();
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            self.engine.layout_dirty = true;
            self.redraw();
            return;
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
        if let Some(fid) = self.engine.focused_input_id() {
            if let Some(s) = self.engine.input_states.get(&fid) {
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
            }
        }
        self.redraw();
    }

    fn do_paste(&mut self) {
        let Ok(txt) = self.engine.clipboard.get_text() else {
            return;
        };
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };

        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };
        let txt = sanitize_clipboard_text(&txt, input.is_multiline);
        if txt.is_empty() {
            return;
        }

        let full = if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.sync_layout(
                &mut fs,
                input.visible_w,
                A::theme().font_body,
                input.is_multiline,
            );
            if !s.delete_selection(&mut fs) {
                s.clear_selection();
            }
            s.editor.insert_string(&txt, None);
            s.normalize_cursor();
            s.snapshot();
            s.update_scroll(
                &mut fs,
                input.visible_w,
                input.visible_h,
                A::theme().font_body,
                input.is_password,
                input.is_multiline,
            );
            s.text()
        } else {
            return;
        };

        A::update(
            &mut self.engine.app_state,
            (input.on_change)(full),
            &mut self.engine.clipboard,
        );
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_select_all(&mut self) {
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };

        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };

        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.sync_layout(
                &mut fs,
                input.visible_w,
                A::theme().font_body,
                input.is_multiline,
            );
            s.select_all(&mut fs);
            s.update_scroll(
                &mut fs,
                input.visible_w,
                input.visible_h,
                A::theme().font_body,
                input.is_password,
                input.is_multiline,
            );
        }
        self.redraw();
    }

    fn do_undo(&mut self) {
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };
        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };

        let full = if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.sync_layout(
                &mut fs,
                input.visible_w,
                A::theme().font_body,
                input.is_multiline,
            );
            s.undo(&mut fs);
            s.update_scroll(
                &mut fs,
                input.visible_w,
                input.visible_h,
                A::theme().font_body,
                input.is_password,
                input.is_multiline,
            );
            Some(s.text())
        } else {
            None
        };

        if let Some(full) = full {
            A::update(
                &mut self.engine.app_state,
                (input.on_change)(full),
                &mut self.engine.clipboard,
            );
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }

    fn do_redo(&mut self) {
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };
        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };

        let full = if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.sync_layout(
                &mut fs,
                input.visible_w,
                A::theme().font_body,
                input.is_multiline,
            );
            s.redo(&mut fs);
            s.update_scroll(
                &mut fs,
                input.visible_w,
                input.visible_h,
                A::theme().font_body,
                input.is_password,
                input.is_multiline,
            );
            Some(s.text())
        } else {
            None
        };

        if let Some(full) = full {
            A::update(
                &mut self.engine.app_state,
                (input.on_change)(full),
                &mut self.engine.clipboard,
            );
        }
        self.engine.cursor_blink.reset();
        self.engine.layout_dirty = true;
        self.redraw();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    use super::{collect_toast_runtime_state, sanitize_clipboard_text};
    use crate::engine::widget_state::{ToastState, WidgetState};

    #[test]
    fn sanitize_clipboard_text_strips_controls_and_ansi_sequences() {
        let raw = "hello\u{0000}\u{001B}[31mworld\u{001B}[0m\u{0008}!";
        assert_eq!(sanitize_clipboard_text(raw, false), "helloworld!");
    }

    #[test]
    fn sanitize_clipboard_text_strips_bidi_override_chars() {
        let raw = "safe \u{202E}spoof\u{202C} text\u{200F}";
        assert_eq!(sanitize_clipboard_text(raw, false), "safe spoof text");
    }

    #[test]
    fn sanitize_clipboard_text_preserves_newlines_only_for_multiline() {
        let raw = "a\r\nb\nc\td";
        assert_eq!(sanitize_clipboard_text(raw, true), "a\nb\nc d");
        assert_eq!(sanitize_clipboard_text(raw, false), "a b c d");
    }

    #[test]
    fn collect_toast_runtime_state_detects_timed_toasts_without_expiring_them() {
        let mut toast = ToastState::new(3000);
        toast.created_at = Instant::now() - Duration::from_millis(250);
        let widget_states = HashMap::from([(7, WidgetState::Toast(toast))]);

        let (expired, has_active_timed) = collect_toast_runtime_state(&widget_states);

        assert!(expired.is_empty());
        assert!(has_active_timed);
    }

    #[test]
    fn collect_toast_runtime_state_marks_expired_toasts() {
        let mut toast = ToastState::new(30);
        toast.created_at = Instant::now() - Duration::from_millis(60);
        let widget_states = HashMap::from([(9, WidgetState::Toast(toast))]);

        let (expired, has_active_timed) = collect_toast_runtime_state(&widget_states);

        assert_eq!(expired, vec![9]);
        assert!(!has_active_timed);
    }
}
