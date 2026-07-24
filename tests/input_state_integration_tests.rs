// ============================================================
// Testes de integração — InputWidgetState (Fase 2)
// Undo/Redo, seleção, scroll, lazy init.
// ============================================================

use cosmic_text::FontSystem;
use rutter::input_limits::{InputKind, InputLimitError, InputLimits};
use rutter::input_state::{InputWidgetState, TextSelection, UndoStack};

fn fs() -> FontSystem {
    FontSystem::new()
}

fn small_limits(max_bytes: usize) -> InputLimits {
    InputLimits {
        max_bytes,
        max_graphemes: max_bytes,
        max_lines: 1,
        max_undo_bytes: 64,
        max_undo_entries: 10,
    }
}

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
    s.undo();
    s.undo();
    s.undo();

    assert_eq!(s.current(), Some("x"));
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
    s.undo(); // agora em v2
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
    let mut s = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "hello world");
    assert_eq!(s.text(), "hello world");
}

#[test]
fn set_text_preserves_newlines() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "line 1\nline 2");
    assert_eq!(s.text(), "line 1\nline 2");
}

#[test]
fn snapshot_undo_redo_full_cycle() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);

    s.snapshot(); // ""
    s.set_text(&mut fs, "step1");
    s.snapshot(); // "step1"
    s.set_text(&mut fs, "step2");
    s.snapshot(); // "step2"

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
    let mut s = InputWidgetState::new(&mut fs);
    s.snapshot();
    s.undo(&mut fs); // volta para ""
    s.undo(&mut fs); // não deve panic ou mudar estado
    assert_eq!(s.text(), "");
}

#[test]
fn redo_at_end_is_noop() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "abc");
    s.snapshot();
    s.redo(&mut fs); // noop
    assert_eq!(s.text(), "abc");
}

#[test]
fn select_all_covers_full_text() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "hello");
    s.select_all(&mut fs);
    let sel = s.selection.expect("deve ter seleção após select_all");
    assert_eq!(sel.start, 0);
    assert!(!sel.is_empty(), "seleção não deve ser vazia");
}

#[test]
fn clear_selection_removes_it() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);
    s.set_text(&mut fs, "text");
    s.select_all(&mut fs);
    assert!(s.selection.is_some());
    s.clear_selection();
    assert!(s.selection.is_none());
}

#[test]
fn select_all_on_empty_text_yields_no_selection() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);
    s.select_all(&mut fs);
    // Texto vazio: select_all retorna sem criar seleção
    assert!(s.selection.is_none());
}

#[test]
fn scroll_x_advances_when_cursor_past_visible() {
    let mut fs = fs();
    let mut s = InputWidgetState::new(&mut fs);
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
    let mut s = InputWidgetState::new(&mut fs);
    s.scroll_x = 100.0;
    let fake_cursor_x = 40.0_f32; // antes do início visível

    if fake_cursor_x < s.scroll_x {
        s.scroll_x = (fake_cursor_x - 20.0).max(0.0);
    }
    assert!((s.scroll_x - 20.0).abs() < f32::EPSILON);
}

#[test]
fn multiple_inputs_independent_state() {
    let mut fs = fs();
    let mut s1 = InputWidgetState::new(&mut fs);
    let mut s2 = InputWidgetState::new(&mut fs);

    s1.set_text(&mut fs, "user");
    s2.set_text(&mut fs, "pass");

    assert_eq!(s1.text(), "user");
    assert_eq!(s2.text(), "pass");
    assert_ne!(s1.text(), s2.text());
}

#[test]
fn text_input_defaults_reject_excess_bytes_graphemes_and_lines() {
    let mut fonts = fs();
    let limits = InputLimits::for_kind(InputKind::TextInput);
    let mut state = InputWidgetState::new_with_limits(&mut fonts, limits);
    let too_many_bytes = "a".repeat(limits.max_bytes + 1);
    let too_many_graphemes = "a".repeat(limits.max_graphemes + 1);

    assert!(matches!(
        state.try_set_text(&mut fonts, &too_many_bytes),
        Err(InputLimitError::BytesExceeded { .. })
    ));
    assert!(matches!(
        state.try_set_text(&mut fonts, &too_many_graphemes),
        Err(InputLimitError::GraphemesExceeded { .. })
    ));
    assert!(matches!(
        state.try_set_text(&mut fonts, "first\nsecond"),
        Err(InputLimitError::LinesExceeded { .. })
    ));
    assert_eq!(state.text(), "");
}

#[test]
fn text_area_accepts_its_line_limit_and_rejects_one_more_line_atomically() {
    let mut fonts = fs();
    let limits = InputLimits::for_kind(InputKind::TextArea);
    let mut state = InputWidgetState::new_with_limits(&mut fonts, limits);
    let mut accepted = "x\n".repeat(limits.max_lines - 1);
    accepted.push('x');
    let mut rejected = accepted.clone();
    rejected.push_str("\nx");

    assert!(state.try_set_text(&mut fonts, &accepted).unwrap());
    assert!(matches!(
        state.try_set_text(&mut fonts, &rejected),
        Err(InputLimitError::LinesExceeded { .. })
    ));
    assert_eq!(state.text(), accepted);
}

