// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — engine/runner.rs
// ============================================================

use std::{collections::HashMap, time::Duration};

use cosmic_text::{Action, Edit, FontSystem, Motion};
use skia_safe::{Point, Rect as SkiaRect};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowId,
};
use zeroize::Zeroize;

use super::run_error::RutterRunError;
use super::{InputRuntime, RutterEngine, validate_runtime_reconstruction};
use crate::app::AppLogic;
use crate::engine::widget_state::WidgetState;
use crate::input_limits::{
    InputLimitError, InputLimits, copy_text_with_reserve, validate_clipboard_source, validate_text,
    validate_utf8_range,
};
use crate::input_state::InputWidgetState;
use crate::render::hit_test::{
    ContextMenuOverlayHit, HitResult, PopoverOverlayHit, find_context_menu_target,
    find_scroll_focus, find_scrollbar_drag_hit, hit_test, hit_test_context_menu_overlay,
    hit_test_popover_overlay,
};

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

fn sanitize_clipboard_text(
    text: &str,
    allow_newlines: bool,
    limits: InputLimits,
) -> Result<String, InputLimitError> {
    validate_clipboard_source(text, limits)?;
    let mut sanitized = reserve_clipboard_text(text.len())?;
    append_sanitized_clipboard(text, allow_newlines, &mut sanitized);
    if let Err(error) = validate_text(&sanitized, limits) {
        sanitized.zeroize();
        return Err(error);
    }
    Ok(sanitized)
}

fn reserve_clipboard_text(bytes: usize) -> Result<String, InputLimitError> {
    let mut sanitized = String::new();
    sanitized
        .try_reserve(bytes)
        .map_err(|_| InputLimitError::AllocationFailed {
            requested_bytes: bytes,
            operation: "sanitized clipboard text",
        })?;
    Ok(sanitized)
}

fn append_sanitized_clipboard(text: &str, allow_newlines: bool, sanitized: &mut String) {
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{001B}' {
            skip_ansi_escape(&mut chars);
            continue;
        }

        if is_bidi_override_char(ch) {
            continue;
        }

        if ch == '\r' && chars.peek() == Some(&'\n') {
            chars.next();
        }
        append_sanitized_character(ch, allow_newlines, sanitized);
    }
}

