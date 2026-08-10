// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::time::{Duration, Instant};

use super::DropdownMenuEntry;
use super::runtime::{
    DropdownMenuEntryAccess, entry_at_path, first_focusable_index, last_focusable_index,
};

const TYPEAHEAD_TIMEOUT: Duration = Duration::from_millis(700);

/// Retained keyboard, submenu, scrolling, and typeahead state for one menu.
#[derive(Debug, Clone, PartialEq)]
pub struct DropdownMenuState {
    is_open: bool,
    active_path: Option<Vec<usize>>,
    open_submenu_path: Vec<usize>,
    scroll_offsets: Vec<f32>,
    reveal_active: bool,
    typeahead_buffer: String,
    typeahead_timestamp: Option<Instant>,
}

impl Default for DropdownMenuState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_path: None,
            open_submenu_path: Vec::new(),
            scroll_offsets: Vec::new(),
            reveal_active: false,
            typeahead_buffer: String::new(),
            typeahead_timestamp: None,
        }
    }
}

impl DropdownMenuState {
    /// Reports whether menu surfaces should be visible.
    pub const fn is_open(&self) -> bool {
        self.is_open
    }

    /// Returns the focused entry path from the root level.
    pub fn active_path(&self) -> Option<&[usize]> {
        self.active_path.as_deref()
    }

    /// Returns the path of submenu entries whose surfaces are open.
    pub fn open_submenu_path(&self) -> &[usize] {
        &self.open_submenu_path
    }

    /// Returns one level's non-negative retained scroll offset.
    pub fn scroll_offset(&self, level: usize) -> f32 {
        self.scroll_offsets.get(level).copied().unwrap_or(0.0)
    }

    pub(crate) const fn should_reveal_active(&self) -> bool {
        self.reveal_active
    }

    /// Returns the currently accumulated typeahead prefix.
    pub fn typeahead_buffer(&self) -> &str {
        &self.typeahead_buffer
    }

