// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — widget.rs
// ============================================================

use std::fmt;

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

use crate::widget_id::{AUTOMATIC_ID_NAMESPACE_BIT, WidgetId, WidgetIdError};

/// Sentinel reservado para IDs gerados automaticamente a partir do caminho da
/// árvore. IDs manuais seguros devem ser criados com [`WidgetId::manual`].
pub const AUTO_ID: u64 = 0;

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

    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    hash ^= tag as u64;
    hash = hash.wrapping_mul(FNV_PRIME);

    for &segment in path {
        hash ^= (segment as u64).wrapping_add(1);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let resolved = hash | AUTOMATIC_ID_NAMESPACE_BIT;
    if resolved == AUTO_ID {
        AUTOMATIC_ID_NAMESPACE_BIT
    } else {
        resolved
    }
}

pub(crate) fn resolve_subwidget_id(base_id: u64, tag: WidgetIdTag, slot: usize) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    hash ^= tag as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= base_id;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= (slot as u64).wrapping_add(1);
    hash = hash.wrapping_mul(FNV_PRIME);

    let resolved = hash | AUTOMATIC_ID_NAMESPACE_BIT;
    if resolved == AUTO_ID {
        AUTOMATIC_ID_NAMESPACE_BIT
    } else {
        resolved
    }
}

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
    StrongText {
        content: String,
        style: Style,
        color: Option<SkiaColor>,
        size: f32,
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
    Popover {
        id: u64,
        open: bool,
        anchor: Box<Widget<'a, Msg>>,
        content: Box<Widget<'a, Msg>>,
        on_dismiss: Option<Msg>,
        style: Style,
        popup_style: Style,
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
}

impl<'a, Msg> Widget<'a, Msg> {
    /// Creates text rendered with an explicitly emboldened font.
    ///
    /// ```rust
    /// use rutter::Widget;
    /// use taffy::prelude::Style;
    ///
    /// let text: Widget<'_, ()> = Widget::strong_text("2026", Style::default(), None, 16.0);
    /// assert!(matches!(text, Widget::StrongText { .. }));
    /// ```
    pub fn strong_text(
        content: impl Into<String>,
        style: Style,
        color: Option<SkiaColor>,
        size: f32,
    ) -> Self {
        Self::StrongText {
            content: content.into(),
            style,
            color,
            size,
        }
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

    /// Creates a virtualized list whose visible rows are rendered from widgets.
    /// Item widgets are visual-only and receive isolated runtime-state maps so
    /// on-demand IDs cannot alias controls in the application tree.
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

    /// Creates a virtualized grid whose visible cells are rendered from widgets.
    /// Cell widgets are visual-only and receive isolated runtime-state maps so
    /// on-demand IDs cannot alias controls in the application tree.
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
            | Self::Select { id: slot, .. }
            | Self::ProgressBar { id: slot, .. }
            | Self::Spinner { id: slot, .. }
            | Self::ScrollView { id: slot, .. }
            | Self::Accordion { id: slot, .. }
            | Self::TabBar { id: slot, .. }
            | Self::Modal { id: slot, .. }
            | Self::Dialog { id: slot, .. }
            | Self::Toast { id: slot, .. }
            | Self::ContextMenu { id: slot, .. }
            | Self::Popover { id: slot, .. }
            | Self::VirtualList { id: slot, .. }
            | Self::VirtualListContent { id: slot, .. }
            | Self::VirtualGrid { id: slot, .. }
            | Self::VirtualGridContent { id: slot, .. } => {
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
            Self::Select { id, .. } => (Some(*id), WidgetIdTag::Select, "Select"),
            Self::ProgressBar { id, .. } => (Some(*id), WidgetIdTag::ProgressBar, "ProgressBar"),
            Self::Spinner { id, .. } => (Some(*id), WidgetIdTag::Spinner, "Spinner"),
            Self::ScrollView { id, .. } => (Some(*id), WidgetIdTag::ScrollView, "ScrollView"),
            Self::Accordion { id, .. } => (Some(*id), WidgetIdTag::Accordion, "Accordion"),
            Self::TabBar { id, .. } => (Some(*id), WidgetIdTag::TabBar, "TabBar"),
            Self::Modal { id, .. } => (Some(*id), WidgetIdTag::Modal, "Modal"),
            Self::Dialog { id, .. } => (Some(*id), WidgetIdTag::Dialog, "Dialog"),
            Self::Toast { id, .. } => (Some(*id), WidgetIdTag::Toast, "Toast"),
            Self::ContextMenu { id, .. } => (Some(*id), WidgetIdTag::ContextMenu, "ContextMenu"),
            Self::Popover { id, .. } => (Some(*id), WidgetIdTag::Popover, "Popover"),
            Self::VirtualList { id, .. } | Self::VirtualListContent { id, .. } => {
                (Some(*id), WidgetIdTag::VirtualList, "VirtualList")
            }
            Self::VirtualGrid { id, .. } | Self::VirtualGridContent { id, .. } => {
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
            | Self::Select { .. }
            | Self::Accordion { .. }
            | Self::TabBar { .. }
            | Self::VirtualList { .. }
            | Self::VirtualListContent { .. }
            | Self::VirtualGrid { .. }
            | Self::VirtualGridContent { .. } => self.resolved_id(path),
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