fn append_sanitized_character(ch: char, allow_newlines: bool, sanitized: &mut String) {
    match ch {
        '\r' => {
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

fn copy_input_text(
    state: &InputWidgetState,
    limits: InputLimits,
) -> Result<String, InputLimitError> {
    let text_bytes = state.text_byte_len();
    if text_bytes > limits.max_bytes {
        return Err(InputLimitError::BytesExceeded {
            actual: text_bytes,
            max: limits.max_bytes,
        });
    }

    let text = state.text();
    let Some(selection) = state.selection.filter(|selection| !selection.is_empty()) else {
        return Ok(text);
    };
    let (start, end) = selection.normalized();
    validate_utf8_range(&text, start, end)?;
    let selected = text
        .get(start..end)
        .ok_or(InputLimitError::InvalidUtf8Range {
            start,
            end,
            text_bytes: text.len(),
        })?;
    copy_text_with_reserve(selected, "clipboard copy")
}

fn update_shift_selection(
    state: &mut InputWidgetState,
    key: &Key,
    ctrl: bool,
    font_system: &mut FontSystem,
) {
    let Ok(before) = state.try_cursor_byte_index() else {
        state.clear_selection();
        return;
    };
    let Some(action) = map_key(key, ctrl) else {
        return;
    };
    state.selection_anchor.get_or_insert(before);
    state.normalize_cursor();
    state.editor.action(font_system, action);
    state.normalize_cursor();
    update_selection_after_motion(state);
}

fn update_selection_after_motion(state: &mut InputWidgetState) {
    let Some(anchor) = state.selection_anchor else {
        return;
    };
    let Ok(end) = state.try_cursor_byte_index() else {
        state.clear_selection();
        return;
    };
    state.selection = Some(crate::input_state::TextSelection { start: anchor, end });
}

fn sensitive_delete_action(
    state: &mut InputWidgetState,
    key: &Key,
    font_system: &mut FontSystem,
) -> Result<bool, InputLimitError> {
    match key {
        Key::Named(NamedKey::Backspace) => state.try_delete_before_cursor(font_system),
        Key::Named(NamedKey::Delete) => state.try_delete_after_cursor(font_system),
        _ => Ok(false),
    }
}

fn zeroize_sensitive_text(is_sensitive: bool, text: &mut String) {
    if is_sensitive {
        text.zeroize();
    }
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

fn input_copy_is_blocked<Msg: Clone>(
    runtime: Option<&InputRuntime<Msg>>,
    state: Option<&InputWidgetState>,
) -> bool {
    runtime.is_some_and(|input| input.is_password)
        || state.is_some_and(InputWidgetState::is_sensitive)
}

pub fn snap_to_step(value: f32, min: f32, max: f32, step: f32) -> f32 {
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max {
        return min.is_finite().then_some(min).unwrap_or(0.0);
    }
    if !step.is_finite() || step <= 0.0 {
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
    fatal_error: Option<RutterRunError>,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    /// Runs the application and reports startup failures to standard error.
    ///
    /// Use [`Self::try_run`] when the caller needs to inspect failures.
    pub fn run() {
        if let Err(error) = Self::try_run() {
            eprintln!("Rutter application failed: {error}");
        }
    }

    /// Runs the application and returns controlled event-loop or widget-ID failures.
    ///
    /// # Example
    /// ```no_run
    /// # use rutter::{AppLogic, RutterRunner};
    /// # fn launch<A: AppLogic + 'static>() {
    /// RutterRunner::<A>::try_run().expect("Rutter application failed");
    /// # }
    /// ```
    pub fn try_run() -> Result<(), RutterRunError> {
        let el = EventLoop::new().map_err(RutterRunError::from)?;
        el.set_control_flow(ControlFlow::Wait);
        let mut r = Self {
            engine: RutterEngine::new()?,
            cursor_pos: Point::new(0.0, 0.0),
            scroll_drag: None,
            mouse_down: false,
            last_click_time: std::time::Instant::now(),
            last_click_pos: Point::new(0.0, 0.0),
            focused_input_rect: None,
            fatal_error: None,
        };
        let event_result = el.run_app(&mut r);
        if let Some(error) = r.fatal_error {
            return Err(error);
        }
        event_result.map_err(RutterRunError::from)
    }
}

impl<A: AppLogic + 'static> ApplicationHandler for RutterRunner<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if let Err(error) = self.engine.handle_resumed(el) {
            self.terminate_for_error(el, error.into());
        }
    }

    fn new_events(&mut self, el: &ActiveEventLoop, _: StartCause) {
        if self.fatal_error.is_some() {
            el.exit();
            return;
        }
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
        if self.fatal_error.is_some() {
            el.exit();
            return;
        }
        self.engine.process_accessibility_event(&event);
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let Err(error) = self.engine.handle_resize(size) {
                        self.terminate_for_error(el, error.into());
                    }
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
                self.engine.last_mouse_pos = Point::new(
                    self.cursor_pos.x / self.engine.scale_factor,
                    self.cursor_pos.y / self.engine.scale_factor,
                );

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
                        } else if let Some(s) = ws.as_vgrid_mut() {
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
                self.mouse_down = button == winit::event::MouseButton::Left;
                let now = std::time::Instant::now();
                let is_double = now.duration_since(self.last_click_time)
                    < Duration::from_millis(300)
                    && (self.cursor_pos.x - self.last_click_pos.x).abs() < 5.0
                    && (self.cursor_pos.y - self.last_click_pos.y).abs() < 5.0;
                self.last_click_time = now;
                self.last_click_pos = self.cursor_pos;

                let size = self.engine.window.as_ref().unwrap().inner_size();
                if let Err(error) = self.engine.try_ensure_widget_states() {
                    self.terminate_for_error(el, error.into());
                    return;
                }
                if let Err(error) = self.engine.try_ensure_layout(size) {
                    self.terminate_for_error(el, error.into());
                    return;
                }

                let cursor = Point::new(
                    self.cursor_pos.x / self.engine.scale_factor,
                    self.cursor_pos.y / self.engine.scale_factor,
                );
                self.engine.last_mouse_pos = cursor;
                let viewport_size = (
                    size.width as f32 / self.engine.scale_factor,
                    size.height as f32 / self.engine.scale_factor,
                );
                let (
                    context_menu_overlay_hit,
                    popover_overlay_hit,
                    context_menu_target,
                    scroll_drag_hit,
                    active_scroll_id,
                    mut hit,
                ) = {
                    let wt = A::view(&mut self.engine.app_state);
                    if let Err(error) = validate_runtime_reconstruction(
                        self.engine.widget_id_snapshot.as_ref(),
                        &wt,
                    ) {
                        drop(wt);
                        self.terminate_for_error(el, error.into());
                        return;
                    }
                    let context_menu_overlay_hit = if button == MouseButton::Left {
                        hit_test_context_menu_overlay(
                            &wt,
                            cursor,
                            viewport_size,
                            &self.engine.widget_states,
                            A::theme().font_body,
                        )
                    } else {
                        None
                    };
                    let popover_overlay_hit = if button == MouseButton::Left {
                        hit_test_popover_overlay(
                            &wt,
                            &self.engine.taffy,
                            self.engine.last_root_node,
                            cursor,
                            viewport_size,
                            &self.engine.widget_states,
                        )
                    } else {
                        None
                    };
                    let context_menu_target = if button == MouseButton::Right {
                        find_context_menu_target(
                            &wt,
                            &self.engine.taffy,
                            self.engine.last_root_node,
                            cursor,
                            Point::new(0.0, 0.0),
                        )
                    } else {
                        None
                    };
                    let scroll_drag_hit = if button == MouseButton::Left {
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
                    (
                        context_menu_overlay_hit,
                        popover_overlay_hit,
                        context_menu_target,
                        scroll_drag_hit,
                        active_scroll_id,
                        hit,
                    )
                };

                if let Some(menu_hit) = context_menu_overlay_hit {
                    self.close_all_selects();
                    match menu_hit {
                        ContextMenuOverlayHit::Item { msg, .. } => {
                            self.engine.close_all_context_menus();
                            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                            self.engine.layout_dirty = true;
                        }
                        ContextMenuOverlayHit::Consume => {}
                        ContextMenuOverlayHit::Dismiss => {
                            self.engine.close_all_context_menus();
                        }
                    }
                    self.redraw();
                    return;
                }

                if let Some(popover_hit) = popover_overlay_hit {
                    match popover_hit {
                        PopoverOverlayHit::Content(content_hit) => {
                            hit = Some(content_hit);
                        }
                        PopoverOverlayHit::Consume => {
                            self.redraw();
                            return;
                        }
                        PopoverOverlayHit::Dismiss { id, on_dismiss } => {
                            self.close_all_selects();
                            if let Some(msg) = on_dismiss {
                                A::update(
                                    &mut self.engine.app_state,
                                    msg,
                                    &mut self.engine.clipboard,
                                );
                            } else if id == 0 {
                                self.engine.close_all_popovers();
                            } else {
                                self.engine.close_popover(id);
                            }
                            self.engine.layout_dirty = true;
                            self.redraw();
                            return;
                        }
                    }
                }

                if button == MouseButton::Right {
                    self.close_all_selects();
                    if let Some(id) = context_menu_target {
                        self.engine.open_context_menu(id, cursor);
                        self.redraw();
                        return;
                    }
                    if self.engine.close_all_context_menus() {
                        self.redraw();
                        return;
                    }
                    if self.engine.close_all_popovers() {
                        self.redraw();
                        return;
                    }
                } else if self.engine.any_context_menu_open() {
                    self.engine.close_all_context_menus();
                    self.redraw();
                    return;
                }

                if button == MouseButton::Left {
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
                        HitResult::VGridSelect { id, index } => {
                            if button == winit::event::MouseButton::Left {
                                self.focus_widget(Some(id));
                                if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                    if let Some(grid) = ws.as_vgrid_mut() {
                                        grid.selected_item = Some(index);
                                    }
                                }
                                let cb = self
                                    .engine
                                    .runtime_caches
                                    .vgrids
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
                    let vgrid_props = self
                        .engine
                        .runtime_caches
                        .vgrids
                        .get(&sid)
                        .map(|v| (v.item_height, v.item_count, v.columns));

                    if let Some(ws) = self.engine.widget_states.get_mut(&sid) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.scroll_by(dy);
                            dirty = true;
                        } else if let Some(s) = ws.as_vlist_mut() {
                            if let Some((ih, ic)) = vlist_props {
                                s.scroll_by(dy, ih, ic);
                                dirty = true;
                            }
                        } else if let Some(s) = ws.as_vgrid_mut() {
                            if let Some((ih, ic, cols)) = vgrid_props {
                                s.scroll_by(dy, ih, ic, cols);
                                dirty = true;
                            }
                        }
                    }
                } else {
                    let mut virtual_to_scroll = None;
                    for (id, ws) in self.engine.widget_states.iter() {
                        if ws.as_vlist().is_some() {
                            if let Some(props) = self
                                .engine
                                .runtime_caches
                                .vlists
                                .get(id)
                                .map(|v| (*id, v.item_height, v.item_count, None))
                            {
                                virtual_to_scroll = Some(props);
                                break;
                            }
                        } else if ws.as_vgrid().is_some() {
                            if let Some(props) = self
                                .engine
                                .runtime_caches
                                .vgrids
                                .get(id)
                                .map(|v| (*id, v.item_height, v.item_count, Some(v.columns)))
                            {
                                virtual_to_scroll = Some(props);
                                break;
                            }
                        }
                    }
                    if let Some((id, ih, ic, cols)) = virtual_to_scroll {
                        if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                            if let Some(vl) = ws.as_vlist_mut() {
                                vl.scroll_by(dy, ih, ic);
                                dirty = true;
                            } else if let Some(grid) = ws.as_vgrid_mut() {
                                grid.scroll_by(dy, ih, ic, cols.unwrap_or(1));
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
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.engine.try_redraw(self.cursor_pos) {
                    self.terminate_for_error(el, error.into());
                }
            }
            _ => {}
        }
    }
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    fn terminate_for_error(&mut self, event_loop: &ActiveEventLoop, error: RutterRunError) {
        if self.fatal_error.is_none() {
            self.fatal_error = Some(error);
        }
        event_loop.exit();
    }

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

        let Some(full) = self.try_insert_committed_text(
            fid,
            text,
            visible_w,
            visible_h,
            is_password,
            is_multiline,
        ) else {
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

    fn try_insert_committed_text(
        &mut self,
        input_id: u64,
        text: &str,
        visible_width: f32,
        visible_height: f32,
        is_password: bool,
        is_multiline: bool,
    ) -> Option<String> {
        let state = self.engine.input_states.get_mut(&input_id)?;
        let mut font_system = self.engine.font_system.borrow_mut();
        state.sync_layout(
            &mut font_system,
            visible_width,
            A::theme().font_body,
            is_multiline,
        );
        let changed = state.try_insert_text(&mut font_system, text).ok()?;
        if !changed {
            return None;
        }
        state.update_scroll(
            &mut font_system,
            visible_width,
            visible_height,
            A::theme().font_body,
            is_password,
            is_multiline,
        );
        Some(state.text())
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

        if let Some(vgrid) = self.engine.runtime_caches.vgrids.get(&fid).cloned() {
            let current = self
                .engine
                .widget_states
                .get(&fid)
                .and_then(|ws| ws.as_vgrid())
                .and_then(|state| state.selected_item)
                .unwrap_or(0);
            let next_index = match key {
                Key::Named(NamedKey::ArrowLeft) => Some(current.saturating_sub(1)),
                Key::Named(NamedKey::ArrowRight) => {
                    Some((current + 1).min(vgrid.item_count.saturating_sub(1)))
                }
                Key::Named(NamedKey::ArrowUp) => Some(current.saturating_sub(vgrid.columns.max(1))),
                Key::Named(NamedKey::ArrowDown) => {
                    Some((current + vgrid.columns.max(1)).min(vgrid.item_count.saturating_sub(1)))
                }
                Key::Named(NamedKey::Home) => Some(0),
                Key::Named(NamedKey::End) => Some(vgrid.item_count.saturating_sub(1)),
                Key::Named(NamedKey::PageUp) => {
                    let rows_per_page = self
                        .engine
                        .widget_states
                        .get(&fid)
                        .and_then(|ws| ws.as_vgrid())
                        .map(|state| (state.viewport_h / vgrid.item_height).floor() as usize)
                        .unwrap_or(1)
                        .max(1);
                    Some(current.saturating_sub(rows_per_page * vgrid.columns.max(1)))
                }
                Key::Named(NamedKey::PageDown) => {
                    let rows_per_page = self
                        .engine
                        .widget_states
                        .get(&fid)
                        .and_then(|ws| ws.as_vgrid())
                        .map(|state| (state.viewport_h / vgrid.item_height).floor() as usize)
                        .unwrap_or(1)
                        .max(1);
                    Some(
                        (current + rows_per_page * vgrid.columns.max(1))
                            .min(vgrid.item_count.saturating_sub(1)),
                    )
                }
                _ if is_activation_key(key) => {
                    Some(current.min(vgrid.item_count.saturating_sub(1)))
                }
                _ => None,
            };
            if let Some(next_index) = next_index {
                if let Some(ws) = self.engine.widget_states.get_mut(&fid) {
                    if let Some(state) = ws.as_vgrid_mut() {
                        state.selected_item = Some(next_index);
                        state.scroll_to_index(
                            next_index,
                            vgrid.item_height,
                            vgrid.item_count,
                            vgrid.columns,
                        );
                    }
                }
                A::update(
                    &mut self.engine.app_state,
                    (vgrid.on_select)(next_index),
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
                self.redraw();
                return true;
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
                let vgrid_props = self
                    .engine
                    .runtime_caches
                    .vgrids
                    .get(&sid)
                    .map(|v| (v.item_height, v.item_count, v.columns, v.on_select));
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
                            } else if let Some(s) = ws.as_vgrid_mut() {
                                if let Some((ih, ic, cols, cb)) = vgrid_props {
                                    let new_sel = (s.selected_item.unwrap_or(0) + cols.max(1))
                                        .min(ic.saturating_sub(1));
                                    s.selected_item = Some(new_sel);
                                    s.scroll_to_index(new_sel, ih, ic, cols);
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
                            } else if let Some(s) = ws.as_vgrid_mut() {
                                if let Some((ih, ic, cols, cb)) = vgrid_props {
                                    let _ = ic;
                                    let new_sel =
                                        s.selected_item.unwrap_or(0).saturating_sub(cols.max(1));
                                    s.selected_item = Some(new_sel);
                                    s.scroll_to_index(new_sel, ih, ic, cols);
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
                            } else if let Some(s) = ws.as_vgrid_mut() {
                                if let Some((ih, ic, cols, _)) = vgrid_props {
                                    s.scroll_by(s.viewport_h * 0.9, ih, ic, cols);
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
                            } else if let Some(s) = ws.as_vgrid_mut() {
                                if let Some((ih, ic, cols, _)) = vgrid_props {
                                    s.scroll_by(-s.viewport_h * 0.9, ih, ic, cols);
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
            self.engine.close_all_context_menus();
            self.engine.close_all_popovers();
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

        let mut content_changed = false;
        let mut visual_changed = false;
        let mut full = None;
        let mut deleted_selection = None;
        let mut selection_rejected = false;
        let mut submit_msg = None;

        if let Some(ist) = self.engine.input_states.get_mut(&fid) {
            {
                let mut fs = self.engine.font_system.borrow_mut();
                ist.sync_layout(&mut fs, visible_w, A::theme().font_body, is_multiline);
            }

            if is_del_key {
                let mut fs = self.engine.font_system.borrow_mut();
                ist.normalize_cursor();
                match ist.try_delete_selection(&mut fs) {
                    Ok(true) => {
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
                    Ok(false) => {}
                    Err(_) => selection_rejected = true,
                }
            }

            if deleted_selection.is_none() && !selection_rejected {
                if is_arrow && is_shift {
                    let mut fs = self.engine.font_system.borrow_mut();
                    update_shift_selection(
                        ist,
                        key,
                        self.engine.modifiers.state().control_key(),
                        &mut fs,
                    );
                    visual_changed = true;
                } else if is_arrow {
                    ist.clear_selection();
                    if let Some(action) = map_key(key, self.engine.modifiers.state().control_key())
                    {
                        let mut fs = self.engine.font_system.borrow_mut();
                        ist.normalize_cursor();
                        ist.editor.action(&mut fs, action);
                        ist.normalize_cursor();
                    }
                    visual_changed = true;
                } else {
                    ist.clear_selection();
                    if let Key::Named(NamedKey::Enter) = key {
                        if is_multiline {
                            let mut fs = self.engine.font_system.borrow_mut();
                            content_changed = ist.try_insert_text(&mut fs, "\n").unwrap_or(false);
                            visual_changed = content_changed;
                        } else {
                            submit_msg = on_submit.clone();
                        }
                    } else if let Some(action) =
                        map_key(key, self.engine.modifiers.state().control_key())
                    {
                        let mut fs = self.engine.font_system.borrow_mut();
                        let before = ist.text_byte_len();
                        content_changed = if ist.is_sensitive() {
                            sensitive_delete_action(ist, key, &mut fs).unwrap_or(false)
                        } else {
                            ist.normalize_cursor();
                            ist.editor.action(&mut fs, action);
                            ist.normalize_cursor();
                            ist.text_byte_len() != before
                        };
                        visual_changed = true;
                    }
                }

                if visual_changed {
                    let mut fs = self.engine.font_system.borrow_mut();
                    ist.update_scroll(
                        &mut fs,
                        visible_w,
                        visible_h,
                        A::theme().font_body,
                        is_password,
                        is_multiline,
                    );
                }
                if content_changed {
                    full = Some(ist.text());
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

        if let Some(full) = full {
            self.engine.cursor_blink.reset();
            self.engine.schedule_snapshot();
            let msg = on_change(full);
            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
            self.engine.layout_dirty = true;
            self.redraw();
            return;
        }

        if visual_changed {
            self.engine.cursor_blink.reset();
            self.redraw();
        }
    }

    fn do_copy(&mut self) {
        if let Some(fid) = self.engine.focused_input_id() {
            let runtime = self.engine.runtime_caches.inputs.get(&fid);
            let state = self.engine.input_states.get(&fid);
            if input_copy_is_blocked(runtime, state) {
                self.redraw();
                return;
            }
            if let (Some(input), Some(state)) = (runtime, state) {
                if let Ok(text_to_copy) = copy_input_text(state, input.limits) {
                    let _ = self.engine.clipboard.set_text(text_to_copy);
                }
            }
        }
        self.redraw();
    }

    fn do_paste(&mut self) {
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };

        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };
        if !self.engine.input_states.contains_key(&fid) {
            return;
        }
        let Ok(mut clipboard_text) = self.engine.clipboard.get_text() else {
            return;
        };
        let mut txt =
            match sanitize_clipboard_text(&clipboard_text, input.is_multiline, input.limits) {
                Ok(text) => text,
                Err(_) => {
                    zeroize_sensitive_text(input.is_password, &mut clipboard_text);
                    return;
                }
            };
        zeroize_sensitive_text(input.is_password, &mut clipboard_text);
        if txt.is_empty() {
            zeroize_sensitive_text(input.is_password, &mut txt);
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
            let changed = match s.try_insert_text(&mut fs, &txt) {
                Ok(changed) => changed,
                Err(_) => {
                    zeroize_sensitive_text(input.is_password, &mut txt);
                    return;
                }
            };
            if !changed {
                zeroize_sensitive_text(input.is_password, &mut txt);
                return;
            }
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
            zeroize_sensitive_text(input.is_password, &mut txt);
            return;
        };

        zeroize_sensitive_text(input.is_password, &mut txt);
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

        let Some(full) = self.restore_input_history(fid, &input, InputWidgetState::try_undo) else {
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

    fn do_redo(&mut self) {
        let Some(fid) = self.engine.focused_input_id() else {
            return;
        };
        let Some(input) = self.engine.runtime_caches.inputs.get(&fid).cloned() else {
            return;
        };

        let Some(full) = self.restore_input_history(fid, &input, InputWidgetState::try_redo) else {
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

    fn restore_input_history(
        &mut self,
        input_id: u64,
        input: &InputRuntime<A::Message>,
        restore: fn(
            &mut InputWidgetState,
            &mut cosmic_text::FontSystem,
        ) -> Result<bool, InputLimitError>,
    ) -> Option<String> {
        let state = self.engine.input_states.get_mut(&input_id)?;
        let mut font_system = self.engine.font_system.borrow_mut();
        state.sync_layout(
            &mut font_system,
            input.visible_w,
            A::theme().font_body,
            input.is_multiline,
        );
        if !restore(state, &mut font_system).ok()? {
            return None;
        }
        state.update_scroll(
            &mut font_system,
            input.visible_w,
            input.visible_h,
            A::theme().font_body,
            input.is_password,
            input.is_multiline,
        );
        Some(state.text())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    use cosmic_text::FontSystem;

    use super::{collect_toast_runtime_state, input_copy_is_blocked, sanitize_clipboard_text};
    use crate::engine::widget_state::{ToastState, WidgetState};
    use crate::input_limits::{InputKind, InputLimits};
    use crate::input_state::InputWidgetState;

    #[test]
    fn sanitize_clipboard_text_strips_controls_and_ansi_sequences() {
        let raw = "hello\u{0000}\u{001B}[31mworld\u{001B}[0m\u{0008}!";
        assert_eq!(
            sanitize_clipboard_text(raw, false, InputLimits::default()).unwrap(),
            "helloworld!"
        );
    }

    #[test]
    fn sanitize_clipboard_text_strips_bidi_override_chars() {
        let raw = "safe \u{202E}spoof\u{202C} text\u{200F}";
        assert_eq!(
            sanitize_clipboard_text(raw, false, InputLimits::default()).unwrap(),
            "safe spoof text"
        );
    }

    #[test]
    fn sanitize_clipboard_text_preserves_newlines_only_for_multiline() {
        let raw = "a\r\nb\nc\td";
        assert_eq!(
            sanitize_clipboard_text(raw, true, InputLimits::default()).unwrap(),
            "a\nb\nc d"
        );
        assert_eq!(
            sanitize_clipboard_text(raw, false, InputKind::TextInput.limits()).unwrap(),
            "a b c d"
        );
    }

    #[test]
    fn sensitive_state_blocks_copy_without_password_runtime_metadata() {
        let mut font_system = FontSystem::new();
        let mut state = InputWidgetState::new(&mut font_system);
        state.set_sensitive(true);

        assert!(input_copy_is_blocked::<()>(None, Some(&state)));
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
