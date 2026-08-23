// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache-2.0.

use std::collections::BTreeSet;

use winit::keyboard::{Key, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VirtualMultiSelectionLayout {
    List,
    Grid { columns: usize },
}

#[derive(Debug, Clone, Default)]
pub(crate) struct VirtualMultiSelectionState {
    selected: BTreeSet<usize>,
    active: Option<usize>,
    anchor: Option<usize>,
    pointer_drag: Option<VirtualMultiPointerDrag>,
}

#[derive(Debug, Clone)]
struct VirtualMultiPointerDrag {
    anchor: usize,
    baseline: BTreeSet<usize>,
    additive: bool,
}

impl VirtualMultiSelectionState {
    pub(crate) fn reconcile(&mut self, configured: &[usize], item_count: usize) {
        self.selected = configured
            .iter()
            .copied()
            .filter(|index| *index < item_count)
            .collect();
        self.active =
            retained_index(self.active, item_count).or_else(|| self.selected.first().copied());
        self.anchor = retained_index(self.anchor, item_count).or(self.active);
        self.reconcile_pointer_drag(item_count);
    }

    pub(crate) fn active(&self) -> Option<usize> {
        self.active
    }

    pub(crate) fn select(
        &mut self,
        index: usize,
        item_count: usize,
        additive: bool,
        range: bool,
    ) -> Option<Vec<usize>> {
        if range {
            return self.select_range(index, item_count, additive);
        }
        if additive {
            return self.toggle(index, item_count);
        }
        self.replace(index, item_count)
    }

    pub(crate) fn move_active(&mut self, index: usize, item_count: usize) -> bool {
        if index >= item_count || self.active == Some(index) {
            return false;
        }
        self.active = Some(index);
        self.anchor = self.anchor.or(Some(index));
        true
    }

    pub(crate) fn select_all(&mut self, item_count: usize) -> Option<Vec<usize>> {
        if item_count == 0 {
            return None;
        }
        let before = self.selected.clone();
        self.selected = (0..item_count).collect();
        self.active = retained_index(self.active, item_count).or(Some(0));
        self.anchor = retained_index(self.anchor, item_count).or(self.active);
        selection_change(&before, &self.selected)
    }

    pub(crate) fn begin_pointer_selection(
        &mut self,
        index: usize,
        item_count: usize,
        additive: bool,
        range: bool,
    ) -> Option<Vec<usize>> {
        let anchor = if range {
            self.range_anchor(index, item_count)
        } else {
            index
        };
        let baseline = self.selected.clone();
        let selection = self.select(index, item_count, additive, range);
        self.pointer_drag = (index < item_count).then_some(VirtualMultiPointerDrag {
            anchor,
            baseline,
            additive,
        });
        selection
    }

    pub(crate) fn extend_pointer_selection(
        &mut self,
        index: usize,
        item_count: usize,
        layout: VirtualMultiSelectionLayout,
    ) -> Option<Vec<usize>> {
        if index >= item_count {
            return None;
        }
        let drag = self.pointer_drag.clone()?;
        let before = self.selected.clone();
        self.selected = pointer_drag_selection(&drag, index, item_count, layout);
        self.active = Some(index);
        self.anchor = Some(drag.anchor);
        selection_change(&before, &self.selected)
    }

    pub(crate) fn end_pointer_selection(&mut self) {
        self.pointer_drag = None;
    }

    fn replace(&mut self, index: usize, item_count: usize) -> Option<Vec<usize>> {
        if index >= item_count {
            return None;
        }
        let before = self.selected.clone();
        self.selected.clear();
        self.selected.insert(index);
        self.active = Some(index);
        self.anchor = Some(index);
        selection_change(&before, &self.selected)
    }

    fn toggle(&mut self, index: usize, item_count: usize) -> Option<Vec<usize>> {
        if index >= item_count {
            return None;
        }
        let before = self.selected.clone();
        if !self.selected.remove(&index) {
            self.selected.insert(index);
        }
        self.active = Some(index);
        self.anchor = Some(index);
        selection_change(&before, &self.selected)
    }

    fn select_range(
        &mut self,
        index: usize,
        item_count: usize,
        additive: bool,
    ) -> Option<Vec<usize>> {
        if index >= item_count {
            return None;
        }
        let before = self.selected.clone();
        let anchor = self.range_anchor(index, item_count);
        if !additive {
            self.selected.clear();
        }
        self.selected.extend(anchor.min(index)..=anchor.max(index));
        self.active = Some(index);
        self.anchor = Some(anchor);
        selection_change(&before, &self.selected)
    }

    fn range_anchor(&self, index: usize, item_count: usize) -> usize {
        retained_index(self.anchor, item_count)
            .or_else(|| retained_index(self.active, item_count))
            .unwrap_or(index)
    }

    fn reconcile_pointer_drag(&mut self, item_count: usize) {
        let Some(drag) = self.pointer_drag.as_mut() else {
            return;
        };
        drag.baseline.retain(|index| *index < item_count);
        if drag.anchor >= item_count {
            self.pointer_drag = None;
        }
    }
}

fn pointer_drag_selection(
    drag: &VirtualMultiPointerDrag,
    index: usize,
    item_count: usize,
    layout: VirtualMultiSelectionLayout,
) -> BTreeSet<usize> {
    let mut selected = if drag.additive {
        drag.baseline.clone()
    } else {
        BTreeSet::new()
    };
    extend_pointer_drag_selection(&mut selected, drag.anchor, index, item_count, layout);
    selected
}

fn extend_pointer_drag_selection(
    selected: &mut BTreeSet<usize>,
    anchor: usize,
    index: usize,
    item_count: usize,
    layout: VirtualMultiSelectionLayout,
) {
    match layout {
        VirtualMultiSelectionLayout::List => {
            selected.extend(anchor.min(index)..=anchor.max(index));
        }
        VirtualMultiSelectionLayout::Grid { columns } => {
            extend_grid_drag_rectangle(selected, anchor, index, item_count, columns);
        }
    }
}

fn extend_grid_drag_rectangle(
    selected: &mut BTreeSet<usize>,
    anchor: usize,
    index: usize,
    item_count: usize,
    columns: usize,
) {
    let columns = columns.max(1);
    let row_range =
        (anchor / columns).min(index / columns)..=(anchor / columns).max(index / columns);
    let column_range =
        (anchor % columns).min(index % columns)..=(anchor % columns).max(index % columns);
    for row in row_range {
        selected.extend(column_range.clone().map(|column| row * columns + column));
    }
    selected.retain(|candidate| *candidate < item_count);
}

pub(crate) fn virtual_selection_navigation_index(
    layout: VirtualMultiSelectionLayout,
    key: &Key,
    active: Option<usize>,
    item_count: usize,
    viewport_h: f32,
    item_height: f32,
) -> Option<usize> {
    if item_count == 0 {
        return None;
    }
    let current = active.unwrap_or_else(|| initial_navigation_index(key, item_count));
    match layout {
        VirtualMultiSelectionLayout::List => {
            list_navigation_index(key, current, item_count, viewport_h, item_height)
        }
        VirtualMultiSelectionLayout::Grid { columns } => {
            grid_navigation_index(key, current, item_count, columns, viewport_h, item_height)
        }
    }
}

fn initial_navigation_index(key: &Key, item_count: usize) -> usize {
    if matches!(
        key,
        Key::Named(NamedKey::ArrowUp | NamedKey::End | NamedKey::PageUp)
    ) {
        return item_count.saturating_sub(1);
    }
    0
}

fn list_navigation_index(
    key: &Key,
    current: usize,
    item_count: usize,
    viewport_h: f32,
    item_height: f32,
) -> Option<usize> {
    let page = virtual_selection_page_span(viewport_h, item_height, 1);
    match key {
        Key::Named(NamedKey::ArrowDown) => Some((current + 1).min(item_count - 1)),
        Key::Named(NamedKey::ArrowUp) => Some(current.saturating_sub(1)),
        Key::Named(NamedKey::Home) => Some(0),
        Key::Named(NamedKey::End) => Some(item_count - 1),
        Key::Named(NamedKey::PageDown) => Some((current + page).min(item_count - 1)),
        Key::Named(NamedKey::PageUp) => Some(current.saturating_sub(page)),
        _ => None,
    }
}

fn grid_navigation_index(
    key: &Key,
    current: usize,
    item_count: usize,
    columns: usize,
    viewport_h: f32,
    item_height: f32,
) -> Option<usize> {
    let columns = columns.max(1);
    let page = virtual_selection_page_span(viewport_h, item_height, columns);
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(current.saturating_sub(1)),
        Key::Named(NamedKey::ArrowRight) => Some((current + 1).min(item_count - 1)),
        Key::Named(NamedKey::ArrowUp) => Some(current.saturating_sub(columns)),
        Key::Named(NamedKey::ArrowDown) => Some((current + columns).min(item_count - 1)),
        Key::Named(NamedKey::Home) => Some(0),
        Key::Named(NamedKey::End) => Some(item_count - 1),
        Key::Named(NamedKey::PageDown) => Some((current + page).min(item_count - 1)),
        Key::Named(NamedKey::PageUp) => Some(current.saturating_sub(page)),
        _ => None,
    }
}

