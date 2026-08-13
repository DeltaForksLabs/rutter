// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — engine/runner.rs
// ============================================================

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use cosmic_text::{Action, Edit, FontSystem, Motion};
use skia_safe::{Point, Rect as SkiaRect};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{WindowAttributes, WindowId},
};
use zeroize::Zeroize;

use super::gpu::{BackendType, GraphicsError};
use super::run_error::RutterRunError;
use super::{InputRuntime, RutterEngine, validate_runtime_reconstruction};
use crate::app::AppLogic;
use crate::engine::widget_state::WidgetState;
use crate::i18n::LayoutDirection;
use crate::input_limits::{
    InputLimitError, InputLimits, copy_text_with_reserve, validate_clipboard_source, validate_text,
    validate_utf8_range,
};
use crate::input_state::InputWidgetState;
use crate::render::dropdown_menu_overlay::{
    DropdownMenuOverlayHit, hit_test_dropdown_menu_overlay,
};
use crate::render::hit_test::{
    ContextMenuOverlayHit, HitResult, PopoverOverlayHit, find_context_menu_target,
    find_scroll_focus, find_scrollbar_drag_hit, hit_test, hit_test_context_menu_overlay,
    hit_test_popover_overlay,
};
use crate::render::select_overlay::collector::collect_open_dropdown_overlays;
use crate::render::select_overlay::hit_test_select_overlay;

mod accessibility_actions;
mod dropdown_keyboard;
mod dropdown_pointer;

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

