// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{WidgetIdOrigin, WidgetIdResult, WidgetIdTag, WidgetIdVisitor};
use crate::widget::Widget;

impl WidgetIdVisitor {
    pub(super) fn register_search_suggestions<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        item_count: usize,
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        let Some(popup_id) = widget.search_popup_id(&self.path) else {
            return Ok(());
        };
        self.insert_subwidget(
            popup_id,
            WidgetIdTag::SearchPopup,
            "SearchPopup",
            &[],
            origin,
        )?;
        for index in 0..item_count {
            let Some(suggestion_id) = widget.search_suggestion_id(&self.path, index) else {
                continue;
            };
            self.insert_subwidget(
                suggestion_id,
                WidgetIdTag::SearchSuggestion,
                "SearchSuggestion",
                &[index],
                origin,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::WidgetIdSnapshot;
    use crate::widget::{Widget, resolve_search_popup_id, resolve_search_suggestion_id};
    use crate::{SearchMatcher, SearchSuggestions};
    use taffy::prelude::Style;

    const ITEMS: &[&str] = &["Alpha", "Beta", "Gamma"];

    fn search_widget() -> Widget<'static, ()> {
        let suggestions =
            SearchSuggestions::new(ITEMS, SearchMatcher::Fuzzy, 3, Some(|_: usize| ())).unwrap();
        Widget::search_bar_with_suggestions(
            |_: String| (),
            None,
            None,
            None,
            "Search",
            suggestions,
            Style::default(),
        )
        .with_id(41)
    }

    #[test]
    fn search_snapshot_reserves_popup_and_suggestion_ids() {
        let widget = search_widget();
        let snapshot = WidgetIdSnapshot::capture(&widget).unwrap();

        assert!(snapshot.owners.contains_key(&resolve_search_popup_id(41)));
        for index in 0..ITEMS.len() {
            assert!(
                snapshot
                    .owners
                    .contains_key(&resolve_search_suggestion_id(41, index))
            );
        }
    }
}
