// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    AppLogic, ContextMenuEntry, RutterRunner, ShortcutEvent, ShortcutKey, ShortcutNamedKey,
    ShortcutOutcome, Theme, Widget,
};
use taffy::prelude::*;

use super::{
    layout::responsive_width,
    theme_selector::{ExampleTheme, example_theme_selector},
};

const CONTEXT_MENU_ID: u64 = 930;
const COPY_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" stroke="#5086E7" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="12" height="12" rx="2"/><path d="M15 9V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h4"/></svg>"##;
const FOLDER_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" stroke="#5086E7" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7V5h7l2 3h9v12H3V7Z"/></svg>"##;
const EDIT_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" stroke="#5086E7" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m4 16 12-12 4 4L8 20H4v-4ZM13 7l4 4"/></svg>"##;
const PASTE_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" stroke="#5086E7" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="5" width="14" height="16" rx="2"/><rect x="9" y="3" width="6" height="4" rx="1"/><path d="M9 12h6M9 16h6"/></svg>"##;
const DELETE_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="none" stroke="#DC6570" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7"/></svg>"##;

pub struct ContextMenuDemoState {
    theme: ExampleTheme,
    last_action: String,
    // ContextMenu borrows its entries; retain them across view reconstruction.
    entries: Vec<ContextMenuEntry<'static, Msg>>,
}

