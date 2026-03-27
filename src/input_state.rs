// ============================================================
// Rutter Framework — engine/input_state.rs
//
// FIXES v6.2:
//   FIX-1  cursor_byte_index() exposto para draw_text_input usar
//          a mesma fonte/buffer de renderização (elimina desencontro
//          de métricas 14px vs 16px que travava o cursor no início).
//   FIX-3b selection_anchor adicionado para suporte a Shift+Seta.
//   FIX-3a select_all() não faz mais set_text no clipboard
//          (era a causa do duplo-copy junto com do_copy()).
// ============================================================

use std::collections::VecDeque;

use cosmic_text::{Action, Buffer, Edit, Editor, FontSystem, Metrics, Motion};

// ── Undo/Redo ─────────────────────────────────────────────────

pub struct UndoStack {
    stack:    VecDeque<String>,
    position: usize,
    max_size: usize,
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self { stack: VecDeque::new(), position: 0, max_size }
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

    pub fn can_undo(&self) -> bool { self.position > 0 }
    pub fn can_redo(&self) -> bool { self.position + 1 < self.stack.len() }
}

impl Default for UndoStack {
    fn default() -> Self { Self::new(100) }
}

// ── Seleção ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextSelection {
    pub start: usize,
    pub end:   usize,
}

impl TextSelection {
    pub fn is_empty(&self) -> bool { self.start == self.end }

    pub fn normalized(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

// ── InputWidgetState ──────────────────────────────────────────

pub struct InputWidgetState {
    pub editor:     Editor<'static>,
    pub scroll_x:   f32,
    pub undo:       UndoStack,
    pub selection:  Option<TextSelection>,
    /// Âncora de seleção: byte-index fixo ao iniciar Shift+Seta.
    /// FIX-3b: necessário para selecionar texto com teclado.
    pub selection_anchor: Option<usize>,
}

impl InputWidgetState {
    pub fn new(fs: &mut FontSystem) -> Self {
        // NOTA: métricas do editor interno (usado apenas para edição
        // via cosmic-text). A posição visual do cursor é calculada em
        // draw_text_input usando o buffer de renderização com font_body
        // correto — veja FIX-1 em render/mod.rs.
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

    // ── Leitura de texto ─────────────────────────────────────

    pub fn text(&self) -> String {
        self.editor.with_buffer(|b| {
            b.lines.iter().map(|l| l.text()).collect::<String>()
        })
    }

    pub fn set_text(&mut self, fs: &mut FontSystem, text: &str) {
        use cosmic_text::{Attrs, Shaping};
        self.editor.with_buffer_mut(|b| {
            b.set_text(fs, text, &Attrs::new(), Shaping::Advanced, None);
        });
        self.editor.action(fs, Action::Motion(Motion::End));
        self.selection = None;
        self.selection_anchor = None;
    }

    // ── FIX-1: byte-index do cursor para render/mod.rs ───────

    /// Retorna o byte-index atual do cursor no texto.
    ///
    /// Usado por `draw_text_input` para calcular a posição visual X
    /// do cursor a partir do buffer de renderização (que usa as
    /// métricas corretas `theme.font_body`), eliminando o bug de
    /// cursor fixo no início causado pela diferença 14px vs 16px.
    pub fn cursor_byte_index(&self) -> usize {
        self.editor.cursor().index
    }

    // ── Seleção ───────────────────────────────────────────────

    /// Seleciona todo o texto (CTRL+A).
    /// FIX-3a: removida a escrita no clipboard daqui.
    /// Quem deve escrever no clipboard é apenas do_copy().
    pub fn select_all(&mut self, fs: &mut FontSystem) {
        let text = self.text();
        if text.is_empty() { return; }
        self.selection = Some(TextSelection {
            start: 0,
            end:   text.len(),
        });
        self.selection_anchor = Some(0);
        self.editor.action(fs, Action::Motion(Motion::End));
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_anchor = None;
    }

    // ── FIX-3b: deletar texto selecionado ────────────────────

    /// Deleta o intervalo selecionado, se houver.
    /// Retorna true se algo foi deletado (para o caller saber que
    /// não precisa processar o Backspace/Delete normal).
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

    // ── Undo/Redo ─────────────────────────────────────────────

    pub fn snapshot(&mut self) {
        let t = self.text();
        self.undo.push(t);
    }

    pub fn undo(&mut self, fs: &mut FontSystem) {
        if let Some(text) = self.undo.undo() {
            let t = text.to_string();
            self.set_text(fs, &t);
        }
    }

    pub fn redo(&mut self, fs: &mut FontSystem) {
        if let Some(text) = self.undo.redo() {
            let t = text.to_string();
            self.set_text(fs, &t);
        }
    }

    // ── Scroll horizontal ─────────────────────────────────────

    /// Calcula cursor_x a partir do buffer do editor interno.
    /// ⚠ Usa as métricas 14px — SOMENTE para atualizar scroll_x.
    /// Para renderizar o cursor visual use cursor_byte_index() +
    /// o buffer de renderização em draw_text_input (FIX-1).
    pub fn cursor_x(&self) -> f32 {
        let cursor = self.editor.cursor();
        let mut cx = 0.0_f32;
        self.editor.with_buffer(|buf| {
            'outer: for run in buf.layout_runs() {
                if run.line_i == cursor.line {
                    for glyph in run.glyphs.iter() {
                        if glyph.start >= cursor.index {
                            cx = glyph.x;
                            break 'outer;
                        }
                        cx = glyph.x + glyph.w;
                    }
                    break;
                }
            }
        });
        cx
    }

    pub fn update_scroll(&mut self, visible_width: f32) {
        let cx = self.cursor_x();
        if cx > self.scroll_x + visible_width {
            self.scroll_x = cx - visible_width + 20.0;
        } else if cx < self.scroll_x {
            self.scroll_x = (cx - 20.0).max(0.0);
        }
    }
}

// ── Testes unitários ─────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_stack_starts_empty() {
        let s = UndoStack::new(10);
        assert!(s.current().is_none());
        assert!(!s.can_undo());
        assert!(!s.can_redo());
    }

