// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use cosmic_text::{Action, Attrs, Buffer, Edit, Editor, FontSystem, Metrics, Motion, Shaping};
use zeroize::Zeroize;

use crate::input_limits::{
    InputKind, InputLimitError, InputLimits, copy_text_with_reserve, replacement_candidate,
    text_metrics, validate_bytes_and_lines, validate_candidate, validate_inserted_bytes,
    validate_text, validate_utf8_range,
};

use super::{InputWidgetState, TextSelection, UndoStack, input_state_edit_helpers as helpers};

#[derive(Clone, Copy)]
enum HistoryStep {
    Undo,
    Redo,
}

impl InputWidgetState {
    /// Creates a state using the bounded TextArea profile for compatibility.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let state = InputWidgetState::new(&mut fonts);
    /// assert!(state.text().is_empty());
    /// ```
    pub fn new(font_system: &mut FontSystem) -> Self {
        Self::new_with_limits(font_system, InputKind::TextArea.limits())
    }

    /// Creates a state that enforces the supplied text and undo limits.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_limits::{InputKind, InputLimits};
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let state = InputWidgetState::new_with_limits(
    ///     &mut fonts,
    ///     InputLimits::for_kind(InputKind::TextInput),
    /// );
    /// assert_eq!(state.limits().max_lines, 1);
    /// ```
    pub fn new_with_limits(font_system: &mut FontSystem, limits: InputLimits) -> Self {
        let limits = limits.clamp_to_hard_caps();
        let buffer = Buffer::new(font_system, Metrics::new(14.0, 18.0));
        let mut editor = Editor::new(buffer);
        let mut undo = UndoStack::with_limits(limits.max_undo_entries, limits.max_undo_bytes);
        editor.set_auto_indent(false);
        undo.push(String::new());
        Self {
            editor,
            scroll_x: 0.0,
            scroll_y: 0.0,
            undo,
            selection: None,
            selection_anchor: None,
            limits,
            sensitive: false,
        }
    }

    /// Returns the limits currently enforced by this state.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let state = InputWidgetState::new(&mut fonts);
    /// assert!(state.limits().max_bytes > 0);
    /// ```
    pub fn limits(&self) -> InputLimits {
        self.limits
    }

    pub(crate) fn set_limits(&mut self, limits: InputLimits) {
        self.limits = limits.clamp_to_hard_caps();
        self.undo
            .set_limits(self.limits.max_undo_entries, self.limits.max_undo_bytes);
    }

    /// Sets text only when it fits this state's configured limits.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// state.set_text(&mut fonts, "bounded text");
    /// ```
    pub fn set_text(&mut self, font_system: &mut FontSystem, text: &str) {
        let _ = self.try_set_text(font_system, text);
    }

    /// Validates and sets text atomically.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// assert!(state.try_set_text(&mut fonts, "bounded text").unwrap());
    /// ```
    pub fn try_set_text(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
    ) -> Result<bool, InputLimitError> {
        validate_text(text, self.limits)?;
        if helpers::buffer_matches_text(&self.editor, text) {
            return Ok(false);
        }
        self.replace_buffer_text(font_system, text, None);
        Ok(true)
    }

    /// Inserts text after validating the final text before any editor mutation.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// assert!(state.try_insert_text(&mut fonts, "text").unwrap());
    /// ```
    pub fn try_insert_text(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
    ) -> Result<bool, InputLimitError> {
        if let Some(selection) = self.selection {
            return self.try_replace_selection(font_system, selection, text);
        }
        self.try_insert_at_cursor(font_system, text)
    }

    fn try_insert_at_cursor(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
    ) -> Result<bool, InputLimitError> {
        if text.is_empty() {
            return Ok(false);
        }
        let cursor_offset = helpers::cursor_flattened_offset(&self.editor, self.editor.cursor())?;
        validate_inserted_bytes(self.text_byte_len(), text.len(), self.limits)?;
        let projected =
            helpers::buffer_metrics(&self.editor).insertion_upper_bound(text_metrics(text))?;
        validate_bytes_and_lines(projected, self.limits)?;
        // Grapheme boundaries can change at insertion edges, unlike scalar counts.
        if projected.scalars > self.limits.max_graphemes {
            return self.try_replace_selection(
                font_system,
                TextSelection {
                    start: cursor_offset,
                    end: cursor_offset,
                },
                text,
            );
        }
        self.insert_validated_text(font_system, text);
        Ok(true)
    }

    fn insert_validated_text(&mut self, font_system: &mut FontSystem, text: &str) {
        self.editor.action(font_system, Action::Escape);
        self.editor.insert_string(text, None);
        self.editor.shape_as_needed(font_system, false);
        self.normalize_cursor();
        self.clear_selection();
    }

    /// Deletes the selected range only when it is a valid UTF-8 range.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::{InputWidgetState, TextSelection};
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// state.set_text(&mut fonts, "text");
    /// state.selection = Some(TextSelection { start: 0, end: 1 });
    /// assert!(state.try_delete_selection(&mut fonts).unwrap());
    /// ```
    pub fn try_delete_selection(
        &mut self,
        font_system: &mut FontSystem,
    ) -> Result<bool, InputLimitError> {
        let Some(selection) = self.selection else {
            self.selection_anchor = None;
            return Ok(false);
        };
        self.try_replace_selection(font_system, selection, "")
    }

    /// Deletes the current selection without panicking on an invalid range.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// assert!(!state.delete_selection(&mut fonts));
    /// ```
    pub fn delete_selection(&mut self, font_system: &mut FontSystem) -> bool {
        self.try_delete_selection(font_system).unwrap_or(false)
    }

    fn try_replace_selection(
        &mut self,
        font_system: &mut FontSystem,
        selection: TextSelection,
        replacement: &str,
    ) -> Result<bool, InputLimitError> {
        if replacement.len() > self.limits.max_bytes {
            return Err(InputLimitError::BytesExceeded {
                actual: replacement.len(),
                max: self.limits.max_bytes,
            });
        }
        let mut current = self.text();
        let result =
            self.try_replace_selection_in_text(font_system, &current, selection, replacement);
        self.zeroize_temporary(&mut current);
        result
    }

    fn try_replace_selection_in_text(
        &mut self,
        font_system: &mut FontSystem,
        current: &str,
        selection: TextSelection,
        replacement: &str,
    ) -> Result<bool, InputLimitError> {
        let (start, end) = selection.normalized();
        validate_utf8_range(current, start, end)?;
        let cursor_offset = helpers::replacement_cursor_offset(start, replacement)?;
        let mut candidate =
            replacement_candidate(current, start, end, replacement, self.limits.max_bytes)?;
        let result =
            self.apply_selection_candidate(font_system, current, &candidate, cursor_offset);
        self.zeroize_temporary(&mut candidate);
        result
    }

    fn apply_selection_candidate(
        &mut self,
        font_system: &mut FontSystem,
        current: &str,
        candidate: &str,
        cursor_offset: usize,
    ) -> Result<bool, InputLimitError> {
        validate_candidate(candidate, self.limits)?;
        if candidate == current {
            self.editor.action(font_system, Action::Escape);
            let _ = helpers::set_cursor_at_flattened_offset(&mut self.editor, cursor_offset);
            self.clear_selection();
            return Ok(false);
        }
        self.replace_buffer_text(font_system, candidate, Some(cursor_offset));
        Ok(true)
    }

    /// Captures the current text within the configured undo budget.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// state.snapshot();
    /// ```
    pub fn snapshot(&mut self) {
        if self.sensitive {
            self.undo.clear_secure();
            return;
        }
        self.undo.push(self.text());
    }

    /// Restores the previous snapshot only if it fits the current limits.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// assert!(!state.try_undo(&mut fonts).unwrap());
    /// ```
    pub fn try_undo(&mut self, font_system: &mut FontSystem) -> Result<bool, InputLimitError> {
        self.try_restore_history(font_system, HistoryStep::Undo)
    }

    /// Restores the next snapshot only if it fits the current limits.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// assert!(!state.try_redo(&mut fonts).unwrap());
    /// ```
    pub fn try_redo(&mut self, font_system: &mut FontSystem) -> Result<bool, InputLimitError> {
        self.try_restore_history(font_system, HistoryStep::Redo)
    }

    /// Performs undo while preserving the legacy no-result API.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// state.undo(&mut fonts);
    /// ```
    pub fn undo(&mut self, font_system: &mut FontSystem) {
        let _ = self.try_undo(font_system);
    }

    /// Performs redo while preserving the legacy no-result API.
    ///
    /// ```no_run
    /// use cosmic_text::FontSystem;
    /// use rutter::input_state::InputWidgetState;
    ///
    /// let mut fonts = FontSystem::new();
    /// let mut state = InputWidgetState::new(&mut fonts);
    /// state.redo(&mut fonts);
    /// ```
    pub fn redo(&mut self, font_system: &mut FontSystem) {
        let _ = self.try_redo(font_system);
    }

    fn try_restore_history(
        &mut self,
        font_system: &mut FontSystem,
        step: HistoryStep,
    ) -> Result<bool, InputLimitError> {
        let Some(mut snapshot) = self.history_candidate(step)? else {
            return Ok(false);
        };
        let moved = match step {
            HistoryStep::Undo => self.undo.move_undo(),
            HistoryStep::Redo => self.undo.move_redo(),
        };
        if moved {
            self.replace_buffer_text(font_system, &snapshot, None);
        }
        self.zeroize_temporary(&mut snapshot);
        Ok(moved)
    }

    fn history_candidate(&self, step: HistoryStep) -> Result<Option<String>, InputLimitError> {
        let candidate = match step {
            HistoryStep::Undo => self.undo.undo_candidate(),
            HistoryStep::Redo => self.undo.redo_candidate(),
        };
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        if candidate.len() > self.limits.max_undo_bytes {
            return Err(InputLimitError::UndoBudgetExceeded {
                actual: candidate.len(),
                max: self.limits.max_undo_bytes,
            });
        }
        validate_candidate(candidate, self.limits)?;
        copy_text_with_reserve(candidate, "undo candidate").map(Some)
    }

    fn replace_buffer_text(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
        cursor_offset: Option<usize>,
    ) {
        if self.sensitive {
            self.zeroize_editor_text();
        }
        self.editor.with_buffer_mut(|buffer| {
            buffer.set_text(font_system, text, &Attrs::new(), Shaping::Advanced, None);
        });
        self.editor.shape_as_needed(font_system, false);
        self.editor.action(font_system, Action::Motion(Motion::End));
        self.editor.action(font_system, Action::Escape);
        if let Some(offset) = cursor_offset {
            let _ = helpers::set_cursor_at_flattened_offset(&mut self.editor, offset);
        }
        self.normalize_cursor();
        self.clear_selection();
    }

    fn zeroize_temporary(&self, text: &mut String) {
        if self.sensitive {
            text.zeroize();
        }
    }
}
