// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use crate::widget::Widget;
use crate::widget_id_error::WidgetIdError;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntry, DropdownMenuEntryKind, OwnedDropdownMenuEntry, entries_at_level,
    entry_at_path, first_focusable_index, flatten_entry_paths, last_focusable_index,
    next_focusable_index, previous_focusable_index, to_owned_entries, typeahead_prefix_match,
};

#[derive(Debug, Clone)]
pub(super) struct DropdownMenuRuntime<Msg> {
    pub(super) entries: Vec<OwnedDropdownMenuEntry<Msg>>,
    item_ids: HashMap<Vec<usize>, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DropdownMenuItemRuntime {
    pub(super) parent_id: u64,
    pub(super) path: Vec<usize>,
}

impl<Msg: Clone> DropdownMenuRuntime<Msg> {
    pub(super) fn from_widget(
        widget: &Widget<'_, Msg>,
        widget_path: &[usize],
        entries: &[DropdownMenuEntry<'_, Msg>],
    ) -> Self {
        let item_ids = menu_item_ids(widget, widget_path, entries);
        Self {
            entries: to_owned_entries(entries),
            item_ids,
        }
    }

    pub(super) fn item_id(&self, path: &[usize]) -> Option<u64> {
        self.item_ids.get(path).copied()
    }

    pub(super) fn entry(&self, path: &[usize]) -> Option<&OwnedDropdownMenuEntry<Msg>> {
        entry_at_path(&self.entries, path)
    }

    pub(super) fn action_message(&self, path: &[usize]) -> Option<Msg> {
        self.entry(path)?.action_message().cloned()
    }

    pub(super) fn entry_kind(&self, path: &[usize]) -> Option<DropdownMenuEntryKind> {
        self.entry(path).map(OwnedDropdownMenuEntry::kind)
    }

    pub(super) fn is_disabled(&self, path: &[usize]) -> bool {
        self.entry(path)
            .is_none_or(OwnedDropdownMenuEntry::is_disabled)
    }

    pub(super) fn first_root_path(&self) -> Option<Vec<usize>> {
        first_focusable_index(&self.entries).map(|index| vec![index])
    }

    pub(super) fn last_root_path(&self) -> Option<Vec<usize>> {
        last_focusable_index(&self.entries).map(|index| vec![index])
    }

    pub(super) fn first_child_path(&self, path: &[usize]) -> Option<Vec<usize>> {
        let children = self.entry(path)?.submenu_entries()?;
        append_index(path, first_focusable_index(children)?)
    }

    pub(super) fn adjacent_path(&self, path: &[usize], forward: bool) -> Option<Vec<usize>> {
        let (current, level_path) = path.split_last()?;
        let entries = entries_at_level(&self.entries, level_path)?;
        let index = if forward {
            next_focusable_index(entries, Some(*current))?
        } else {
            previous_focusable_index(entries, Some(*current))?
        };
        append_index(level_path, index)
    }

    pub(super) fn boundary_path(&self, path: &[usize], first: bool) -> Option<Vec<usize>> {
        let (_, level_path) = path.split_last()?;
        let entries = entries_at_level(&self.entries, level_path)?;
        let index = if first {
            first_focusable_index(entries)?
        } else {
            last_focusable_index(entries)?
        };
        append_index(level_path, index)
    }

    pub(super) fn typeahead_path(&self, path: &[usize], prefix: &str) -> Option<Vec<usize>> {
        let (current, level_path) = path.split_last()?;
        let entries = entries_at_level(&self.entries, level_path)?;
        let index = typeahead_prefix_match(entries, Some(*current), prefix)?;
        append_index(level_path, index)
    }

    pub(super) fn item_focus_entries(&self, parent_id: u64) -> Vec<(u64, DropdownMenuItemRuntime)> {
        self.item_ids
            .iter()
            .map(|(path, id)| {
                (
                    *id,
                    DropdownMenuItemRuntime {
                        parent_id,
                        path: path.clone(),
                    },
                )
            })
            .collect()
    }

    pub(super) fn validate_entry_identities(&self, menu_id: u64) -> Result<(), WidgetIdError> {
        validate_identity_level(&self.entries, menu_id, &mut Vec::new())
    }

    pub(super) fn has_same_topology(&self, next: &Self) -> bool {
        let paths = flatten_entry_paths(&self.entries);
        if paths != flatten_entry_paths(&next.entries) {
            return false;
        }
        paths
            .iter()
            .all(|path| self.entry_identity(path) == next.entry_identity(path))
    }

    pub(super) fn path_is_reachable(&self, path: &[usize], open_path: &[usize]) -> bool {
        if self.entry(path).is_none() {
            return false;
        }
        (1..path.len()).all(|depth| {
            let ancestor = &path[..depth];
            self.entry(ancestor).is_some_and(|entry| {
                entry.kind() == DropdownMenuEntryKind::Submenu
                    && !entry.is_disabled()
                    && open_path.starts_with(ancestor)
            })
        })
    }

    fn entry_identity(
        &self,
        path: &[usize],
    ) -> Option<(DropdownMenuEntryKind, &str, bool, Option<u64>)> {
        let entry = self.entry(path)?;
        Some((
            entry.kind(),
            entry.label().unwrap_or_default(),
            entry.is_disabled(),
            entry.key(),
        ))
    }
}

fn validate_identity_level<Msg>(
    entries: &[OwnedDropdownMenuEntry<Msg>],
    menu_id: u64,
    level_path: &mut Vec<usize>,
) -> Result<(), WidgetIdError> {
    validate_sibling_identities(entries, menu_id, level_path)?;
    for (index, entry) in entries.iter().enumerate() {
        let Some(children) = entry.submenu_entries() else {
            continue;
        };
        level_path.push(index);
        validate_identity_level(children, menu_id, level_path)?;
        level_path.pop();
    }
    Ok(())
}

fn validate_sibling_identities<Msg>(
    entries: &[OwnedDropdownMenuEntry<Msg>],
    menu_id: u64,
    level_path: &[usize],
) -> Result<(), WidgetIdError> {
    for first in 0..entries.len() {
        for second in (first + 1)..entries.len() {
            validate_identity_pair(entries, menu_id, level_path, first, second)?;
        }
    }
    Ok(())
}

fn validate_identity_pair<Msg>(
    entries: &[OwnedDropdownMenuEntry<Msg>],
    menu_id: u64,
    level_path: &[usize],
    first: usize,
    second: usize,
) -> Result<(), WidgetIdError> {
    let left = &entries[first];
    let right = &entries[second];
    if left.kind() == DropdownMenuEntryKind::Separator
        || right.kind() == DropdownMenuEntryKind::Separator
    {
        return Ok(());
    }
    if base_entry_identity(left) != base_entry_identity(right) || distinct_keys(left, right) {
        return Ok(());
    }
    Err(ambiguous_identity_error(
        menu_id, level_path, first, second, left, right,
    ))
}

fn base_entry_identity<Msg>(entry: &OwnedDropdownMenuEntry<Msg>) -> (DropdownMenuEntryKind, &str) {
    (entry.kind(), entry.label().unwrap_or_default())
}

fn distinct_keys<Msg>(
    left: &OwnedDropdownMenuEntry<Msg>,
    right: &OwnedDropdownMenuEntry<Msg>,
) -> bool {
    matches!((left.key(), right.key()), (Some(left), Some(right)) if left != right)
}

fn ambiguous_identity_error<Msg>(
    menu_id: u64,
    level_path: &[usize],
    first: usize,
    second: usize,
    left: &OwnedDropdownMenuEntry<Msg>,
    right: &OwnedDropdownMenuEntry<Msg>,
) -> WidgetIdError {
    WidgetIdError::AmbiguousDropdownEntryIdentity {
        menu_id,
        label: left.label().unwrap_or_default().to_owned(),
        first_path: identity_path(level_path, first),
        second_path: identity_path(level_path, second),
        first_key: left.key(),
        second_key: right.key(),
    }
}

fn identity_path(level_path: &[usize], index: usize) -> Vec<usize> {
    let mut path = level_path.to_vec();
    path.push(index);
    path
}

fn menu_item_ids<Msg>(
    widget: &Widget<'_, Msg>,
    widget_path: &[usize],
    entries: &[DropdownMenuEntry<'_, Msg>],
) -> HashMap<Vec<usize>, u64> {
    flatten_entry_paths(entries)
        .into_iter()
        .filter_map(|path| menu_item_id(widget, widget_path, entries, path))
        .collect()
}

fn menu_item_id<Msg>(
    widget: &Widget<'_, Msg>,
    widget_path: &[usize],
    entries: &[DropdownMenuEntry<'_, Msg>],
    path: Vec<usize>,
) -> Option<(Vec<usize>, u64)> {
    if !entry_at_path(entries, &path)?.is_focusable() {
        return None;
    }
    let id = widget.dropdown_menu_item_focus_id(widget_path, &path)?;
    Some((path, id))
}

fn append_index(path: &[usize], index: usize) -> Option<Vec<usize>> {
    let mut next = path.to_vec();
    next.push(index);
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::Style;

    fn runtime() -> DropdownMenuRuntime<u8> {
        let entries = vec![
            DropdownMenuEntry::disabled_item("Disabled"),
            DropdownMenuEntry::separator(),
            DropdownMenuEntry::submenu("More", vec![DropdownMenuEntry::item("Child", 2)]),
            DropdownMenuEntry::item("Run", 3),
        ];
        let widget = Widget::dropdown_menu("Actions", entries.clone(), Style::default());
        DropdownMenuRuntime::from_widget(&widget, &[], &entries)
    }

    #[test]
    fn runtime_navigation_wraps_and_skips_separators() {
        let runtime = runtime();
        assert_eq!(runtime.first_root_path(), Some(vec![0]));
        assert_eq!(runtime.adjacent_path(&[0], true), Some(vec![2]));
        assert_eq!(runtime.adjacent_path(&[3], true), Some(vec![0]));
        assert_eq!(runtime.boundary_path(&[2], false), Some(vec![3]));
    }

    #[test]
    fn runtime_resolves_nested_focus_and_actions() {
        let runtime = runtime();
        assert!(runtime.item_id(&[2, 0]).is_some());
        assert_eq!(runtime.first_child_path(&[2]), Some(vec![2, 0]));
        assert_eq!(runtime.action_message(&[2, 0]), Some(2));
        assert!(runtime.is_disabled(&[0]));
    }

    #[test]
    fn runtime_typeahead_searches_current_level() {
        let runtime = runtime();
        assert_eq!(runtime.typeahead_path(&[0], "r"), Some(vec![3]));
        assert_eq!(runtime.typeahead_path(&[2, 0], "r"), None);
    }

    #[test]
    fn runtime_topology_ignores_state_but_rejects_command_reordering() {
        let original = runtime();
        let mut changed_state = runtime();
        let OwnedDropdownMenuEntry::Item { message, .. } = &mut changed_state.entries[3] else {
            panic!("expected action item at path [3]");
        };
        *message = Some(9);
        assert!(original.has_same_topology(&changed_state));

        let mut reordered = runtime();
        reordered.entries.swap(0, 3);
        assert!(!original.has_same_topology(&reordered));
    }

    #[test]
    fn runtime_topology_rejects_submenu_disablement() {
        let enabled_entries = vec![DropdownMenuEntry::submenu(
            "More",
            vec![DropdownMenuEntry::item("Child", 2)],
        )];
        let disabled_entries = vec![DropdownMenuEntry::disabled_submenu(
            "More",
            vec![DropdownMenuEntry::item("Child", 2)],
        )];
        let enabled_widget =
            Widget::dropdown_menu("Actions", enabled_entries.clone(), Style::default());
        let disabled_widget =
            Widget::dropdown_menu("Actions", disabled_entries.clone(), Style::default());
        let enabled = DropdownMenuRuntime::from_widget(&enabled_widget, &[], &enabled_entries);
        let disabled = DropdownMenuRuntime::from_widget(&disabled_widget, &[], &disabled_entries);

        assert!(!enabled.has_same_topology(&disabled));
    }

    #[test]
    fn duplicate_labels_require_distinct_explicit_keys() {
        let entries = vec![
            DropdownMenuEntry::item("Open", 1),
            DropdownMenuEntry::item("Open", 2),
        ];
        let widget = Widget::dropdown_menu("Actions", entries.clone(), Style::default());
        let runtime = DropdownMenuRuntime::from_widget(&widget, &[], &entries);

        assert!(matches!(
            runtime.validate_entry_identities(7),
            Err(WidgetIdError::AmbiguousDropdownEntryIdentity { menu_id: 7, .. })
        ));
    }

    #[test]
    fn duplicate_separators_do_not_require_keys() {
        let entries = vec![
            DropdownMenuEntry::<u8>::separator(),
            DropdownMenuEntry::separator(),
        ];
        let widget = Widget::dropdown_menu("Actions", entries.clone(), Style::default());
        let runtime = DropdownMenuRuntime::from_widget(&widget, &[], &entries);

        assert!(runtime.validate_entry_identities(7).is_ok());
    }

    #[test]
    fn matching_labels_require_keys_across_enabled_states() {
        let entries = vec![
            DropdownMenuEntry::item("Open", 1),
            DropdownMenuEntry::disabled_item("Open"),
        ];
        let widget = Widget::dropdown_menu("Actions", entries.clone(), Style::default());
        let runtime = DropdownMenuRuntime::from_widget(&widget, &[], &entries);

        assert!(runtime.validate_entry_identities(7).is_err());
    }

    #[test]
    fn keyed_duplicate_labels_detect_command_reordering() {
        let first_entries = vec![
            DropdownMenuEntry::item("Open", 1).with_key(10),
            DropdownMenuEntry::item("Open", 2).with_key(20),
        ];
        let next_entries = vec![
            DropdownMenuEntry::item("Open", 2).with_key(20),
            DropdownMenuEntry::item("Open", 1).with_key(10),
        ];
        let first_widget =
            Widget::dropdown_menu("Actions", first_entries.clone(), Style::default());
        let next_widget = Widget::dropdown_menu("Actions", next_entries.clone(), Style::default());
        let first = DropdownMenuRuntime::from_widget(&first_widget, &[], &first_entries);
        let next = DropdownMenuRuntime::from_widget(&next_widget, &[], &next_entries);

        assert!(first.validate_entry_identities(7).is_ok());
        assert!(!first.has_same_topology(&next));
    }

    #[test]
    fn runtime_reachability_requires_open_enabled_submenu_ancestors() {
        let runtime = runtime();
        assert!(runtime.path_is_reachable(&[2, 0], &[2]));
        assert!(!runtime.path_is_reachable(&[2, 0], &[]));
        assert!(runtime.path_is_reachable(&[3], &[]));
    }
}
