// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{DropdownMenuEntry, DropdownMenuEntryKind};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OwnedDropdownMenuEntry<Msg> {
    Item {
        label: String,
        message: Option<Msg>,
        key: Option<u64>,
    },
    Checkbox {
        label: String,
        checked: bool,
        message: Option<Msg>,
        key: Option<u64>,
    },
    Radio {
        label: String,
        selected: bool,
        message: Option<Msg>,
        key: Option<u64>,
    },
    Submenu {
        label: String,
        entries: Vec<Self>,
        enabled: bool,
        key: Option<u64>,
    },
    Separator,
}

impl<Msg> OwnedDropdownMenuEntry<Msg> {
    pub(crate) fn label(&self) -> Option<&str> {
        match self {
            Self::Item { label, .. }
            | Self::Checkbox { label, .. }
            | Self::Radio { label, .. }
            | Self::Submenu { label, .. } => Some(label),
            Self::Separator => None,
        }
    }

    pub(crate) fn key(&self) -> Option<u64> {
        match self {
            Self::Item { key, .. }
            | Self::Checkbox { key, .. }
            | Self::Radio { key, .. }
            | Self::Submenu { key, .. } => *key,
            Self::Separator => None,
        }
    }

    pub(crate) fn is_disabled(&self) -> bool {
        match self {
            Self::Item { message, .. }
            | Self::Checkbox { message, .. }
            | Self::Radio { message, .. } => message.is_none(),
            Self::Submenu { enabled, .. } => !enabled,
            Self::Separator => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_focusable(&self) -> bool {
        self.kind() != DropdownMenuEntryKind::Separator
    }

    #[cfg(test)]
    pub(crate) fn is_activatable(&self) -> bool {
        self.is_focusable() && !self.is_disabled()
    }

    pub(crate) fn submenu_entries(&self) -> Option<&[Self]> {
        match self {
            Self::Submenu { entries, .. } => Some(entries),
            _ => None,
        }
    }

    pub(crate) fn action_message(&self) -> Option<&Msg> {
        match self {
            Self::Item { message, .. }
            | Self::Checkbox { message, .. }
            | Self::Radio { message, .. } => message.as_ref(),
            Self::Submenu { .. } | Self::Separator => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn checked(&self) -> Option<bool> {
        match self {
            Self::Checkbox { checked, .. } => Some(*checked),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn selected(&self) -> Option<bool> {
        match self {
            Self::Radio { selected, .. } => Some(*selected),
            _ => None,
        }
    }

    pub(crate) fn kind(&self) -> DropdownMenuEntryKind {
        match self {
            Self::Item { .. } => DropdownMenuEntryKind::Item,
            Self::Checkbox { .. } => DropdownMenuEntryKind::Checkbox,
            Self::Radio { .. } => DropdownMenuEntryKind::Radio,
            Self::Submenu { .. } => DropdownMenuEntryKind::Submenu,
            Self::Separator => DropdownMenuEntryKind::Separator,
        }
    }
}

pub(crate) trait DropdownMenuEntryAccess: Sized {
    fn entry_label(&self) -> Option<&str>;
    fn entry_is_disabled(&self) -> bool;
    fn entry_kind(&self) -> DropdownMenuEntryKind;
    fn child_entries(&self) -> Option<&[Self]>;

    fn entry_is_focusable(&self) -> bool {
        self.entry_kind() != DropdownMenuEntryKind::Separator
    }

    fn entry_is_activatable(&self) -> bool {
        self.entry_is_focusable() && !self.entry_is_disabled()
    }
}

impl<Msg> DropdownMenuEntryAccess for DropdownMenuEntry<'_, Msg> {
    fn entry_label(&self) -> Option<&str> {
        self.label()
    }

    fn entry_is_disabled(&self) -> bool {
        self.is_disabled()
    }

    fn entry_kind(&self) -> DropdownMenuEntryKind {
        self.kind()
    }

    fn child_entries(&self) -> Option<&[Self]> {
        self.submenu_entries()
    }
}

impl<Msg> DropdownMenuEntryAccess for OwnedDropdownMenuEntry<Msg> {
    fn entry_label(&self) -> Option<&str> {
        self.label()
    }

    fn entry_is_disabled(&self) -> bool {
        self.is_disabled()
    }

    fn entry_kind(&self) -> DropdownMenuEntryKind {
        self.kind()
    }

    fn child_entries(&self) -> Option<&[Self]> {
        self.submenu_entries()
    }
}

pub(crate) fn to_owned_entries<Msg: Clone>(
    entries: &[DropdownMenuEntry<'_, Msg>],
) -> Vec<OwnedDropdownMenuEntry<Msg>> {
    entries.iter().map(to_owned_entry).collect()
}

fn to_owned_entry<Msg: Clone>(entry: &DropdownMenuEntry<'_, Msg>) -> OwnedDropdownMenuEntry<Msg> {
    match entry {
        DropdownMenuEntry::Item {
            label,
            message,
            key,
        } => owned_item(label, message, *key),
        DropdownMenuEntry::Checkbox {
            label,
            checked,
            message,
            key,
        } => owned_checkbox(label, *checked, message, *key),
        DropdownMenuEntry::Radio {
            label,
            selected,
            message,
            key,
        } => owned_radio(label, *selected, message, *key),
        DropdownMenuEntry::Submenu {
            label,
            entries,
            enabled,
            key,
        } => OwnedDropdownMenuEntry::Submenu {
            label: (*label).to_owned(),
            entries: to_owned_entries(entries),
            enabled: *enabled,
            key: *key,
        },
        DropdownMenuEntry::Separator => OwnedDropdownMenuEntry::Separator,
    }
}

fn owned_item<Msg: Clone>(
    label: &str,
    message: &Option<Msg>,
    key: Option<u64>,
) -> OwnedDropdownMenuEntry<Msg> {
    OwnedDropdownMenuEntry::Item {
        label: label.to_owned(),
        message: message.clone(),
        key,
    }
}

fn owned_checkbox<Msg: Clone>(
    label: &str,
    checked: bool,
    message: &Option<Msg>,
    key: Option<u64>,
) -> OwnedDropdownMenuEntry<Msg> {
    OwnedDropdownMenuEntry::Checkbox {
        label: label.to_owned(),
        checked,
        message: message.clone(),
        key,
    }
}

fn owned_radio<Msg: Clone>(
    label: &str,
    selected: bool,
    message: &Option<Msg>,
    key: Option<u64>,
) -> OwnedDropdownMenuEntry<Msg> {
    OwnedDropdownMenuEntry::Radio {
        label: label.to_owned(),
        selected,
        message: message.clone(),
        key,
    }
}

pub(crate) fn entry_at_path<'a, Entry: DropdownMenuEntryAccess>(
    entries: &'a [Entry],
    path: &[usize],
) -> Option<&'a Entry> {
    let (last, parents) = path.split_last()?;
    let level = entries_at_level(entries, parents)?;
    level.get(*last)
}

pub(crate) fn entries_at_level<'a, Entry: DropdownMenuEntryAccess>(
    entries: &'a [Entry],
    level_path: &[usize],
) -> Option<&'a [Entry]> {
    let mut current = entries;
    for index in level_path {
        current = current.get(*index)?.child_entries()?;
    }
    Some(current)
}

pub(crate) fn first_focusable_index<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
) -> Option<usize> {
    entries.iter().position(Entry::entry_is_focusable)
}

pub(crate) fn last_focusable_index<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
) -> Option<usize> {
    entries.iter().rposition(Entry::entry_is_focusable)
}