impl Default for ContextMenuDemoState {
    fn default() -> Self {
        Self {
            theme: ExampleTheme::Dark,
            last_action: "No action selected yet.".into(),
            entries: menu_entries(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    Action(&'static str),
}

pub struct ContextMenuDemo;

impl AppLogic for ContextMenuDemo {
    type State = ContextMenuDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        ContextMenuDemoState::default()
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Msg> {
        Widget::Column {
            children: demo_children(state),
            style: root_style(),
        }
    }

    fn update(state: &mut Self::State, message: Msg, _: &mut Clipboard) {
        apply_demo_message(state, message);
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }

    fn shortcut(_: &Self::State, event: ShortcutEvent) -> ShortcutOutcome<Msg> {
        demo_shortcut(&event)
    }
}

fn demo_children(state: &ContextMenuDemoState) -> Vec<Widget<'_, Msg>> {
    vec![
        example_theme_selector(state.theme, Msg::ThemeChanged),
        demo_text("ContextMenu", 24.0),
        demo_text("Right-click the highlighted area to open the menu.", 14.0),
        Widget::context_menu(
            context_area(&state.theme.resolve()),
            &state.entries,
            responsive_width(560.0, Dimension::auto()),
        )
        .with_id(CONTEXT_MENU_ID),
        demo_text(format!("Last action: {}", state.last_action), 14.0),
        demo_text(
            "Try Organize > Move to > Projects. Actions only update the text above; no files are changed.",
            13.0,
        ),
        demo_text(
            "Shortcuts: CTRL+C Copy, CTRL+O Open, F2 Rename, DELETE Delete. Paste stays disabled.",
            13.0,
        ),
        demo_text(
            "Once open: arrows navigate, Enter/Space select, Left/Right close/open submenus, Escape or an outside click dismisses.",
            13.0,
        ),
    ]
}

fn menu_entries() -> Vec<ContextMenuEntry<'static, Msg>> {
    vec![
        ContextMenuEntry::item("Open", Msg::Action("Open"))
            .with_svg_icon(FOLDER_ICON_SVG)
            .with_shortcut_label("CTRL+O"),
        ContextMenuEntry::item("Copy", Msg::Action("Copy"))
            .with_svg_icon(COPY_ICON_SVG)
            .with_shortcut_label("CTRL+C"),
        ContextMenuEntry::submenu(
            "Open with",
            vec![
                ContextMenuEntry::item("Text editor", Msg::Action("Open with text editor")),
                ContextMenuEntry::item("Viewer", Msg::Action("Open with viewer")),
            ],
        )
        .with_svg_icon(FOLDER_ICON_SVG),
        ContextMenuEntry::submenu(
            "Organize",
            vec![
                ContextMenuEntry::submenu(
                    "Move to",
                    vec![
                        ContextMenuEntry::item("Projects", Msg::Action("Move to Projects"))
                            .with_svg_icon(FOLDER_ICON_SVG),
                        ContextMenuEntry::item("Archive", Msg::Action("Move to Archive"))
                            .with_svg_icon(FOLDER_ICON_SVG),
                    ],
                )
                .with_svg_icon(FOLDER_ICON_SVG),
                ContextMenuEntry::item("Mark as favorite", Msg::Action("Mark as favorite")),
                ContextMenuEntry::disabled_submenu(
                    "Sync (offline)",
                    vec![ContextMenuEntry::item(
                        "Cloud",
                        Msg::Action("Sync to cloud"),
                    )],
                ),
            ],
        )
        .with_svg_icon(FOLDER_ICON_SVG),
        ContextMenuEntry::separator(),
        ContextMenuEntry::item("Rename", Msg::Action("Rename"))
            .with_svg_icon(EDIT_ICON_SVG)
            .with_shortcut_label("F2"),
        ContextMenuEntry::disabled("Paste (unavailable)")
            .with_svg_icon(PASTE_ICON_SVG)
            .with_shortcut_label("CTRL+V"),
        ContextMenuEntry::item("Delete", Msg::Action("Delete"))
            .with_svg_icon(DELETE_ICON_SVG)
            .with_shortcut_label("DELETE"),
    ]
}

fn apply_demo_message(state: &mut ContextMenuDemoState, message: Msg) {
    match message {
        Msg::ThemeChanged(theme) => state.theme = theme,
        Msg::Action(action) => state.last_action = format!("Selected: {action}"),
    }
}

fn demo_shortcut(event: &ShortcutEvent) -> ShortcutOutcome<Msg> {
    if event.alt || event.shift || event.super_key {
        return ShortcutOutcome::Ignored;
    }
    let action = match (&event.key, event.control) {
        (ShortcutKey::Character(key), true) if key.eq_ignore_ascii_case("c") => "Copy",
        (ShortcutKey::Character(key), true) if key.eq_ignore_ascii_case("o") => "Open",
        (ShortcutKey::Named(ShortcutNamedKey::Function(2)), false) => "Rename",
        (ShortcutKey::Named(ShortcutNamedKey::Delete), false) => "Delete",
        _ => return ShortcutOutcome::Ignored,
    };
    if event.repeat {
        return ShortcutOutcome::Consumed;
    }
    ShortcutOutcome::Message(Msg::Action(action))
}

fn context_area<'a>(theme: &Theme) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Column {
            children: vec![
                demo_text("Right-click here", 20.0),
                demo_text("Submenus, SVG icons and shortcut labels", 14.0),
            ],
            style: Style {
                size: Size::percent(1.0_f32),
                flex_direction: FlexDirection::Column,
                justify_content: Some(JustifyContent::Center),
                gap: Size::length(12.0_f32),
                ..Style::default()
            },
        }),
        style: Style {
            padding: Rect::length(24.0_f32),
            ..responsive_width(560.0, Dimension::length(200.0))
        },
        color: Some(Theme::alpha(theme.primary, 28)),
        radius: theme.radius_md,
    }
}

fn demo_text<'a>(content: impl Into<String>, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: responsive_width(560.0, Dimension::auto()),
        color: None,
        size,
    }
}

fn root_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FlexStart),
        size: Size::percent(1.0_f32),
        padding: Rect::length(32.0_f32),
        gap: Size::length(16.0_f32),
        ..Style::default()
    }
}

pub fn run() {
    RutterRunner::<ContextMenuDemo>::run();
}

#[cfg(test)]
#[path = "../../tests/unit/context_menu_demo_unit_tests.rs"]
mod tests;
