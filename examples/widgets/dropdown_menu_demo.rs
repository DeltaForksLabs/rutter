// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{AppLogic, DropdownMenuEntry, RutterRunner, Theme, Widget};
use taffy::prelude::*;

use super::theme_selector::{ExampleTheme, example_theme_selector};

pub struct DropdownMenuDemoState {
    pub theme: ExampleTheme,
    pub line_numbers: bool,
    pub density: &'static str,
    pub last_action: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ThemeChanged(ExampleTheme),
    Action(&'static str),
    ToggleLineNumbers,
    SetDensity(&'static str),
}

pub struct DropdownMenuDemo;

impl AppLogic for DropdownMenuDemo {
    type State = DropdownMenuDemoState;
    type Message = Msg;

    fn new(_: &mut FontSystem) -> Self::State {
        DropdownMenuDemoState {
            theme: ExampleTheme::Dark,
            line_numbers: true,
            density: "Comfortable",
            last_action: "Open the menu with mouse, Enter, Space, Arrow Down, or Arrow Up.".into(),
        }
    }

    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message> {
        Widget::Column {
            children: demo_children(state),
            style: root_style(),
        }
    }

    fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
        match message {
            Msg::ThemeChanged(theme) => state.theme = theme,
            Msg::Action(action) => state.last_action = format!("Activated: {action}"),
            Msg::ToggleLineNumbers => toggle_line_numbers(state),
            Msg::SetDensity(density) => set_density(state, density),
        }
    }

    fn theme_for(state: &Self::State) -> Theme {
        state.theme.resolve()
    }
}

fn demo_children(state: &DropdownMenuDemoState) -> Vec<Widget<'_, Msg>> {
    vec![
        example_theme_selector(state.theme, Msg::ThemeChanged),
        heading("Accessible DropdownMenu", 24.0),
        heading(
            "Commands, disabled items, checkbox/radio state, scrolling, typeahead, and nested submenus.",
            14.0,
        ),
        Widget::dropdown_menu("Project actions", menu_entries(state), trigger_style()).with_id(920),
        heading(&state.last_action, 14.0),
        heading(
            "Keyboard: arrows/Home/End navigate, Enter/Space activate, inline arrow opens/closes submenus, Escape restores trigger focus.",
            13.0,
        ),
    ]
}

fn menu_entries(state: &DropdownMenuDemoState) -> Vec<DropdownMenuEntry<'_, Msg>> {
    vec![
        DropdownMenuEntry::item("New file", Msg::Action("New file")),
        DropdownMenuEntry::item("Open workspace", Msg::Action("Open workspace")),
        DropdownMenuEntry::disabled_item("Publish (permission required)"),
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::checkbox(
            "Show line numbers",
            state.line_numbers,
            Msg::ToggleLineNumbers,
        ),
        density_submenu(state.density),
        export_submenu(),
        recent_files_submenu(),
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::item("Close project", Msg::Action("Close project")),
    ]
}

fn density_submenu(selected: &str) -> DropdownMenuEntry<'_, Msg> {
    DropdownMenuEntry::submenu(
        "Editor density",
        vec![
            density_entry("Compact", selected),
            density_entry("Comfortable", selected),
            density_entry("Spacious", selected),
        ],
    )
}

fn density_entry(label: &'static str, selected: &str) -> DropdownMenuEntry<'static, Msg> {
    DropdownMenuEntry::radio(label, label == selected, Msg::SetDensity(label))
}

fn export_submenu() -> DropdownMenuEntry<'static, Msg> {
    DropdownMenuEntry::submenu(
        "Export",
        vec![
            DropdownMenuEntry::item("JSON", Msg::Action("Export JSON")),
            DropdownMenuEntry::item("Markdown", Msg::Action("Export Markdown")),
            DropdownMenuEntry::disabled_item("PDF (not installed)"),
            DropdownMenuEntry::submenu(
                "Advanced",
                vec![
                    DropdownMenuEntry::item("Archive", Msg::Action("Export archive")),
                    DropdownMenuEntry::item("Signed bundle", Msg::Action("Export signed bundle")),
                ],
            ),
        ],
    )
}

fn recent_files_submenu() -> DropdownMenuEntry<'static, Msg> {
    let names = [
        "main.rs",
        "lib.rs",
        "theme.rs",
        "widget.rs",
        "layout.rs",
        "runner.rs",
        "mod.rs",
        "README.md",
        "Cargo.toml",
        "DEVLOG.md",
        "accessibility.rs",
        "render.rs",
    ];
    let entries = names
        .into_iter()
        .map(|name| DropdownMenuEntry::item(name, Msg::Action(name)))
        .collect();
    DropdownMenuEntry::submenu("Recent files", entries)
}

fn toggle_line_numbers(state: &mut DropdownMenuDemoState) {
    state.line_numbers = !state.line_numbers;
    state.last_action = format!(
        "Line numbers: {}",
        if state.line_numbers {
            "shown"
        } else {
            "hidden"
        }
    );
}

fn set_density(state: &mut DropdownMenuDemoState, density: &'static str) {
    state.density = density;
    state.last_action = format!("Editor density: {density}");
}

fn heading<'a>(content: &str, size: f32) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.to_owned(),
        style: Style::default(),
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
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(16.0),
        },
        ..Style::default()
    }
}

fn trigger_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(240.0),
            height: Dimension::length(42.0),
        },
        ..Style::default()
    }
}

pub fn run() {
    RutterRunner::<DropdownMenuDemo>::run();
}

#[cfg(test)]
#[path = "../../tests/unit/dropdown_menu_demo_unit_tests.rs"]
mod tests;
