// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Pure keyboard-navigation rules for search suggestions, extracted so they
//! stay unit-testable without a window or engine.

use winit::keyboard::{Key, NamedKey};

use super::RutterRunner;
use crate::app::AppLogic;
use crate::engine::widget_state::WidgetState;
use crate::widget::resolve_search_suggestion_id;
use crate::widgets::search::SearchMatch;

/// What the runner should do with a key pressed while a suggestions popup is
/// open on the focused search bar.
///
/// Escape is intentionally absent: the global key handler consumes it before
/// text inputs see it, so dismissal happens there via
/// [`RutterRunner::dismiss_focused_search_suggestions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchKeyOutcome {
    /// Highlight the given row position.
    MoveHover(usize),
    /// Activate the highlighted row (Enter with an active hover).
    Select(usize),
}

/// Maps a navigation key to its outcome against `result_count` rows.
///
/// Enter only selects when a row is hovered; otherwise it returns `None` so
/// the input keeps its normal submit behavior. `page_step` is the number of
/// rows jumped by PageUp/PageDown (the configured `max_results`).
pub(crate) fn search_key_outcome(
    key: &Key,
    result_count: usize,
    current_hover: Option<usize>,
    page_step: usize,
) -> Option<SearchKeyOutcome> {
    if result_count == 0 {
        return None;
    }
    let last = result_count - 1;
    let current_hover = current_hover.filter(|hover| *hover <= last);
    let outcome = match key {
        Key::Named(NamedKey::ArrowDown) => SearchKeyOutcome::MoveHover(
            current_hover.map_or(0, |hover| hover.saturating_add(1).min(last)),
        ),
        Key::Named(NamedKey::ArrowUp) => {
            SearchKeyOutcome::MoveHover(current_hover.map_or(last, |hover| hover.saturating_sub(1)))
        }
        Key::Named(NamedKey::Home) => SearchKeyOutcome::MoveHover(0),
        Key::Named(NamedKey::End) => SearchKeyOutcome::MoveHover(last),
        Key::Named(NamedKey::PageDown) => SearchKeyOutcome::MoveHover(
            current_hover.map_or(0, |hover| hover.saturating_add(page_step.max(1)).min(last)),
        ),
        Key::Named(NamedKey::PageUp) => SearchKeyOutcome::MoveHover(
            current_hover.map_or(0, |hover| hover.saturating_sub(page_step.max(1))),
        ),
        Key::Named(NamedKey::Enter) => current_hover
            .map(|hover| hover.min(last))
            .map(SearchKeyOutcome::Select)?,
        _ => return None,
    };
    Some(outcome)
}

/// Next hovered row for a wheel tick over an open suggestions popup.
///
/// Positive deltas scroll toward later results, matching Select and virtual
/// collections. `None` means the popup has no row change; the runner still
/// keeps the wheel event from leaking to covered page content.
pub(crate) fn wheel_search_hover(
    current_hover: Option<usize>,
    result_count: usize,
    delta_y: f32,
) -> Option<usize> {
    if result_count == 0 || delta_y.abs() <= f32::EPSILON {
        return None;
    }
    let last = result_count - 1;
    let current = current_hover.map(|hover| hover.min(last));
    let next = match current {
        None if delta_y > 0.0 => 0,
        None => last,
        Some(hover) if delta_y > 0.0 => hover.saturating_add(1).min(last),
        Some(hover) => hover.saturating_sub(1),
    };
    (current != Some(next)).then_some(next)
}

