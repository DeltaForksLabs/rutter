//! Controlled editable option picking.
//!
//! The application owns the draft and committed source index. Unmatched drafts
//! are allowed; blur, Escape and Tab never commit. Active descendant is separate
//! from committed selection. Keep source order stable while accessibility
//! actions may be queued.
//!
//! ```
//! use rutter::Widget;
//! use rutter::taffy::Style;
//! use rutter::combo_box::ComboBox;
//! #[derive(Clone)]
//! enum Msg { Draft(String), Pick(usize) }
//! let items = ["Lisboa", "São Paulo", "東京"];
//! let options = ComboBox::new(&items, Msg::Pick)
//!     .with_label("Destination").with_placeholder("Type or browse")
//!     .with_selected_index(Some(1));
//! let widget: Widget<'_, Msg> = Widget::combo_box(
//!     62, "São Paulo", Msg::Draft, options, Style::default(),
//! );
//! // On Pick(index), store index and replace the draft with items[index].
//! ```

use crate::search::{SearchConfigError, SearchLabels, SearchMatcher, SearchSuggestions};
use crate::{InputState, Widget};
use taffy::Style;

// Control dimensions, not spacing tokens. Physical right placement matches the
// native-style reference in both writing directions; the editor origin stays put.
pub(crate) fn toggle_rect(body: skia_safe::Rect) -> skia_safe::Rect {
    let side = 32.0_f32
        .min((body.height() - 8.0).max(0.0))
        .min((body.width() - 8.0).max(0.0));
    skia_safe::Rect::from_xywh(
        body.right - 4.0 - side,
        body.center_y() - side / 2.0,
        side,
        side,
    )
}

pub(crate) fn text_width(size: (f32, f32), spacing: f32) -> f32 {
    let button = toggle_rect(skia_safe::Rect::from_xywh(0.0, 0.0, size.0, size.1));
    // One 4px separation owns the text/button relationship.
    (button.left - 4.0 - spacing * 2.0).max(1.0)
}

/// Borrowed source and presentation for [`Widget::combo_box`](crate::Widget::combo_box).
#[derive(Debug, Clone)]
pub struct ComboBox<'a, Msg> {
    pub(crate) suggestions: SearchSuggestions<'a, Msg>,
    pub(crate) label: &'a str,
    pub(crate) placeholder: &'a str,
    pub(crate) state: InputState,
    pub(crate) error_msg: Option<String>,
    pub(crate) selected_index: Option<usize>,
}

impl<'a, Msg> ComboBox<'a, Msg> {
    /// Fuzzy matching and six visible rows. Selection reports the original source index.
    pub fn new(items: &'a [&'a str], on_select: fn(usize) -> Msg) -> Self {
        Self {
            suggestions: SearchSuggestions::new(items, SearchMatcher::Fuzzy, 6, Some(on_select))
                .expect("six visible rows is valid"),
            label: "",
            placeholder: "",
            state: InputState::Idle,
            error_msg: None,
            selected_index: None,
        }
    }
    /// Sets the field label, preserving full accessible text.
    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }
    /// Sets the empty editor hint.
    pub fn with_placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }
    /// Sets the visual input state. Use `Widget::enabled(false)` to disable interaction.
    pub fn with_state(mut self, state: InputState) -> Self {
        self.state = state;
        self
    }
    /// Reserves an annotation row for validation feedback.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error_msg = Some(error.into());
        self
    }
    /// Sets application-owned committed selection independently of hover.
    /// Out-of-range indices are treated as no selection.
    pub fn with_selected_index(mut self, index: Option<usize>) -> Self {
        self.selected_index = index.filter(|index| *index < self.suggestions.items.len());
        self
    }
    /// Reuses the search matching strategy.
    pub fn with_matcher(mut self, matcher: SearchMatcher) -> Self {
        self.suggestions.matcher = matcher;
        self
    }
    /// Sets the visible row window, retaining every match. Zero returns an error.
    pub fn with_visible_rows(mut self, rows: usize) -> Result<Self, SearchConfigError> {
        if rows == 0 {
            return Err(SearchConfigError::MaxResults { value: rows });
        }
        self.suggestions.max_results = rows;
        Ok(self)
    }
    /// Localizes popup feedback.
    pub fn with_labels(mut self, labels: SearchLabels<'a>) -> Self {
        self.suggestions.labels = labels;
        self
    }
}

impl<'a, Msg> Widget<'a, Msg> {
    /// Controlled editable picker. See [`crate::combo_box`] for a complete example.
    /// Echo `on_change` into `query`; the configuration's selection callback
    /// receives the original source index. Blur and Tab never commit a draft.
    /// Uses the single-line `InputKind::TextInput` limits and editor pipeline.
    pub fn combo_box(
        id: u64,
        query: &'a str,
        on_change: fn(String) -> Msg,
        options: ComboBox<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::ComboBox {
            id,
            model_value: Some(query),
            on_change,
            suggestions: options.suggestions,
            selected_index: options.selected_index,
            label: options.label,
            placeholder: options.placeholder,
            state: options.state,
            error_msg: options.error_msg,
            style,
        }
    }
}
