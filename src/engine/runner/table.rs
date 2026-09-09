// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::keyboard::{Key, ModifiersState, NamedKey};

use super::{RutterRunner, is_activation_key};
use crate::app::AppLogic;
use crate::engine::table_runtime::{TableInteractionModifiers, TableNavigation, TableRuntime};
use crate::engine::widget_state::WidgetState;
use crate::i18n::LayoutDirection;
use crate::widgets::table::{TableCellTarget, TableRowKey};

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn activate_table_target(&mut self, id: u64, target: TableCellTarget) {
        let modifiers = table_interaction_modifiers(self.engine.modifiers.state());
        self.activate_table_target_with_modifiers(id, target, modifiers);
    }

    pub(super) fn click_table_accessibility_target(&mut self, node_id: u64) -> bool {
        let Some(route) = self
            .engine
            .runtime_caches
            .table_targets
            .get(&node_id)
            .copied()
        else {
            return false;
        };
        let id = route.parent_id;
        if !self.engine.runtime_caches.tables.contains_key(&id) {
            return false;
        }
        self.activate_table_target_with_modifiers(
            id,
            route.target,
            TableInteractionModifiers {
                toggle: false,
                range: false,
            },
        );
        true
    }

    pub(super) fn focus_table_accessibility_target(&mut self, node_id: u64) -> bool {
        let Some(route) = self
            .engine
            .runtime_caches
            .table_targets
            .get(&node_id)
            .copied()
        else {
            return false;
        };
        let Some(runtime) = self
            .engine
            .runtime_caches
            .tables
            .get(&route.parent_id)
            .cloned()
        else {
            return false;
        };
        self.focus_widget(Some(route.parent_id));
        self.set_table_active_target(route.parent_id, route.target, true, &runtime);
        self.engine.layout_dirty = true;
        true
    }

    pub(super) fn reveal_table_accessibility_target(&mut self, node_id: u64) -> bool {
        let Some(route) = self
            .engine
            .runtime_caches
            .table_targets
            .get(&node_id)
            .copied()
        else {
            return false;
        };
        let Some(runtime) = self.engine.runtime_caches.tables.get(&route.parent_id) else {
            return false;
        };
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&route.parent_id)
            .and_then(WidgetState::as_table_mut)
        else {
            return false;
        };
        let changed = runtime.reveal_target(state, route.target);
        self.engine.layout_dirty |= changed;
        changed
    }

    fn activate_table_target_with_modifiers(
        &mut self,
        id: u64,
        target: TableCellTarget,
        modifiers: TableInteractionModifiers,
    ) {
        let Some(runtime) = self.engine.runtime_caches.tables.get(&id).cloned() else {
            return;
        };
        let anchor = self.table_selection_anchor(id);
        let message = runtime.message_for_target(target, anchor, modifiers);
        self.focus_widget(Some(id));
        self.set_table_active_target(id, target, modifiers.range, &runtime);
        if let Some(message) = message {
            A::update(
                &mut self.engine.app_state,
                message,
                &mut self.engine.clipboard,
            );
        }
        self.engine.layout_dirty = true;
    }

    pub(super) fn handle_table_key(&mut self, id: u64, key: &Key) -> bool {
        let Some(runtime) = self.engine.runtime_caches.tables.get(&id).cloned() else {
            return false;
        };
        if let Some(navigation) = table_navigation(key) {
            return self.navigate_table(id, navigation, &runtime);
        }
        if !is_activation_key(key) {
            return false;
        }
        self.activate_table_active_target(id, key, &runtime)
    }

    fn navigate_table(
        &mut self,
        id: u64,
        navigation: TableNavigation,
        runtime: &TableRuntime<A::Message>,
    ) -> bool {
        let active = self
            .engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_table)
            .and_then(|state| state.active);
        let modifiers = self.engine.modifiers.state();
        let target = runtime.move_target(
            active,
            navigation,
            A::locale().direction() == LayoutDirection::Rtl,
            primary_table_modifier(modifiers),
        );
        let Some(target) = target else { return false };
        self.set_table_active_target(id, target, false, runtime);
        self.engine.layout_dirty = true;
        self.redraw();
        true
    }

    fn activate_table_active_target(
        &mut self,
        id: u64,
        key: &Key,
        runtime: &TableRuntime<A::Message>,
    ) -> bool {
        let Some(target) = self
            .engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_table)
            .and_then(|state| state.active)
        else {
            return false;
        };
        if target.row.is_some() && matches!(key, Key::Named(NamedKey::Enter)) {
            return false;
        }
        let modifiers = table_interaction_modifiers(self.engine.modifiers.state());
        let anchor = self.table_selection_anchor(id);
        let Some(message) = runtime.message_for_target(target, anchor, modifiers) else {
            return false;
        };
        self.set_table_active_target(id, target, modifiers.range, runtime);
        A::update(
            &mut self.engine.app_state,
            message,
            &mut self.engine.clipboard,
        );
        self.engine.layout_dirty = true;
        self.redraw();
        true
    }

    fn set_table_active_target(
        &mut self,
        id: u64,
        target: TableCellTarget,
        preserve_anchor: bool,
        runtime: &TableRuntime<A::Message>,
    ) {
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_table_mut)
        else {
            return;
        };
        state.active = Some(target);
        runtime.reveal_target(state, target);
        if let Some(row) = target.row
            && (!preserve_anchor || state.selection_anchor.is_none())
        {
            state.selection_anchor = Some(row);
        }
    }

    fn table_selection_anchor(&self, id: u64) -> Option<TableRowKey> {
        self.engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_table)
            .and_then(|state| state.selection_anchor)
    }

    pub(super) fn scroll_table(&mut self, id: u64, delta_x: f32, delta_y: f32) -> bool {
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_table_mut)
        else {
            return false;
        };
        let mut horizontal = match A::locale().direction() {
            LayoutDirection::Ltr => delta_x,
            LayoutDirection::Rtl => -delta_x,
        };
        if horizontal.abs() <= f32::EPSILON && state.viewport.max_scroll_y() <= f32::EPSILON {
            horizontal = delta_y;
        }
        state.scroll_by(horizontal, delta_y)
    }
}

fn table_navigation(key: &Key) -> Option<TableNavigation> {
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(TableNavigation::Left),
        Key::Named(NamedKey::ArrowRight) => Some(TableNavigation::Right),
        Key::Named(NamedKey::ArrowUp) => Some(TableNavigation::Up),
        Key::Named(NamedKey::ArrowDown) => Some(TableNavigation::Down),
        Key::Named(NamedKey::Home) => Some(TableNavigation::Home),
        Key::Named(NamedKey::End) => Some(TableNavigation::End),
        Key::Named(NamedKey::PageUp) => Some(TableNavigation::PageUp),
        Key::Named(NamedKey::PageDown) => Some(TableNavigation::PageDown),
        _ => None,
    }
}

fn table_interaction_modifiers(modifiers: ModifiersState) -> TableInteractionModifiers {
    TableInteractionModifiers {
        toggle: primary_table_modifier(modifiers),
        range: modifiers.shift_key(),
    }
}

fn primary_table_modifier(modifiers: ModifiersState) -> bool {
    if cfg!(target_os = "macos") {
        return modifiers.super_key();
    }
    modifiers.control_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_mapping_covers_two_axis_table_navigation() {
        assert_eq!(
            table_navigation(&Key::Named(NamedKey::PageDown)),
            Some(TableNavigation::PageDown)
        );
        assert_eq!(table_navigation(&Key::Character("x".into())), None);
    }
}