fn virtual_selection_page_span(viewport_h: f32, item_height: f32, columns: usize) -> usize {
    if !viewport_h.is_finite() || !item_height.is_finite() || item_height <= 0.0 {
        return columns.max(1);
    }
    ((viewport_h / item_height).floor() as usize)
        .max(1)
        .saturating_mul(columns.max(1))
}

fn retained_index(index: Option<usize>, item_count: usize) -> Option<usize> {
    index.filter(|value| *value < item_count)
}

fn selection_change(before: &BTreeSet<usize>, after: &BTreeSet<usize>) -> Option<Vec<usize>> {
    (before != after).then(|| after.iter().copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_selection_discards_duplicates_and_out_of_bounds_values() {
        let mut state = VirtualMultiSelectionState::default();
        state.reconcile(&[4, 1, 1, 9], 5);

        assert_eq!(
            state.selected.iter().copied().collect::<Vec<_>>(),
            vec![1, 4]
        );
        assert_eq!(state.active(), Some(1));
    }

    #[test]
    fn selection_gestures_emit_sorted_replace_toggle_and_range_sets() {
        let mut state = VirtualMultiSelectionState::default();
        assert_eq!(state.select(2, 8, false, false), Some(vec![2]));
        assert_eq!(state.select(5, 8, true, false), Some(vec![2, 5]));
        assert_eq!(state.select(6, 8, false, true), Some(vec![5, 6]));
        assert_eq!(state.select(3, 8, true, true), Some(vec![3, 4, 5, 6]));
    }

    #[test]
    fn moving_active_item_leaves_the_controlled_selection_intact() {
        let mut state = VirtualMultiSelectionState::default();
        state.reconcile(&[1, 3], 5);

        assert!(state.move_active(4, 5));
        assert_eq!(state.active(), Some(4));
        assert_eq!(
            state.selected.iter().copied().collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn pointer_drag_selects_list_ranges_and_grid_rectangles() {
        let mut list = VirtualMultiSelectionState::default();
        assert_eq!(
            list.begin_pointer_selection(2, 12, false, false),
            Some(vec![2])
        );
        assert_eq!(
            list.extend_pointer_selection(5, 12, VirtualMultiSelectionLayout::List),
            Some(vec![2, 3, 4, 5])
        );

        let mut grid = VirtualMultiSelectionState::default();
        grid.reconcile(&[0], 12);
        assert_eq!(
            grid.begin_pointer_selection(5, 12, true, false),
            Some(vec![0, 5])
        );
        assert_eq!(
            grid.extend_pointer_selection(10, 12, VirtualMultiSelectionLayout::Grid { columns: 4 }),
            Some(vec![0, 5, 6, 9, 10])
        );
    }

    #[test]
    fn shift_pointer_drag_keeps_the_existing_selection_anchor() {
        let mut state = VirtualMultiSelectionState::default();
        state.reconcile(&[1], 8);

        assert_eq!(
            state.begin_pointer_selection(4, 8, false, true),
            Some(vec![1, 2, 3, 4])
        );
        assert_eq!(
            state.extend_pointer_selection(6, 8, VirtualMultiSelectionLayout::List),
            Some(vec![1, 2, 3, 4, 5, 6])
        );
    }

    #[test]
    fn navigation_uses_list_rows_and_physical_grid_columns() {
        let list = VirtualMultiSelectionLayout::List;
        let grid = VirtualMultiSelectionLayout::Grid { columns: 3 };

        assert_eq!(
            virtual_selection_navigation_index(
                list,
                &Key::Named(NamedKey::PageDown),
                Some(1),
                10,
                80.0,
                20.0,
            ),
            Some(5)
        );
        assert_eq!(
            virtual_selection_navigation_index(
                grid,
                &Key::Named(NamedKey::ArrowDown),
                Some(1),
                10,
                80.0,
                20.0,
            ),
            Some(4)
        );
    }
}
