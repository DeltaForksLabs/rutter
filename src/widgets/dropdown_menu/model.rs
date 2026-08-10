// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

/// Semantic role of one dropdown menu entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropdownMenuEntryKind {
    /// A command item.
    Item,
    /// A checkable item.
    Checkbox,
    /// A radio-selection item.
    Radio,
    /// An item that opens a nested menu.
    Submenu,
    /// A non-interactive visual separator.
    Separator,
}

/// One recursively owned menu entry with borrowed display text.
#[derive(Debug, Clone, PartialEq)]
pub enum DropdownMenuEntry<'a, Msg> {
    Item {
        label: &'a str,
        message: Option<Msg>,
        key: Option<u64>,
    },
    Checkbox {
        label: &'a str,
        checked: bool,
        message: Option<Msg>,
        key: Option<u64>,
    },
    Radio {
        label: &'a str,
        selected: bool,
        message: Option<Msg>,
        key: Option<u64>,
    },
    Submenu {
        label: &'a str,
        entries: Vec<Self>,
        enabled: bool,
        key: Option<u64>,
    },
    Separator,
}

impl<'a, Msg> DropdownMenuEntry<'a, Msg> {
    /// Creates an enabled command that emits `message` when activated.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::item("Save", 1_u8);
    /// assert!(entry.is_activatable());
    /// ```
    pub const fn item(label: &'a str, message: Msg) -> Self {
        Self::Item {
            label,
            message: Some(message),
            key: None,
        }
    }

    /// Creates a focusable command that cannot be activated.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::<u8>::disabled_item("Unavailable");
    /// assert!(entry.is_disabled());
    /// ```
    pub const fn disabled_item(label: &'a str) -> Self {
        Self::Item {
            label,
            message: None,
            key: None,
        }
    }

    /// Creates an enabled checkbox command with its current checked value.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::checkbox("Grid", true, 2_u8);
    /// assert_eq!(entry.checked(), Some(true));
    /// ```
    pub const fn checkbox(label: &'a str, checked: bool, message: Msg) -> Self {
        Self::Checkbox {
            label,
            checked,
            message: Some(message),
            key: None,
        }
    }

    /// Creates a focusable checkbox that cannot be toggled.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::<u8>::disabled_checkbox("Grid", false);
    /// assert!(entry.is_disabled());
    /// ```
    pub const fn disabled_checkbox(label: &'a str, checked: bool) -> Self {
        Self::Checkbox {
            label,
            checked,
            message: None,
            key: None,
        }
    }

    /// Creates an enabled radio command with its current selection value.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::radio("Compact", true, 3_u8);
    /// assert_eq!(entry.selected(), Some(true));
    /// ```
    pub const fn radio(label: &'a str, selected: bool, message: Msg) -> Self {
        Self::Radio {
            label,
            selected,
            message: Some(message),
            key: None,
        }
    }

    /// Creates a focusable radio entry that cannot change selection.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::<u8>::disabled_radio("Compact", false);
    /// assert!(entry.is_disabled());
    /// ```
    pub const fn disabled_radio(label: &'a str, selected: bool) -> Self {
        Self::Radio {
            label,
            selected,
            message: None,
            key: None,
        }
    }

    /// Creates an enabled entry that opens recursively owned child entries.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let child = DropdownMenuEntry::item("Copy", 4_u8);
    /// let entry = DropdownMenuEntry::submenu("Edit", vec![child]);
    /// assert_eq!(entry.submenu_entries().unwrap().len(), 1);
    /// ```
    pub fn submenu(label: &'a str, entries: Vec<Self>) -> Self {
        Self::Submenu {
            label,
            entries,
            enabled: true,
            key: None,
        }
    }

    /// Creates a focusable submenu entry whose children cannot be opened.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::<u8>::disabled_submenu("Edit", Vec::new());
    /// assert!(entry.is_disabled());
    /// ```
    pub fn disabled_submenu(label: &'a str, entries: Vec<Self>) -> Self {
        Self::Submenu {
            label,
            entries,
            enabled: false,
            key: None,
        }
    }

    /// Creates a non-focusable visual boundary between groups.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::<u8>::separator();
    /// assert!(!entry.is_focusable());
    /// ```
    pub const fn separator() -> Self {
        Self::Separator
    }

    /// Assigns identity used to distinguish otherwise identical sibling commands.
    ///
    /// ```
    /// # use rutter::dropdown_menu::DropdownMenuEntry;
    /// let entry = DropdownMenuEntry::item("Open", 1_u8).with_key(10);
    /// assert_eq!(entry.key(), Some(10));
    /// ```
    pub fn with_key(mut self, stable_key: u64) -> Self {
        match &mut self {
            Self::Item { key, .. }
            | Self::Checkbox { key, .. }
            | Self::Radio { key, .. }
            | Self::Submenu { key, .. } => *key = Some(stable_key),
            Self::Separator => {}
        }
        self
    }

