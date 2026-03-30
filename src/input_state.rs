// ============================================================
// Rutter Framework — engine/input_state.rs
// ============================================================

use std::collections::VecDeque;

use cosmic_text::{Action, Attrs, Buffer, Edit, Editor, FontSystem, Metrics, Motion, Shaping};

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
    pub undo: UndoStack,
    pub selection: Option<TextSelection>,
    pub selection_anchor: Option<usize>,
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
            undo,
            selection: None,
            selection_anchor: None,
        }
    }

    pub fn text(&self) -> String {
        self.editor
            .with_buffer(|b| b.lines.iter().map(|l| l.text()).collect::<String>())
    }

    pub fn set_text(&mut self, fs: &mut FontSystem, text: &str) {
        self.editor
            .with_buffer_mut(|b| b.set_text(fs, text, &Attrs::new(), Shaping::Advanced, None));
        self.editor.action(fs, Action::Motion(Motion::End));
        self.selection = None;
        self.selection_anchor = None;
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
    ) -> f32 {
        let display = self.display_text(is_password);
        let mut buf = Buffer::new(fs, Metrics::new(font_size, font_size * 1.3));
        buf.set_size(fs, Some(10_000.0), None);
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
            if run.line_i == mapped_cursor.line {
                for glyph in run.glyphs.iter() {
                    if glyph.start >= mapped_cursor.index {
                        cx = glyph.x;
                        break;
                    }
                    cx = glyph.x + glyph.w;
                }
                break;
            }
        }
        cx
    }

    pub fn update_scroll(
        &mut self,
        fs: &mut FontSystem,
        visible_width: f32,
        font_size: f32,
        is_password: bool,
    ) {
        let cx = self.cursor_x_with_metrics(fs, font_size, is_password);
        if cx > self.scroll_x + visible_width {
            self.scroll_x = cx - visible_width + 20.0;
        } else if cx < self.scroll_x {
            self.scroll_x = (cx - 20.0).max(0.0);
        }
    }
}