pub(crate) fn next_focusable_index<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
    current: Option<usize>,
) -> Option<usize> {
    wrapped_focusable_index(entries, current, 1)
}

pub(crate) fn previous_focusable_index<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
    current: Option<usize>,
) -> Option<usize> {
    wrapped_focusable_index(entries, current, -1)
}

fn wrapped_focusable_index<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
    current: Option<usize>,
    step: isize,
) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }
    let start = navigation_start(entries.len(), current, step);
    (0..entries.len())
        .map(|offset| wrapped_index(start, offset, step, entries.len()))
        .find(|index| entries[*index].entry_is_focusable())
}

fn navigation_start(length: usize, current: Option<usize>, step: isize) -> usize {
    match (current.filter(|index| *index < length), step > 0) {
        (Some(index), true) => (index + 1) % length,
        (Some(index), false) => (index + length - 1) % length,
        (None, true) => 0,
        (None, false) => length - 1,
    }
}

fn wrapped_index(start: usize, offset: usize, step: isize, length: usize) -> usize {
    if step > 0 {
        return (start + offset) % length;
    }
    (start + length - offset % length) % length
}

pub(crate) fn typeahead_prefix_match<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
    current: Option<usize>,
    prefix: &str,
) -> Option<usize> {
    if entries.is_empty() || prefix.is_empty() {
        return None;
    }
    let prefix = prefix.to_lowercase();
    let start = navigation_start(entries.len(), current, 1);
    (0..entries.len())
        .map(|offset| (start + offset) % entries.len())
        .find(|index| label_starts_with(&entries[*index], &prefix))
}

