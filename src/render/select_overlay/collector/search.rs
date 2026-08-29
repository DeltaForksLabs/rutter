// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Point, Rect as SkiaRect};
use taffy::prelude::{NodeId, TaffyTree};

use super::{OverlayOwner, SelectOverlayCollector, rects_overlap};
use crate::engine::widget_state::{SearchState, WidgetState};
use crate::input_state::InputWidgetState;
use crate::layout::RutterContext;
use crate::widget::Widget;
use crate::widgets::search::{SearchMatch, SearchSuggestions, filter_ranked};

/// Open suggestions popup of a focused search bar with attached items.
///
/// `matches` is recomputed on every collection from the field's current text,
/// mirroring the engine's cached ranking so painting and hit-testing agree.
pub(crate) struct SearchOverlay<'a> {
    pub(crate) id: u64,
    pub(crate) anchor: SkiaRect,
    pub(crate) hovered_option: Option<usize>,
    pub(crate) matches: Vec<SearchMatch>,
    pub(crate) items: &'a [&'a str],
    pub(crate) empty_label: &'a str,
    pub(crate) selectable: bool,
    owner: OverlayOwner,
}

/// Collects the open suggestions popups of focused search bars.
///
/// A popup is captured only when its field owns keyboard focus, has
/// suggestions attached, was not dismissed with Escape, and carries
/// non-empty text; an empty result set still produces a popup so the
/// no-matches label can render.
pub(crate) fn collect_open_search_overlays<'r, 'p, Msg>(
    widget: &'r Widget<'p, Msg>,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget_states: &HashMap<u64, WidgetState>,
    input_states: &HashMap<u64, InputWidgetState>,
    focused_id: Option<u64>,
    viewport: (f32, f32),
) -> Vec<SearchOverlay<'p>>
where
    'p: 'r,
{
    let mut collector = SelectOverlayCollector::new(taffy, widget_states, viewport)
        .with_search_context(input_states, focused_id);
    collector.visit(widget, root, Point::new(0.0, 0.0));
    collector.finish_searches()
}

impl<'tree, 'widget, 'entry: 'widget, Msg> SelectOverlayCollector<'tree, 'widget, 'entry, Msg> {
    fn with_search_context(
        mut self,
        input_states: &'tree HashMap<u64, InputWidgetState>,
        focused_id: Option<u64>,
    ) -> Self {
        self.search_context = Some((input_states, focused_id));
        self
    }

    fn finish_searches(mut self) -> Vec<SearchOverlay<'entry>> {
        self.searches
            .retain(|overlay| overlay.owner == self.top_owner);
        self.searches
    }

    pub(super) fn capture_search(
        &mut self,
        widget: &'widget Widget<'entry, Msg>,
        suggestions: &SearchSuggestions<'entry, Msg>,
        absolute: Point,
        size: (f32, f32),
    ) {
        let Some((input_states, focused_id)) = self.search_context else {
            return;
        };
        let id = widget.resolved_id(&self.path).unwrap();
        if focused_id != Some(id) {
            return;
        }
        let Some(state) = self.open_search_state(id).cloned() else {
            return;
        };
        let Some(input) = input_states.get(&id) else {
            return;
        };
        let query = input.text();
        if query.trim().is_empty() {
            return;
        }
        self.push_search(widget, suggestions, &state, &query, absolute, size);
    }

    fn open_search_state(&self, id: u64) -> Option<&SearchState> {
        self.widget_states
            .get(&id)
            .and_then(WidgetState::as_search)
            .filter(|state| !state.dismissed)
    }

    fn push_search(
        &mut self,
        widget: &Widget<'entry, Msg>,
        suggestions: &SearchSuggestions<'entry, Msg>,
        state: &SearchState,
        query: &str,
        absolute: Point,
        size: (f32, f32),
    ) {
        let anchor = SkiaRect::from_xywh(absolute.x, absolute.y, size.0, size.1);
        if self.clip.is_some_and(|clip| !rects_overlap(anchor, clip)) {
            return;
        }
        self.searches.push(SearchOverlay {
            id: widget.resolved_id(&self.path).unwrap(),
            anchor,
            hovered_option: state.hovered_option,
            matches: filter_ranked(
                suggestions.items,
                query,
                suggestions.matcher,
                suggestions.max_results,
            ),
            items: suggestions.items,
            empty_label: suggestions.labels.no_results,
            selectable: suggestions.on_select.is_some(),
            owner: self.active_owner,
        });
    }
}