fn original_search_item_index(results: &[SearchMatch], row: usize) -> Option<usize> {
    results.get(row).map(|result| result.index)
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn is_search_accessibility_control(&self, target: u64) -> bool {
        self.engine.runtime_caches.searches.contains_key(&target)
    }

    pub(super) fn focus_search_accessibility_target(&mut self, target: u64) -> bool {
        let Some((search_id, row, _)) = self.search_accessibility_target(target) else {
            return false;
        };
        self.move_search_hover(search_id, row)
    }

    pub(super) fn click_search_accessibility_target(&mut self, target: u64) -> bool {
        let Some((search_id, row, item_index)) = self.search_accessibility_target(target) else {
            return false;
        };
        self.move_search_hover(search_id, row);
        self.activate_search_original_index(search_id, item_index)
    }

    pub(super) fn expand_search_accessibility_target(&mut self, target: u64) -> bool {
        if !self.is_search_accessibility_control(target) {
            return false;
        }
        let has_query = self
            .engine
            .input_states
            .get(&target)
            .is_some_and(|state| !state.text().trim().is_empty());
        if !has_query {
            return false;
        }
        self.focus_widget(Some(target));
        let Some(state) = self.search_state_mut(target) else {
            return false;
        };
        state.dismissed = false;
        state.hovered_option = None;
        self.engine.layout_dirty = true;
        true
    }

    pub(super) fn collapse_search_accessibility_target(&mut self, target: u64) -> bool {
        if !self.search_popup_is_open(target) {
            return false;
        }
        self.dismiss_search_popup(target)
    }

    pub(super) fn handle_search_input_key(&mut self, id: u64, key: &Key) -> bool {
        if !self.search_popup_is_open(id) {
            return false;
        }
        let Some(runtime) = self.engine.runtime_caches.searches.get(&id).cloned() else {
            return false;
        };
        let hover = self.search_hover(id);
        let Some(outcome) =
            search_key_outcome(key, runtime.results.len(), hover, runtime.max_results)
        else {
            return false;
        };
        let handled = match outcome {
            SearchKeyOutcome::MoveHover(row) => self.move_search_hover(id, row),
            SearchKeyOutcome::Select(row) => self.activate_search_result(id, row),
        };
        if handled {
            self.redraw();
        }
        handled
    }

    pub(super) fn dismiss_focused_search_suggestions(&mut self) -> bool {
        let Some(id) = self.engine.focused_widget_id else {
            return false;
        };
        if !self.search_popup_is_open(id) {
            return false;
        }
        self.dismiss_search_popup(id)
    }

    pub(super) fn refresh_search_suggestions_after_edit(&mut self, id: u64) {
        let query = self
            .engine
            .input_states
            .get(&id)
            .map(|state| state.text())
            .unwrap_or_default();
        if let Some(runtime) = self.engine.runtime_caches.searches.get_mut(&id) {
            runtime.refresh_results(&query);
        }
        if let Some(state) = self.search_state_mut(id) {
            state.dismissed = false;
            state.hovered_option = None;
        }
    }

    pub(super) fn scroll_open_search(&mut self, id: u64, delta_y: f32) -> bool {
        if !self.search_popup_is_open(id) {
            return false;
        }
        let Some(runtime) = self.engine.runtime_caches.searches.get(&id) else {
            return false;
        };
        let Some(next) = wheel_search_hover(self.search_hover(id), runtime.results.len(), delta_y)
        else {
            return false;
        };
        self.move_search_hover(id, next)
    }

    fn activate_search_result(&mut self, id: u64, row: usize) -> bool {
        let Some(runtime) = self.engine.runtime_caches.searches.get(&id).cloned() else {
            return false;
        };
        let Some(original_index) = original_search_item_index(&runtime.results, row) else {
            return false;
        };
        self.activate_search_original_index(id, original_index)
    }

    pub(super) fn activate_search_original_index(&mut self, id: u64, index: usize) -> bool {
        let Some(on_select) = self
            .engine
            .runtime_caches
            .searches
            .get(&id)
            .and_then(|runtime| runtime.on_select)
        else {
            return false;
        };
        self.dismiss_search_popup(id);
        A::update(
            &mut self.engine.app_state,
            on_select(index),
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
        true
    }

    fn dismiss_search_popup(&mut self, id: u64) -> bool {
        let Some(state) = self.search_state_mut(id) else {
            return false;
        };
        state.dismissed = true;
        state.hovered_option = None;
        self.engine.layout_dirty = true;
        true
    }

    fn move_search_hover(&mut self, id: u64, row: usize) -> bool {
        let Some(state) = self.search_state_mut(id) else {
            return false;
        };
        state.hovered_option = Some(row);
        true
    }

    fn search_popup_is_open(&self, id: u64) -> bool {
        self.engine.focused_widget_id == Some(id)
            && self.engine.runtime_caches.searches.contains_key(&id)
            && self
                .engine
                .input_states
                .get(&id)
                .is_some_and(|state| !state.text().trim().is_empty())
            && self
                .engine
                .widget_states
                .get(&id)
                .and_then(WidgetState::as_search)
                .is_some_and(|state| !state.dismissed)
    }

    fn search_hover(&self, id: u64) -> Option<usize> {
        self.engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_search)
            .and_then(|state| state.hovered_option)
    }

    fn search_accessibility_target(&self, target: u64) -> Option<(u64, usize, usize)> {
        if !self
            .engine
            .runtime_caches
            .visible_search_suggestion_ids
            .contains(&target)
        {
            return None;
        }
        self.engine
            .runtime_caches
            .searches
            .iter()
            .filter(|(search_id, _)| self.search_popup_is_open(**search_id))
            .find_map(|(&search_id, runtime)| {
                runtime
                    .results
                    .iter()
                    .enumerate()
                    .find_map(|(row, matched)| {
                        (resolve_search_suggestion_id(search_id, matched.index) == target)
                            .then_some((search_id, row, matched.index))
                    })
            })
    }

    fn search_state_mut(
        &mut self,
        id: u64,
    ) -> Option<&mut crate::engine::widget_state::SearchState> {
        self.engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_search_mut)
    }
}

