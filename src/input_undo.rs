// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::VecDeque;

use zeroize::Zeroize;

use crate::input_limits::{InputKind, InputLimits};

/// Stores bounded text snapshots for undo and redo operations.
pub struct UndoStack {
    stack: VecDeque<String>,
    position: usize,
    max_entries: usize,
    max_bytes: usize,
    retained_bytes: usize,
    sensitive: bool,
}

impl UndoStack {
    /// Creates an entry-bounded stack with the safe TextInput undo-byte budget.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::new(2);
    /// undo.push("before".into());
    /// ```
    pub fn new(max_size: usize) -> Self {
        Self::with_limits(max_size, InputKind::TextInput.limits().max_undo_bytes)
    }

    /// Creates a stack bounded by both snapshot count and cumulative retained bytes.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let undo = UndoStack::with_limits(32, 128 * 1024);
    /// assert!(!undo.can_undo());
    /// ```
    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        let hard_caps = InputKind::TextArea.limits();
        Self {
            stack: VecDeque::new(),
            position: 0,
            max_entries: max_entries.min(hard_caps.max_undo_entries),
            max_bytes: max_bytes.min(hard_caps.max_undo_bytes),
            retained_bytes: 0,
            sensitive: false,
        }
    }

    pub(crate) fn set_limits(&mut self, max_entries: usize, max_bytes: usize) {
        let hard_caps = InputKind::TextArea.limits();
        self.max_entries = max_entries.min(hard_caps.max_undo_entries);
        self.max_bytes = max_bytes.min(hard_caps.max_undo_bytes);
        if self.undo_disabled() {
            self.clear_secure();
            return;
        }
        self.trim_to_limits();
    }

    pub(crate) fn set_sensitive(&mut self, sensitive: bool) {
        if self.sensitive == sensitive {
            return;
        }
        self.sensitive = sensitive;
        self.clear_secure();
    }

    pub(crate) fn clear_secure(&mut self) {
        for text in &mut self.stack {
            text.zeroize();
        }
        self.stack.clear();
        self.stack.shrink_to_fit();
        self.position = 0;
        self.retained_bytes = 0;
    }

    /// Records a snapshot, evicting oldest snapshots to stay within both budgets.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::with_limits(2, 16);
    /// undo.push("first".into());
    /// undo.push("second".into());
    /// assert_eq!(undo.current(), Some("second"));
    /// ```
    pub fn push(&mut self, text: String) {
        let Some(snapshot) = self.prepare_snapshot(text) else {
            return;
        };
        self.store_snapshot(snapshot);
    }

    fn prepare_snapshot(&mut self, mut text: String) -> Option<String> {
        if self.snapshot_is_rejected(&text) {
            self.reject_snapshot(text);
            return None;
        }
        self.discard_redo_entries();
        if self.stack.back().is_some_and(|current| current == &text) {
            text.zeroize();
            return None;
        }
        let snapshot = self.compact_snapshot(&text);
        text.zeroize();
        snapshot
    }

    fn snapshot_is_rejected(&self, text: &str) -> bool {
        self.sensitive || self.undo_disabled() || text.len() > self.max_bytes
    }

    fn reject_snapshot(&mut self, mut text: String) {
        text.zeroize();
        self.clear_secure();
    }

    fn compact_snapshot(&mut self, text: &str) -> Option<String> {
        let Some(mut snapshot) = copy_compact_snapshot(text) else {
            self.clear_secure();
            return None;
        };
        if snapshot.capacity() <= self.max_bytes {
            return Some(snapshot);
        }
        snapshot.zeroize();
        self.clear_secure();
        None
    }

    fn store_snapshot(&mut self, mut snapshot: String) {
        let Some(retained_bytes) = self.retained_bytes.checked_add(snapshot.capacity()) else {
            snapshot.zeroize();
            self.clear_secure();
            return;
        };
        self.retained_bytes = retained_bytes;
        self.stack.push_back(snapshot);
        self.position = self.stack.len().saturating_sub(1);
        self.trim_to_limits();
        self.position = self.stack.len().saturating_sub(1);
    }

    fn undo_disabled(&self) -> bool {
        self.max_entries == 0 || self.max_bytes == 0
    }

    fn discard_redo_entries(&mut self) {
        self.normalize_position();
        while self.stack.len() > self.position.saturating_add(1) {
            self.pop_back_secure();
        }
    }

    fn trim_to_limits(&mut self) {
        self.normalize_position();
        while !self.stack.is_empty()
            && (self.stack.len() > self.max_entries || self.retained_bytes > self.max_bytes)
        {
            if self.position > 0 {
                self.pop_front_secure();
                self.position -= 1;
            } else {
                self.pop_back_secure();
            }
        }
        self.normalize_position();
    }

    fn normalize_position(&mut self) {
        self.position = self.position.min(self.stack.len().saturating_sub(1));
    }

    fn pop_back_secure(&mut self) {
        if let Some(mut dropped) = self.stack.pop_back() {
            self.retained_bytes = self.retained_bytes.saturating_sub(dropped.capacity());
            dropped.zeroize();
        }
    }

    fn pop_front_secure(&mut self) {
        if let Some(mut dropped) = self.stack.pop_front() {
            self.retained_bytes = self.retained_bytes.saturating_sub(dropped.capacity());
            dropped.zeroize();
        }
    }

    /// Moves to and returns the previous snapshot, if one exists.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::new(2);
    /// undo.push("one".into());
    /// undo.push("two".into());
    /// assert_eq!(undo.undo(), Some("one"));
    /// ```
    pub fn undo(&mut self) -> Option<&str> {
        if !self.move_undo() {
            return None;
        }
        self.current()
    }

    /// Moves to and returns the next snapshot, if one exists.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::new(2);
    /// undo.push("one".into());
    /// undo.push("two".into());
    /// let _ = undo.undo();
    /// assert_eq!(undo.redo(), Some("two"));
    /// ```
    pub fn redo(&mut self) -> Option<&str> {
        if !self.move_redo() {
            return None;
        }
        self.current()
    }

    /// Returns the current snapshot without changing the history position.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::new(1);
    /// undo.push("current".into());
    /// assert_eq!(undo.current(), Some("current"));
    /// ```
    pub fn current(&self) -> Option<&str> {
        self.stack.get(self.position).map(String::as_str)
    }

    /// Returns the allocation-capacity budget retained by history snapshots.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::with_limits(2, 16);
    /// undo.push("text".into());
    /// assert!(undo.retained_bytes() <= 16);
    /// ```
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Reports whether a previous snapshot is available.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::new(2);
    /// undo.push("one".into());
    /// undo.push("two".into());
    /// assert!(undo.can_undo());
    /// ```
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    /// Reports whether a later snapshot is available.
    ///
    /// ```
    /// use rutter::input_state::UndoStack;
    ///
    /// let mut undo = UndoStack::new(2);
    /// undo.push("one".into());
    /// undo.push("two".into());
    /// let _ = undo.undo();
    /// assert!(undo.can_redo());
    /// ```
    pub fn can_redo(&self) -> bool {
        self.position.saturating_add(1) < self.stack.len()
    }

    pub(crate) fn undo_candidate(&self) -> Option<&str> {
        self.can_undo()
            .then(|| self.stack.get(self.position - 1).map(String::as_str))
            .flatten()
    }

    pub(crate) fn redo_candidate(&self) -> Option<&str> {
        self.can_redo()
            .then(|| self.stack.get(self.position + 1).map(String::as_str))
            .flatten()
    }

    pub(crate) fn move_undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.position -= 1;
        true
    }

    pub(crate) fn move_redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        self.position += 1;
        true
    }
}

impl Drop for UndoStack {
    fn drop(&mut self) {
        self.clear_secure();
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        let limits = InputLimits::for_kind(InputKind::TextInput);
        Self::with_limits(limits.max_undo_entries, limits.max_undo_bytes)
    }
}

fn copy_compact_snapshot(text: &str) -> Option<String> {
    let mut snapshot = String::new();
    snapshot.try_reserve_exact(text.len()).ok()?;
    snapshot.push_str(text);
    Some(snapshot)
}