fn label_starts_with<Entry: DropdownMenuEntryAccess>(entry: &Entry, prefix: &str) -> bool {
    entry.entry_is_focusable()
        && entry
            .entry_label()
            .is_some_and(|label| label.to_lowercase().starts_with(prefix))
}

pub(crate) fn flatten_entry_paths<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
) -> Vec<Vec<usize>> {
    let mut paths = Vec::new();
    append_entry_paths(entries, &mut Vec::new(), &mut paths);
    paths
}

fn append_entry_paths<Entry: DropdownMenuEntryAccess>(
    entries: &[Entry],
    parent: &mut Vec<usize>,
    paths: &mut Vec<Vec<usize>>,
) {
    for (index, entry) in entries.iter().enumerate() {
        parent.push(index);
        paths.push(parent.clone());
        if let Some(children) = entry.child_entries() {
            append_entry_paths(children, parent, paths);
        }
        parent.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_entries() -> Vec<DropdownMenuEntry<'static, u8>> {
        vec![
            DropdownMenuEntry::separator(),
            DropdownMenuEntry::disabled_item("Disabled"),
            DropdownMenuEntry::submenu(
                "More",
                vec![
                    DropdownMenuEntry::item("Alpha", 1),
                    DropdownMenuEntry::separator(),
                    DropdownMenuEntry::item("Beta", 2),
                ],
            ),
            DropdownMenuEntry::item("Bravo", 3),
        ]
    }

    #[test]
    fn owned_conversion_clones_recursive_labels_and_messages() {
        let owned = to_owned_entries(&nested_entries());
        assert_eq!(owned[1].label(), Some("Disabled"));
        assert!(owned[1].is_disabled());
        let children = owned[2].submenu_entries().unwrap();
        assert_eq!(children[0].action_message(), Some(&1));
        assert_eq!(children[2].label(), Some("Beta"));
        assert!(owned[2].is_activatable());
    }

    #[test]
    fn owned_role_values_match_the_borrowed_model() {
        let borrowed = vec![
            DropdownMenuEntry::checkbox("Check", true, 1_u8),
            DropdownMenuEntry::radio("Radio", false, 2),
            DropdownMenuEntry::separator(),
        ];
        let owned = to_owned_entries(&borrowed);
        assert_eq!(owned[0].checked(), Some(true));
        assert_eq!(owned[1].selected(), Some(false));
        assert_eq!(owned[2].kind(), DropdownMenuEntryKind::Separator);
        assert!(!owned[2].is_focusable());
    }

    #[test]
    fn navigation_skips_separators_and_wraps_over_disabled_entries() {
        let entries = nested_entries();
        assert_eq!(first_focusable_index(&entries), Some(1));
        assert_eq!(last_focusable_index(&entries), Some(3));
        assert_eq!(next_focusable_index(&entries, Some(3)), Some(1));
        assert_eq!(previous_focusable_index(&entries, Some(1)), Some(3));
        assert_eq!(next_focusable_index(&entries, None), Some(1));
        assert_eq!(previous_focusable_index(&entries, None), Some(3));
    }

    #[test]
    fn typeahead_starts_after_current_and_wraps_case_insensitively() {
        let entries = nested_entries();
        assert_eq!(typeahead_prefix_match(&entries, Some(3), "dIS"), Some(1));
        assert_eq!(typeahead_prefix_match(&entries, Some(1), "br"), Some(3));
        assert_eq!(typeahead_prefix_match(&entries, None, "missing"), None);
    }

    #[test]
    fn nested_traversal_and_flattening_preserve_stable_paths() {
        let entries = nested_entries();
        assert_eq!(
            entry_at_path(&entries, &[2, 2]).unwrap().label(),
            Some("Beta")
        );
        assert_eq!(entries_at_level(&entries, &[2]).unwrap().len(), 3);
        assert_eq!(
            flatten_entry_paths(&entries),
            vec![
                vec![0],
                vec![1],
                vec![2],
                vec![2, 0],
                vec![2, 1],
                vec![2, 2],
                vec![3],
            ]
        );
    }

    #[test]
    fn empty_navigation_and_invalid_paths_return_none() {
        let empty: [DropdownMenuEntry<'_, u8>; 0] = [];
        assert_eq!(next_focusable_index(&empty, None), None);
        assert_eq!(previous_focusable_index(&empty, None), None);
        assert_eq!(entry_at_path(&nested_entries(), &[9]), None);
        assert_eq!(entries_at_level(&nested_entries(), &[1]), None);
    }
}
