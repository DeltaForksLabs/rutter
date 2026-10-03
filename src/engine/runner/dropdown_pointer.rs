// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::dpi::PhysicalSize;
use winit::event::MouseButton;

use super::super::{DropdownMenuRuntime, validate_runtime_reconstruction};
use super::RutterRunner;
use crate::app::AppLogic;
use crate::engine::dropdown_menu_runtime::MenuHoverOutcome;
use crate::engine::run_error::RutterRunError;
use crate::engine::widget_state::WidgetState;
use crate::render::dropdown_menu_overlay::{
    DropdownMenuOverlayHit, DropdownMenuScrollTarget, dropdown_menu_entry_hover_at,
    dropdown_menu_scroll_target_at,
};
use crate::render::hit_test::ContextMenuOverlayHit;
use crate::render::select_overlay::collector::collect_open_dropdown_overlays;

#[derive(Debug, Clone, Default)]
struct DropdownCursorTargets {
    hover: Option<DropdownMenuOverlayHit>,
    previous_hover: Option<DropdownMenuOverlayHit>,
    scroll: Option<DropdownMenuScrollTarget>,
}

#[derive(Debug, Default)]
pub(super) struct DropdownHoverUpdate {
    pub(super) hover: Option<DropdownMenuOverlayHit>,
    pub(super) previous_hover: Option<DropdownMenuOverlayHit>,
    pub(super) scroll_target: Option<DropdownMenuScrollTarget>,
    pub(super) outcome: MenuHoverOutcome,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn handle_dropdown_pointer_hit(
        &mut self,
        hit: DropdownMenuOverlayHit,
        button: MouseButton,
    ) -> bool {
        if button == MouseButton::Right {
            return self.handle_dropdown_right_click(hit);
        }
        self.handle_dropdown_left_click(hit);
        true
    }

    fn handle_dropdown_left_click(&mut self, hit: DropdownMenuOverlayHit) {
        match hit {
            DropdownMenuOverlayHit::Entry { id, path, .. } => {
                self.activate_dropdown_entry(id, path)
            }
            DropdownMenuOverlayHit::Dismiss { id } => self.close_dropdown_menu(id, true),
            DropdownMenuOverlayHit::Surface { .. } | DropdownMenuOverlayHit::Trigger { .. } => {}
        }
    }

    fn handle_dropdown_right_click(&mut self, hit: DropdownMenuOverlayHit) -> bool {
        let consumes_click = dropdown_right_click_consumes(&hit);
        let id = match hit {
            DropdownMenuOverlayHit::Entry { id, .. }
            | DropdownMenuOverlayHit::Surface { id, .. }
            | DropdownMenuOverlayHit::Dismiss { id }
            | DropdownMenuOverlayHit::Trigger { id } => id,
        };
        self.close_dropdown_menu(id, true);
        consumes_click
    }

    pub(super) fn toggle_dropdown_menu(&mut self, id: u64, open_at_last: bool) {
        let is_open = self.dropdown_is_open(id);
        if is_open {
            self.close_dropdown_menu(id, true);
            return;
        }
        self.close_all_dropdown_menus();
        self.engine.close_all_context_menus();
        let path = self.dropdown_boundary_path(id, !open_at_last);
        self.open_dropdown_menu(id, path);
    }

