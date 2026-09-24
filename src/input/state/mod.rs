// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — input/state/mod.rs
// ============================================================

use crate::input_limits::InputLimits;
use cosmic_text::{
    Action, Attrs, Buffer, Cursor, Edit, Editor, FontSystem, LayoutRun, Metrics, Motion, Selection,
    Shaping, Wrap,
};

mod buffer;
mod edit;
mod normalization;
mod undo;

pub use undo::UndoStack;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextSelection {
    pub start: usize,
    pub end: usize,
}

impl TextSelection {
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
    pub fn normalized(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

pub struct InputWidgetState {
    pub editor: Editor<'static>,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub undo: UndoStack,
    pub selection: Option<TextSelection>,
    pub selection_anchor: Option<usize>,
    limits: InputLimits,
    sensitive: bool,
}

pub(crate) fn cursor_x_in_run(cursor: Cursor, run: &LayoutRun<'_>) -> Option<f32> {
    if cursor.line != run.line_i {
        return None;
    }

    for glyph in run.glyphs.iter() {
        if cursor.index == glyph.start {
            return Some(glyph.x);
        }
        if cursor.index > glyph.start && cursor.index < glyph.end {
            let cluster = &run.text[glyph.start..glyph.end];
            let mut before = 0;
            let mut total = 0;
            for (index, _) in cluster.char_indices() {
                if glyph.start + index < cursor.index {
                    before += 1;
                }
                total += 1;
            }
            let offset = if total == 0 {
                0.0
            } else {
                glyph.w * (before as f32) / (total as f32)
            };
            return Some(if glyph.level.is_rtl() {
                glyph.x + glyph.w - offset
            } else {
                glyph.x + offset
            });
        }
    }

    match run.glyphs.last() {
        Some(glyph) if cursor.index == glyph.end => Some(if glyph.level.is_rtl() {
            glyph.x
        } else {
            glyph.x + glyph.w
        }),
        None if cursor.index == 0 => Some(0.0),
        _ => None,
    }
}

impl InputWidgetState {
    pub(crate) fn set_sensitive(&mut self, sensitive: bool) {
        // A live buffer must be removed before losing sensitivity so stale secrets never become copyable.
        if self.sensitive || !sensitive {
            return;
        }
        self.sensitive = true;
        self.undo.set_sensitive(true);
    }

    pub(crate) fn is_sensitive(&self) -> bool {
        self.sensitive
    }

    pub fn text(&self) -> String {
        self.editor.with_buffer(|b| {
            let mut text = String::new();
            for (index, line) in b.lines.iter().enumerate() {
                if index > 0 {
                    text.push('\n');
                }
                text.push_str(line.text());
            }
            text
        })
    }

    pub(crate) fn text_is_empty(&self) -> bool {
        self.editor
            .with_buffer(|b| b.lines.iter().all(|line| line.text().is_empty()))
    }

    pub(crate) fn text_byte_len(&self) -> usize {
        self.editor.with_buffer(|b| {
            b.lines
                .iter()
                .enumerate()
                .fold(0usize, |total, (index, line)| {
                    total
                        .saturating_add(line.text().len())
                        .saturating_add(usize::from(index > 0))
                })
        })
    }

    fn text_char_count(&self) -> usize {
        self.editor.with_buffer(|b| {
            b.lines
                .iter()
                .enumerate()
                .fold(0usize, |total, (index, line)| {
                    total
                        .saturating_add(line.text().chars().count())
                        .saturating_add(usize::from(index > 0))
                })
        })
    }

    pub(crate) fn password_display_index_for_line(
        &self,
        line_index: usize,
        byte_index: usize,
    ) -> usize {
        const BULLET_UTF8_LEN: usize = "•".len();
        let chars = self.chars_before_line_byte_index(line_index, byte_index);
        chars * BULLET_UTF8_LEN
    }

    pub(crate) fn selection_in_display_line(
        &self,
        selection: TextSelection,
        line_index: usize,
        is_password: bool,
    ) -> Option<TextSelection> {
        self.editor.with_buffer(|buffer| {
            let line = buffer.lines.get(line_index)?;
            let line_start = flattened_line_start(buffer, line_index)?;
            let line_end = line_start.checked_add(line.text().len())?;
            line_local_selection(selection, line_start, line_end, line.text(), is_password)
        })
    }

    fn chars_before_line_byte_index(&self, line_index: usize, byte_index: usize) -> usize {
        self.editor.with_buffer(|b| {
            let Some(line) = b.lines.get(line_index) else {
                return 0;
            };
            let index = floor_char_boundary(line.text(), byte_index);
            line.text()[..index].chars().count()
        })
    }

    pub fn set_metrics(&mut self, fs: &mut FontSystem, font_size: f32) {
        self.editor.with_buffer_mut(|b| {
            b.set_metrics(fs, Metrics::new(font_size, font_size * 1.3));
        });
    }

    pub fn sync_layout(
        &mut self,
        fs: &mut FontSystem,
        visible_width: f32,
        font_size: f32,
        is_multiline: bool,
    ) {
        let width = if is_multiline {
            visible_width.max(1.0)
        } else {
            10_000.0
        };
        let wrap = if is_multiline {
            Wrap::WordOrGlyph
        } else {
            Wrap::None
        };

        self.editor.with_buffer_mut(|buffer| {
            buffer.set_metrics(fs, Metrics::new(font_size, font_size * 1.3));
            buffer.set_wrap(fs, wrap);
            buffer.set_size(fs, Some(width), None);
        });
        self.editor.shape_as_needed(fs, false);
        self.normalize_cursor();
    }

    pub fn normalize_cursor(&mut self) {
        let cursor = self.editor.cursor();
        let normalized = self.editor.with_buffer(|buffer| {
            let last_line = buffer.lines.len().saturating_sub(1);
            let line = cursor.line.min(last_line);
            let index = buffer
                .lines
                .get(line)
                .map(|line| floor_char_boundary(line.text(), cursor.index))
                .unwrap_or(0);
            Cursor::new_with_affinity(line, index, cursor.affinity)
        });
        if normalized != cursor {
            self.editor.set_cursor(normalized);
        }
    }

    pub fn cursor_byte_index(&self) -> usize {
        self.try_cursor_byte_index()
            .unwrap_or_else(|_| self.text_byte_len())
    }

    pub(crate) fn try_cursor_byte_index(
        &self,
    ) -> Result<usize, crate::input_limits::InputLimitError> {
        buffer::cursor_flattened_offset(&self.editor, self.editor.cursor())
    }

    pub fn select_all(&mut self, fs: &mut FontSystem) {
        let text_len = self.text_byte_len();
        if text_len == 0 {
            return;
        }
        self.selection = Some(TextSelection {
            start: 0,
            end: text_len,
        });
        self.selection_anchor = Some(0);
        self.editor.action(fs, Action::Motion(Motion::End));
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_anchor = None;
    }

    pub fn sync_selection(&mut self) {
        let Some((start, end)) = self.editor.selection_bounds() else {
            self.clear_selection();
            return;
        };
        let cursor = self.editor.cursor();
        let anchor = if cursor.line == start.line && cursor.index == start.index {
            end
        } else {
            start
        };
        let Ok(cursor_offset) = buffer::cursor_flattened_offset(&self.editor, cursor) else {
            self.clear_selection();
            return;
        };
        let Ok(anchor_offset) = buffer::cursor_flattened_offset(&self.editor, anchor) else {
            self.clear_selection();
            return;
        };
        if cursor_offset == anchor_offset {
            self.clear_selection();
            return;
        }
        self.selection = Some(TextSelection {
            start: anchor_offset,
            end: cursor_offset,
        });
        self.selection_anchor = Some(anchor_offset);
    }

    /// Selects a whole identifier-like word, including transitions between
    /// lower- and uppercase letters that the editor's Unicode word mode splits.
    pub(crate) fn double_click_word(&mut self, fs: &mut FontSystem, x: i32, y: i32) {
        self.editor.action(fs, Action::DoubleClick { x, y });
        if let Some((start, end)) = self.editor.selection_bounds()
            && start.line == end.line
            && let Some((word_start, word_end)) = self.editor.with_buffer(|buffer| {
                let text = buffer.lines.get(start.line)?.text();
                let selected = text.get(start.index..end.index)?;
                if selected.is_empty() || !selected.chars().all(is_word_character) {
                    return None;
                }
                let word_start = text[..start.index]
                    .char_indices()
                    .rev()
                    .take_while(|(_, ch)| is_word_character(*ch))
                    .last()
                    .map_or(start.index, |(index, _)| index);
                let word_end = end.index
                    + text[end.index..]
                        .chars()
                        .take_while(|ch| is_word_character(*ch))
                        .map(char::len_utf8)
                        .sum::<usize>();
                Some((word_start, word_end))
            })
        {
            self.editor
                .set_selection(Selection::Normal(Cursor::new(start.line, word_start)));
            self.editor.set_cursor(Cursor::new(end.line, word_end));
        }
        self.normalize_cursor();
        self.sync_selection();
    }

    /// Snapshots selected bytes only when the press hits the highlighted text.
    /// Password buffers never expose their contents through drag callbacks.
    pub(crate) fn selected_text_at(&self, x: f32, y: f32, max_bytes: usize) -> Option<String> {
        if self.sensitive {
            return None;
        }
        let (start, end) = self
            .selection
            .filter(|selection| !selection.is_empty())?
            .normalized();
        if end.checked_sub(start)? > max_bytes {
            return None;
        }
        let cursor = self.editor.with_buffer(|buffer| buffer.hit(x, y))?;
        let offset = buffer::cursor_flattened_offset(&self.editor, cursor).ok()?;
        if !(start..end).contains(&offset) {
            return None;
        }
        self.text().get(start..end).map(str::to_owned)
    }

    pub fn display_text(&self, is_password: bool) -> String {
        if is_password {
            "•".repeat(self.text_char_count())
        } else {
            self.text()
        }
    }

    pub fn cursor_x_with_metrics(
        &self,
        fs: &mut FontSystem,
        font_size: f32,
        is_password: bool,
        visible_width: f32,
        is_multiline: bool,
    ) -> f32 {
        let display = self.display_text(is_password);
        let mut buf = Buffer::new(fs, Metrics::new(font_size, font_size * 1.3));
        buf.set_wrap(
            fs,
            if is_multiline {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            },
        );
        buf.set_size(
            fs,
            Some(if is_multiline {
                visible_width.max(1.0)
            } else {
                10_000.0
            }),
            None,
        );
        buf.set_text(fs, &display, &Attrs::new(), Shaping::Advanced, None);
        buf.shape_until_scroll(fs, false);

        let cursor = self.editor.cursor();
        let mapped_cursor = if is_password {
            Cursor::new(
                cursor.line,
                self.password_display_index_for_line(cursor.line, cursor.index),
            )
        } else {
            cursor
        };

        let mut cx = 0.0;

        for run in buf.layout_runs() {
            if let Some(run_x) = cursor_x_in_run(mapped_cursor, &run) {
                cx = run_x;
                break;
            }
        }
        cx
    }

    pub fn update_scroll(
        &mut self,
        fs: &mut FontSystem,
        visible_width: f32,
        visible_height: f32,
        font_size: f32,
        is_password: bool,
        is_multiline: bool,
    ) {
        self.editor.shape_as_needed(fs, false);
        self.normalize_cursor();

        let display = self.display_text(is_password);
        let line_height = font_size * 1.3;
        let mut buf = Buffer::new(fs, Metrics::new(font_size, line_height));
        buf.set_wrap(
            fs,
            if is_multiline {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            },
        );
        buf.set_size(
            fs,
            Some(if is_multiline {
                visible_width.max(1.0)
            } else {
                10_000.0
            }),
            None,
        );
        buf.set_text(fs, &display, &Attrs::new(), Shaping::Advanced, None);
        buf.shape_until_scroll(fs, false);

        let cursor = self.editor.cursor();
        let mapped_cursor = if is_password {
            Cursor::new(
                cursor.line,
                self.password_display_index_for_line(cursor.line, cursor.index),
            )
        } else {
            cursor
        };

        let mut line_top = 0.0;
        let mut cursor_h = line_height;
        let mut content_height = line_height;

        for run in buf.layout_runs() {
            content_height = content_height.max(run.line_top + run.line_height);
            if cursor_x_in_run(mapped_cursor, &run).is_some() {
                line_top = run.line_top;
                cursor_h = run.line_height;
            }
        }

        if is_multiline {
            self.scroll_x = 0.0;
            let margin = 8.0;
            let visible_height = visible_height.max(cursor_h);
            let cursor_bottom = line_top + cursor_h;
            if cursor_bottom > self.scroll_y + visible_height {
                self.scroll_y = (cursor_bottom - visible_height + margin).max(0.0);
            } else if line_top < self.scroll_y {
                self.scroll_y = (line_top - margin).max(0.0);
            }
            self.scroll_y = self
                .scroll_y
                .min((content_height - visible_height).max(0.0));
        } else {
            self.scroll_y = 0.0;
            let cx = self.cursor_x_with_metrics(fs, font_size, is_password, visible_width, false);
            if cx > self.scroll_x + visible_width {
                self.scroll_x = cx - visible_width + 20.0;
            } else if cx < self.scroll_x {
                self.scroll_x = (cx - 20.0).max(0.0);
            }
        }
    }

    fn zeroize_editor_text(&mut self) {
        self.editor.with_buffer_mut(|buffer| {
            for line in &mut buffer.lines {
                let zeroed = "\0".repeat(line.text().len());
                let attrs = line.attrs_list().clone();
                line.set_text(zeroed, line.ending(), attrs);
            }
        });
    }
}

fn is_word_character(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let index = index.min(text.len());
    if text.is_char_boundary(index) {
        return index;
    }
    text.char_indices()
        .map(|(offset, _)| offset)
        .take_while(|offset| *offset < index)
        .last()
        .unwrap_or(0)
}

fn flattened_line_start(buffer: &Buffer, line_index: usize) -> Option<usize> {
    buffer
        .lines
        .iter()
        .take(line_index)
        .try_fold(0usize, |offset, line| {
            offset.checked_add(line.text().len())?.checked_add(1)
        })
}

fn line_local_selection(
    selection: TextSelection,
    line_start: usize,
    line_end: usize,
    line_text: &str,
    is_password: bool,
) -> Option<TextSelection> {
    let (start, end) = selection.normalized();
    let start = start.max(line_start);
    let end = end.min(line_end);
    if start >= end
        || !line_text.is_char_boundary(start - line_start)
        || !line_text.is_char_boundary(end - line_start)
    {
        return None;
    }
    let start = start - line_start;
    let end = end - line_start;
    Some(display_selection(line_text, start, end, is_password))
}

fn display_selection(
    line_text: &str,
    start: usize,
    end: usize,
    is_password: bool,
) -> TextSelection {
    if !is_password {
        return TextSelection { start, end };
    }
    TextSelection {
        start: bullet_byte_offset(line_text, start),
        end: bullet_byte_offset(line_text, end),
    }
}

fn bullet_byte_offset(line_text: &str, byte_index: usize) -> usize {
    line_text[..byte_index].chars().count() * "•".len()
}

impl Drop for InputWidgetState {
    fn drop(&mut self) {
        if self.sensitive {
            self.zeroize_editor_text();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs() -> FontSystem {
        FontSystem::new()
    }

    #[test]
    fn normalize_cursor_clamps_stale_line_before_backspace() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "abc");
        state.editor.set_cursor(Cursor::new(3, 3));

        state.normalize_cursor();
        state.editor.action(&mut fs, Action::Backspace);

        assert_eq!(state.editor.cursor(), Cursor::new(0, 2));
        assert_eq!(state.text(), "ab");
    }

    #[test]
    fn multiline_scroll_follows_wrapped_caret() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(
            &mut fs,
            "this is a long line that should wrap across multiple visual rows",
        );
        state.sync_layout(&mut fs, 72.0, 14.0, true);
        state.update_scroll(&mut fs, 72.0, 20.0, 14.0, false, true);

        assert!(state.scroll_y > 0.0);
        assert!((state.scroll_x - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sensitive_snapshot_does_not_keep_undo_text() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_sensitive(true);
        state.set_text(&mut fs, "correct horse battery staple");

        state.snapshot();

        assert!(!state.undo.can_undo());
        assert_eq!(state.undo.current(), None);
        assert!(state.is_sensitive());
        state.set_sensitive(false);
        assert!(state.is_sensitive());
    }

    #[test]
    fn password_display_text_uses_bullets_without_plaintext() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_sensitive(true);
        state.set_text(&mut fs, "secret");

        assert_eq!(state.display_text(true), "••••••");
        assert!(!state.text_is_empty());
        assert_eq!(state.password_display_index_for_line(0, 3), "•••".len());
    }

    #[test]
    fn selection_in_display_line_uses_flattened_offsets_for_each_line() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "éx\nbeta");
        let selection = TextSelection { start: 2, end: 6 };

        assert_eq!(
            state.selection_in_display_line(selection, 0, false),
            Some(TextSelection { start: 2, end: 3 })
        );
        assert_eq!(
            state.selection_in_display_line(selection, 1, true),
            Some(TextSelection {
                start: 0,
                end: "••".len()
            })
        );
    }