    /// Returns the explicit identity of a focusable entry.
    pub const fn key(&self) -> Option<u64> {
        match self {
            Self::Item { key, .. }
            | Self::Checkbox { key, .. }
            | Self::Radio { key, .. }
            | Self::Submenu { key, .. } => *key,
            Self::Separator => None,
        }
    }

    /// Returns display text, or `None` for a separator.
    pub const fn label(&self) -> Option<&'a str> {
        match self {
            Self::Item { label, .. }
            | Self::Checkbox { label, .. }
            | Self::Radio { label, .. }
            | Self::Submenu { label, .. } => Some(label),
            Self::Separator => None,
        }
    }

    /// Reports whether activation is intentionally unavailable.
    pub fn is_disabled(&self) -> bool {
        match self {
            Self::Item { message, .. }
            | Self::Checkbox { message, .. }
            | Self::Radio { message, .. } => message.is_none(),
            Self::Submenu { enabled, .. } => !enabled,
            Self::Separator => false,
        }
    }

    /// Reports whether keyboard focus may land on this entry.
    pub const fn is_focusable(&self) -> bool {
        !matches!(self, Self::Separator)
    }

    /// Reports whether this command can emit a message or open a submenu.
    pub fn is_activatable(&self) -> bool {
        self.is_focusable() && !self.is_disabled()
    }

    /// Returns nested entries when this is a submenu.
    pub fn submenu_entries(&self) -> Option<&[Self]> {
        match self {
            Self::Submenu { entries, .. } => Some(entries),
            _ => None,
        }
    }

    /// Returns the action message used by enabled item-like entries.
    pub const fn action_message(&self) -> Option<&Msg> {
        match self {
            Self::Item { message, .. }
            | Self::Checkbox { message, .. }
            | Self::Radio { message, .. } => message.as_ref(),
            Self::Submenu { .. } | Self::Separator => None,
        }
    }

    /// Returns the checkbox value only for checkbox entries.
    pub const fn checked(&self) -> Option<bool> {
        match self {
            Self::Checkbox { checked, .. } => Some(*checked),
            _ => None,
        }
    }

    /// Returns the selection value only for radio entries.
    pub const fn selected(&self) -> Option<bool> {
        match self {
            Self::Radio { selected, .. } => Some(*selected),
            _ => None,
        }
    }

    /// Returns the entry's stable semantic kind.
    pub const fn kind(&self) -> DropdownMenuEntryKind {
        match self {
            Self::Item { .. } => DropdownMenuEntryKind::Item,
            Self::Checkbox { .. } => DropdownMenuEntryKind::Checkbox,
            Self::Radio { .. } => DropdownMenuEntryKind::Radio,
            Self::Submenu { .. } => DropdownMenuEntryKind::Submenu,
            Self::Separator => DropdownMenuEntryKind::Separator,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_expose_all_kinds_and_values() {
        let entries = [
            DropdownMenuEntry::item("Item", 1_u8),
            DropdownMenuEntry::checkbox("Check", true, 2),
            DropdownMenuEntry::radio("Radio", false, 3),
            DropdownMenuEntry::submenu("More", vec![DropdownMenuEntry::item("Child", 4)]),
            DropdownMenuEntry::separator(),
        ];
        assert_eq!(entries[0].kind(), DropdownMenuEntryKind::Item);
        assert_eq!(entries[1].checked(), Some(true));
        assert_eq!(entries[2].selected(), Some(false));
        assert_eq!(entries[3].submenu_entries().unwrap().len(), 1);
        assert_eq!(entries[4].kind(), DropdownMenuEntryKind::Separator);
    }

    #[test]
    fn disabled_constructors_remain_focusable_without_actions() {
        let entries = [
            DropdownMenuEntry::<u8>::disabled_item("Item"),
            DropdownMenuEntry::disabled_checkbox("Check", true),
            DropdownMenuEntry::disabled_radio("Radio", false),
            DropdownMenuEntry::disabled_submenu("More", Vec::new()),
        ];
        assert!(entries.iter().all(DropdownMenuEntry::is_focusable));
        assert!(entries.iter().all(DropdownMenuEntry::is_disabled));
        assert!(entries.iter().all(|entry| !entry.is_activatable()));
    }

    #[test]
    fn labels_messages_and_role_specific_values_are_precise() {
        let item = DropdownMenuEntry::item("Save", 9_u8);
        let separator = DropdownMenuEntry::<u8>::separator();
        assert_eq!(item.label(), Some("Save"));
        assert_eq!(item.action_message(), Some(&9));
        assert_eq!(item.checked(), None);
        assert_eq!(item.selected(), None);
        assert_eq!(separator.label(), None);
        assert!(!separator.is_focusable());
    }

    #[test]
    fn explicit_key_distinguishes_duplicate_command_labels() {
        let entry = DropdownMenuEntry::item("Open", 9_u8).with_key(42);

        assert_eq!(entry.key(), Some(42));
    }
}