    pub(super) fn dropdown_is_open(&self, id: u64) -> bool {
        self.engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::menu_navigation)
            .is_some_and(|state| state.is_open())
    }

    pub(super) fn dropdown_boundary_path(&self, id: u64, first: bool) -> Option<Vec<usize>> {
        let runtime = self.engine.runtime_caches.dropdown_menus.get(&id)?;
        if first {
            runtime.first_root_path()
        } else {
            runtime.last_root_path()
        }
    }

    pub(super) fn open_dropdown_menu(&mut self, id: u64, path: Option<Vec<usize>>) {
        let root_index = path.as_ref().and_then(|path| path.first()).copied();
        if let Some(state) = self.dropdown_state_mut(id) {
            state.open_at_index(root_index);
        }
        match path {
            Some(path) => self.focus_dropdown_path(id, path),
            None => self.focus_widget(Some(id)),
        }
        self.engine.layout_dirty = true;
    }

    pub(super) fn activate_dropdown_entry(&mut self, id: u64, path: Vec<usize>) {
        let Some(runtime) = self.engine.runtime_caches.dropdown_menus.get(&id) else {
            return;
        };
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::menu_navigation_mut)
        else {
            return;
        };
        match runtime.activate_entry(state, path) {
            super::super::dropdown_menu_runtime::MenuActivation::Focus(path) => {
                self.focus_dropdown_path(id, path)
            }
            super::super::dropdown_menu_runtime::MenuActivation::Action(message) => {
                self.close_dropdown_menu(id, true);
                A::update(
                    &mut self.engine.app_state,
                    message,
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
            }
            super::super::dropdown_menu_runtime::MenuActivation::Ignored => {}
        }
    }

    pub(super) fn dropdown_path_is_reachable(
        &self,
        id: u64,
        path: &[usize],
        runtime: &DropdownMenuRuntime<A::Message>,
    ) -> bool {
        let Some(state) = self
            .engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::menu_navigation)
        else {
            return false;
        };
        state.is_open() && runtime.path_is_reachable(path, state.open_submenu_path())
    }

    pub(super) fn focus_dropdown_path(&mut self, id: u64, path: Vec<usize>) {
        if let Some(state) = self.dropdown_state_mut(id) {
            state.activate_path(path.clone());
        }
        self.focus_dropdown_entry_widget(id, &path);
    }

    fn focus_dropdown_entry_widget(&mut self, id: u64, path: &[usize]) {
        let focus_id = self
            .engine
            .runtime_caches
            .dropdown_menus
            .get(&id)
            .and_then(|runtime| runtime.item_id(path));
        // Context menus capture keys while preserving the invoking widget's focus.
        if self
            .engine
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_context_menu)
            .is_some()
        {
            return;
        }
        self.focus_widget(focus_id.or(Some(id)));
    }

    pub(super) fn dropdown_state_mut(
        &mut self,
        id: u64,
    ) -> Option<&mut crate::dropdown_menu::DropdownMenuState> {
        self.engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::menu_navigation_mut)
    }

    pub(super) fn close_dropdown_menu(&mut self, id: u64, restore_focus: bool) {
        if let Some(menu) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::as_context_menu_mut)
        {
            menu.close();
            // Context navigation never moves focus away from the invoking widget.
            self.engine.layout_dirty = true;
            return;
        }
        if let Some(state) = self.dropdown_state_mut(id) {
            state.close();
        }
        if restore_focus {
            self.focus_widget(Some(id));
        }
        self.engine.layout_dirty = true;
    }

    pub(super) fn close_all_dropdown_menus(&mut self) -> bool {
        let restore_id = self.focused_open_dropdown_parent();
        let mut changed = false;
        for state in self.engine.widget_states.values_mut() {
            if let Some(menu) = state.as_dropdown_menu_mut().filter(|menu| menu.is_open()) {
                menu.close();
                changed = true;
            }
        }
        if let Some(id) = restore_id {
            self.focus_widget(Some(id));
        }
        self.engine.layout_dirty |= changed;
        changed
    }

    pub(super) fn close_dropdowns_except(&mut self, retained_id: Option<u64>) {
        for (id, state) in &mut self.engine.widget_states {
            if Some(*id) == retained_id {
                continue;
            }
            if let Some(menu) = state.as_dropdown_menu_mut().filter(|menu| menu.is_open()) {
                menu.close();
                self.engine.layout_dirty = true;
            }
        }
    }

    fn focused_open_dropdown_parent(&self) -> Option<u64> {
        let focus_id = self.engine.focused_widget_id?;
        let (parent_id, _) = self.dropdown_focus_target(focus_id)?;
        self.dropdown_is_open(parent_id).then_some(parent_id)
    }

    pub(super) fn close_active_dropdown_menu(&mut self) -> bool {
        let focused_parent = self
            .engine
            .focused_widget_id
            .and_then(|focus_id| self.dropdown_focus_target(focus_id))
            .map(|(id, _)| id);
        let open_id = focused_parent.filter(|id| self.dropdown_is_open(*id));
        let id = open_id.or_else(|| self.first_open_dropdown_id());
        let Some(id) = id else { return false };
        self.close_dropdown_menu(id, true);
        true
    }

    fn first_open_dropdown_id(&self) -> Option<u64> {
        self.engine.widget_states.iter().find_map(|(id, state)| {
            state
                .menu_navigation()
                .is_some_and(|menu| menu.is_open())
                .then_some(*id)
        })
    }

    pub(super) fn refresh_dropdown_hover(&mut self) -> Result<(), RutterRunError> {
        self.refresh_dropdown_hover_from(None).map(|_| ())
    }

    pub(super) fn refresh_dropdown_hover_from(
        &mut self,
        previous_point: Option<skia_safe::Point>,
    ) -> Result<DropdownHoverUpdate, RutterRunError> {
        if !self.any_dropdown_menu_open() {
            return Ok(DropdownHoverUpdate::default());
        }
        let targets = self.dropdown_cursor_targets(previous_point)?;
        let outcome = match &targets.hover {
            Some(DropdownMenuOverlayHit::Entry { id, path, .. }) => {
                self.apply_dropdown_hover(*id, path.clone())
            }
            _ => MenuHoverOutcome::Ignored,
        };
        Ok(DropdownHoverUpdate {
            hover: targets.hover,
            previous_hover: targets.previous_hover,
            scroll_target: targets.scroll,
            outcome,
        })
    }

    pub(super) fn any_dropdown_menu_open(&self) -> bool {
        self.engine
            .widget_states
            .values()
            .any(|state| state.menu_navigation().is_some_and(|menu| menu.is_open()))
    }

    fn apply_dropdown_hover(&mut self, id: u64, path: Vec<usize>) -> MenuHoverOutcome {
        let Some(runtime) = self.engine.runtime_caches.dropdown_menus.get(&id) else {
            return MenuHoverOutcome::Ignored;
        };
        let Some(state) = self
            .engine
            .widget_states
            .get_mut(&id)
            .and_then(WidgetState::menu_navigation_mut)
        else {
            return MenuHoverOutcome::Ignored;
        };
        let outcome = runtime.hover_entry(state, path.clone());
        if outcome != MenuHoverOutcome::Ignored {
            // Navigation was already updated by hover_entry. Ordinary dropdowns
            // still need input focus/accessibility updates even for a settled row.
            self.focus_dropdown_entry_widget(id, &path);
        }
        outcome
    }

    pub(super) fn refresh_dropdown_scroll_target(
        &mut self,
    ) -> Result<Option<DropdownMenuScrollTarget>, RutterRunError> {
        Ok(self.dropdown_cursor_targets(None)?.scroll)
    }

    fn dropdown_cursor_targets(
        &mut self,
        previous_point: Option<skia_safe::Point>,
    ) -> Result<DropdownCursorTargets, RutterRunError> {
        let size = self.engine.window.as_ref().unwrap().surface_size();
        self.engine.try_ensure_widget_states()?;
        self.engine.try_ensure_layout(size)?;
        let viewport = self.logical_viewport(size);
        let point = self.engine.last_mouse_pos;
        let direction = A::locale().direction();
        let font_size = A::theme_for(&self.engine.app_state).font_body;
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        let mut contexts = Vec::new();
        crate::render::collect_open_context_menus(
            &widget,
            &self.engine.widget_states,
            &mut Vec::new(),
            &mut contexts,
        );
        if !contexts.is_empty() {
            let font = crate::render::text::get_cached_font(
                &mut self.engine.font_cache,
                "sans-serif",
                font_size,
            );
            let (hover, scroll) = crate::render::context_menu_overlay::context_cursor_targets(
                &contexts, point, viewport, direction, &font,
            );
            let previous_hover = previous_point.and_then(|previous| {
                crate::render::context_menu_overlay::context_cursor_targets(
                    &contexts, previous, viewport, direction, &font,
                )
                .0
            });
            return Ok(DropdownCursorTargets {
                hover,
                previous_hover,
                scroll,
            });
        }
        let overlays = collect_open_dropdown_overlays(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            &self.engine.widget_states,
            viewport,
        );
        Ok(DropdownCursorTargets {
            hover: dropdown_menu_entry_hover_at(&overlays, point, viewport, direction),
            previous_hover: None,
            scroll: dropdown_menu_scroll_target_at(&overlays, point, viewport, direction),
        })
    }

    fn logical_viewport(&self, size: PhysicalSize<u32>) -> (f32, f32) {
        (
            size.width as f32 / self.engine.scale_factor,
            size.height as f32 / self.engine.scale_factor,
        )
    }

    pub(super) fn scroll_open_dropdown(
        &mut self,
        target: DropdownMenuScrollTarget,
        delta_y: f32,
    ) -> bool {
        let Some(state) = self.dropdown_state_mut(target.id) else {
            return false;
        };
        let changed = state.scroll_level_with_descendant_collapse(
            target.level,
            target.current_scroll,
            target.current_scroll + delta_y,
            target.max_scroll,
        );
        if !changed {
            return false;
        }
        self.engine.layout_dirty = true;
        true
    }
}

