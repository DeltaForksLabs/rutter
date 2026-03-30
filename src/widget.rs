// ============================================================
// Rutter Framework — widget.rs
// ============================================================

use skia_safe::Color as SkiaColor;
use taffy::prelude::Style;

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
