// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache-2.0.

use winit::keyboard::{Key, ModifiersState};

use super::{RutterRunner, is_activation_key};
use crate::app::AppLogic;
use crate::engine::VirtualMultiSelectionRuntime;
use crate::engine::virtual_selection::{
    VirtualMultiSelectionLayout, VirtualMultiSelectionState, virtual_selection_navigation_index,
};
use crate::engine::widget_state::WidgetState;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn select_virtual_multi_item(&mut self, id: u64, index: usize) -> bool {
        let modifiers = self.engine.modifiers.state();
        let selected = self.update_virtual_multi_selection(id, |state, item_count| {
            state.begin_pointer_selection(
                index,
                item_count,
                primary_selection_modifier(modifiers),
                modifiers.shift_key(),
            )
        });
        if selected {
            self.virtual_multi_pointer_capture = Some(id);
        }
        selected
    }

    pub(super) fn extend_virtual_multi_pointer_selection(&mut self, id: u64, index: usize) -> bool {
        let Some(layout) = self
            .engine
            .runtime_caches
            .virtual_multi_selections
            .get(&id)
            .map(|runtime| runtime.layout)
        else {
            return false;
        };
        self.update_virtual_multi_selection(id, |state, item_count| {
            state.extend_pointer_selection(index, item_count, layout)
        })
    }

    pub(super) fn end_virtual_multi_pointer_selection(&mut self, id: u64) {
        if let Some(state) = self.engine.virtual_multi_selection_states.get_mut(&id) {
            state.end_pointer_selection();
        }
    }

    pub(super) fn scroll_virtual_multi_target(&mut self, id: u64, delta_y: f32) -> bool {
        let Some(runtime) = self
            .engine
            .runtime_caches
            .virtual_multi_selections
            .get(&id)
            .cloned()
        else {
            return false;
        };
        let Some(state) = self.engine.widget_states.get_mut(&id) else {
            return false;
        };
        scroll_virtual_multi_state(state, &runtime, delta_y)
    }

    pub(super) fn handle_virtual_multi_selection_key(
        &mut self,
        id: u64,
        key: &Key,
        repeat: bool,
    ) -> bool {
        let Some(runtime) = self
            .engine
            .runtime_caches
            .virtual_multi_selections
            .get(&id)
            .cloned()
        else {
            return false;
        };
        let modifiers = self.engine.modifiers.state();
        if non_primary_control_modifier(modifiers) {
            return false;
        }
        let handled = self.handle_virtual_multi_key_action(id, key, repeat, modifiers, &runtime);
        if handled {
            self.redraw();
        }
        handled
    }

    fn handle_virtual_multi_key_action(
        &mut self,
        id: u64,
        key: &Key,
        repeat: bool,
        modifiers: ModifiersState,
        runtime: &VirtualMultiSelectionRuntime<A::Message>,
    ) -> bool {
        if primary_selection_modifier(modifiers) && is_select_all_key(key) {
            return !repeat && self.select_all_virtual_multi_items(id);
        }
        if let Some(index) = self.virtual_multi_navigation_target(id, key, runtime) {
            return self.apply_virtual_multi_navigation(id, index, modifiers);
        }
        self.apply_virtual_multi_activation(id, key, repeat, modifiers, runtime.item_count)
    }

    fn virtual_multi_navigation_target(
        &self,
        id: u64,
        key: &Key,
        runtime: &VirtualMultiSelectionRuntime<A::Message>,
    ) -> Option<usize> {
        let active = self
            .engine
            .virtual_multi_selection_states
            .get(&id)
            .and_then(VirtualMultiSelectionState::active);
        let viewport_h = self.virtual_multi_viewport_height(id, runtime.layout);
        virtual_selection_navigation_index(
            runtime.layout,
            key,
            active,
            runtime.item_count,
            viewport_h,
            runtime.item_height,
        )
    }

    fn apply_virtual_multi_navigation(
        &mut self,
        id: u64,
        index: usize,
        modifiers: ModifiersState,
    ) -> bool {
        if primary_selection_modifier(modifiers) && !modifiers.shift_key() {
            return self.update_virtual_multi_selection(id, |state, item_count| {
                state.move_active(index, item_count);
                None
            });
        }
        self.update_virtual_multi_selection(id, |state, item_count| {
            state.select(
                index,
                item_count,
                primary_selection_modifier(modifiers),
                modifiers.shift_key(),
            )
        })
    }

    fn apply_virtual_multi_activation(
        &mut self,
        id: u64,
        key: &Key,
        repeat: bool,
        modifiers: ModifiersState,
        item_count: usize,
    ) -> bool {
        if !is_activation_key(key) || item_count == 0 {
            return false;
        }
        if repeat && primary_selection_modifier(modifiers) && !modifiers.shift_key() {
            return true;
        }
        let active = self.virtual_multi_active_or_first(id, item_count);
        self.update_virtual_multi_selection(id, |state, count| {
            state.select(
                active,
                count,
                primary_selection_modifier(modifiers),
                modifiers.shift_key(),
            )
        })
    }

    fn virtual_multi_active_or_first(&self, id: u64, item_count: usize) -> usize {
        self.engine
            .virtual_multi_selection_states
            .get(&id)
            .and_then(VirtualMultiSelectionState::active)
            .filter(|index| *index < item_count)
            .unwrap_or(0)
    }

    fn select_all_virtual_multi_items(&mut self, id: u64) -> bool {
        self.update_virtual_multi_selection(id, |state, item_count| state.select_all(item_count))
    }

    fn update_virtual_multi_selection<F>(&mut self, id: u64, update: F) -> bool
    where
        F: FnOnce(&mut VirtualMultiSelectionState, usize) -> Option<Vec<usize>>,
    {
        let Some(runtime) = self
            .engine
            .runtime_caches
            .virtual_multi_selections
            .get(&id)
            .cloned()
        else {
            return false;
        };
        let Some((selection, active)) =
            self.apply_virtual_multi_state(id, runtime.item_count, update)
        else {
            return false;
        };
        self.sync_virtual_multi_active(id, runtime.layout, active);
        self.scroll_to_virtual_multi_active(id, &runtime, active);
        self.dispatch_virtual_multi_selection(&runtime, selection);
        self.engine.layout_dirty = true;
        true
    }

    fn apply_virtual_multi_state<F>(
        &mut self,
        id: u64,
        item_count: usize,
        update: F,
    ) -> Option<(Option<Vec<usize>>, Option<usize>)>
    where
        F: FnOnce(&mut VirtualMultiSelectionState, usize) -> Option<Vec<usize>>,
    {
        let state = self.engine.virtual_multi_selection_states.get_mut(&id)?;
        let selection = update(state, item_count);
        Some((selection, state.active()))
    }

    fn sync_virtual_multi_active(
        &mut self,
        id: u64,
        layout: VirtualMultiSelectionLayout,
        active: Option<usize>,
    ) {
        match layout {
            VirtualMultiSelectionLayout::List => {
                if let Some(state) = self
                    .engine
                    .widget_states
                    .get_mut(&id)
                    .and_then(WidgetState::as_vlist_mut)
                {
                    state.selected_row = active;
                }
            }
            VirtualMultiSelectionLayout::Grid { .. } => {
                if let Some(state) = self
                    .engine
                    .widget_states
                    .get_mut(&id)
                    .and_then(WidgetState::as_vgrid_mut)
                {
                    state.selected_item = active;
                }
            }
        }
    }

    fn scroll_to_virtual_multi_active(
        &mut self,
        id: u64,
        runtime: &VirtualMultiSelectionRuntime<A::Message>,
        active: Option<usize>,
    ) {
        let Some(index) = active else {
            return;
        };
        match runtime.layout {
            VirtualMultiSelectionLayout::List => {
                if let Some(state) = self
                    .engine
                    .widget_states
                    .get_mut(&id)
                    .and_then(WidgetState::as_vlist_mut)
                {
                    state.scroll_to_index(index, runtime.item_height, runtime.item_count);
                }
            }
            VirtualMultiSelectionLayout::Grid { columns } => {
                if let Some(state) = self
                    .engine
                    .widget_states
                    .get_mut(&id)
                    .and_then(WidgetState::as_vgrid_mut)
                {
                    state.scroll_to_index(index, runtime.item_height, runtime.item_count, columns);
                }
            }
        }
    }

    fn dispatch_virtual_multi_selection(
        &mut self,
        runtime: &VirtualMultiSelectionRuntime<A::Message>,
        selection: Option<Vec<usize>>,
    ) {
        let Some(selection) = selection else {
            return;
        };
        A::update(
            &mut self.engine.app_state,
            (runtime.on_change)(selection),
            &mut self.engine.clipboard,
        );
    }

    fn virtual_multi_viewport_height(&self, id: u64, layout: VirtualMultiSelectionLayout) -> f32 {
        match layout {
            VirtualMultiSelectionLayout::List => self
                .engine
                .widget_states
                .get(&id)
                .and_then(WidgetState::as_vlist)
                .map(|state| state.viewport_h),
            VirtualMultiSelectionLayout::Grid { .. } => self
                .engine
                .widget_states
                .get(&id)
                .and_then(WidgetState::as_vgrid)
                .map(|state| state.viewport_h),
        }
        .unwrap_or(0.0)
    }
}

