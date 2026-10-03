// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::time::Instant;

use winit::keyboard::{Key, NamedKey};

use super::{RutterRunner, is_activation_key};
use crate::app::AppLogic;
use crate::dropdown_menu::DropdownMenuState;
use crate::engine::DropdownMenuRuntime;
use crate::i18n::LayoutDirection;
use crate::widgets::dropdown_menu::DropdownMenuEntryKind;

enum MenuKeyOutcome {
    Focus(Vec<usize>),
    Activate(Vec<usize>),
    Ignored,
}

/// Pure retained navigation used by both trigger- and pointer-anchored menus.
fn navigate_menu_key<Msg: Clone>(
    runtime: &DropdownMenuRuntime<Msg>,
    state: &mut DropdownMenuState,
    path: Vec<usize>,
    key: &Key,
    direction: LayoutDirection,
    control: bool,
) -> MenuKeyOutcome {
    let next = match key {
        Key::Named(NamedKey::ArrowDown) => runtime.adjacent_path(&path, true),
        Key::Named(NamedKey::ArrowUp) => runtime.adjacent_path(&path, false),
        Key::Named(NamedKey::Home) => runtime.boundary_path(&path, true),
        Key::Named(NamedKey::End) => runtime.boundary_path(&path, false),
        _ if is_activation_key(key) => return MenuKeyOutcome::Activate(path),
        Key::Named(NamedKey::ArrowRight | NamedKey::ArrowLeft) => {
            if dropdown_inline_forward(key, direction) {
                return if runtime.entry_kind(&path) == Some(DropdownMenuEntryKind::Submenu) {
                    MenuKeyOutcome::Activate(path)
                } else {
                    MenuKeyOutcome::Ignored
                };
            }
            if !state.collapse_submenu() {
                return MenuKeyOutcome::Ignored;
            }
            state.active_path().map(<[usize]>::to_vec)
        }
        Key::Character(text) if !control && !text.chars().all(char::is_control) => {
            let prefix = state.update_typeahead(text, Instant::now());
            runtime
                .typeahead_path(&path, prefix)
                .or_else(|| runtime.typeahead_path(&path, text))
        }
        _ => None,
    };
    next.map(MenuKeyOutcome::Focus)
        .unwrap_or(MenuKeyOutcome::Ignored)
}

fn dropdown_inline_forward(key: &Key, direction: LayoutDirection) -> bool {
    matches!(
        (key, direction),
        (Key::Named(NamedKey::ArrowRight), LayoutDirection::Ltr)
            | (Key::Named(NamedKey::ArrowLeft), LayoutDirection::Rtl)
    )
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn handle_context_menu_key(&mut self, key: &Key) -> bool {
        let Some(id) = self.engine.widget_states.iter().find_map(|(id, state)| {
            state
                .as_context_menu()
                .filter(|menu| menu.is_open)
                .map(|_| *id)
        }) else {
            return false;
        };
        if matches!(key, Key::Named(NamedKey::Escape | NamedKey::Tab)) {
            self.close_dropdown_menu(id, true);
            self.redraw();
            return !matches!(key, Key::Named(NamedKey::Tab));
        }
        let path = self
            .dropdown_state_mut(id)
            .and_then(|state| state.active_path().map(<[usize]>::to_vec))
            .or_else(|| self.dropdown_boundary_path(id, true));
        if let Some(path) = path {
            self.handle_dropdown_item_key(id, path, key);
            self.redraw();
        }
        // Open context menus own keyboard navigation even if their entries are empty.
        true
    }
    pub(super) fn handle_dropdown_key(&mut self, focus_id: u64, key: &Key) -> bool {
        let Some((parent_id, path)) = self.dropdown_focus_target(focus_id) else {
            return false;
        };
        if let Some(path) = path.as_ref() {
            let reachable = self
                .engine
                .runtime_caches
                .dropdown_menus
                .get(&parent_id)
                .is_some_and(|runtime| self.dropdown_path_is_reachable(parent_id, path, runtime));
            if !reachable {
                return false;
            }
        }
        match path {
            Some(path) => self.handle_dropdown_item_key(parent_id, path, key),
            None => self.handle_dropdown_root_key(parent_id, key),
        }
    }

    pub(super) fn dropdown_focus_target(&self, focus_id: u64) -> Option<(u64, Option<Vec<usize>>)> {
        if self
            .engine
            .runtime_caches
            .dropdown_menus
            .contains_key(&focus_id)
        {
            return Some((focus_id, None));
        }
        let item = self
            .engine
            .runtime_caches
            .dropdown_menu_items
            .get(&focus_id)?;
        Some((item.parent_id, Some(item.path.clone())))
    }

    fn handle_dropdown_root_key(&mut self, id: u64, key: &Key) -> bool {
        match key {
            _ if is_activation_key(key) => {
                self.toggle_dropdown_menu(id, false);
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.open_dropdown_from_key(id, false);
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.open_dropdown_from_key(id, true);
                true
            }
            _ => false,
        }
    }

    fn open_dropdown_from_key(&mut self, id: u64, last: bool) {
        if self.dropdown_is_open(id) {
            return;
        }
        self.close_all_dropdown_menus();
        self.engine.close_all_context_menus();
        let path = self.dropdown_boundary_path(id, !last);
        self.open_dropdown_menu(id, path);
        self.redraw();
    }

    fn handle_dropdown_item_key(&mut self, id: u64, path: Vec<usize>, key: &Key) -> bool {
        let control = self.engine.modifiers.state().control_key();
        let Some(runtime) = self.engine.runtime_caches.dropdown_menus.get(&id) else {
            return false;
        };
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(crate::engine::widget_state::WidgetState::menu_navigation_mut)
        else {
            return false;
        };
        let outcome =
            navigate_menu_key(runtime, state, path, key, A::locale().direction(), control);
        match outcome {
            MenuKeyOutcome::Focus(path) => self.focus_dropdown_path(id, path),
            MenuKeyOutcome::Activate(path) => self.activate_dropdown_entry(id, path),
            MenuKeyOutcome::Ignored => return false,
        }
        self.redraw();
        true
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/context_menu_navigation_unit_tests.rs"]
mod context_menu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ltr_dropdown_menu_inline_forward_uses_right_arrow() {
        assert!(dropdown_inline_forward(
            &Key::Named(NamedKey::ArrowRight),
            LayoutDirection::Ltr
        ));
        assert!(!dropdown_inline_forward(
            &Key::Named(NamedKey::ArrowLeft),
            LayoutDirection::Ltr
        ));
    }

    #[test]
    fn rtl_dropdown_menu_inline_forward_uses_left_arrow() {
        assert!(dropdown_inline_forward(
            &Key::Named(NamedKey::ArrowLeft),
            LayoutDirection::Rtl
        ));
        assert!(!dropdown_inline_forward(
            &Key::Named(NamedKey::ArrowRight),
            LayoutDirection::Rtl
        ));
    }
}
