// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — engine/input_state.rs
// ============================================================

use std::collections::VecDeque;

use cosmic_text::{
    Action, Attrs, Buffer, Cursor, Edit, Editor, FontSystem, LayoutRun, Metrics, Motion, Shaping,
    Wrap,
};

pub struct UndoStack {
    stack: VecDeque<String>,
    position: usize,
    max_size: usize,
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            stack: VecDeque::new(),
            position: 0,
            max_size,
        }
    }
    pub fn push(&mut self, text: String) {
        if self.stack.back().map(|s| s == &text).unwrap_or(false) {
            return;
        }
        while self.stack.len() > self.position + 1 {
            self.stack.pop_back();
        }
        self.stack.push_back(text);
        if self.stack.len() > self.max_size {
            self.stack.pop_front();
        }
        self.position = self.stack.len().saturating_sub(1);
    }
    pub fn undo(&mut self) -> Option<&str> {
        if self.position > 0 {
            self.position -= 1;
            self.stack.get(self.position).map(String::as_str)
        } else {
            None
        }
    }
    pub fn redo(&mut self) -> Option<&str> {
        if self.position + 1 < self.stack.len() {
            self.position += 1;
            self.stack.get(self.position).map(String::as_str)
        } else {
            None
        }
    }
    pub fn current(&self) -> Option<&str> {
        self.stack.get(self.position).map(String::as_str)
    }
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }
    pub fn can_redo(&self) -> bool {
        self.position + 1 < self.stack.len()
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(100)
    }
}

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
    pub fn new(fs: &mut FontSystem) -> Self {
        let buf = Buffer::new(fs, Metrics::new(14.0, 18.0));
        let mut editor = Editor::new(buf);
        let mut undo = UndoStack::default();
        undo.push(String::new());
        editor.set_auto_indent(false);
        Self {
            editor,
            scroll_x: 0.0,
            scroll_y: 0.0,
            undo,
            selection: None,
            selection_anchor: None,
        }
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

    pub fn set_text(&mut self, fs: &mut FontSystem, text: &str) {
        self.editor
            .with_buffer_mut(|b| b.set_text(fs, text, &Attrs::new(), Shaping::Advanced, None));
        self.editor.shape_as_needed(fs, false);
        self.editor.action(fs, Action::Motion(Motion::End));
        self.normalize_cursor();
        self.selection = None;
        self.selection_anchor = None;
    }

    pub fn normalize_cursor(&mut self) {
        let cursor = self.editor.cursor();
        let normalized = self.editor.with_buffer(|buffer| {
            let last_line = buffer.lines.len().saturating_sub(1);
            let line = cursor.line.min(last_line);
            let index = buffer
                .lines
                .get(line)
                .map(|line| cursor.index.min(line.text().len()))
                .unwrap_or(0);
            Cursor::new_with_affinity(line, index, cursor.affinity)
        });
        if normalized != cursor {
            self.editor.set_cursor(normalized);
        }
    }

    pub fn cursor_byte_index(&self) -> usize {
        self.editor.cursor().index
    }

    pub fn select_all(&mut self, fs: &mut FontSystem) {
        let text = self.text();
        if text.is_empty() {
            return;
        }
        self.selection = Some(TextSelection {
            start: 0,
            end: text.len(),
        });
        self.selection_anchor = Some(0);
        self.editor.action(fs, Action::Motion(Motion::End));
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_anchor = None;
    }

    pub fn sync_selection(&mut self) {
        if let Some((start, end)) = self.editor.selection_bounds() {
            let cursor_idx = self.editor.cursor().index;
            let anchor_idx = if cursor_idx == start.index {
                end.index
            } else {
                start.index
            };
            if cursor_idx != anchor_idx {
                self.selection = Some(TextSelection {
                    start: anchor_idx,
                    end: cursor_idx,
                });
                self.selection_anchor = Some(anchor_idx);
                return;
            }
        }
        self.selection = None;
        self.selection_anchor = None;
    }

    pub fn delete_selection(&mut self, fs: &mut FontSystem) -> bool {
        let Some(sel) = self.selection.take() else {
            self.selection_anchor = None;
            return false;
        };
        if sel.is_empty() {
            self.selection_anchor = None;
            return false;
        }
        let text = self.text();
        let (a, b) = sel.normalized();
        let b = b.min(text.len());
        let new_text = format!("{}{}", &text[..a], &text[b..]);
        self.set_text(fs, &new_text);
        true
    }

    pub fn snapshot(&mut self) {
        let t = self.text();
        self.undo.push(t);
    }

    pub fn undo(&mut self, fs: &mut FontSystem) {
        if let Some(text) = self.undo.undo().map(str::to_owned) {
            self.set_text(fs, &text);
        }
    }

    pub fn redo(&mut self, fs: &mut FontSystem) {
        if let Some(text) = self.undo.redo().map(str::to_owned) {
            self.set_text(fs, &text);
        }
    }

    pub fn display_text(&self, is_password: bool) -> String {
        let text = self.text();
        if is_password {
            "•".repeat(text.chars().count())
        } else {
            text
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
            let text = self.text();
            let chars_before = text[..cursor.index].chars().count();
            cosmic_text::Cursor::new(cursor.line, chars_before * 3)
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
            let text = self.text();
            let chars_before = text[..cursor.index].chars().count();
            Cursor::new(cursor.line, chars_before * 3)
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
}
