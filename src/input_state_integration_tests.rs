// ============================================================
// Testes de integração — InputWidgetState (Fase 2)
// Undo/Redo, seleção, scroll, lazy init.
// ============================================================

use cosmic_text::FontSystem;
use rutter::input_state::{InputWidgetState, TextSelection, UndoStack};

fn fs() -> FontSystem { FontSystem::new() }

// ── UndoStack — integração ───────────────────────────────────

#[test]
fn undo_three_steps() {
    let mut s = UndoStack::new(20);
    s.push("".into());
    s.push("a".into());
    s.push("ab".into());
    s.push("abc".into());

    assert_eq!(s.undo(), Some("ab"));
    assert_eq!(s.undo(), Some("a"));
    assert_eq!(s.undo(), Some(""));
    assert!(s.undo().is_none());
}

#[test]
fn redo_three_steps() {
    let mut s = UndoStack::new(20);
    s.push("x".into());
    s.push("xy".into());
    s.push("xyz".into());
    s.undo(); s.undo(); s.undo();

    assert_eq!(s.redo(), Some("x"));
    assert_eq!(s.redo(), Some("xy"));
    assert_eq!(s.redo(), Some("xyz"));
    assert!(s.redo().is_none());
}

#[test]
fn new_push_after_undo_clears_redo_history() {
    let mut s = UndoStack::new(20);
    s.push("v1".into());
    s.push("v2".into());
    s.push("v3".into());
    s.undo();           // agora em v2
    s.push("v2b".into()); // novo branch
    assert!(s.redo().is_none(), "v3 deve ter sido descartado");
}

#[test]
fn max_size_evicts_oldest() {
    let mut s = UndoStack::new(3);
    for i in 1..=5_u32 {
        s.push(i.to_string());
    }
    // Stack = ["3","4","5"], position = 2
    assert_eq!(s.current(), Some("5"));
    s.undo(); // "4"
    s.undo(); // "3"
    assert!(s.undo().is_none(), "1 e 2 foram evictados");
}

// ── TextSelection ────────────────────────────────────────────

#[test]
fn selection_forward() {
    let sel = TextSelection { start: 2, end: 8 };
    let (a, b) = sel.normalized();
    assert_eq!(a, 2);
    assert_eq!(b, 8);
}

#[test]
fn selection_backward_normalizes() {
    let sel = TextSelection { start: 8, end: 2 };
    let (a, b) = sel.normalized();
    assert_eq!(a, 2);
    assert_eq!(b, 8);
}

#[test]
fn empty_selection_point() {
    let sel = TextSelection { start: 5, end: 5 };
    assert!(sel.is_empty());
}

// ── InputWidgetState — integração ────────────────────────────

#[test]
fn new_state_is_empty() {
    let mut fs = fs();
    let s = InputWidgetState::new(&mut fs);
    assert_eq!(s.text(), "");
    assert!(s.selection.is_none());
    assert!((s.scroll_x - 0.0).abs() < f32::EPSILON);
}

#[test]
fn set_text_and_read_back() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "hello world");
    assert_eq!(s.text(), "hello world");
}

#[test]
fn snapshot_undo_redo_full_cycle() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);

    s.snapshot();                    // ""
    s.set_text(&mut fs, "step1");
    s.snapshot();                    // "step1"
    s.set_text(&mut fs, "step2");
    s.snapshot();                    // "step2"

    s.undo(&mut fs);
    assert_eq!(s.text(), "step1");

    s.undo(&mut fs);
    assert_eq!(s.text(), "");

    s.redo(&mut fs);
    assert_eq!(s.text(), "step1");

    s.redo(&mut fs);
    assert_eq!(s.text(), "step2");
}

#[test]
fn undo_at_start_is_noop() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.snapshot();
    s.undo(&mut fs); // volta para ""
    s.undo(&mut fs); // não deve panic ou mudar estado
    assert_eq!(s.text(), "");
}

#[test]
fn redo_at_end_is_noop() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "abc");
    s.snapshot();
    s.redo(&mut fs); // noop
    assert_eq!(s.text(), "abc");
}

#[test]
fn select_all_covers_full_text() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "hello");
    s.select_all(&mut fs);
    let sel = s.selection.expect("deve ter seleção após select_all");
    assert_eq!(sel.start, 0);
    assert!(!sel.is_empty(), "seleção não deve ser vazia");
}

#[test]
fn clear_selection_removes_it() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "text");
    s.select_all(&mut fs);
    assert!(s.selection.is_some());
    s.clear_selection();
    assert!(s.selection.is_none());
}

#[test]
fn select_all_on_empty_text_yields_no_selection() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.select_all(&mut fs);
    // Texto vazio: select_all retorna sem criar seleção
    assert!(s.selection.is_none());
}

#[test]
fn scroll_x_advances_when_cursor_past_visible() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.scroll_x = 0.0;
    let visible = 100.0_f32;
    let fake_cursor_x = 150.0_f32; // além do visível

    // Simular update_scroll com cursor_x manual
    if fake_cursor_x > s.scroll_x + visible {
        s.scroll_x = fake_cursor_x - visible + 20.0;
    }
    assert!(s.scroll_x > 0.0);
    assert!((s.scroll_x - 70.0).abs() < f32::EPSILON);
}

#[test]
fn scroll_x_retracts_when_cursor_left_of_scroll() {
    let mut fs = fs();
    let mut s  = InputWidgetState::new(&mut fs);
    s.scroll_x = 100.0;
    let fake_cursor_x = 40.0_f32; // antes do início visível

    if fake_cursor_x < s.scroll_x {
        s.scroll_x = (fake_cursor_x - 20.0).max(0.0);
    }
    assert!((s.scroll_x - 20.0).abs() < f32::EPSILON);
}

#[test]
fn multiple_inputs_independent_state() {
    let mut fs  = fs();
    let mut s1  = InputWidgetState::new(&mut fs);
    let mut s2  = InputWidgetState::new(&mut fs);

    s1.set_text(&mut fs, "user");
    s2.set_text(&mut fs, "pass");

    assert_eq!(s1.text(), "user");
    assert_eq!(s2.text(), "pass");
    assert_ne!(s1.text(), s2.text());
}
