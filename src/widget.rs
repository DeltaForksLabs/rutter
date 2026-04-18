// ============================================================
// Rutter Framework — widget.rs
// ============================================================

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

/// Sentinel reservado para IDs gerados automaticamente a partir do caminho da
/// árvore. Qualquer valor diferente de zero continua sendo tratado como ID
/// manual estável.
pub const AUTO_ID: u64 = 0;

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
    const AUTO_MASK: u64 = 1 << 63;

    let mut hash = FNV_OFFSET;
    hash ^= tag as u64;
    hash = hash.wrapping_mul(FNV_PRIME);

    for &segment in path {
        hash ^= (segment as u64).wrapping_add(1);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let resolved = hash | AUTO_MASK;
    if resolved == AUTO_ID {
        AUTO_MASK
    } else {
        resolved
    }
}

pub(crate) fn resolve_subwidget_id(base_id: u64, tag: WidgetIdTag, slot: usize) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    const AUTO_MASK: u64 = 1 << 63;

    let mut hash = FNV_OFFSET;
    hash ^= tag as u64;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= base_id;
    hash = hash.wrapping_mul(FNV_PRIME);
    hash ^= (slot as u64).wrapping_add(1);
    hash = hash.wrapping_mul(FNV_PRIME);

    let resolved = hash | AUTO_MASK;
    if resolved == AUTO_ID {
        AUTO_MASK
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
    VirtualList {
        id: u64,
        item_height: f32,
        item_count: usize,
        items: &'a dyn Fn(usize) -> Option<String>,
        on_select: fn(usize) -> Msg,
        style: Style,
    },
}

impl<'a, Msg> Widget<'a, Msg> {
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
            style,
            child: Box::new(child),
        }
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

    pub fn with_id(mut self, id: u64) -> Self {
        match &mut self {
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
            | Self::VirtualList { id: slot, .. } => *slot = id,
            _ => {}
        }
        self
    }

    pub fn with_auto_id(self) -> Self {
        self.with_id(AUTO_ID)
    }

    pub(crate) fn resolved_id(&self, path: &[usize]) -> Option<u64> {
        match self {
            Self::TextInput { id, .. } => {
                Some(resolve_widget_id(*id, WidgetIdTag::TextInput, path))
            }
            Self::TextArea { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::TextArea, path)),
            Self::SearchBar { id, .. } => {
                Some(resolve_widget_id(*id, WidgetIdTag::SearchBar, path))
            }
            Self::Slider { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::Slider, path)),
            Self::Select { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::Select, path)),
            Self::ProgressBar { id, .. } => {
                Some(resolve_widget_id(*id, WidgetIdTag::ProgressBar, path))
            }
            Self::Spinner { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::Spinner, path)),
            Self::ScrollView { id, .. } => {
                Some(resolve_widget_id(*id, WidgetIdTag::ScrollView, path))
            }
            Self::Accordion { id, .. } => {
                Some(resolve_widget_id(*id, WidgetIdTag::Accordion, path))
            }
            Self::TabBar { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::TabBar, path)),
            Self::Modal { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::Modal, path)),
            Self::Dialog { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::Dialog, path)),
            Self::Toast { id, .. } => Some(resolve_widget_id(*id, WidgetIdTag::Toast, path)),
            Self::VirtualList { id, .. } => {
                Some(resolve_widget_id(*id, WidgetIdTag::VirtualList, path))
            }
            _ => None,
        }
    }

    pub(crate) fn keyboard_focus_id(&self, path: &[usize]) -> Option<u64> {
        match self {
            Self::Button { .. } => Some(resolve_widget_id(AUTO_ID, WidgetIdTag::Button, path)),
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
            | Self::VirtualList { .. } => self.resolved_id(path),
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
