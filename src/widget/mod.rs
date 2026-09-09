// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — widget/mod.rs
// ============================================================

pub(crate) mod id;

use std::fmt;

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

use self::id::{AUTOMATIC_ID_NAMESPACE_BIT, WidgetId, WidgetIdError};
use crate::widgets::carousel::CarouselConfig;
use crate::widgets::dropdown_menu::{DropdownMenuEntry, entry_at_path, flatten_entry_paths};
use crate::widgets::rich_text::RichText;
use crate::widgets::search::SearchSuggestions;
use crate::widgets::table::{
    TableColumnKey, TableModel, TableOptions, TableRowKey, TableSelection,
};
use crate::widgets::table_of_contents::{
    HeadingLevel, TableOfContentsAccordion, TableOfContentsOptions,
};
use crate::widgets::time::{ClockConfig, TimeZone};

/// Sentinel reservado para IDs gerados automaticamente a partir do caminho da
/// árvore. IDs manuais seguros devem ser criados com [`WidgetId::manual`].
pub const AUTO_ID: u64 = 0;

const WIDGET_ID_HASH_OFFSET: u64 = 0xcbf29ce484222325;
const WIDGET_ID_HASH_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetConfigError {
    InvalidSliderRange,
    InvalidVirtualItemHeight,
    InvalidVirtualGridColumns,
    SelectedIndexOutOfBounds,
}

impl fmt::Display for WidgetConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSliderRange => {
                "invalid slider values, expected finite value/min/max/step with min <= max and step > 0"
            }
            Self::InvalidVirtualItemHeight => {
                "invalid virtual item height, expected a finite value > 0"
            }
            Self::InvalidVirtualGridColumns => "invalid virtual grid columns, expected columns > 0",
            Self::SelectedIndexOutOfBounds => {
                "invalid selected index, expected an index within the options slice"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for WidgetConfigError {}

/// Reports an invalid integer counter configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterConfigError {
    InvalidRange { min: i64, max: i64 },
    ValueOutOfRange { value: i64, min: i64, max: i64 },
    InvalidStep { step: i64 },
}

impl fmt::Display for CounterConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { min, max } => {
                write!(
                    formatter,
                    "invalid counter range {min}..={max}, expected min <= max"
                )
            }
            Self::ValueOutOfRange { value, min, max } => write!(
                formatter,
                "invalid counter value {value}, expected a value within {min}..={max}"
            ),
            Self::InvalidStep { step } => {
                write!(formatter, "invalid counter step {step}, expected step > 0")
            }
        }
    }
}

impl std::error::Error for CounterConfigError {}

/// Validates the value, range, and step supplied to an integer counter.
///
/// ```rust
/// use rutter::validate_counter;
///
/// assert!(validate_counter(1, 0, 9, 1).is_ok());
/// ```
pub fn validate_counter(
    value: i64,
    min: i64,
    max: i64,
    step: i64,
) -> Result<(), CounterConfigError> {
    if min > max {
        return Err(CounterConfigError::InvalidRange { min, max });
    }
    if value < min || value > max {
        return Err(CounterConfigError::ValueOutOfRange { value, min, max });
    }
    if step <= 0 {
        return Err(CounterConfigError::InvalidStep { step });
    }
    Ok(())
}

pub fn validate_slider(value: f32, min: f32, max: f32, step: f32) -> Result<(), WidgetConfigError> {
    if value.is_finite()
        && min.is_finite()
        && max.is_finite()
        && step.is_finite()
        && min <= max
        && step > 0.0
    {
        return Ok(());
    }
    Err(WidgetConfigError::InvalidSliderRange)
}

pub fn validate_virtual_list(item_height: f32) -> Result<(), WidgetConfigError> {
    if item_height.is_finite() && item_height > 0.0 {
        return Ok(());
    }
    Err(WidgetConfigError::InvalidVirtualItemHeight)
}

pub fn validate_virtual_grid(columns: usize, item_height: f32) -> Result<(), WidgetConfigError> {
    if columns == 0 {
        return Err(WidgetConfigError::InvalidVirtualGridColumns);
    }
    validate_virtual_list(item_height)
}

/// Selection configuration for a virtual list or grid.
///
/// [`VirtualSelection::Multiple`] callbacks receive sorted, unique, in-range
/// indices. Keep the returned selection in application state and provide it
/// again on the next view. Pointer dragging selects a range in lists and a
/// rectangle in grids; Ctrl/Command-drag adds that region.
///
/// ```rust
/// use rutter::VirtualSelection;
///
/// let selection = VirtualSelection::multiple(&[1, 4], |_| ());
/// assert!(matches!(selection, VirtualSelection::Multiple { .. }));
/// ```
#[derive(Clone, Copy)]
pub enum VirtualSelection<'a, Msg> {
    Single(fn(usize) -> Msg),
    Multiple {
        selected: &'a [usize],
        on_change: fn(Vec<usize>) -> Msg,
    },
}

impl<'a, Msg> VirtualSelection<'a, Msg> {
    /// Creates a single-selection configuration.
    ///
    /// ```rust
    /// use rutter::VirtualSelection;
    ///
    /// let selection: VirtualSelection<'_, ()> = VirtualSelection::single(|_| ());
    /// assert!(matches!(selection, VirtualSelection::Single(_)));
    /// ```
    pub const fn single(on_select: fn(usize) -> Msg) -> Self {
        Self::Single(on_select)
    }

