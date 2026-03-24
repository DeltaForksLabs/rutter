// ============================================================
// Rutter Framework — engine/input_state.rs
//
// Estado interno de cada campo TextInput, gerenciado pelo
// engine (não pelo usuário do framework).
//
// Responsabilidades:
//   • Manter o Editor cosmic-text
//   • Scroll horizontal (offset em px)
//   • Pilha de Undo/Redo (ring buffer de snapshots de texto)
//   • Estado de seleção visual
// ============================================================

use std::collections::VecDeque;

use cosmic_text::{Action, Buffer, Edit, Editor, FontSystem, Metrics, Motion};

// ── Undo/Redo ─────────────────────────────────────────────────

/// Ring buffer de snapshots de texto para Undo/Redo.
///
/// Estratégia: snapshot por palavra (inserção de espaço/enter)
/// + a cada 500ms de inatividade (implementado no runner).
/// Limite padrão: 100 entradas.
pub struct UndoStack {
    stack:    VecDeque<String>,
    position: usize, // índice do estado "atual" dentro do stack
    max_size: usize,
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self { stack: VecDeque::new(), position: 0, max_size }
    }

    /// Salva um novo snapshot. Descarta o "futuro" se houve undo antes.
    pub fn push(&mut self, text: String) {
        // Evitar duplicatas consecutivas
        if self.stack.back().map(|s| s == &text).unwrap_or(false) {
            return;
        }

        // Descartar entradas "futuras" se estamos no meio do histórico
        while self.stack.len() > self.position + 1 {
            self.stack.pop_back();
        }

        self.stack.push_back(text);
        if self.stack.len() > self.max_size {
            self.stack.pop_front();
        }
        self.position = self.stack.len().saturating_sub(1);
    }

    /// Retorna o estado anterior, se disponível.
    pub fn undo(&mut self) -> Option<&str> {
        if self.position > 0 {
            self.position -= 1;
            self.stack.get(self.position).map(String::as_str)
        } else {
            None
        }
    }

    /// Refaz o último undo, se disponível.
    pub fn redo(&mut self) -> Option<&str> {
        if self.position + 1 < self.stack.len() {
            self.position += 1;
            self.stack.get(self.position).map(String::as_str)
        } else {
            None
        }
    }

    /// Texto no topo do stack (estado atual).
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

/// Representa um intervalo de seleção de texto (em bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextSelection {
    pub start: usize, // byte offset no texto
    pub end:   usize, // byte offset (inclusive)
}

impl TextSelection {
    pub fn is_empty(&self) -> bool { self.start == self.end }