    #[test]
    fn double_click_selection_can_be_dragged_only_from_inside_the_selected_word() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "River and Forest");
        state.sync_layout(&mut fs, 240.0, 16.0, false);
        state
            .editor
            .action(&mut fs, Action::DoubleClick { x: 4, y: 8 });
        state.sync_selection();

        assert_eq!(
            state.selected_text_at(4.0, 8.0, 64).as_deref(),
            Some("River")
        );
        assert_eq!(state.selected_text_at(170.0, 8.0, 64), None);
        assert_eq!(state.selected_text_at(4.0, 8.0, 4), None);
        assert_eq!(state.text(), "River and Forest");
    }

    #[test]
    fn double_click_in_middle_selects_entire_word() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "chatGPT follows");
        state.sync_layout(&mut fs, 240.0, 16.0, false);
        let middle_x = state.editor.with_buffer(|buffer| {
            buffer
                .layout_runs()
                .find_map(|run| cursor_x_in_run(Cursor::new(0, 4), &run))
                .unwrap()
        });
        state.double_click_word(&mut fs, middle_x as i32, 8);

        assert_eq!(state.selection.unwrap().normalized(), (0, "chatGPT".len()));
    }

    #[test]
    fn double_click_keeps_unicode_letters_and_stops_at_punctuation() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "caféÉcole/next");
        state.sync_layout(&mut fs, 240.0, 16.0, false);
        let middle_x = state.editor.with_buffer(|buffer| {
            buffer
                .layout_runs()
                .find_map(|run| cursor_x_in_run(Cursor::new(0, 5), &run))
                .unwrap()
        });
        state.double_click_word(&mut fs, middle_x as i32, 8);

        assert_eq!(
            state.selection.unwrap().normalized(),
            (0, "caféÉcole".len())
        );
    }

    #[test]
    fn sensitive_input_never_exposes_a_selected_snapshot() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "secret");
        state.sync_layout(&mut fs, 180.0, 16.0, false);
        state.select_all(&mut fs);
        state.set_sensitive(true);

        assert_eq!(state.selected_text_at(3.0, 8.0, 64), None);
    }

    #[test]
    fn selected_drag_preserves_utf8_boundaries_and_rejects_oversized_snapshots() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_text(&mut fs, "école and more");
        state.sync_layout(&mut fs, 240.0, 16.0, false);
        state.selection = Some(TextSelection {
            start: 0,
            end: "école".len(),
        });

        assert_eq!(
            state.selected_text_at(4.0, 8.0, 64).as_deref(),
            Some("école")
        );
        assert_eq!(state.selected_text_at(4.0, 8.0, 5), None);
        state.selection = Some(TextSelection { start: 1, end: 6 });
        assert_eq!(state.selected_text_at(4.0, 8.0, 64), None);
    }

    #[test]
    fn sensitive_edits_replace_buffer_without_undo_snapshots() {
        let mut fs = fs();
        let mut state = InputWidgetState::new(&mut fs);
        state.set_sensitive(true);
        state.try_insert_text(&mut fs, "éx").unwrap();

        assert!(state.try_delete_before_cursor(&mut fs).unwrap());
        assert_eq!(state.text(), "é");
        assert!(!state.undo.can_undo());
    }

    #[test]
    fn programmatic_edits_preserve_line_limit_validation_after_normalization() {
        let raw = "first\r\nsecond\tthird\u{0000}";
        let mut fs = fs();
        let mut multiline = InputWidgetState::new(&mut fs);
        let mut single_line = InputWidgetState::new_with_limits(
            &mut fs,
            crate::input_limits::InputKind::TextInput.limits(),
        );

        assert!(multiline.try_set_text(&mut fs, raw).unwrap());
        let error = single_line.try_insert_text(&mut fs, raw).unwrap_err();
        assert_eq!(multiline.text(), "first\nsecond third");
        assert!(matches!(
            error,
            crate::input_limits::InputLimitError::LinesExceeded { .. }
        ));
        assert!(single_line.text().is_empty());
    }
}