fn sanitize_input_text(
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

fn sanitize_clipboard_text(
    text: &str,
    allow_newlines: bool,
    limits: InputLimits,
) -> Result<String, InputLimitError> {
    sanitize_input_text(text, allow_newlines, limits)
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
    matches!(key, Key::Named(NamedKey::Enter | NamedKey::Space))
        || matches!(key, Key::Character(ch) if ch == " ")
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

fn collect_open_popover_dismissals<Msg: Clone>(
    widget_states: &HashMap<u64, WidgetState>,
    dismiss_callbacks: &HashMap<u64, Msg>,
) -> Vec<Msg> {
    let mut dismissals = widget_states
        .iter()
        .filter(|(_, state)| state.as_popover().is_some_and(|popover| popover.is_open))
        .filter_map(|(id, _)| {
            dismiss_callbacks
                .get(id)
                .cloned()
                .map(|message| (*id, message))
        })
        .collect::<Vec<_>>();
    dismissals.sort_by_key(|(id, _)| *id);
    dismissals.into_iter().map(|(_, message)| message).collect()
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

fn wheel_deltas(delta: MouseScrollDelta) -> (f32, f32) {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => (-x * 40.0, -y * 40.0),
        MouseScrollDelta::PixelDelta(point) => (-point.x as f32, -point.y as f32),
    }
}

fn wheel_select_index(current: usize, option_count: usize, delta_y: f32) -> Option<usize> {
    if option_count == 0 || delta_y.abs() <= f32::EPSILON {
        return None;
    }
    let next = if delta_y > 0.0 {
        (current + 1).min(option_count - 1)
    } else {
        current.saturating_sub(1)
    };
    (next != current).then_some(next)
}

fn carousel_key_index(
    key: &Key,
    current: usize,
    item_count: usize,
    direction: LayoutDirection,
) -> Option<usize> {
    if item_count == 0 {
        return None;
    }
    if is_activation_key(key) {
        return Some(current.min(item_count - 1));
    }
    carousel_navigation_index(key, current, item_count, direction).filter(|next| *next != current)
}

fn carousel_navigation_index(
    key: &Key,
    current: usize,
    item_count: usize,
    direction: LayoutDirection,
) -> Option<usize> {
    let backward = current.saturating_sub(1);
    let forward = current.saturating_add(1).min(item_count - 1);
    match (key, direction) {
        (Key::Named(NamedKey::ArrowLeft), LayoutDirection::Ltr) => Some(backward),
        (Key::Named(NamedKey::ArrowRight), LayoutDirection::Ltr) => Some(forward),
        (Key::Named(NamedKey::ArrowLeft), LayoutDirection::Rtl) => Some(forward),
        (Key::Named(NamedKey::ArrowRight), LayoutDirection::Rtl) => Some(backward),
        (Key::Named(NamedKey::Home), _) => Some(0),
        (Key::Named(NamedKey::End), _) => Some(item_count - 1),
        _ => None,
    }
}

fn carousel_wheel_delta(delta_x: f32, delta_y: f32, direction: LayoutDirection) -> f32 {
    if delta_x.abs() <= delta_y.abs() {
        return delta_y;
    }
    match direction {
        LayoutDirection::Ltr => delta_x,
        LayoutDirection::Rtl => -delta_x,
    }
}

#[derive(Debug, Clone)]
struct ScrollDrag {
    id: u64,
    start_y: f32,
    start_offset: f32,
    viewport_h: f32,
    content_h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowEventDestination {
    ActiveWindow,
    Discard,
}

fn classify_window_event<Identifier: Eq>(
    active_window_id: Option<&Identifier>,
    received_window_id: &Identifier,
) -> WindowEventDestination {
    match active_window_id {
        Some(active_window_id) if active_window_id == received_window_id => {
            WindowEventDestination::ActiveWindow
        }
        Some(_) | None => WindowEventDestination::Discard,
    }
}

pub struct RutterRunner<A: AppLogic> {
    engine: RutterEngine<A>,
    active_window_id: Option<WindowId>,
    cursor_pos: Point,
    scroll_drag: Option<ScrollDrag>,
    mouse_down: bool,
    last_click_time: std::time::Instant,
    last_click_pos: Point,
    focused_input_rect: Option<SkiaRect>,
    fatal_error: Option<RutterRunError>,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(crate) fn with_engine(engine: RutterEngine<A>) -> Self {
        Self {
            engine,
            active_window_id: None,
            cursor_pos: Point::new(0.0, 0.0),
            scroll_drag: None,
            mouse_down: false,
            last_click_time: Instant::now(),
            last_click_pos: Point::new(0.0, 0.0),
            focused_input_rect: None,
            fatal_error: None,
        }
    }

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
        let mut r = Self::with_engine(RutterEngine::new()?);
        r.engine.set_accessibility_waker(el.create_proxy());
        let event_result = el.run_app(&mut r);
        if let Some(error) = r.fatal_error {
            return Err(error);
        }
        event_result.map_err(RutterRunError::from)
    }
}

impl<A: AppLogic + 'static> ApplicationHandler for RutterRunner<A> {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: ()) {
        self.process_accessibility_actions();
        if self.fatal_error.is_some() {
            event_loop.exit();
        }
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.active_window_id.is_some() {
            return;
        }
        match self.resume_surface(el, None, None) {
            Ok(_) => {}
            Err(error) => self.terminate_for_error(el, error.into()),
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.release_surface();
    }

    fn new_events(&mut self, el: &ActiveEventLoop, _: StartCause) {
        if self.fatal_error.is_some() {
            el.exit();
            return;
        }
        match self.process_scheduled_work() {
            Some(deadline) => el.set_control_flow(ControlFlow::WaitUntil(deadline)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if self.fatal_error.is_some() {
            el.exit();
            return;
        }
        if classify_window_event(self.active_window_id.as_ref(), &window_id)
            == WindowEventDestination::Discard
        {
            // Failed backend probes can leave queued events for a provisional window.
            return;
        }
        self.engine.process_accessibility_event(&event);
        self.process_accessibility_actions();
        if self.fatal_error.is_some() {
            el.exit();
            return;
        }
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
            WindowEvent::Ime(Ime::Commit(text)) => self.insert_text_commit(&text),
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
                            let theme = A::theme_for(&self.engine.app_state);
                            let pad_x = theme.spacing * 2.0;
                            let pad_y = theme.spacing;
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
                if let Err(error) = self.refresh_dropdown_hover() {
                    self.terminate_for_error(el, error);
                    return;
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
                let theme = A::theme_for(&self.engine.app_state);
                let (
                    context_menu_overlay_hit,
                    dropdown_menu_overlay_hit,
                    select_overlay_hit,
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
                            theme.font_body,
                        )
                    } else {
                        None
                    };
                    let dropdown_menu_overlay_hit =
                        if matches!(button, MouseButton::Left | MouseButton::Right) {
                            let overlays = collect_open_dropdown_overlays(
                                &wt,
                                &self.engine.taffy,
                                self.engine.last_root_node,
                                &self.engine.widget_states,
                                viewport_size,
                            );
                            hit_test_dropdown_menu_overlay(
                                &overlays,
                                cursor,
                                viewport_size,
                                A::locale().direction(),
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
                    let select_overlay_hit = if button == MouseButton::Left {
                        hit_test_select_overlay(
                            &wt,
                            &self.engine.taffy,
                            self.engine.last_root_node,
                            &self.engine.widget_states,
                            cursor,
                            viewport_size,
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
                    let scroll_drag_hit =
                        if button == MouseButton::Left && select_overlay_hit.is_none() {
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
                        &self.engine.widget_states,
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
                        dropdown_menu_overlay_hit,
                        select_overlay_hit,
                        popover_overlay_hit,
                        context_menu_target,
                        scroll_drag_hit,
                        active_scroll_id,
                        hit,
                    )
                };

                if let Some(menu_hit) = context_menu_overlay_hit {
                    self.close_all_selects();
                    self.close_all_dropdown_menus();
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

                if let Some(dropdown_hit) = dropdown_menu_overlay_hit {
                    self.close_all_selects();
                    let route_through_dropdown = button == MouseButton::Right
                        || !matches!(dropdown_hit, DropdownMenuOverlayHit::Trigger { .. });
                    if route_through_dropdown {
                        let consumed = self.handle_dropdown_pointer_hit(dropdown_hit, button);
                        if consumed {
                            self.redraw();
                            return;
                        }
                    }
                }

                if let Some(select_hit) = select_overlay_hit {
                    hit = Some(HitResult::SelectOption {
                        id: select_hit.id,
                        index: select_hit.index,
                    });
                } else if let Some(popover_hit) = popover_overlay_hit {
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
                    self.close_all_dropdown_menus();
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
                        self.close_all_selects();
                        self.begin_scroll_drag(hit);
                        return;
                    }
                }

                self.engine.active_scroll_id = active_scroll_id;

                let select_was_open = match &hit {
                    Some(HitResult::SelectToggle(id)) => self
                        .engine
                        .widget_states
                        .get(id)
                        .and_then(WidgetState::as_select)
                        .is_some_and(|state| state.is_open),
                    _ => false,
                };
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
                                if let Some(ws) = self.engine.widget_states.get_mut(&id) {
                                    if let Some(s) = ws.as_select_mut() {
                                        s.is_open = !select_was_open;
                                    }
                                }
                                self.engine.layout_dirty = true;
                            }
                        }
                        HitResult::DropdownMenuToggle(id) => {
                            if button == winit::event::MouseButton::Left {
                                self.toggle_dropdown_menu(id, false);
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
                        HitResult::CarouselSelect { id, index } => {
                            if button == winit::event::MouseButton::Left {
                                self.activate_carousel_item(id, index);
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
                let dropdown_target = match self.refresh_dropdown_scroll_target() {
                    Ok(target) => target,
                    Err(error) => {
                        self.terminate_for_error(el, error);
                        return;
                    }
                };
                let select_popup = match self.refresh_scroll_target_at_cursor() {
                    Ok(select_popup) => select_popup,
                    Err(error) => {
                        self.terminate_for_error(el, error);
                        return;
                    }
                };
                let (delta_x, delta_y) = wheel_deltas(delta);
                if let Some(target) = dropdown_target {
                    let _ = self.scroll_open_dropdown(target, delta_y);
                    self.redraw();
                    return;
                }
                let select_changed =
                    select_popup.is_some_and(|id| self.scroll_open_select(id, delta_y));
                if select_changed || self.scroll_active_target(delta_x, delta_y) {
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
    pub(crate) fn resume_surface(
        &mut self,
        event_loop: &ActiveEventLoop,
        attributes: Option<WindowAttributes>,
        required_backend: Option<BackendType>,
    ) -> Result<(WindowId, BackendType), GraphicsError> {
        match attributes {
            Some(attributes) => self.engine.handle_resumed_with_attributes(
                event_loop,
                attributes,
                required_backend,
            )?,
            None => self.engine.handle_resumed(event_loop)?,
        }
        self.register_resumed_window()
    }

    fn register_resumed_window(&mut self) -> Result<(WindowId, BackendType), GraphicsError> {
        let backend = self
            .engine
            .backend_type()
            .ok_or(GraphicsError::BackendUnavailable {
                operation: "retaining the resumed surface backend",
            })?;
        let window_id = self
            .engine
            .window
            .as_ref()
            .map(|window| window.id())
            .ok_or(GraphicsError::BackendUnavailable {
                operation: "registering a resumed native window",
            })?;
        self.active_window_id = Some(window_id);
        Ok((window_id, backend))
    }

    pub(crate) fn release_surface(&mut self) {
        self.active_window_id = None;
        self.scroll_drag = None;
        self.mouse_down = false;
        self.focused_input_rect = None;
        self.engine.release_surface();
    }

    pub(crate) fn app_state_mut(&mut self) -> &mut A::State {
        &mut self.engine.app_state
    }

    pub(crate) fn invalidate_and_redraw(&mut self) {
        self.engine.layout_dirty = true;
        self.redraw();
    }

    pub(crate) fn set_surface_visible(&self, visible: bool) {
        if let Some(window) = self.engine.window.as_ref() {
            window.set_visible(visible);
        }
    }

    pub(crate) fn request_surface_redraw(&self) {
        self.redraw();
    }

    pub(crate) fn take_fatal_error(&mut self) -> Option<RutterRunError> {
        self.fatal_error.take()
    }

    pub(crate) fn process_scheduled_work(&mut self) -> Option<Instant> {
        self.engine.maybe_snapshot();
        let timed_toasts = self.expire_timed_toasts();
        if self.tick_surface_animations() || timed_toasts {
            self.redraw();
            return Some(Instant::now() + Duration::from_millis(16));
        }
        self.focused_input_deadline()
    }

    fn expire_timed_toasts(&mut self) -> bool {
        let (expired, has_active) = collect_toast_runtime_state(&self.engine.widget_states);
        for id in expired.iter().copied() {
            self.dismiss_expired_toast(id);
        }
        if !expired.is_empty() {
            self.engine.layout_dirty = true;
            self.redraw();
        }
        has_active
    }

    fn dismiss_expired_toast(&mut self, id: u64) {
        if let Some(toast) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(|state| state.as_toast_mut())
        {
            toast.visible = false;
        }
        if let Some(message) = self.engine.runtime_caches.toast_dismiss.get(&id).cloned() {
            A::update(
                &mut self.engine.app_state,
                message,
                &mut self.engine.clipboard,
            );
        }
    }

    fn tick_surface_animations(&mut self) -> bool {
        if !self.engine.has_animated {
            return false;
        }
        if self.engine.tick_animations() {
            self.redraw();
        }
        true
    }

    fn focused_input_deadline(&mut self) -> Option<Instant> {
        self.engine.focused_input_id()?;
        if self.engine.cursor_blink.tick() {
            self.redraw();
        }
        Some(self.engine.cursor_blink.next_tick_at())
    }

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

        let mut sanitized = match sanitize_input_text(text, is_multiline, input.limits) {
            Ok(value) if !value.is_empty() => value,
            Ok(_) | Err(_) => return,
        };
        let full = self.try_insert_committed_text(
            fid,
            &sanitized,
            visible_w,
            visible_h,
            is_password,
            is_multiline,
        );
        if is_password {
            sanitized.zeroize();
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

    fn try_insert_committed_text(
        &mut self,
        input_id: u64,
        text: &str,
        visible_width: f32,
        visible_height: f32,
        is_password: bool,
        is_multiline: bool,
    ) -> Option<String> {
        let theme = A::theme_for(&self.engine.app_state);
        let state = self.engine.input_states.get_mut(&input_id)?;
        let mut font_system = self.engine.font_system.borrow_mut();
        state.sync_layout(
            &mut font_system,
            visible_width,
            theme.font_body,
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
            theme.font_body,
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
        let retained_menu = focus_id
            .and_then(|id| self.dropdown_focus_target(id))
            .map(|(id, _)| id);
        self.close_dropdowns_except(retained_menu);
        if self.engine.focused_widget_id == focus_id {
            return;
        }
        if self
            .engine
            .focused_widget_id
            .is_some_and(|id| self.engine.runtime_caches.selects.contains_key(&id))
        {
            self.close_all_selects();
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

    fn activate_carousel_item(&mut self, id: u64, index: usize) {
        let Some(runtime) = self.engine.runtime_caches.carousels.get(&id).cloned() else {
            return;
        };
        self.focus_widget(Some(id));
        self.dispatch_carousel_selection(id, index, &runtime);
    }

    fn refresh_scroll_target_at_cursor(&mut self) -> Result<Option<u64>, RutterRunError> {
        let size = self.engine.window.as_ref().unwrap().inner_size();
        self.engine.try_ensure_widget_states()?;
        self.engine.try_ensure_layout(size)?;
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        let viewport = (
            size.width as f32 / self.engine.scale_factor,
            size.height as f32 / self.engine.scale_factor,
        );
        let select_popup = hit_test_select_overlay(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            &self.engine.widget_states,
            self.engine.last_mouse_pos,
            viewport,
        )
        .map(|hit| hit.id);
        self.engine.active_scroll_id = select_popup
            .is_none()
            .then(|| {
                find_scroll_focus(
                    &widget,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    self.engine.last_mouse_pos,
                    Point::new(0.0, 0.0),
                    &self.engine.widget_states,
                )
            })
            .flatten();
        Ok(select_popup)
    }

    fn scroll_open_select(&mut self, id: u64, delta_y: f32) -> bool {
        let Some(select) = self.engine.runtime_caches.selects.get(&id).cloned() else {
            return false;
        };
        let Some(next) = wheel_select_index(select.selected_index, select.option_count, delta_y)
        else {
            return false;
        };
        if let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_select_mut)
        {
            state.hovered_option = Some(next);
        }
        A::update(
            &mut self.engine.app_state,
            (select.on_change)(next),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
        true
    }

    fn scroll_active_target(&mut self, delta_x: f32, delta_y: f32) -> bool {
        let Some(id) = self.engine.active_scroll_id else {
            return false;
        };
        if let Some(runtime) = self.engine.runtime_caches.carousels.get(&id).cloned() {
            return self.scroll_carousel(id, delta_x, delta_y, &runtime);
        }
        self.scroll_vertical_target(id, delta_y)
    }

    fn scroll_carousel(
        &mut self,
        id: u64,
        delta_x: f32,
        delta_y: f32,
        runtime: &super::CarouselRuntime<A::Message>,
    ) -> bool {
        let delta = carousel_wheel_delta(delta_x, delta_y, A::locale().direction());
        self.engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_carousel_mut)
            .is_some_and(|state| state.scroll_by_pixels(delta, &runtime.config, runtime.item_count))
    }

    fn scroll_vertical_target(&mut self, id: u64, delta_y: f32) -> bool {
        let list = self.engine.runtime_caches.vlists.get(&id).cloned();
        let grid = self.engine.runtime_caches.vgrids.get(&id).cloned();
        let Some(state) = self.engine.widget_states.get_mut(&id) else {
            return false;
        };
        if let Some(scroll) = state.as_scroll_mut() {
            scroll.scroll_by(delta_y);
            return true;
        }
        if let (Some(list), Some(vlist)) = (list, state.as_vlist_mut()) {
            vlist.scroll_by(delta_y, list.item_height, list.item_count);
            return true;
        }
        if let (Some(grid), Some(vgrid)) = (grid, state.as_vgrid_mut()) {
            vgrid.scroll_by(delta_y, grid.item_height, grid.item_count, grid.columns);
            return true;
        }
        false
    }

    fn close_all_selects(&mut self) {
        for ws in self.engine.widget_states.values_mut() {
            if let Some(s) = ws.as_select_mut() {
                s.is_open = false;
                s.hovered_option = None;
            }
        }
        self.engine.layout_dirty = true;
    }

    fn dismiss_open_popovers(&mut self) {
        let messages = collect_open_popover_dismissals(
            &self.engine.widget_states,
            &self.engine.runtime_caches.popover_dismiss,
        );
        self.engine.close_all_popovers();
        for message in messages {
            A::update(
                &mut self.engine.app_state,
                message,
                &mut self.engine.clipboard,
            );
            self.engine.layout_dirty = true;
        }
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

        let theme = A::theme_for(&self.engine.app_state);
        let pad_x = theme.spacing * 2.0;
        let pad_y = theme.spacing;
        let input = self.engine.runtime_caches.inputs.get(&id).cloned();
        if let Some(ist) = self.engine.input_states.get_mut(&id) {
            let mut fs = self.engine.font_system.borrow_mut();
            if let Some(input) = input.as_ref() {
                ist.sync_layout(
                    &mut fs,
                    input.visible_w,
                    theme.font_body,
                    input.is_multiline,
                );
            } else {
                ist.set_metrics(&mut fs, theme.font_body);
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
                theme.font_body,
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
        if self.handle_dropdown_key(fid, key) {
            return true;
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
                let was_open = self
                    .engine
                    .widget_states
                    .get(&fid)
                    .and_then(WidgetState::as_select)
                    .is_some_and(|state| state.is_open);
                self.close_all_selects();
                if let Some(ws) = self.engine.widget_states.get_mut(&fid) {
                    if let Some(state) = ws.as_select_mut() {
                        state.is_open = !was_open;
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

        if self.handle_carousel_key(fid, key) {
            return true;
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

    fn handle_carousel_key(&mut self, id: u64, key: &Key) -> bool {
        let Some(runtime) = self.engine.runtime_caches.carousels.get(&id).cloned() else {
            return false;
        };
        let current = self.carousel_current_index(id, runtime.item_count);
        let Some(next) =
            carousel_key_index(key, current, runtime.item_count, A::locale().direction())
        else {
            return false;
        };
        self.dispatch_carousel_selection(id, next, &runtime);
        true
    }

    fn carousel_current_index(&self, id: u64, item_count: usize) -> usize {
        self.engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_carousel)
            .and_then(|state| state.current_index(item_count))
            .unwrap_or(0)
    }

    fn dispatch_carousel_selection(
        &mut self,
        id: u64,
        index: usize,
        runtime: &super::CarouselRuntime<A::Message>,
    ) {
        if let Some(WidgetState::Carousel(state)) = self.engine.widget_states.get_mut(&id) {
            state.select(index, &runtime.config, runtime.item_count);
        }
        A::update(
            &mut self.engine.app_state,
            (runtime.on_select)(index),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
        self.redraw();
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
            if self.close_active_dropdown_menu() {
                self.redraw();
                return;
            }
            self.close_all_selects();
            self.engine.close_all_context_menus();
            self.dismiss_open_popovers();
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
        if let Some((id, _)) = self
            .engine
            .focused_widget_id
            .and_then(|focus_id| self.dropdown_focus_target(focus_id))
        {
            self.close_dropdown_menu(id, true);
        }
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
        let theme = A::theme_for(&self.engine.app_state);

        let mut content_changed = false;
        let mut visual_changed = false;
        let mut full = None;
        let mut deleted_selection = None;
        let mut selection_rejected = false;
        let mut submit_msg = None;

        if let Some(ist) = self.engine.input_states.get_mut(&fid) {
            {
                let mut fs = self.engine.font_system.borrow_mut();
                ist.sync_layout(&mut fs, visible_w, theme.font_body, is_multiline);
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
                            theme.font_body,
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
                        theme.font_body,
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

        let theme = A::theme_for(&self.engine.app_state);
        let full = if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.sync_layout(
                &mut fs,
                input.visible_w,
                theme.font_body,
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
                theme.font_body,
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

        let theme = A::theme_for(&self.engine.app_state);
        if let Some(s) = self.engine.input_states.get_mut(&fid) {
            let mut fs = self.engine.font_system.borrow_mut();
            s.sync_layout(
                &mut fs,
                input.visible_w,
                theme.font_body,
                input.is_multiline,
            );
            s.select_all(&mut fs);
            s.update_scroll(
                &mut fs,
                input.visible_w,
                input.visible_h,
                theme.font_body,
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
        let theme = A::theme_for(&self.engine.app_state);
        let state = self.engine.input_states.get_mut(&input_id)?;
        let mut font_system = self.engine.font_system.borrow_mut();
        state.sync_layout(
            &mut font_system,
            input.visible_w,
            theme.font_body,
            input.is_multiline,
        );
        if !restore(state, &mut font_system).ok()? {
            return None;
        }
        state.update_scroll(
            &mut font_system,
            input.visible_w,
            input.visible_h,
            theme.font_body,
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
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;
    use winit::keyboard::{Key, NamedKey};

    use super::{
        WindowEventDestination, carousel_key_index, carousel_wheel_delta, classify_window_event,
        collect_open_popover_dismissals, collect_toast_runtime_state, input_copy_is_blocked,
        sanitize_clipboard_text, sanitize_input_text, wheel_deltas, wheel_select_index,
    };
    use crate::LayoutDirection;
    use crate::engine::widget_state::{PopoverState, ToastState, WidgetState};
    use crate::input_limits::{InputKind, InputLimits};
    use crate::input_state::InputWidgetState;

    #[test]
    fn wheel_deltas_preserve_horizontal_trackpad_input() {
        assert_eq!(
            wheel_deltas(MouseScrollDelta::LineDelta(2.0, -1.0)),
            (-80.0, 40.0)
        );
        assert_eq!(
            wheel_deltas(MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                12.0, -8.0
            ))),
            (-12.0, 8.0)
        );
    }

    #[test]
    fn select_wheel_navigation_clamps_to_option_boundaries() {
        assert_eq!(wheel_select_index(1, 4, 40.0), Some(2));
        assert_eq!(wheel_select_index(1, 4, -40.0), Some(0));
        assert_eq!(wheel_select_index(3, 4, 40.0), None);
        assert_eq!(wheel_select_index(0, 4, -40.0), None);
    }

    #[test]
    fn carousel_arrow_navigation_follows_layout_direction() {
        let right = Key::Named(NamedKey::ArrowRight);
        let left = Key::Named(NamedKey::ArrowLeft);
        assert_eq!(
            carousel_key_index(&right, 2, 5, LayoutDirection::Ltr),
            Some(3)
        );
        assert_eq!(
            carousel_key_index(&left, 2, 5, LayoutDirection::Rtl),
            Some(3)
        );
    }

    #[test]
    fn carousel_wheel_uses_dominant_axis_and_mirrors_rtl_horizontal_input() {
        assert_eq!(carousel_wheel_delta(1.0, 40.0, LayoutDirection::Ltr), 40.0);
        assert_eq!(carousel_wheel_delta(40.0, 1.0, LayoutDirection::Ltr), 40.0);
        assert_eq!(carousel_wheel_delta(40.0, 1.0, LayoutDirection::Rtl), -40.0);
    }

    #[test]
    fn carousel_boundary_navigation_does_not_repeat_selection() {
        let left = Key::Named(NamedKey::ArrowLeft);
        let right = Key::Named(NamedKey::ArrowRight);
        let home = Key::Named(NamedKey::Home);
        let end = Key::Named(NamedKey::End);
        assert_eq!(carousel_key_index(&left, 0, 5, LayoutDirection::Ltr), None);
        assert_eq!(carousel_key_index(&right, 4, 5, LayoutDirection::Ltr), None);
        assert_eq!(carousel_key_index(&home, 0, 5, LayoutDirection::Ltr), None);
        assert_eq!(carousel_key_index(&end, 4, 5, LayoutDirection::Ltr), None);
    }

    #[test]
    fn empty_carousel_keyboard_navigation_emits_no_index() {
        let end = Key::Named(NamedKey::End);
        assert_eq!(carousel_key_index(&end, 0, 0, LayoutDirection::Ltr), None);
    }

    #[test]
    fn foreign_window_events_are_discarded_after_active_window_registration() {
        let provisional_window_id = 41_u64;
        let active_window_id = 73_u64;

        assert_eq!(
            classify_window_event(Some(&active_window_id), &active_window_id),
            WindowEventDestination::ActiveWindow
        );
        assert_eq!(
            classify_window_event(Some(&active_window_id), &provisional_window_id),
            WindowEventDestination::Discard
        );
        assert_eq!(
            classify_window_event(None, &provisional_window_id),
            WindowEventDestination::Discard
        );
    }

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
    fn sanitize_input_text_applies_clipboard_policy_to_key_and_ime_commits() {
        let raw = "safe\u{202E}\u{001b}[31m\nvalue\u{0007}";

        assert_eq!(
            sanitize_input_text(raw, false, InputLimits::default()).unwrap(),
            "safe value"
        );
        assert_eq!(
            sanitize_input_text(raw, true, InputLimits::default()).unwrap(),
            "safe\nvalue"
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

    #[test]
    fn escape_dismissals_include_only_open_popovers_with_callbacks() {
        let mut open = PopoverState::default();
        open.set_open(true);
        let states = HashMap::from([
            (
                5,
                WidgetState::Popover(PopoverState {
                    is_open: true,
                    ..PopoverState::default()
                }),
            ),
            (7, WidgetState::Popover(open)),
            (9, WidgetState::Popover(PopoverState::default())),
        ]);
        let callbacks = HashMap::from([
            (5, "close first"),
            (7, "close active"),
            (9, "close inactive"),
        ]);

        assert_eq!(
            collect_open_popover_dismissals(&states, &callbacks),
            vec!["close first", "close active"]
        );
    }
}