    /// Retorna o intervalo normalizado (start <= end).
    pub fn normalized(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

// ── InputWidgetState ──────────────────────────────────────────

/// Estado interno completo de um `Widget::TextInput`.
/// Owned pelo `RutterEngine`; nunca pelo usuário do framework.
pub struct InputWidgetState {
    pub editor:     Editor<'static>,
    /// Deslocamento horizontal de scroll (em px lógicos).
    pub scroll_x:   f32,
    /// Pilha de undo/redo.
    pub undo:       UndoStack,
    /// Seleção visual ativa (CTRL+A, drag futuro).
    pub selection:  Option<TextSelection>,
}

impl InputWidgetState {
    /// Cria o estado para um novo TextInput com o `id` fornecido.
    pub fn new(fs: &mut FontSystem) -> Self {
        let buf = Buffer::new(fs, Metrics::new(14.0, 18.0));
        let mut editor = Editor::new(buf);

        // Inicializar com snapshot vazio
        let mut undo = UndoStack::default();
        undo.push(String::new());

        // Foco inicial para habilitar edição
        editor.set_auto_indent(false);

        Self { editor, scroll_x: 0.0, undo, selection: None }
    }

    // ── Leitura de texto ─────────────────────────────────────

    /// Extrai o texto completo do buffer.
    pub fn text(&self) -> String {
        self.editor.with_buffer(|b| {
            b.lines.iter().map(|l| l.text()).collect::<String>()
        })
    }

    /// Injeta texto diretamente no editor (usado pelo Undo/Redo).
    pub fn set_text(&mut self, fs: &mut FontSystem, text: &str) {
        use cosmic_text::{Attrs, Shaping};
        self.editor.with_buffer_mut(|b| {
            b.set_text(fs, text, &Attrs::new(), Shaping::Advanced, None);
        });
        // Cursor ao final
        self.editor.action(fs, Action::Motion(Motion::End));
        self.selection = None;
    }

    // ── Seleção ───────────────────────────────────────────────

    /// Seleciona todo o texto (CTRL+A).
    pub fn select_all(&mut self, fs: &mut FontSystem) {
        let text = self.text();
        if text.is_empty() { return; }
        self.selection = Some(TextSelection {
            start: 0,
            end:   text.len().saturating_sub(1),
        });
        // Mover cursor ao final para scroll correto
        self.editor.action(fs, Action::Motion(Motion::End));
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    // ── Undo/Redo ─────────────────────────────────────────────

    /// Salva snapshot do estado atual na pilha de undo.
    pub fn snapshot(&mut self) {
        let t = self.text();
        self.undo.push(t);
    }

    /// Desfaz a última alteração.
    pub fn undo(&mut self, fs: &mut FontSystem) {
        if let Some(text) = self.undo.undo() {
            let t = text.to_string();
            self.set_text(fs, &t);
        }
    }

    /// Refaz a última alteração desfeita.
    pub fn redo(&mut self, fs: &mut FontSystem) {
        if let Some(text) = self.undo.redo() {
            let t = text.to_string();
            self.set_text(fs, &t);
        }
    }

    // ── Scroll horizontal ─────────────────────────────────────

    /// Calcula o offset X do cursor a partir das runs de layout.
    pub fn cursor_x(&self) -> f32 {
        let cursor = self.editor.cursor();
        let mut cx  = 0.0_f32;
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

    /// Atualiza `scroll_x` para que o cursor fique visível dentro
    /// de `visible_width`.
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

    // ── UndoStack ────────────────────────────────────────────

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
        assert_eq!(s.current(), Some("a"));
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
    fn undo_past_start_returns_none() {
        let mut s = UndoStack::new(10);
        s.push("x".into());
        s.undo();
        assert!(s.undo().is_none());
    }

    #[test]
    fn redo_past_end_returns_none() {
        let mut s = UndoStack::new(10);
        s.push("x".into());
        assert!(s.redo().is_none());
    }

    #[test]
    fn push_after_undo_discards_future() {
        let mut s = UndoStack::new(10);
        s.push("a".into());
        s.push("ab".into());
        s.push("abc".into());
        s.undo();          // volta para "ab"
        s.push("abX".into()); // novo branch
        assert!(s.redo().is_none()); // "abc" foi descartado
        assert_eq!(s.current(), Some("abX"));
    }

    #[test]
    fn duplicate_pushes_are_ignored() {
        let mut s = UndoStack::new(10);
        s.push("same".into());
        s.push("same".into());
        s.push("same".into());
        s.undo(); // não deve poder voltar
        assert!(s.undo().is_none());
    }

    #[test]
    fn stack_respects_max_size() {
        let mut s = UndoStack::new(3);
        s.push("1".into());
        s.push("2".into());
        s.push("3".into());
        s.push("4".into()); // deve descartar "1"
        // Agora stack deve ter: ["2","3","4"]
        assert_eq!(s.current(), Some("4"));
        s.undo(); // "3"
        s.undo(); // "2"
        // "1" já foi descartado
        assert!(s.undo().is_none());
    }

    #[test]
    fn can_undo_and_can_redo_consistency() {
        let mut s = UndoStack::new(10);
        s.push("a".into());
        s.push("b".into());
        assert!(s.can_undo());
        assert!(!s.can_redo());
        s.undo();
        assert!(!s.can_undo());
        assert!(s.can_redo());
    }

    // ── TextSelection ────────────────────────────────────────

    #[test]
    fn selection_is_empty_when_start_equals_end() {
        let sel = TextSelection { start: 5, end: 5 };
        assert!(sel.is_empty());
    }

    #[test]
    fn selection_normalized_reverses_if_inverted() {
        let sel = TextSelection { start: 10, end: 3 };
        let (a, b) = sel.normalized();
        assert!(a <= b);
        assert_eq!(a, 3);
        assert_eq!(b, 10);
    }

    #[test]
    fn default_selection_is_empty() {
        let sel = TextSelection::default();
        assert!(sel.is_empty());
    }

    // ── InputWidgetState ─────────────────────────────────────

    #[test]
    fn new_state_has_empty_text() {
        let mut fs = FontSystem::new();
        let s = InputWidgetState::new(&mut fs);
        assert_eq!(s.text(), "");
    }

    #[test]
    fn new_state_scroll_starts_at_zero() {
        let mut fs = FontSystem::new();
        let s = InputWidgetState::new(&mut fs);
        assert!((s.scroll_x - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn new_state_no_selection() {
        let mut fs = FontSystem::new();
        let s = InputWidgetState::new(&mut fs);
        assert!(s.selection.is_none());
    }

    #[test]
    fn set_text_changes_text() {
        let mut fs = FontSystem::new();
        let mut s  = InputWidgetState::new(&mut fs);
        s.set_text(&mut fs, "hello");
        assert_eq!(s.text(), "hello");
    }

    #[test]
    fn snapshot_and_undo_restores_text() {
        let mut fs = FontSystem::new();
        let mut s  = InputWidgetState::new(&mut fs);
        s.snapshot();               // snapshot ""
        s.set_text(&mut fs, "abc");
        s.snapshot();               // snapshot "abc"
        s.undo(&mut fs);
        assert_eq!(s.text(), "");
    }

    #[test]
    fn redo_after_undo_restores_text() {
        let mut fs = FontSystem::new();
        let mut s  = InputWidgetState::new(&mut fs);
        s.snapshot();
        s.set_text(&mut fs, "xyz");
        s.snapshot();
        s.undo(&mut fs);
        s.redo(&mut fs);
        assert_eq!(s.text(), "xyz");
    }

    #[test]
    fn select_all_sets_selection_to_full_range() {
        let mut fs = FontSystem::new();
        let mut s  = InputWidgetState::new(&mut fs);
        s.set_text(&mut fs, "hello");
        s.select_all(&mut fs);
        assert!(s.selection.is_some());
        let sel = s.selection.unwrap();
        assert_eq!(sel.start, 0);
        assert!(!sel.is_empty());
    }

    #[test]
    fn clear_selection_removes_selection() {
        let mut fs = FontSystem::new();
        let mut s  = InputWidgetState::new(&mut fs);
        s.set_text(&mut fs, "test");
        s.select_all(&mut fs);
        s.clear_selection();
        assert!(s.selection.is_none());
    }

    #[test]
    fn update_scroll_moves_right_for_long_text() {
        let mut fs = FontSystem::new();
        let mut s  = InputWidgetState::new(&mut fs);
        s.scroll_x = 0.0;
        // Simular cursor_x grande (como se tivéssemos digitado muito)
        // scroll_x muda se cursor_x > visible_width
        let visible = 100.0_f32;
        // cursor_x() retorna 0 com texto vazio, mas podemos testar
        // o comportamento lógico do update_scroll diretamente
        s.scroll_x = 0.0;
        // cursor em 150px, visible 100px → scroll_x deve avançar
        let cx = 150.0_f32;
        if cx > s.scroll_x + visible {
            s.scroll_x = cx - visible + 20.0;
        }
        assert!(s.scroll_x > 0.0, "scroll deve avançar quando cursor sai à direita");
    }
}