    #[test]
    fn push_stores_first_entry() {
        let mut s = UndoStack::new(10);
        s.push("hello".into());
        assert_eq!(s.current(), Some("hello"));
    }

    #[test]
    fn undo_returns_previous_state() {
        let mut s = UndoStack::new(10);
        s.push("a".into());
        s.push("ab".into());
        assert_eq!(s.undo(), Some("a"));
    }

    #[test]
    fn redo_after_undo() {
        let mut s = UndoStack::new(10);
        s.push("a".into());
        s.push("ab".into());
        s.undo();
        assert_eq!(s.redo(), Some("ab"));
    }

    #[test]
    fn push_after_undo_discards_future() {
        let mut s = UndoStack::new(10);
        s.push("a".into());
        s.push("ab".into());
        s.push("abc".into());
        s.undo();
        s.push("abX".into());
        assert!(s.redo().is_none());
        assert_eq!(s.current(), Some("abX"));
    }

    #[test]
    fn stack_respects_max_size() {
        let mut s = UndoStack::new(3);
        s.push("1".into());
        s.push("2".into());
        s.push("3".into());
        s.push("4".into());
        assert_eq!(s.current(), Some("4"));
        s.undo();
        s.undo();
        assert!(s.undo().is_none());
    }

    #[test]
    fn selection_normalized_reverses_if_inverted() {
        let sel = TextSelection { start: 10, end: 3 };
        let (a, b) = sel.normalized();
        assert_eq!(a, 3);
        assert_eq!(b, 10);
    }

    #[test]
    fn new_state_has_empty_text() {
        let mut fs = FontSystem::new();
        let s = InputWidgetState::new(&mut fs);
        assert_eq!(s.text(), "");
    }

    #[test]
    fn new_state_no_selection_anchor() {
        let mut fs = FontSystem::new();
        let s = InputWidgetState::new(&mut fs);
        assert!(s.selection.is_none());
        assert!(s.selection_anchor.is_none());
    }

    #[test]
    fn cursor_byte_index_starts_at_zero() {
        let mut fs = FontSystem::new();
        let s = InputWidgetState::new(&mut fs);
        assert_eq!(s.cursor_byte_index(), 0);
    }

    #[test]
    fn delete_selection_removes_range() {
        let mut fs = FontSystem::new();
        let mut s = InputWidgetState::new(&mut fs);
        s.set_text(&mut fs, "hello world");
        s.selection = Some(TextSelection { start: 0, end: 5 });
        let deleted = s.delete_selection(&mut fs);
        assert!(deleted);
        assert_eq!(s.text(), " world");
        assert!(s.selection.is_none());
    }

    #[test]
    fn delete_selection_returns_false_when_empty() {
        let mut fs = FontSystem::new();
        let mut s = InputWidgetState::new(&mut fs);
        s.set_text(&mut fs, "hello");
        let deleted = s.delete_selection(&mut fs);
        assert!(!deleted);
        assert_eq!(s.text(), "hello");
    }

    #[test]
    fn select_all_does_not_write_clipboard() {
        // Garantir que select_all() só atualiza selection,
        // sem side effects de clipboard (o clipboard fica com do_copy()).
        let mut fs = FontSystem::new();
        let mut s = InputWidgetState::new(&mut fs);
        s.set_text(&mut fs, "abc");
        s.select_all(&mut fs);
        assert!(s.selection.is_some());
        let sel = s.selection.unwrap();
        assert_eq!(sel.start, 0);
        assert_eq!(sel.end, 3);
    }

    #[test]
    fn snapshot_and_undo_restores_text() {
        let mut fs = FontSystem::new();
        let mut s = InputWidgetState::new(&mut fs);
        s.snapshot();
        s.set_text(&mut fs, "abc");
        s.snapshot();
        s.undo(&mut fs);
        assert_eq!(s.text(), "");
    }
}
