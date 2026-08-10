// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::keyboard::{Key, NamedKey};

use super::RutterRunner;
use crate::app::AppLogic;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(crate) fn process_accessibility_actions(&mut self) {
        let requests = self.engine.take_accessibility_actions();
        if requests.is_empty() {
            return;
        }
        let Some(size) = self
            .engine
            .window
            .as_ref()
            .map(|window| window.inner_size())
        else {
            return;
        };
        let refreshed = self
            .engine
            .try_ensure_widget_states()
            .and_then(|()| self.engine.try_ensure_layout(size));
        if let Err(error) = refreshed {
            self.fatal_error.get_or_insert_with(|| error.into());
            return;
        }
        if self.handle_accessibility_actions(requests) {
            self.redraw();
        }
    }

    fn handle_accessibility_actions(&mut self, requests: Vec<accesskit::ActionRequest>) -> bool {
        let mut changed = false;
        for request in requests {
            if request.target_tree != accesskit::TreeId::ROOT {
                continue;
            }
            changed |= self.handle_accessibility_action(request.action, request.target_node.0);
        }
        changed
    }

    fn handle_accessibility_action(&mut self, action: accesskit::Action, target: u64) -> bool {
        match action {
            accesskit::Action::Focus => self.focus_accessibility_target(target),
            accesskit::Action::Click => self.click_accessibility_target(target),
            accesskit::Action::Expand => self.expand_accessibility_target(target),
            accesskit::Action::Collapse => self.collapse_accessibility_target(target),
            accesskit::Action::Increment => self.adjust_accessibility_target(target, true),
            accesskit::Action::Decrement => self.adjust_accessibility_target(target, false),
            _ => false,
        }
    }

    fn focus_accessibility_target(&mut self, target: u64) -> bool {
        if let Some((id, Some(path))) = self.dropdown_focus_target(target) {
            let reachable = self
                .engine
                .runtime_caches
                .dropdown_menus
                .get(&id)
                .is_some_and(|runtime| self.dropdown_path_is_reachable(id, &path, runtime));
            if !reachable {
                return false;
            }
            self.focus_dropdown_path(id, path);
            return true;
        }
        if !self.accessibility_focus_id_is_live(target) {
            return false;
        }
        self.focus_widget(Some(target));
        true
    }

    fn accessibility_focus_id_is_live(&self, target: u64) -> bool {
        if self
            .engine
            .runtime_caches
            .dropdown_menus
            .contains_key(&target)
        {
            return self
                .engine
                .runtime_caches
                .visible_dropdown_triggers
                .contains(&target);
        }
        if self.engine.runtime_caches.focus_order.contains(&target) {
            return true;
        }
        self.engine
            .runtime_caches
            .dropdown_menu_items
            .get(&target)
            .is_some_and(|item| {
                self.engine
                    .runtime_caches
                    .visible_dropdown_menus
                    .contains(&item.parent_id)
            })
    }

    fn click_accessibility_target(&mut self, target: u64) -> bool {
        if let Some((id, path)) = self.dropdown_focus_target(target) {
            if path.is_none()
                && !self
                    .engine
                    .runtime_caches
                    .visible_dropdown_triggers
                    .contains(&id)
            {
                return false;
            }
            if path.is_some() && !self.dropdown_is_open(id) {
                return false;
            }
            match path {
                Some(path) => self.activate_dropdown_entry(id, path),
                None => self.toggle_dropdown_menu(id, false),
            }
            return true;
        }
        if !self.focus_accessibility_target(target) {
            return false;
        }
        self.handle_focused_widget_key(&Key::Named(NamedKey::Enter))
    }

    fn expand_accessibility_target(&mut self, target: u64) -> bool {
        let Some((id, path)) = self.dropdown_focus_target(target) else {
            return self.click_accessibility_target(target);
        };
        if path.is_none()
            && !self
                .engine
                .runtime_caches
                .visible_dropdown_triggers
                .contains(&id)
        {
            return false;
        }
        if let Some(path) = path.as_ref() {
            if !self.dropdown_submenu_can_expand(id, path) {
                return false;
            }
        }
        match path {
            Some(path) => self.activate_dropdown_entry(id, path),
            None if !self.dropdown_is_open(id) => self.toggle_dropdown_menu(id, false),
            None => {}
        }
        true
    }

    fn dropdown_submenu_can_expand(&self, id: u64, path: &[usize]) -> bool {
        let Some(runtime) = self.engine.runtime_caches.dropdown_menus.get(&id) else {
            return false;
        };
        if !self.dropdown_path_is_reachable(id, path, runtime) || runtime.is_disabled(path) {
            return false;
        }
        if runtime.entry_kind(path)
            != Some(crate::widgets::dropdown_menu::DropdownMenuEntryKind::Submenu)
        {
            return false;
        }
        self.engine
            .widget_states
            .get(&id)
            .and_then(crate::engine::widget_state::WidgetState::as_dropdown_menu)
            .is_some_and(|state| !state.open_submenu_path().starts_with(path))
    }

    fn collapse_accessibility_target(&mut self, target: u64) -> bool {
        let Some((id, path)) = self.dropdown_focus_target(target) else {
            return self.click_accessibility_target(target);
        };
        if path.is_none()
            && !self
                .engine
                .runtime_caches
                .visible_dropdown_triggers
                .contains(&id)
        {
            return false;
        }
        if let Some(path) = path {
            if !self.dropdown_submenu_can_collapse(id, &path) {
                return false;
            }
            let collapsed = self
                .dropdown_state_mut(id)
                .is_some_and(|state| state.collapse_to_submenu(path.clone()));
            if collapsed {
                self.focus_dropdown_path(id, path);
            }
        } else if self.dropdown_is_open(id) {
            self.close_dropdown_menu(id, true);
        } else {
            return false;
        }
        true
    }

    fn dropdown_submenu_can_collapse(&self, id: u64, path: &[usize]) -> bool {
        let Some(runtime) = self.engine.runtime_caches.dropdown_menus.get(&id) else {
            return false;
        };
        if !self.dropdown_path_is_reachable(id, path, runtime) {
            return false;
        }
        runtime.entry_kind(path)
            == Some(crate::widgets::dropdown_menu::DropdownMenuEntryKind::Submenu)
            && self
                .engine
                .widget_states
                .get(&id)
                .and_then(crate::engine::widget_state::WidgetState::as_dropdown_menu)
                .is_some_and(|state| state.open_submenu_path().starts_with(path))
    }

    fn adjust_accessibility_target(&mut self, target: u64, increment: bool) -> bool {
        if !self.focus_accessibility_target(target) {
            return false;
        }
        let key = if increment {
            NamedKey::ArrowRight
        } else {
            NamedKey::ArrowLeft
        };
        self.handle_focused_widget_key(&Key::Named(key))
    }
}