#[cfg(test)]
mod tests {
    use super::SearchKeyOutcome::*;
    use super::{original_search_item_index, search_key_outcome};
    use crate::widgets::search::SearchMatch;
    use winit::keyboard::{Key, NamedKey};

    fn key(named: NamedKey) -> Key {
        Key::Named(named)
    }

    #[test]
    fn arrows_start_from_the_edges_when_nothing_is_hovered() {
        assert_eq!(
            search_key_outcome(&key(NamedKey::ArrowDown), 3, None, 5),
            Some(MoveHover(0))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::ArrowUp), 3, None, 5),
            Some(MoveHover(2))
        );
    }

    #[test]
    fn movement_clamps_at_both_ends() {
        assert_eq!(
            search_key_outcome(&key(NamedKey::ArrowDown), 3, Some(2), 5),
            Some(MoveHover(2))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::ArrowUp), 3, Some(0), 5),
            Some(MoveHover(0))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::End), 3, None, 5),
            Some(MoveHover(2))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::Home), 3, Some(2), 5),
            Some(MoveHover(0))
        );
    }

    #[test]
    fn pages_jump_by_the_configured_window() {
        assert_eq!(
            search_key_outcome(&key(NamedKey::PageDown), 10, Some(0), 4),
            Some(MoveHover(4))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::PageUp), 10, Some(9), 4),
            Some(MoveHover(5))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::PageDown), 10, Some(1), usize::MAX),
            Some(MoveHover(9))
        );
    }

    #[test]
    fn stale_hover_is_treated_as_no_active_row() {
        assert_eq!(
            search_key_outcome(&key(NamedKey::ArrowUp), 3, Some(99), 5),
            Some(MoveHover(2))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::PageUp), 3, Some(99), 5),
            Some(MoveHover(0))
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::Enter), 3, Some(99), 5),
            None
        );
    }

    #[test]
    fn enter_selects_only_an_active_hover() {
        assert_eq!(
            search_key_outcome(&key(NamedKey::Enter), 3, Some(1), 5),
            Some(Select(1))
        );
        assert_eq!(search_key_outcome(&key(NamedKey::Enter), 3, None, 5), None);
    }

    #[test]
    fn escape_is_not_handled_here_global_handler_owns_it() {
        assert_eq!(
            search_key_outcome(&key(NamedKey::Escape), 3, Some(1), 5),
            None
        );
        assert_eq!(
            search_key_outcome(&key(NamedKey::ArrowLeft), 3, Some(1), 5),
            None
        );
    }

    #[test]
    fn empty_results_disable_every_navigation_key() {
        for named in [NamedKey::ArrowDown, NamedKey::ArrowUp, NamedKey::Enter] {
            assert_eq!(search_key_outcome(&key(named), 0, Some(0), 5), None);
        }
    }

    #[test]
    fn ranked_rows_resolve_to_original_item_indices() {
        let results = [
            SearchMatch {
                index: 8,
                score: 90,
            },
            SearchMatch {
                index: 2,
                score: 70,
            },
        ];

        assert_eq!(original_search_item_index(&results, 0), Some(8));
        assert_eq!(original_search_item_index(&results, 1), Some(2));
        assert_eq!(original_search_item_index(&results, 2), None);
    }

    use super::wheel_search_hover;

    #[test]
    fn wheel_moves_hover_within_result_bounds() {
        assert_eq!(wheel_search_hover(None, 3, 1.0), Some(0));
        assert_eq!(wheel_search_hover(None, 3, -1.0), Some(2));
        assert_eq!(wheel_search_hover(Some(0), 3, -1.0), None);
        assert_eq!(wheel_search_hover(Some(0), 3, 1.0), Some(1));
        assert_eq!(wheel_search_hover(Some(2), 3, -1.0), Some(1));
        assert_eq!(wheel_search_hover(Some(2), 3, 1.0), None);
        assert_eq!(wheel_search_hover(Some(1), 3, 0.0), None);
        assert_eq!(wheel_search_hover(Some(1), 0, 1.0), None);
    }
}