    /// Opens the root level and focuses its first non-separator entry.
    pub fn open_at_first<Msg>(&mut self, entries: &[DropdownMenuEntry<'_, Msg>]) {
        self.open_with_index(first_focusable_index(entries));
    }

    /// Opens the root level and focuses its last non-separator entry.
    pub fn open_at_last<Msg>(&mut self, entries: &[DropdownMenuEntry<'_, Msg>]) {
        self.open_with_index(last_focusable_index(entries));
    }

    pub(crate) fn open_at_index(&mut self, index: Option<usize>) {
        self.open_with_index(index);
    }

    /// Closes surfaces and clears transient focus, submenu, and typeahead state.
    pub fn close(&mut self) {
        self.is_open = false;
        self.active_path = None;
        self.open_submenu_path.clear();
        self.reveal_active = false;
        self.clear_typeahead();
    }

    /// Restores closed state and discards retained scroll offsets.
    pub fn reset(&mut self) {
        self.close();
        self.scroll_offsets.clear();
    }

    /// Focuses an entry path and closes submenus outside that path.
    pub fn activate_path(&mut self, path: Vec<usize>) {
        self.is_open = true;
        self.retain_open_ancestors(&path);
        self.active_path = Some(path);
        self.reveal_active = true;
    }

    /// Opens an enabled submenu and focuses its first focusable child.
    pub fn open_submenu<Msg>(
        &mut self,
        entries: &[DropdownMenuEntry<'_, Msg>],
        path: Vec<usize>,
    ) -> bool {
        let Some(submenu) = entry_at_path(entries, &path) else {
            return false;
        };
        let Some(children) = enabled_children(submenu) else {
            return false;
        };
        self.open_submenu_level(path, first_focusable_index(children));
        true
    }

    /// Closes the deepest submenu and focuses its parent submenu entry.
    pub fn collapse_submenu(&mut self) -> bool {
        if self.open_submenu_path.is_empty() {
            return false;
        }
        self.active_path = Some(self.open_submenu_path.clone());
        self.open_submenu_path.pop();
        true
    }

    pub(crate) fn collapse_to_submenu(&mut self, path: Vec<usize>) -> bool {
        if !self.open_submenu_path.starts_with(&path) {
            return false;
        }
        self.open_submenu_path
            .truncate(path.len().saturating_sub(1));
        self.active_path = Some(path);
        self.reveal_active = true;
        true
    }

    pub(crate) fn expand_submenu(&mut self, path: Vec<usize>, child: Option<usize>) {
        self.open_submenu_level(path, child);
    }

    /// Applies a pixel delta and clamps the result to one level's range.
    pub fn scroll_level(&mut self, level: usize, delta: f32, max_scroll: f32) -> f32 {
        let current = self.scroll_offset(level);
        let target = if delta.is_finite() {
            current + delta
        } else {
            current
        };
        self.reveal_active = false;
        self.set_scroll_offset(level, target, max_scroll)
    }

    /// Clamps one retained level after its effective geometry changes.
    pub fn clamp_scroll_level(&mut self, level: usize, max_scroll: f32) -> f32 {
        let current = self.scroll_offset(level);
        self.set_scroll_offset(level, current, max_scroll)
    }

    pub(crate) fn set_scroll_level(&mut self, level: usize, value: f32, max_scroll: f32) -> f32 {
        self.reveal_active = false;
        self.set_scroll_offset(level, value, max_scroll)
    }

    pub(crate) fn collapse_descendants_for_scroll(&mut self, level: usize) {
        if self.open_submenu_path.len() <= level {
            return;
        }
        self.active_path = Some(self.open_submenu_path[..=level].to_vec());
        self.open_submenu_path.truncate(level);
        self.reveal_active = false;
    }

    pub(crate) fn scroll_level_with_descendant_collapse(
        &mut self,
        level: usize,
        visible_scroll: f32,
        value: f32,
        max_scroll: f32,
    ) -> bool {
        let after = self.set_scroll_level(level, value, max_scroll);
        if (visible_scroll - after).abs() <= f32::EPSILON {
            return false;
        }
        self.collapse_descendants_for_scroll(level);
        true
    }

    /// Appends typeahead text, replacing stale text after roughly 700 ms.
    pub fn update_typeahead(&mut self, input: &str, timestamp: Instant) -> &str {
        if self.typeahead_expired(timestamp) {
            self.typeahead_buffer.clear();
        }
        self.typeahead_buffer.push_str(input);
        self.typeahead_timestamp = Some(timestamp);
        &self.typeahead_buffer
    }

    /// Clears accumulated typeahead text and its timeout timestamp.
    pub fn clear_typeahead(&mut self) {
        self.typeahead_buffer.clear();
        self.typeahead_timestamp = None;
    }

    fn open_with_index(&mut self, index: Option<usize>) {
        self.is_open = true;
        self.active_path = index.map(|value| vec![value]);
        self.open_submenu_path.clear();
        self.reveal_active = true;
        self.clear_typeahead();
    }

    fn retain_open_ancestors(&mut self, active_path: &[usize]) {
        let common = self
            .open_submenu_path
            .iter()
            .zip(active_path)
            .take_while(|(open, active)| open == active)
            .count();
        self.open_submenu_path.truncate(common);
    }

    fn open_submenu_level(&mut self, path: Vec<usize>, child: Option<usize>) {
        let shared_depth = shared_prefix_length(&self.open_submenu_path, &path);
        self.scroll_offsets.truncate(shared_depth.saturating_add(1));
        self.is_open = true;
        self.open_submenu_path = path.clone();
        let mut active_path = path;
        if let Some(index) = child {
            active_path.push(index);
        }
        self.active_path = Some(active_path);
        self.reveal_active = true;
    }

    fn set_scroll_offset(&mut self, level: usize, value: f32, maximum: f32) -> f32 {
        let maximum = finite_nonnegative(maximum);
        let value = finite_nonnegative(value).min(maximum);
        self.scroll_offsets.resize(level.saturating_add(1), 0.0);
        self.scroll_offsets[level] = value;
        value
    }

    fn typeahead_expired(&self, timestamp: Instant) -> bool {
        self.typeahead_timestamp.is_some_and(|previous| {
            timestamp
                .checked_duration_since(previous)
                .is_some_and(|elapsed| elapsed >= TYPEAHEAD_TIMEOUT)
        })
    }
}

fn enabled_children<Entry: DropdownMenuEntryAccess>(entry: &Entry) -> Option<&[Entry]> {
    if !entry.entry_is_activatable() {
        return None;
    }
    entry.child_entries()
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        return value.max(0.0);
    }
    0.0
}

fn shared_prefix_length(left: &[usize], right: &[usize]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<DropdownMenuEntry<'static, u8>> {
        vec![
            DropdownMenuEntry::separator(),
            DropdownMenuEntry::disabled_item("Disabled"),
            DropdownMenuEntry::submenu(
                "More",
                vec![
                    DropdownMenuEntry::separator(),
                    DropdownMenuEntry::item("Child", 1),
                ],
            ),
            DropdownMenuEntry::disabled_submenu("Locked", Vec::new()),
        ]
    }

    #[test]
    fn open_first_last_and_close_manage_focus() {
        let mut state = DropdownMenuState::default();
        state.open_at_first(&entries());
        assert!(state.is_open());
        assert_eq!(state.active_path(), Some([1].as_slice()));
        state.open_at_last(&entries());
        assert_eq!(state.active_path(), Some([3].as_slice()));
        state.close();
        assert!(!state.is_open());
        assert_eq!(state.active_path(), None);
    }

    #[test]
    fn submenu_open_and_collapse_focus_child_then_parent() {
        let mut state = DropdownMenuState::default();
        assert!(state.open_submenu(&entries(), vec![2]));
        assert_eq!(state.open_submenu_path(), [2]);
        assert_eq!(state.active_path(), Some([2, 1].as_slice()));
        assert!(state.collapse_submenu());
        assert_eq!(state.active_path(), Some([2].as_slice()));
        assert!(state.open_submenu_path().is_empty());
        assert!(!state.collapse_submenu());
    }

    #[test]
    fn disabled_or_invalid_submenus_do_not_open() {
        let mut state = DropdownMenuState::default();
        assert!(!state.open_submenu(&entries(), vec![3]));
        assert!(!state.open_submenu(&entries(), vec![9]));
        assert!(!state.is_open());
    }

    #[test]
    fn activating_an_unrelated_path_closes_open_submenus() {
        let mut state = DropdownMenuState::default();
        assert!(state.open_submenu(&entries(), vec![2]));
        state.activate_path(vec![1]);
        assert_eq!(state.active_path(), Some([1].as_slice()));
        assert!(state.open_submenu_path().is_empty());
    }

    #[test]
    fn scrolling_clamps_each_level_and_reset_discards_offsets() {
        let mut state = DropdownMenuState::default();
        assert_eq!(state.scroll_level(2, 80.0, 50.0), 50.0);
        assert_eq!(state.scroll_offset(2), 50.0);
        assert_eq!(state.scroll_level(2, -70.0, 50.0), 0.0);
        assert_eq!(state.scroll_level(0, f32::NAN, 20.0), 0.0);
        assert_eq!(state.clamp_scroll_level(2, f32::NAN), 0.0);
        assert_eq!(state.set_scroll_level(1, 12.0, 20.0), 12.0);
        state.reset();
        assert_eq!(state.scroll_offset(2), 0.0);
    }

    #[test]
    fn typeahead_accumulates_then_expires_at_timeout() {
        let mut state = DropdownMenuState::default();
        let start = Instant::now();
        assert_eq!(state.update_typeahead("a", start), "a");
        assert_eq!(
            state.update_typeahead("b", start + Duration::from_millis(699)),
            "ab"
        );
        assert_eq!(
            state.update_typeahead("c", start + Duration::from_millis(1_399)),
            "c"
        );
        state.clear_typeahead();
        assert_eq!(state.typeahead_buffer(), "");
    }

    #[test]
    fn collapsing_specific_submenu_retains_its_parent_chain() {
        let mut state = DropdownMenuState::default();
        state.expand_submenu(vec![2, 1], Some(0));

        assert!(state.collapse_to_submenu(vec![2, 1]));
        assert_eq!(state.open_submenu_path(), [2]);
        assert_eq!(state.active_path(), Some([2, 1].as_slice()));
        assert!(!state.collapse_to_submenu(vec![9]));
    }

    #[test]
    fn changing_submenu_branch_resets_only_deeper_scroll_levels() {
        let mut state = DropdownMenuState::default();
        state.set_scroll_level(0, 20.0, 100.0);
        state.expand_submenu(vec![2], None);
        state.set_scroll_level(1, 80.0, 100.0);

        state.expand_submenu(vec![3], None);

        assert_eq!(state.scroll_offset(0), 20.0);
        assert_eq!(state.scroll_offset(1), 0.0);
    }

    #[test]
    fn scrolling_parent_level_collapses_detached_descendants() {
        let mut state = DropdownMenuState::default();
        state.expand_submenu(vec![2, 1], Some(0));

        state.collapse_descendants_for_scroll(0);

        assert!(state.open_submenu_path().is_empty());
        assert_eq!(state.active_path(), Some([2].as_slice()));
    }

    #[test]
    fn no_op_parent_scroll_preserves_open_descendants() {
        let mut state = DropdownMenuState::default();
        state.expand_submenu(vec![2], Some(0));

        let changed = state.scroll_level_with_descendant_collapse(0, 0.0, 0.0, 40.0);

        assert!(!changed);
        assert_eq!(state.open_submenu_path(), [2]);
    }

    #[test]
    fn reveal_offset_no_op_scroll_preserves_open_descendants() {
        let mut state = DropdownMenuState::default();
        state.expand_submenu(vec![2], Some(0));
        state.set_scroll_level(0, 20.0, 40.0);

        let changed = state.scroll_level_with_descendant_collapse(0, 0.0, 0.0, 40.0);

        assert!(!changed);
        assert_eq!(state.open_submenu_path(), [2]);
    }
}