fn primary_selection_modifier(modifiers: ModifiersState) -> bool {
    if cfg!(target_os = "macos") {
        return modifiers.super_key();
    }
    modifiers.control_key()
}

fn non_primary_control_modifier(modifiers: ModifiersState) -> bool {
    cfg!(target_os = "macos") && modifiers.control_key() && !modifiers.super_key()
}

fn is_select_all_key(key: &Key) -> bool {
    matches!(key, Key::Character(value) if value.to_lowercase() == "a")
}

fn scroll_virtual_multi_state<Msg>(
    state: &mut WidgetState,
    runtime: &VirtualMultiSelectionRuntime<Msg>,
    delta_y: f32,
) -> bool {
    match runtime.layout {
        VirtualMultiSelectionLayout::List => state.as_vlist_mut().is_some_and(|list| {
            list.scroll_by(delta_y, runtime.item_height, runtime.item_count);
            true
        }),
        VirtualMultiSelectionLayout::Grid { columns } => state.as_vgrid_mut().is_some_and(|grid| {
            grid.scroll_by(delta_y, runtime.item_height, runtime.item_count, columns);
            true
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_all_key_is_case_insensitive() {
        assert!(is_select_all_key(&Key::Character("A".into())));
        assert!(is_select_all_key(&Key::Character("a".into())));
        assert!(!is_select_all_key(&Key::Character("b".into())));
    }

    #[test]
    fn multi_selection_wheel_scrolls_list_and_grid_states() {
        let list_runtime = VirtualMultiSelectionRuntime {
            on_change: |_| (),
            item_count: 20,
            item_height: 20.0,
            layout: VirtualMultiSelectionLayout::List,
        };
        let grid_runtime = VirtualMultiSelectionRuntime {
            on_change: |_| (),
            item_count: 20,
            item_height: 20.0,
            layout: VirtualMultiSelectionLayout::Grid { columns: 2 },
        };
        let mut list = WidgetState::VList(crate::engine::widget_state::VirtualListState {
            viewport_h: 100.0,
            ..Default::default()
        });
        let mut grid = WidgetState::VGrid(crate::engine::widget_state::VirtualGridState {
            viewport_h: 100.0,
            ..Default::default()
        });

        assert!(scroll_virtual_multi_state(&mut list, &list_runtime, 40.0));
        assert!(scroll_virtual_multi_state(&mut grid, &grid_runtime, 40.0));
        assert_eq!(list.as_vlist().map(|state| state.scroll_y), Some(40.0));
        assert_eq!(grid.as_vgrid().map(|state| state.scroll_y), Some(40.0));
    }
}