#[test]
fn oversized_insert_with_selection_is_atomic() {
    let mut fonts = fs();
    let mut state = InputWidgetState::new_with_limits(&mut fonts, small_limits(4));
    state.try_set_text(&mut fonts, "abcd").unwrap();
    state.selection = Some(TextSelection { start: 1, end: 3 });
    state.selection_anchor = Some(1);

    assert!(matches!(
        state.try_insert_text(&mut fonts, "xyz"),
        Err(InputLimitError::BytesExceeded { .. })
    ));
    assert_eq!(state.text(), "abcd");
    assert_eq!(state.selection, Some(TextSelection { start: 1, end: 3 }));
    assert_eq!(state.selection_anchor, Some(1));
}

#[test]
fn invalid_utf8_selection_range_returns_error_without_mutation() {
    let mut fonts = fs();
    let mut state = InputWidgetState::new_with_limits(&mut fonts, small_limits(8));
    state.try_set_text(&mut fonts, "é").unwrap();
    state.selection = Some(TextSelection { start: 1, end: 2 });
    state.selection_anchor = Some(1);

    assert!(matches!(
        state.try_delete_selection(&mut fonts),
        Err(InputLimitError::InvalidUtf8Range { .. })
    ));
    assert!(!state.delete_selection(&mut fonts));
    assert_eq!(state.text(), "é");
    assert_eq!(state.selection, Some(TextSelection { start: 1, end: 2 }));
}

#[test]
fn multiline_selection_deletes_the_flattened_document_range() {
    let mut fonts = fs();
    let mut limits = small_limits(16);
    limits.max_lines = 2;
    let mut state = InputWidgetState::new_with_limits(&mut fonts, limits);
    state.try_set_text(&mut fonts, "éx\nbeta").unwrap();
    state.selection = Some(TextSelection { start: 2, end: 6 });

    assert!(state.try_delete_selection(&mut fonts).unwrap());
    assert_eq!(state.text(), "éta");
    assert_eq!(state.cursor_byte_index(), 2);
}

#[test]
fn undo_byte_budget_evicts_old_snapshots_and_rejects_large_snapshots() {
    let mut undo = UndoStack::with_limits(10, 3);
    undo.push("a".into());
    undo.push("bb".into());
    undo.push("cc".into());

    assert_eq!(undo.current(), Some("cc"));
    assert!(undo.undo().is_none());

    undo.push("too large".into());

    assert_eq!(undo.current(), None);
    assert!(!undo.can_undo());
}

#[test]
fn limit_rejected_undo_and_redo_keep_history_position_and_text() {
    let mut fonts = fs();
    let limits = small_limits(2);
    let mut undo_state = InputWidgetState::new_with_limits(&mut fonts, limits);
    undo_state.try_set_text(&mut fonts, "ok").unwrap();
    undo_state.undo = UndoStack::with_limits(10, 64);
    undo_state.undo.push("oversized".into());
    undo_state.undo.push("ok".into());

    assert!(matches!(
        undo_state.try_undo(&mut fonts),
        Err(InputLimitError::BytesExceeded { .. })
    ));
    assert_eq!(undo_state.undo.current(), Some("ok"));
    assert!(undo_state.undo.can_undo());
    assert_eq!(undo_state.text(), "ok");

    let mut redo_state = InputWidgetState::new_with_limits(&mut fonts, limits);
    redo_state.try_set_text(&mut fonts, "ok").unwrap();
    redo_state.undo = UndoStack::with_limits(10, 64);
    redo_state.undo.push("ok".into());
    redo_state.undo.push("oversized".into());
    assert_eq!(redo_state.undo.undo(), Some("ok"));

    assert!(matches!(
        redo_state.try_redo(&mut fonts),
        Err(InputLimitError::BytesExceeded { .. })
    ));
    assert_eq!(redo_state.undo.current(), Some("ok"));
    assert!(redo_state.undo.can_redo());
    assert_eq!(redo_state.text(), "ok");
}

#[test]
fn legacy_set_text_rejects_text_larger_than_its_default_limit() {
    let mut fonts = fs();
    let mut state = InputWidgetState::new(&mut fonts);
    let limits = InputKind::TextArea.limits();
    let oversized = "x".repeat(limits.max_bytes + 1);

    state.set_text(&mut fonts, &oversized);

    assert_eq!(state.text(), "");
}

#[test]
fn application_limits_cannot_exceed_framework_hard_caps() {
    let requested = InputLimits {
        max_bytes: usize::MAX,
        max_graphemes: usize::MAX,
        max_lines: usize::MAX,
        max_undo_bytes: usize::MAX,
        max_undo_entries: usize::MAX,
    };
    let mut fonts = fs();
    let state = InputWidgetState::new_with_limits(&mut fonts, requested);

    assert_eq!(state.limits().max_bytes, 1024 * 1024);
    assert_eq!(state.limits().max_graphemes, 262_144);
    assert_eq!(state.limits().max_lines, 16_384);
    assert_eq!(state.limits().max_undo_bytes, 8 * 1024 * 1024);
    assert_eq!(state.limits().max_undo_entries, 100);
}

#[test]
fn undo_stack_discards_excess_string_capacity() {
    let mut undo = UndoStack::with_limits(2, 8);
    let mut oversized_capacity = String::with_capacity(4096);
    oversized_capacity.push('x');

    undo.push(oversized_capacity);

    assert_eq!(undo.current(), Some("x"));
    assert!(undo.retained_bytes() <= 8);
}
