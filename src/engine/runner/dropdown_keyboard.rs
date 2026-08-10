// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::time::Instant;

use winit::keyboard::{Key, NamedKey};

use super::{RutterRunner, is_activation_key};
use crate::app::AppLogic;
use crate::i18n::LayoutDirection;
use crate::widgets::dropdown_menu::DropdownMenuEntryKind;

fn dropdown_inline_forward(key: &Key, direction: LayoutDirection) -> bool {
    matches!(
        (key, direction),
        (Key::Named(NamedKey::ArrowRight), LayoutDirection::Ltr)
            | (Key::Named(NamedKey::ArrowLeft), LayoutDirection::Rtl)
    )
}

impl<A: AppLogic + 'static> RutterRunner<A> {
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
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
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
        match key {
            Key::Named(NamedKey::ArrowDown) => self.move_dropdown_adjacent(id, path, true),
            Key::Named(NamedKey::ArrowUp) => self.move_dropdown_adjacent(id, path, false),
            Key::Named(NamedKey::Home) => self.move_dropdown_boundary(id, path, true),
            Key::Named(NamedKey::End) => self.move_dropdown_boundary(id, path, false),
            _ if is_activation_key(key) => {
                self.activate_dropdown_entry(id, path);
                true
            }
            Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowLeft) => {
                self.handle_dropdown_inline_key(id, path, key)
            }
            Key::Character(text) => self.handle_dropdown_typeahead(id, path, text),
            _ => false,
        }
    }

    fn move_dropdown_adjacent(&mut self, id: u64, path: Vec<usize>, forward: bool) -> bool {
        let next = self
            .engine
            .runtime_caches
            .dropdown_menus
            .get(&id)
            .and_then(|runtime| runtime.adjacent_path(&path, forward));
        self.focus_optional_dropdown_path(id, next)
    }

    fn move_dropdown_boundary(&mut self, id: u64, path: Vec<usize>, first: bool) -> bool {
        let next = self
            .engine
            .runtime_caches
            .dropdown_menus
            .get(&id)
            .and_then(|runtime| runtime.boundary_path(&path, first));
        self.focus_optional_dropdown_path(id, next)
    }

    fn focus_optional_dropdown_path(&mut self, id: u64, path: Option<Vec<usize>>) -> bool {
        let Some(path) = path else { return false };
        self.focus_dropdown_path(id, path);
        self.redraw();
        true
    }

    fn handle_dropdown_inline_key(&mut self, id: u64, path: Vec<usize>, key: &Key) -> bool {
        let forward = dropdown_inline_forward(key, A::locale().direction());
        if forward {
            let is_submenu = self
                .engine
                .runtime_caches
                .dropdown_menus
                .get(&id)
                .is_some_and(|runtime| {
                    runtime.entry_kind(&path) == Some(DropdownMenuEntryKind::Submenu)
                });
            if !is_submenu {
                return false;
            }
            self.activate_dropdown_entry(id, path);
            self.redraw();
            return true;
        }
        self.collapse_dropdown_submenu(id)
    }

    fn collapse_dropdown_submenu(&mut self, id: u64) -> bool {
        let parent_path = self.dropdown_state_mut(id).and_then(|state| {
            if !state.collapse_submenu() {
                return None;
            }
            state.active_path().map(<[usize]>::to_vec)
        });
        let Some(parent_path) = parent_path else {
            return false;
        };
        self.focus_dropdown_path(id, parent_path);
        self.redraw();
        true
    }

    fn handle_dropdown_typeahead(&mut self, id: u64, path: Vec<usize>, text: &str) -> bool {
        if text.chars().all(char::is_control) || self.engine.modifiers.state().control_key() {
            return false;
        }
        let prefix = self.dropdown_typeahead_prefix(id, text);
        let next = self
            .engine
            .runtime_caches
            .dropdown_menus
            .get(&id)
            .and_then(|runtime| {
                runtime
                    .typeahead_path(&path, &prefix)
                    .or_else(|| runtime.typeahead_path(&path, text))
            });
        self.focus_optional_dropdown_path(id, next)
    }

    fn dropdown_typeahead_prefix(&mut self, id: u64, text: &str) -> String {
        self.dropdown_state_mut(id)
            .map(|state| state.update_typeahead(text, Instant::now()).to_owned())
            .unwrap_or_default()
    }
}

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