fn dropdown_right_click_consumes(_: &DropdownMenuOverlayHit) -> bool {
    true
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn handle_context_menu_hit(&mut self, hit: ContextMenuOverlayHit<A::Message>) {
        match hit {
            ContextMenuOverlayHit::Item { id, msg } => {
                self.close_dropdown_menu(id, true);
                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                self.engine.layout_dirty = true;
            }
            ContextMenuOverlayHit::Submenu { id, path } => self.activate_dropdown_entry(id, path),
            ContextMenuOverlayHit::Consume => {}
            ContextMenuOverlayHit::Dismiss => {
                self.engine.close_all_context_menus();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::dropdown_menu::DropdownMenuEntryKind;

    #[test]
    fn open_dropdown_consumes_every_right_click_while_dismissing() {
        assert!(dropdown_right_click_consumes(
            &DropdownMenuOverlayHit::Dismiss { id: 7 }
        ));
        assert!(dropdown_right_click_consumes(
            &DropdownMenuOverlayHit::Trigger { id: 7 }
        ));
        assert!(dropdown_right_click_consumes(
            &DropdownMenuOverlayHit::Entry {
                id: 7,
                path: vec![0],
                kind: DropdownMenuEntryKind::Item,
                disabled: false,
            }
        ));
    }

    #[test]
    fn context_menu_surface_blocks_dropdown_hover_refresh() {
        let entries = [crate::ContextMenuEntry::<()>::disabled("Locked")];
        let mut state = crate::dropdown_menu::DropdownMenuState::default();
        state.open_at_index(None);
        let menus = [crate::render::ContextMenuOverlay {
            id: 42,
            entries: &entries,
            anchor: skia_safe::Point::new(20.0, 20.0),
            state,
        }];
        let (hover, _) = crate::render::context_menu_overlay::context_cursor_targets(
            &menus,
            skia_safe::Point::new(40.0, 40.0),
            (500.0, 400.0),
            crate::i18n::LayoutDirection::Ltr,
            &crate::render::text::get_cached_font(
                &mut std::collections::HashMap::new(),
                "sans-serif",
                14.0,
            ),
        );
        assert!(matches!(
            hover,
            Some(DropdownMenuOverlayHit::Entry {
                id: 42,
                disabled: true,
                ..
            })
        ));
    }
}