    /// Creates a controlled multiselection configuration.
    ///
    /// ```rust
    /// use rutter::VirtualSelection;
    ///
    /// let selection: VirtualSelection<'_, ()> = VirtualSelection::multiple(&[1, 4], |_| ());
    /// assert!(matches!(selection, VirtualSelection::Multiple { .. }));
    /// ```
    pub const fn multiple(selected: &'a [usize], on_change: fn(Vec<usize>) -> Msg) -> Self {
        Self::Multiple {
            selected,
            on_change,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InputState {
    #[default]
    Idle,
    Focused,
    Error,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Ghost,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ToastPosition {
    TopLeft,
    TopRight,
    #[default]
    BottomRight,
    BottomLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogPosition {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuEntry<'a, Msg> {
    Item {
        label: &'a str,
        on_select: Option<Msg>,
    },
    Separator,
}

impl<'a, Msg> ContextMenuEntry<'a, Msg> {
    pub fn item(label: &'a str, on_select: Msg) -> Self {
        Self::Item {
            label,
            on_select: Some(on_select),
        }
    }

    pub fn disabled(label: &'a str) -> Self {
        Self::Item {
            label,
            on_select: None,
        }
    }

    pub const fn separator() -> Self {
        Self::Separator
    }

    pub(crate) fn label(&self) -> Option<&'a str> {
        match self {
            Self::Item { label, .. } => Some(label),
            Self::Separator => None,
        }
    }
}

pub(crate) const CONTEXT_MENU_ITEM_H: f32 = 32.0;
pub(crate) const CONTEXT_MENU_SEPARATOR_H: f32 = 10.0;
pub(crate) const CONTEXT_MENU_MIN_W: f32 = 168.0;
pub(crate) const CONTEXT_MENU_PAD_X: f32 = 12.0;
pub(crate) const CONTEXT_MENU_PAD_Y: f32 = 6.0;
pub(crate) const CONTEXT_MENU_VIEWPORT_MARGIN: f32 = 8.0;
pub(crate) const POPOVER_GAP: f32 = 8.0;
pub(crate) const POPOVER_VIEWPORT_MARGIN: f32 = 8.0;

pub(crate) fn estimate_context_menu_width<Msg>(
    entries: &[ContextMenuEntry<'_, Msg>],
    font_size: f32,
) -> f32 {
    let mut max_w = CONTEXT_MENU_MIN_W;
    for entry in entries {
        if let Some(label) = entry.label() {
            let label = crate::text_controls::normalize_text_controls(
                label,
                crate::text_controls::TextControlPolicy::FlattenLineBreaks,
            );
            let estimate =
                label.chars().count() as f32 * font_size * 0.62 + CONTEXT_MENU_PAD_X * 2.0 + 24.0;
            max_w = max_w.max(estimate);
        }
    }
    max_w
}

pub(crate) fn estimate_context_menu_height<Msg>(entries: &[ContextMenuEntry<'_, Msg>]) -> f32 {
    entries
        .iter()
        .map(|entry| match entry {
            ContextMenuEntry::Item { .. } => CONTEXT_MENU_ITEM_H,
            ContextMenuEntry::Separator => CONTEXT_MENU_SEPARATOR_H,
        })
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WidgetIdTag {
    TextInput = 1,
    TextArea = 2,
    SearchBar = 3,
    Slider = 4,
    Select = 5,
    ProgressBar = 6,
    Spinner = 7,
    ScrollView = 8,
    Accordion = 9,
    TabBar = 10,
    Modal = 11,
    Dialog = 12,
    Toast = 13,
    VirtualList = 14,
    Button = 15,
    Checkbox = 16,
    Switch = 17,
    Radio = 18,
    Tab = 19,
    DialogConfirm = 20,
    DialogCancel = 21,
    VirtualGrid = 22,
    ContextMenu = 23,
    Popover = 24,
    AccessibilityLeaf = 25,
    CarouselView = 26,
    DropdownMenu = 27,
    DropdownMenuPopup = 28,
    DropdownMenuItem = 29,
    Counter = 30,
    Clock = 31,
    SearchPopup = 32,
    SearchSuggestion = 33,
    TableOfContents = 34,
    TableOfContentsEntry = 35,
    TableOfContentsNavigation = 36,
    TableOfContentsViewport = 37,
    TableOfContentsAccordion = 38,
    Table = 39,
    TableHeader = 40,
    TableRow = 41,
    TableCell = 42,
    TableEmpty = 43,
    TableHeaderRow = 44,
    TableEmptyRow = 45,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogAction {
    Confirm,
    Cancel,
}

pub(crate) fn resolve_widget_id(raw_id: u64, tag: WidgetIdTag, path: &[usize]) -> u64 {
    if raw_id != AUTO_ID {
        return raw_id;
    }
    let mut hash = hash_widget_id_segment(WIDGET_ID_HASH_OFFSET, tag as u64);
    for &segment in path {
        hash = hash_widget_id_segment(hash, (segment as u64).wrapping_add(1));
    }
    hash | AUTOMATIC_ID_NAMESPACE_BIT
}

pub(crate) fn resolve_subwidget_id(base_id: u64, tag: WidgetIdTag, slot: usize) -> u64 {
    let path: [usize; 1] = [slot];
    resolve_subwidget_path_id(base_id, tag, &path)
}

pub(crate) fn resolve_subwidget_path_id(base_id: u64, tag: WidgetIdTag, path: &[usize]) -> u64 {
    let mut hash = hash_widget_id_segment(WIDGET_ID_HASH_OFFSET, tag as u64);
    hash = hash_widget_id_segment(hash, base_id);
    for &segment in path {
        hash = hash_widget_id_segment(hash, (segment as u64).wrapping_add(1));
    }
    hash | AUTOMATIC_ID_NAMESPACE_BIT
}

pub(crate) fn resolve_search_popup_id(search_id: u64) -> u64 {
    resolve_subwidget_path_id(search_id, WidgetIdTag::SearchPopup, &[])
}

pub(crate) fn resolve_search_suggestion_id(search_id: u64, item_index: usize) -> u64 {
    resolve_subwidget_id(search_id, WidgetIdTag::SearchSuggestion, item_index)
}

pub(crate) fn resolve_table_of_contents_navigation_id(table_id: u64) -> u64 {
    resolve_subwidget_id(table_id, WidgetIdTag::TableOfContentsNavigation, 0)
}

pub(crate) fn resolve_table_of_contents_viewport_id(table_id: u64) -> u64 {
    resolve_subwidget_id(table_id, WidgetIdTag::TableOfContentsViewport, 0)
}

pub(crate) fn resolve_table_of_contents_accordion_id(table_id: u64) -> u64 {
    resolve_subwidget_id(table_id, WidgetIdTag::TableOfContentsAccordion, 0)
}

pub(crate) fn resolve_table_header_id(table_id: u64, column: TableColumnKey) -> u64 {
    resolve_keyed_subwidget_id(table_id, WidgetIdTag::TableHeader, &[column.get()])
}

pub(crate) fn resolve_table_row_id(table_id: u64, row: TableRowKey) -> u64 {
    resolve_keyed_subwidget_id(table_id, WidgetIdTag::TableRow, &[row.get()])
}

pub(crate) fn resolve_table_cell_id(
    table_id: u64,
    row: TableRowKey,
    column: TableColumnKey,
) -> u64 {
    resolve_keyed_subwidget_id(table_id, WidgetIdTag::TableCell, &[row.get(), column.get()])
}

pub(crate) fn resolve_table_empty_id(table_id: u64) -> u64 {
    resolve_keyed_subwidget_id(table_id, WidgetIdTag::TableEmpty, &[])
}

pub(crate) fn resolve_table_header_row_id(table_id: u64) -> u64 {
    resolve_keyed_subwidget_id(table_id, WidgetIdTag::TableHeaderRow, &[])
}

pub(crate) fn resolve_table_empty_row_id(table_id: u64) -> u64 {
    resolve_keyed_subwidget_id(table_id, WidgetIdTag::TableEmptyRow, &[])
}

fn resolve_keyed_subwidget_id(base_id: u64, tag: WidgetIdTag, keys: &[u64]) -> u64 {
    let mut hash = hash_widget_id_segment(WIDGET_ID_HASH_OFFSET, tag as u64);
    hash = hash_widget_id_segment(hash, base_id);
    for key in keys {
        hash = hash_widget_id_segment(hash, key.wrapping_add(1));
    }
    hash | AUTOMATIC_ID_NAMESPACE_BIT
}

fn hash_widget_id_segment(hash: u64, segment: u64) -> u64 {
    let mixed: u64 = hash ^ segment;
    mixed.wrapping_mul(WIDGET_ID_HASH_PRIME)
}

// Keep public Popover fields unboxed so callers can construct this variant directly.
#[allow(clippy::large_enum_variant)]
pub enum Widget<'a, Msg> {
    Column {
        children: Vec<Widget<'a, Msg>>,
        style: Style,
    },
    Row {
        children: Vec<Widget<'a, Msg>>,
        style: Style,
    },
    Container {
        child: Box<Widget<'a, Msg>>,
        style: Style,
        color: Option<SkiaColor>,
        radius: f32,
    },
    Spacer {
        style: Style,
    },
    Divider {
        style: Style,
        orientation: Orientation,
    },
    Text {
        content: String,
        style: Style,
        color: Option<SkiaColor>,
        size: f32,
    },
    Heading {
        content: String,
        level: HeadingLevel,
        style: Style,
        color: Option<SkiaColor>,
    },
    RichText {
        content: RichText<'a>,
        style: Style,
    },
    Image {
        data: &'a [u8],
        style: Style,
        radius: f32,
    },
    Button {
        text: &'a str,
        on_press: Msg,
        style: Style,
        color: Option<SkiaColor>,
        variant: ButtonVariant,
    },
    ButtonContent {
        label: &'a str,
        child: Box<Widget<'a, Msg>>,
        on_press: Msg,
        style: Style,
        color: Option<SkiaColor>,
        variant: ButtonVariant,
    },
    TextInput {
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        style: Style,
        id: u64,
        label: &'a str,
        placeholder: &'a str,
        state: InputState,
        error_msg: Option<String>,
        is_password: bool,
    },
    TextArea {
        id: u64,
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        style: Style,
        label: &'a str,
        state: InputState,
        placeholder: &'a str,
        error_msg: Option<String>,
    },
    SearchBar {
        id: u64,
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        on_search: Option<Msg>,
        on_clear: Option<Msg>,
        placeholder: &'a str,
        style: Style,
        /// Integrated suggestion source; `None` keeps the plain input
        /// behavior. Constructed via [`Widget::search_bar_with_suggestions`].
        suggestions: Option<SearchSuggestions<'a, Msg>>,
    },
    Checkbox {
        checked: bool,
        on_change: fn(bool) -> Msg,
        label: &'a str,
        style: Style,
    },
    Switch {
        checked: bool,
        on_change: fn(bool) -> Msg,
        style: Style,
    },
    Radio {
        selected: bool,
        on_select: fn() -> Msg,
        label: &'a str,
        style: Style,
    },
    Slider {
        id: u64,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        on_change: fn(f32) -> Msg,
        style: Style,
        label: &'a str,
    },
    Counter {
        id: u64,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
        on_change: fn(i64) -> Msg,
        style: Style,
        label: &'a str,
    },
    Clock {
        id: u64,
        time_zone: TimeZone,
        config: ClockConfig,
        style: Style,
        label: &'a str,
    },
    Select {
        id: u64,
        options: &'a [&'a str],
        selected_index: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
        label: &'a str,
        placeholder: &'a str,
    },
    ProgressBar {
        id: u64,
        value: f32,
        indeterminate: bool,
        style: Style,
    },
    Spinner {
        id: u64,
        style: Style,
    },
    ScrollView {
        id: u64,
        child: Box<Widget<'a, Msg>>,
        style: Style,
    },
    /// A document viewport with an automatically generated navigation list.
    TableOfContents {
        id: u64,
        title: &'a str,
        child: Box<Widget<'a, Msg>>,
        style: Style,
        options: TableOfContentsOptions<Msg>,
    },
    /// A keyed, text-based table with sticky headers and controlled interactions.
    Table {
        id: u64,
        model: TableModel<'a>,
        options: TableOptions<'a, Msg>,
        style: Style,
    },
    Tooltip {
        child: Box<Widget<'a, Msg>>,
        text: &'a str,
        style: Style,
    },
    Accordion {
        id: u64,
        title: &'a str,
        expanded: bool,
        on_toggle: Msg,
        child: Box<Widget<'a, Msg>>,
        style: Style,
    },
    TabBar {
        id: u64,
        tabs: &'a [&'a str],
        active: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
    },
    Modal {
        id: u64,
        visible: bool,
        child: Box<Widget<'a, Msg>>,
        on_dismiss: Option<Msg>,
        style: Style,
    },
    Dialog {
        id: u64,
        title: &'a str,
        message: &'a str,
        confirm_label: &'a str,
        cancel_label: &'a str,
        visible: bool,
        on_confirm: Msg,
        on_cancel: Msg,
        on_dismiss: Option<Msg>,
        position: DialogPosition,
        style: Style,
        child: Box<Widget<'a, Msg>>,
    },
    Toast {
        id: u64,
        visible: bool,
        message: &'a str,
        kind: ToastKind,
        position: ToastPosition,
        duration_ms: u32,
        on_dismiss: Option<Msg>,
    },
    ContextMenu {
        id: u64,
        child: Box<Widget<'a, Msg>>,
        entries: &'a [ContextMenuEntry<'a, Msg>],
        style: Style,
    },
    DropdownMenu {
        id: u64,
        label: &'a str,
        entries: Vec<DropdownMenuEntry<'a, Msg>>,
        style: Style,
    },
    Popover {
        id: u64,
        open: bool,
        anchor: Box<Widget<'a, Msg>>,
        content: Box<Widget<'a, Msg>>,
        on_dismiss: Option<Msg>,
        style: Style,
        popup_style: Style,
    },
    CarouselView {
        id: u64,
        item_count: usize,
        items: Box<dyn Fn(usize) -> Option<Widget<'a, Msg>> + 'a>,
        on_select: fn(usize) -> Msg,
        config: CarouselConfig,
        style: Style,
    },
    VirtualList {
        id: u64,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        on_select: fn(usize) -> Msg,
        style: Style,
    },
    VirtualListContent {
        id: u64,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        on_select: fn(usize) -> Msg,
        style: Style,
    },
    VirtualGrid {
        id: u64,
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        on_select: fn(usize) -> Msg,
        style: Style,
    },
    VirtualGridContent {
        id: u64,
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        on_select: fn(usize) -> Msg,
        style: Style,
    },
    VirtualListWithSelection {
        id: u64,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    },
    VirtualListContentWithSelection {
        id: u64,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    },
    VirtualGridWithSelection {
        id: u64,
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    },
    VirtualGridContentWithSelection {
        id: u64,
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    },
}

impl<'a, Msg> Widget<'a, Msg> {
    /// Creates one non-interactive text leaf from styled spans.
    ///
    /// ```rust
    /// use rutter::{RichText, RichTextSpan, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let content = RichText::from_span(RichTextSpan::new("2026").bold());
    /// let text: Widget<'_, ()> = Widget::rich_text(content, Style::default());
    /// assert!(matches!(text, Widget::RichText { .. }));
    /// ```
    pub fn rich_text(content: RichText<'a>, style: Style) -> Self {
        Self::RichText { content, style }
    }

    /// Creates a button whose visual content is another widget.
    ///
    /// Example:
    /// ```
    /// # use rutter::{ButtonVariant, Widget};
    /// # use taffy::prelude::Style;
    /// # enum Msg { Save }
    /// let button = Widget::button_content(
    ///     "Save",
    ///     Widget::Text {
    ///         content: "Save".into(),
    ///         style: Style::default(),
    ///         color: None,
    ///         size: 14.0,
    ///     },
    ///     Msg::Save,
    ///     Style::default(),
    ///     None,
    ///     ButtonVariant::Primary,
    /// );
    /// ```
    pub fn button_content(
        label: &'a str,
        child: Widget<'a, Msg>,
        on_press: Msg,
        style: Style,
        color: Option<SkiaColor>,
        variant: ButtonVariant,
    ) -> Self {
        Self::ButtonContent {
            label,
            child: Box::new(child),
            on_press,
            style,
            color,
            variant,
        }
    }

    // This positional constructor is public API; changing it would be breaking.
    #[allow(clippy::too_many_arguments)]
    pub fn text_input(
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        style: Style,
        label: &'a str,
        placeholder: &'a str,
        state: InputState,
        error_msg: Option<String>,
        is_password: bool,
    ) -> Self {
        Self::TextInput {
            on_change,
            on_submit,
            style,
            id: AUTO_ID,
            label,
            placeholder,
            state,
            error_msg,
            is_password,
        }
    }

    pub fn text_area(
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        style: Style,
        label: &'a str,
        state: InputState,
        placeholder: &'a str,
        error_msg: Option<String>,
    ) -> Self {
        Self::TextArea {
            id: AUTO_ID,
            on_change,
            on_submit,
            style,
            label,
            state,
            placeholder,
            error_msg,
        }
    }

    pub fn search_bar(
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        on_search: Option<Msg>,
        on_clear: Option<Msg>,
        placeholder: &'a str,
        style: Style,
    ) -> Self {
        Self::SearchBar {
            id: AUTO_ID,
            on_change,
            on_submit,
            on_search,
            on_clear,
            placeholder,
            style,
            suggestions: None,
        }
    }

    /// Search input with an integrated suggestions popup.
    ///
    /// The popup opens while the field is focused and its text is non-empty;
    /// it ranks `suggestions` items with the configured matcher. Selecting a
    /// row dispatches `on_select(original_index)`; keyboard navigation
    /// (arrows/Home/End/PageUp/PageDown to move, Enter to select, Escape to
    /// dismiss) mirrors the [`Widget::Select`] behavior.
    ///
    /// ```
    /// use rutter::{SearchMatcher, Widget};
    /// use rutter::search::SearchSuggestions;
    /// use taffy::prelude::Style;
    ///
    /// enum Msg {
    ///     Query(String),
    ///     Picked(usize),
    /// }
    ///
    /// let movies = ["Matrix", "Interestelar"];
    /// let suggestions = SearchSuggestions::new(
    ///     &movies,
    ///     SearchMatcher::Fuzzy,
    ///     5,
    ///     Some(Msg::Picked),
    /// )
    /// .unwrap();
    /// let bar: Widget<'_, Msg> = Widget::search_bar_with_suggestions(
    ///     Msg::Query,
    ///     None,
    ///     None,
    ///     None,
    ///     "Buscar...",
    ///     suggestions,
    ///     Style::default(),
    /// );
    /// ```
    pub fn search_bar_with_suggestions(
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        on_search: Option<Msg>,
        on_clear: Option<Msg>,
        placeholder: &'a str,
        suggestions: SearchSuggestions<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::SearchBar {
            id: AUTO_ID,
            on_change,
            on_submit,
            on_search,
            on_clear,
            placeholder,
            style,
            suggestions: Some(suggestions),
        }
    }

    pub fn slider(
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        on_change: fn(f32) -> Msg,
        style: Style,
        label: &'a str,
    ) -> Self {
        Self::Slider {
            id: AUTO_ID,
            value,
            min,
            max,
            step,
            on_change,
            style,
            label,
        }
    }

    pub fn try_slider(
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        on_change: fn(f32) -> Msg,
        style: Style,
        label: &'a str,
    ) -> Result<Self, WidgetConfigError> {
        validate_slider(value, min, max, step)?;
        Ok(Self::slider(value, min, max, step, on_change, style, label))
    }

    /// Creates a controlled integer counter rendered as `- value +`.
    ///
    /// ```rust
    /// use rutter::Widget;
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { QuantityChanged(i64) }
    /// let counter = Widget::counter(1, 0, 9, 1, Msg::QuantityChanged, Style::default(), "Quantity");
    /// assert!(matches!(counter, Widget::Counter { value: 1, .. }));
    /// ```
    pub fn counter(
        value: i64,
        min: i64,
        max: i64,
        step: i64,
        on_change: fn(i64) -> Msg,
        style: Style,
        label: &'a str,
    ) -> Self {
        Self::Counter {
            id: AUTO_ID,
            value,
            min,
            max,
            step,
            on_change,
            style,
            label,
        }
    }

    /// Validates and creates a controlled integer counter.
    ///
    /// ```rust
    /// use rutter::Widget;
    /// use taffy::prelude::Style;
    ///
    /// let counter: Widget<'_, ()> = Widget::try_counter(1, 0, 9, 1, |_| (), Style::default(), "Quantity")?;
    /// # Ok::<(), rutter::CounterConfigError>(())
    /// ```
    pub fn try_counter(
        value: i64,
        min: i64,
        max: i64,
        step: i64,
        on_change: fn(i64) -> Msg,
        style: Style,
        label: &'a str,
    ) -> Result<Self, CounterConfigError> {
        validate_counter(value, min, max, step)?;
        Ok(Self::counter(
            value, min, max, step, on_change, style, label,
        ))
    }

    pub fn select(
        options: &'a [&'a str],
        selected_index: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
        label: &'a str,
        placeholder: &'a str,
    ) -> Self {
        Self::Select {
            id: AUTO_ID,
            options,
            selected_index,
            on_change,
            style,
            label,
            placeholder,
        }
    }

    pub fn try_select(
        options: &'a [&'a str],
        selected_index: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
        label: &'a str,
        placeholder: &'a str,
    ) -> Result<Self, WidgetConfigError> {
        if options.is_empty() || selected_index >= options.len() {
            return Err(WidgetConfigError::SelectedIndexOutOfBounds);
        }
        Ok(Self::select(
            options,
            selected_index,
            on_change,
            style,
            label,
            placeholder,
        ))
    }

    pub fn progress_bar(value: f32, indeterminate: bool, style: Style) -> Self {
        Self::ProgressBar {
            id: AUTO_ID,
            value,
            indeterminate,
            style,
        }
    }

    pub fn spinner(style: Style) -> Self {
        Self::Spinner { id: AUTO_ID, style }
    }

    pub fn scroll_view(child: Widget<'a, Msg>, style: Style) -> Self {
        Self::ScrollView {
            id: AUTO_ID,
            child: Box::new(child),
            style,
        }
    }

    /// Creates a semantic heading that an enclosing table of contents discovers.
    ///
    /// ```rust
    /// use rutter::{HeadingLevel, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let heading: Widget<'_, ()> = Widget::heading(HeadingLevel::H2, "Install", Style::default());
    /// ```
    pub fn heading(level: HeadingLevel, content: impl Into<String>, style: Style) -> Self {
        Self::Heading {
            content: content.into(),
            level,
            style,
            color: None,
        }
    }

    /// Creates a scrollable document with a single-column inline navigation list.
    ///
    /// An absolute style height constrains the document viewport. Relative and
    /// flex sizing establish the table's minimum allocation while navigation
    /// remains fully visible without receiving a scrollbar.
    ///
    /// ```rust
    /// use rutter::{HeadingLevel, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let document: Widget<'_, ()> = Widget::table_of_contents(
    ///     "Contents",
    ///     Widget::Column {
    ///         children: vec![Widget::heading(HeadingLevel::H1, "Overview", Style::default())],
    ///         style: Style::default(),
    ///     },
    ///     Style::default(),
    /// );
    /// ```
    pub fn table_of_contents(title: &'a str, child: Widget<'a, Msg>, style: Style) -> Self {
        Self::table_of_contents_with_options(title, child, style, TableOfContentsOptions::default())
    }

    /// Creates a scrollable document with configurable navigation columns and accordion behavior.
    ///
    /// ```
    /// use rutter::{TableOfContentsOptions, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let options = TableOfContentsOptions::new(2).unwrap().with_accordion(());
    /// let contents: Widget<'_, ()> = Widget::table_of_contents_with_options(
    ///     "On this page",
    ///     Widget::Column { children: vec![], style: Style::default() },
    ///     Style::default(),
    ///     options,
    /// );
    /// ```
    pub fn table_of_contents_with_options(
        title: &'a str,
        child: Widget<'a, Msg>,
        style: Style,
        options: TableOfContentsOptions<Msg>,
    ) -> Self {
        Self::TableOfContents {
            id: AUTO_ID,
            title,
            child: Box::new(child),
            style,
            options,
        }
    }

    /// Creates a semantic table with sticky headers and no row interaction.
    ///
    /// ```rust
    /// use rutter::{TableColumn, TableColumnKey, TableColumnWidth, TableModel, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let columns = vec![TableColumn::new(
    ///     TableColumnKey::new(1),
    ///     "Name",
    ///     TableColumnWidth::flex(120.0, 1).unwrap(),
    /// )];
    /// let model = TableModel::new("People", columns, Vec::new()).unwrap();
    /// let table: Widget<'_, ()> = Widget::table(model, Style::default());
    /// ```
    pub fn table(model: TableModel<'a>, style: Style) -> Self {
        Self::table_with_options(model, TableOptions::default(), style)
    }

    /// Creates a semantic table with controlled sorting and row selection.
    ///
    /// ```rust
    /// use rutter::{TableColumn, TableColumnKey, TableColumnWidth, TableModel, TableOptions, Widget};
    /// use taffy::prelude::Style;
    ///
    /// let columns = vec![TableColumn::new(
    ///     TableColumnKey::new(1), "Name", TableColumnWidth::fixed(160.0).unwrap(),
    /// )];
    /// let model = TableModel::new("People", columns, Vec::new()).unwrap();
    /// let table: Widget<'_, ()> = Widget::table_with_options(
    ///     model, TableOptions::default(), Style::default(),
    /// );
    /// ```
    pub fn table_with_options(
        model: TableModel<'a>,
        options: TableOptions<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::Table {
            id: AUTO_ID,
            model,
            options,
            style,
        }
    }

    pub fn accordion(
        title: &'a str,
        expanded: bool,
        on_toggle: Msg,
        child: Widget<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::Accordion {
            id: AUTO_ID,
            title,
            expanded,
            on_toggle,
            child: Box::new(child),
            style,
        }
    }

    pub fn tab_bar(
        tabs: &'a [&'a str],
        active: usize,
        on_change: fn(usize) -> Msg,
        style: Style,
    ) -> Self {
        Self::TabBar {
            id: AUTO_ID,
            tabs,
            active,
            on_change,
            style,
        }
    }

    pub fn modal(
        visible: bool,
        child: Widget<'a, Msg>,
        on_dismiss: Option<Msg>,
        style: Style,
    ) -> Self {
        Self::Modal {
            id: AUTO_ID,
            visible,
            child: Box::new(child),
            on_dismiss,
            style,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dialog(
        title: &'a str,
        message: &'a str,
        confirm_label: &'a str,
        cancel_label: &'a str,
        visible: bool,
        on_confirm: Msg,
        on_cancel: Msg,
        on_dismiss: Option<Msg>,
        style: Style,
        child: Widget<'a, Msg>,
    ) -> Self {
        Self::Dialog {
            id: AUTO_ID,
            title,
            message,
            confirm_label,
            cancel_label,
            visible,
            on_confirm,
            on_cancel,
            on_dismiss,
            position: DialogPosition::Center,
            style,
            child: Box::new(child),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dialog_positioned(
        title: &'a str,
        message: &'a str,
        confirm_label: &'a str,
        cancel_label: &'a str,
        visible: bool,
        position: DialogPosition,
        on_confirm: Msg,
        on_cancel: Msg,
        on_dismiss: Option<Msg>,
        style: Style,
        child: Widget<'a, Msg>,
    ) -> Self {
        Self::Dialog {
            id: AUTO_ID,
            title,
            message,
            confirm_label,
            cancel_label,
            visible,
            on_confirm,
            on_cancel,
            on_dismiss,
            position,
            style,
            child: Box::new(child),
        }
    }

    pub fn with_dialog_position(mut self, position: DialogPosition) -> Self {
        if let Self::Dialog { position: slot, .. } = &mut self {
            *slot = position;
        }
        self
    }

    pub fn toast(
        visible: bool,
        message: &'a str,
        kind: ToastKind,
        position: ToastPosition,
        duration_ms: u32,
        on_dismiss: Option<Msg>,
    ) -> Self {
        Self::Toast {
            id: AUTO_ID,
            visible,
            message,
            kind,
            position,
            duration_ms,
            on_dismiss,
        }
    }

    pub fn context_menu(
        child: Widget<'a, Msg>,
        entries: &'a [ContextMenuEntry<'a, Msg>],
        style: Style,
    ) -> Self {
        Self::ContextMenu {
            id: AUTO_ID,
            child: Box::new(child),
            entries,
            style,
        }
    }

    /// Creates an engine-owned menu button whose label is also its accessibility label.
    ///
    /// ```rust
    /// use rutter::{DropdownMenuEntry, Widget};
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { Save }
    /// let menu = Widget::dropdown_menu(
    ///     "File",
    ///     vec![DropdownMenuEntry::item("Save", Msg::Save)],
    ///     Style::default(),
    /// );
    /// assert!(matches!(menu, Widget::DropdownMenu { label: "File", .. }));
    /// ```
    pub fn dropdown_menu(
        label: &'a str,
        entries: Vec<DropdownMenuEntry<'a, Msg>>,
        style: Style,
    ) -> Self {
        Self::DropdownMenu {
            id: AUTO_ID,
            label,
            entries,
            style,
        }
    }

    pub fn popover(
        open: bool,
        anchor: Widget<'a, Msg>,
        content: Widget<'a, Msg>,
        on_dismiss: Option<Msg>,
        style: Style,
        popup_style: Style,
    ) -> Self {
        Self::Popover {
            id: AUTO_ID,
            open,
            anchor: Box::new(anchor),
            content: Box::new(content),
            on_dismiss,
            style,
            popup_style,
        }
    }

    /// Creates a horizontally virtualized carousel from visual item widgets.
    ///
    /// Item widgets are materialized only for the visible range plus one
    /// overscan item and receive isolated runtime state, matching
    /// [`Widget::virtual_list_content`].
    ///
    /// ```rust
    /// use rutter::{CarouselConfig, Widget};
    /// use taffy::prelude::Style;
    ///
    /// #[derive(Clone)]
    /// enum Msg { Select(usize) }
    /// let cards = |index| Some(Widget::Text {
    ///     content: format!("Card {}", index + 1),
    ///     style: Style::default(),
    ///     color: None,
    ///     size: 14.0,
    /// });
    /// let config = CarouselConfig::weighted([1, 6, 1]).unwrap();
    /// let carousel = Widget::carousel_view(20, cards, Msg::Select, config, Style::default());
    /// ```
    pub fn carousel_view<ItemBuilder>(
        item_count: usize,
        items: ItemBuilder,
        on_select: fn(usize) -> Msg,
        config: CarouselConfig,
        style: Style,
    ) -> Self
    where
        ItemBuilder: Fn(usize) -> Option<Widget<'a, Msg>> + 'a,
    {
        Self::CarouselView {
            id: AUTO_ID,
            item_count,
            items: Box::new(items),
            on_select,
            config,
            style,
        }
    }

    pub fn virtual_list(
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        on_select: fn(usize) -> Msg,
        style: Style,
    ) -> Self {
        Self::VirtualList {
            id: AUTO_ID,
            item_height,
            item_count,
            items,
            on_select,
            style,
        }
    }

    /// Creates a virtual list with an explicit typed selection configuration.
    ///
    /// [`VirtualSelection::Single`] keeps the single-index callback. With
    /// [`VirtualSelection::Multiple`], pointer dragging selects list ranges and
    /// the callback receives the full sorted selection.
    ///
    /// ```rust
    /// use rutter::{VirtualSelection, Widget};
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { SelectionChanged(Vec<usize>) }
    /// let selected: [usize; 0] = [];
    /// let rows = |_| Some(String::from("Row"));
    /// let _ = Widget::virtual_list_with_selection(
    ///     32.0, 10, &rows, VirtualSelection::multiple(&selected, Msg::SelectionChanged), Style::default(),
    /// );
    /// ```
    pub fn virtual_list_with_selection(
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::VirtualListWithSelection {
            id: AUTO_ID,
            item_height,
            item_count,
            items,
            selection,
            style,
        }
    }

    /// Creates a virtualized list whose visible rows are rendered from widgets.
    /// Item widgets are visual-only and receive isolated runtime-state maps so
    /// on-demand IDs cannot alias controls in the application tree. A surrounding
    /// [`Widget::context_menu`] exposes the pressed row through
    /// [`crate::ContextMenuTarget::virtual_item`].
    ///
    /// Example:
    ///
    /// ```rust
    /// use rutter::Widget;
    /// use taffy::prelude::Style;
    ///
    /// #[derive(Clone)]
    /// enum Msg {
    ///     Select(usize),
    /// }
    ///
    /// let rows = |index| Some(Widget::Text {
    ///     content: (if index == 0 { "Inbox" } else { "Archive" }).into(),
    ///     style: Style::default(),
    ///     color: None,
    ///     size: 14.0,
    /// });
    /// let list = Widget::virtual_list_content(40.0, 2, &rows, Msg::Select, Style::default());
    /// ```
    pub fn virtual_list_content(
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        on_select: fn(usize) -> Msg,
        style: Style,
    ) -> Self {
        Self::VirtualListContent {
            id: AUTO_ID,
            item_height,
            item_count,
            items,
            on_select,
            style,
        }
    }

    /// Creates a content-based virtual list with explicit selection configuration.
    ///
    /// Visible item widgets remain visual-only, matching
    /// [`Widget::virtual_list_content`], while selection remains controlled by
    /// [`VirtualSelection`].
    ///
    /// ```rust
    /// use rutter::{VirtualSelection, Widget};
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { SelectionChanged(Vec<usize>) }
    /// let selected: [usize; 0] = [];
    /// let rows = |_| Some(Widget::Text { content: "Row".into(), color: None, size: 14.0, style: Style::default() });
    /// let _ = Widget::virtual_list_content_with_selection(
    ///     32.0, 10, &rows, VirtualSelection::multiple(&selected, Msg::SelectionChanged), Style::default(),
    /// );
    /// ```
    pub fn virtual_list_content_with_selection(
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::VirtualListContentWithSelection {
            id: AUTO_ID,
            item_height,
            item_count,
            items,
            selection,
            style,
        }
    }

    pub fn virtual_grid(
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        on_select: fn(usize) -> Msg,
        style: Style,
    ) -> Self {
        Self::VirtualGrid {
            id: AUTO_ID,
            columns,
            item_height,
            item_count,
            items,
            on_select,
            style,
        }
    }

    /// Creates a virtual grid with an explicit typed selection configuration.
    ///
    /// [`VirtualSelection::Single`] keeps the single-index callback. With
    /// [`VirtualSelection::Multiple`], pointer dragging selects grid rectangles
    /// and the callback receives the full sorted selection.
    ///
    /// ```rust
    /// use rutter::{VirtualSelection, Widget};
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { SelectionChanged(Vec<usize>) }
    /// let selected: [usize; 0] = [];
    /// let cells = |_| Some(String::from("Cell"));
    /// let _ = Widget::virtual_grid_with_selection(
    ///     3, 48.0, 12, &cells, VirtualSelection::multiple(&selected, Msg::SelectionChanged), Style::default(),
    /// );
    /// ```
    pub fn virtual_grid_with_selection(
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::VirtualGridWithSelection {
            id: AUTO_ID,
            columns,
            item_height,
            item_count,
            items,
            selection,
            style,
        }
    }

    /// Creates a virtualized grid whose visible cells are rendered from widgets.
    /// Cell widgets are visual-only and receive isolated runtime-state maps so
    /// on-demand IDs cannot alias controls in the application tree. A surrounding
    /// [`Widget::context_menu`] exposes the pressed cell through
    /// [`crate::ContextMenuTarget::virtual_item`].
    ///
    /// Example:
    ///
    /// ```rust
    /// use rutter::Widget;
    /// use taffy::prelude::Style;
    ///
    /// #[derive(Clone)]
    /// enum Msg {
    ///     Open(usize),
    /// }
    ///
    /// let cells = |index| Some(Widget::Text {
    ///     content: (if index == 0 { "File" } else { "Folder" }).into(),
    ///     style: Style::default(),
    ///     color: None,
    ///     size: 14.0,
    /// });
    /// let grid = Widget::virtual_grid_content(3, 48.0, 6, &cells, Msg::Open, Style::default());
    /// ```
    pub fn virtual_grid_content(
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        on_select: fn(usize) -> Msg,
        style: Style,
    ) -> Self {
        Self::VirtualGridContent {
            id: AUTO_ID,
            columns,
            item_height,
            item_count,
            items,
            on_select,
            style,
        }
    }

    /// Creates a content-based virtual grid with explicit selection configuration.
    ///
    /// Visible item widgets remain visual-only, matching
    /// [`Widget::virtual_grid_content`], while pointer dragging selects a
    /// controlled rectangular cell region.
    ///
    /// ```rust
    /// use rutter::{VirtualSelection, Widget};
    /// use taffy::prelude::Style;
    ///
    /// enum Msg { SelectionChanged(Vec<usize>) }
    /// let selected: [usize; 0] = [];
    /// let cells = |_| Some(Widget::Text { content: "Cell".into(), color: None, size: 14.0, style: Style::default() });
    /// let _ = Widget::virtual_grid_content_with_selection(
    ///     3, 48.0, 12, &cells, VirtualSelection::multiple(&selected, Msg::SelectionChanged), Style::default(),
    /// );
    /// ```
    pub fn virtual_grid_content_with_selection(
        columns: usize,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<Widget<'a, Msg>>,
        selection: VirtualSelection<'a, Msg>,
        style: Style,
    ) -> Self {
        Self::VirtualGridContentWithSelection {
            id: AUTO_ID,
            columns,
            item_height,
            item_count,
            items,
            selection,
            style,
        }
    }

    pub fn with_id(mut self, id: u64) -> Self {
        self.assign_raw_id(id);
        self
    }

    fn assign_raw_id(&mut self, id: u64) -> bool {
        match self {
            Self::TextInput { id: slot, .. }
            | Self::TextArea { id: slot, .. }
            | Self::SearchBar { id: slot, .. }
            | Self::Slider { id: slot, .. }
            | Self::Counter { id: slot, .. }
            | Self::Clock { id: slot, .. }
            | Self::Select { id: slot, .. }
            | Self::ProgressBar { id: slot, .. }
            | Self::Spinner { id: slot, .. }
            | Self::ScrollView { id: slot, .. }
            | Self::TableOfContents { id: slot, .. }
            | Self::Table { id: slot, .. }
            | Self::Accordion { id: slot, .. }
            | Self::TabBar { id: slot, .. }
            | Self::Modal { id: slot, .. }
            | Self::Dialog { id: slot, .. }
            | Self::Toast { id: slot, .. }
            | Self::ContextMenu { id: slot, .. }
            | Self::DropdownMenu { id: slot, .. }
            | Self::Popover { id: slot, .. }
            | Self::CarouselView { id: slot, .. }
            | Self::VirtualList { id: slot, .. }
            | Self::VirtualListContent { id: slot, .. }
            | Self::VirtualListWithSelection { id: slot, .. }
            | Self::VirtualListContentWithSelection { id: slot, .. }
            | Self::VirtualGrid { id: slot, .. }
            | Self::VirtualGridContent { id: slot, .. }
            | Self::VirtualGridWithSelection { id: slot, .. }
            | Self::VirtualGridContentWithSelection { id: slot, .. } => {
                *slot = id;
                true
            }
            _ => false,
        }
    }

    /// Assigns an already validated manual ID while preserving legacy `u64` fields.
    ///
    /// # Example
    /// ```
    /// use rutter::{Widget, WidgetId};
    /// use taffy::prelude::Style;
    ///
    /// let widget: Widget<'_, ()> = Widget::spinner(Style::default())
    ///     .with_widget_id(WidgetId::manual(42).unwrap()).unwrap();
    /// ```
    pub fn with_widget_id(mut self, id: WidgetId) -> Result<Self, WidgetIdError> {
        if self.assign_raw_id(id.get()) {
            return Ok(self);
        }
        Err(WidgetIdError::UnsupportedWidget { value: id.get() })
    }

    /// Validates and assigns a manual ID, rejecting zero and the automatic namespace.
    ///
    /// # Example
    /// ```
    /// use rutter::Widget;
    /// use taffy::prelude::Style;
    ///
    /// let widget: Widget<'_, ()> = Widget::spinner(Style::default()).try_with_id(42).unwrap();
    /// ```
    pub fn try_with_id(self, raw: u64) -> Result<Self, WidgetIdError> {
        self.with_widget_id(WidgetId::manual(raw)?)
    }

    pub fn with_auto_id(self) -> Self {
        self.with_id(AUTO_ID)
    }

    pub(crate) fn id_owner_metadata(&self) -> Option<(Option<u64>, WidgetIdTag, &'static str)> {
        Some(match self {
            Self::Button { .. } | Self::ButtonContent { .. } => {
                (None, WidgetIdTag::Button, "Button")
            }
            Self::Checkbox { .. } => (None, WidgetIdTag::Checkbox, "Checkbox"),
            Self::Switch { .. } => (None, WidgetIdTag::Switch, "Switch"),
            Self::Radio { .. } => (None, WidgetIdTag::Radio, "Radio"),
            Self::TextInput {
                id,
                is_password: true,
                ..
            } => (Some(*id), WidgetIdTag::TextInput, "PasswordTextInput"),
            Self::TextInput { id, .. } => (Some(*id), WidgetIdTag::TextInput, "TextInput"),
            Self::TextArea { id, .. } => (Some(*id), WidgetIdTag::TextArea, "TextArea"),
            Self::SearchBar { id, .. } => (Some(*id), WidgetIdTag::SearchBar, "SearchBar"),
            Self::Slider { id, .. } => (Some(*id), WidgetIdTag::Slider, "Slider"),
            Self::Counter { id, .. } => (Some(*id), WidgetIdTag::Counter, "Counter"),
            Self::Clock { id, .. } => (Some(*id), WidgetIdTag::Clock, "Clock"),
            Self::Select { id, .. } => (Some(*id), WidgetIdTag::Select, "Select"),
            Self::ProgressBar { id, .. } => (Some(*id), WidgetIdTag::ProgressBar, "ProgressBar"),
            Self::Spinner { id, .. } => (Some(*id), WidgetIdTag::Spinner, "Spinner"),
            Self::ScrollView { id, .. } => (Some(*id), WidgetIdTag::ScrollView, "ScrollView"),
            Self::TableOfContents { id, .. } => {
                (Some(*id), WidgetIdTag::TableOfContents, "TableOfContents")
            }
            Self::Table { id, .. } => (Some(*id), WidgetIdTag::Table, "Table"),
            Self::Accordion { id, .. } => (Some(*id), WidgetIdTag::Accordion, "Accordion"),
            Self::TabBar { id, .. } => (Some(*id), WidgetIdTag::TabBar, "TabBar"),
            Self::Modal { id, .. } => (Some(*id), WidgetIdTag::Modal, "Modal"),
            Self::Dialog { id, .. } => (Some(*id), WidgetIdTag::Dialog, "Dialog"),
            Self::Toast { id, .. } => (Some(*id), WidgetIdTag::Toast, "Toast"),
            Self::ContextMenu { id, .. } => (Some(*id), WidgetIdTag::ContextMenu, "ContextMenu"),
            Self::DropdownMenu { id, .. } => (Some(*id), WidgetIdTag::DropdownMenu, "DropdownMenu"),
            Self::Popover { id, .. } => (Some(*id), WidgetIdTag::Popover, "Popover"),
            Self::CarouselView { id, .. } => (Some(*id), WidgetIdTag::CarouselView, "CarouselView"),
            Self::VirtualList { id, .. }
            | Self::VirtualListContent { id, .. }
            | Self::VirtualListWithSelection { id, .. }
            | Self::VirtualListContentWithSelection { id, .. } => {
                (Some(*id), WidgetIdTag::VirtualList, "VirtualList")
            }
            Self::VirtualGrid { id, .. }
            | Self::VirtualGridContent { id, .. }
            | Self::VirtualGridWithSelection { id, .. }
            | Self::VirtualGridContentWithSelection { id, .. } => {
                (Some(*id), WidgetIdTag::VirtualGrid, "VirtualGrid")
            }
            _ => return None,
        })
    }

    pub(crate) fn resolved_id(&self, path: &[usize]) -> Option<u64> {
        let (raw_id, tag, _) = self.id_owner_metadata()?;
        Some(resolve_widget_id(raw_id?, tag, path))
    }

    pub(crate) fn keyboard_focus_id(&self, path: &[usize]) -> Option<u64> {
        match self {
            Self::Button { .. } | Self::ButtonContent { .. } => {
                Some(resolve_widget_id(AUTO_ID, WidgetIdTag::Button, path))
            }
            Self::Checkbox { .. } => Some(resolve_widget_id(AUTO_ID, WidgetIdTag::Checkbox, path)),
            Self::Switch { .. } => Some(resolve_widget_id(AUTO_ID, WidgetIdTag::Switch, path)),
            Self::Radio { .. } => Some(resolve_widget_id(AUTO_ID, WidgetIdTag::Radio, path)),
            Self::TextInput { .. }
            | Self::TextArea { .. }
            | Self::SearchBar { .. }
            | Self::Slider { .. }
            | Self::Counter { .. }
            | Self::Select { .. }
            | Self::DropdownMenu { .. }
            | Self::Accordion { .. }
            | Self::TabBar { .. }
            | Self::CarouselView { .. }
            | Self::VirtualList { .. }
            | Self::VirtualListContent { .. }
            | Self::VirtualListWithSelection { .. }
            | Self::VirtualListContentWithSelection { .. }
            | Self::VirtualGrid { .. }
            | Self::VirtualGridContent { .. }
            | Self::VirtualGridWithSelection { .. }
            | Self::VirtualGridContentWithSelection { .. } => self.resolved_id(path),
            Self::Table { .. } if self.table_is_interactive() => self.resolved_id(path),
            _ => None,
        }
    }

    pub(crate) fn tab_focus_id(&self, path: &[usize], index: usize) -> Option<u64> {
        match self {
            Self::TabBar { .. } => Some(resolve_subwidget_id(
                self.resolved_id(path)?,
                WidgetIdTag::Tab,
                index,
            )),
            _ => None,
        }
    }

    pub(crate) fn table_of_contents_entry_focus_id(
        &self,
        path: &[usize],
        index: usize,
    ) -> Option<u64> {
        match self {
            Self::TableOfContents { .. } => Some(resolve_subwidget_id(
                self.resolved_id(path)?,
                WidgetIdTag::TableOfContentsEntry,
                index,
            )),
            _ => None,
        }
    }

    pub(crate) fn table_of_contents_accordion_focus_id(&self, path: &[usize]) -> Option<u64> {
        let Self::TableOfContents { options, .. } = self else {
            return None;
        };
        let table_id = self.resolved_id(path)?;
        options
            .accordion()
            .map(|_| resolve_table_of_contents_accordion_id(table_id))
    }

    pub(crate) fn table_of_contents_entries_visible(&self) -> bool {
        match self {
            Self::TableOfContents { options, .. } => options.is_expanded(),
            _ => false,
        }
    }

    pub(crate) fn table_of_contents_accordion(&self) -> Option<&TableOfContentsAccordion<Msg>> {
        let Self::TableOfContents { options, .. } = self else {
            return None;
        };
        options.accordion()
    }

    pub(crate) fn table_is_interactive(&self) -> bool {
        let Self::Table { model, options, .. } = self else {
            return false;
        };
        let selectable = !matches!(options.selection(), TableSelection::None);
        let sortable = options.sorting().is_some()
            && model.columns().iter().any(|column| column.is_sortable());
        selectable || sortable
    }

    pub(crate) fn search_popup_id(&self, path: &[usize]) -> Option<u64> {
        let Self::SearchBar {
            suggestions: Some(_),
            ..
        } = self
        else {
            return None;
        };
        Some(resolve_search_popup_id(self.resolved_id(path)?))
    }

    pub(crate) fn search_suggestion_id(&self, path: &[usize], index: usize) -> Option<u64> {
        let Self::SearchBar {
            suggestions: Some(suggestions),
            ..
        } = self
        else {
            return None;
        };
        let search_id = self.resolved_id(path)?;
        (index < suggestions.items.len()).then(|| resolve_search_suggestion_id(search_id, index))
    }

    pub(crate) fn dropdown_menu_popup_id(&self, widget_path: &[usize]) -> Option<u64> {
        match self {
            Self::DropdownMenu { .. } => Some(resolve_subwidget_path_id(
                self.resolved_id(widget_path)?,
                WidgetIdTag::DropdownMenuPopup,
                &[],
            )),
            _ => None,
        }
    }

    pub(crate) fn dropdown_menu_submenu_popup_id(
        &self,
        widget_path: &[usize],
        entry_path: &[usize],
    ) -> Option<u64> {
        match self {
            Self::DropdownMenu { .. } => Some(resolve_subwidget_path_id(
                self.resolved_id(widget_path)?,
                WidgetIdTag::DropdownMenuPopup,
                entry_path,
            )),
            _ => None,
        }
    }

    pub(crate) fn dropdown_menu_item_focus_id(
        &self,
        widget_path: &[usize],
        entry_path: &[usize],
    ) -> Option<u64> {
        match self {
            Self::DropdownMenu { .. } => Some(resolve_subwidget_path_id(
                self.resolved_id(widget_path)?,
                WidgetIdTag::DropdownMenuItem,
                entry_path,
            )),
            _ => None,
        }
    }

    pub(crate) fn dropdown_menu_item_paths(&self) -> Vec<Vec<usize>> {
        let Self::DropdownMenu { entries, .. } = self else {
            return Vec::new();
        };
        flatten_entry_paths(entries)
            .into_iter()
            .filter(|path| {
                !matches!(
                    entry_at_path(entries, path),
                    Some(DropdownMenuEntry::Separator)
                )
            })
            .collect()
    }

    pub(crate) fn dialog_action_focus_id(
        &self,
        path: &[usize],
        action: DialogAction,
    ) -> Option<u64> {
        let tag = match action {
            DialogAction::Confirm => WidgetIdTag::DialogConfirm,
            DialogAction::Cancel => WidgetIdTag::DialogCancel,
        };
        match self {
            Self::Dialog { .. } => Some(resolve_subwidget_id(self.resolved_id(path)?, tag, 0)),
            _ => None,
        }
    }
}
