// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::{HashMap, hash_map::Entry};
use std::num::NonZeroU64;

use crate::widget::{AUTO_ID, DialogAction, Widget, WidgetIdTag};
pub use crate::widget_id_error::WidgetIdError;
use crate::widget_structure::{WidgetStructureKind, widget_structure_kind};
use crate::widgets::dropdown_menu::{DropdownMenuEntryKind, entry_at_path};

pub(crate) const AUTOMATIC_ID_NAMESPACE_BIT: u64 = 1 << 63;
const ACCESSIBILITY_PATH_HASH_OFFSET: u64 = 0x6a09e667f3bcc909;
const ACCESSIBILITY_PATH_HASH_PRIME: u64 = 0x100000001b3;
const ACCESSIBILITY_PATH_TAG: u64 = 31;

/// A validated, non-zero ID in the namespace reserved for manually keyed widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(NonZeroU64);

impl WidgetId {
    /// Validates a stable manual widget ID before it is assigned to a widget.
    ///
    /// ```
    /// use rutter::WidgetId;
    /// let id = WidgetId::manual(42).unwrap();
    /// assert_eq!(id.get(), 42);
    /// ```
    pub fn manual(raw: u64) -> Result<Self, WidgetIdError> {
        match NonZeroU64::new(raw) {
            Some(value) if raw & AUTOMATIC_ID_NAMESPACE_BIT == 0 => Ok(Self(value)),
            _ => Err(WidgetIdError::ReservedValue { value: raw }),
        }
    }

    /// Returns the validated raw value for a legacy `id: u64` field.
    ///
    /// ```
    /// use rutter::WidgetId;
    /// assert_eq!(WidgetId::manual(7).unwrap().get(), 7);
    /// ```
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

type WidgetIdResult<T = ()> = Result<T, WidgetIdError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WidgetIdOrigin {
    Manual,
    Automatic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WidgetIdOwner {
    value: u64,
    family: WidgetIdTag,
    widget_type: &'static str,
    path: Vec<usize>,
    origin: WidgetIdOrigin,
}

/// Immutable ownership snapshot used to compare widget-tree reconstructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetIdSnapshot {
    owners: HashMap<u64, WidgetIdOwner>,
    structure: Vec<(usize, WidgetStructureKind)>,
}

impl WidgetIdSnapshot {
    /// Validates all resolved IDs in a widget tree and captures their owners.
    /// Hidden declared branches reserve their IDs. Widgets materialized by virtual
    /// item callbacks are excluded because rendering isolates them from runtime state.
    ///
    /// ```
    /// use rutter::{Widget, WidgetIdSnapshot};
    /// use taffy::prelude::Style;
    /// let tree: Widget<'_, ()> = Widget::spinner(Style::default());
    /// WidgetIdSnapshot::capture(&tree).unwrap();
    /// ```
    pub fn capture<Msg>(root: &Widget<'_, Msg>) -> WidgetIdResult<Self> {
        let mut visitor = WidgetIdVisitor::new();
        visitor.visit(root)?;
        Ok(visitor.snapshot)
    }

    /// Rejects incompatible reuse while allowing a manual key to move paths.
    ///
    /// ```
    /// use rutter::{Widget, WidgetIdSnapshot};
    /// use taffy::prelude::Style;
    /// let first: Widget<'_, ()> = Widget::spinner(Style::default()).with_id(7);
    /// let next: Widget<'_, ()> = Widget::spinner(Style::default()).with_id(7);
    /// WidgetIdSnapshot::capture(&first).unwrap()
    ///     .validate_transition_to(&WidgetIdSnapshot::capture(&next).unwrap()).unwrap();
    /// ```
    pub fn validate_transition_to(&self, next: &Self) -> WidgetIdResult {
        for (value, next_owner) in &next.owners {
            if let Some(previous_owner) = self.owners.get(value) {
                validate_owner_transition(*value, previous_owner, next_owner)?;
            }
        }
        Ok(())
    }

    /// Requires a rebuilt tree to contain exactly the previously validated owners.
    ///
    /// ```
    /// use rutter::{Widget, WidgetIdSnapshot};
    /// use taffy::prelude::Style;
    /// let tree: Widget<'_, ()> = Widget::spinner(Style::default());
    /// let first = WidgetIdSnapshot::capture(&tree).unwrap();
    /// let rebuilt = WidgetIdSnapshot::capture(&tree).unwrap();
    /// first.validate_reconstruction(&rebuilt).unwrap();
    /// ```
    pub fn validate_reconstruction(&self, rebuilt: &Self) -> WidgetIdResult {
        let changed_value = self
            .owners
            .keys()
            .chain(rebuilt.owners.keys())
            .find(|value| self.owners.get(*value) != rebuilt.owners.get(*value));
        if let Some(value) = changed_value.copied() {
            return Err(inconsistent_tree_error(value, self, rebuilt));
        }
        validate_structure(&self.structure, &rebuilt.structure)
    }

    pub(crate) fn validate_owner_type(
        &self,
        value: u64,
        expected_type: &'static str,
    ) -> WidgetIdResult {
        let actual_type = self.owners.get(&value).map(|owner| owner.widget_type);
        if actual_type == Some(expected_type) {
            return Ok(());
        }
        Err(WidgetIdError::UnexpectedOwner {
            value,
            actual_type,
            expected_type,
        })
    }

    fn insert_owner(&mut self, owner: WidgetIdOwner) -> WidgetIdResult {
        match self.owners.entry(owner.value) {
            Entry::Occupied(entry) => Err(duplicate_owner_error(entry.get(), &owner)),
            Entry::Vacant(entry) => {
                entry.insert(owner);
                Ok(())
            }
        }
    }
}

struct WidgetIdVisitor {
    snapshot: WidgetIdSnapshot,
    path: Vec<usize>,
}

impl WidgetIdVisitor {
    fn new() -> Self {
        Self {
            snapshot: WidgetIdSnapshot {
                owners: HashMap::new(),
                structure: Vec::new(),
            },
            path: Vec::new(),
        }
    }

    fn visit<Msg>(&mut self, widget: &Widget<'_, Msg>) -> WidgetIdResult {
        self.snapshot
            .structure
            .push((self.path.len(), widget_structure_kind(widget)));
        let origin = self.register_primary(widget)?;
        self.register_subwidgets(widget, origin)?;
        self.visit_children(widget)
    }

    fn visit_children<Msg>(&mut self, widget: &Widget<'_, Msg>) -> WidgetIdResult {
        match widget {
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                self.visit_indexed(children)
            }
            Widget::Container { child, .. }
            | Widget::ButtonContent { child, .. }
            | Widget::ScrollView { child, .. }
            | Widget::Tooltip { child, .. }
            | Widget::Accordion { child, .. }
            | Widget::Modal { child, .. }
            | Widget::Dialog { child, .. }
            | Widget::ContextMenu { child, .. } => self.visit_single(child, 0),
            Widget::Popover {
                anchor, content, ..
            } => self.visit_popover(anchor, content),
            _ => Ok(()),
        }
    }

    fn visit_indexed<Msg>(&mut self, children: &[Widget<'_, Msg>]) -> WidgetIdResult {
        for (index, child) in children.iter().enumerate() {
            self.visit_single(child, index)?;
        }
        Ok(())
    }

    fn visit_popover<Msg>(
        &mut self,
        anchor: &Widget<'_, Msg>,
        content: &Widget<'_, Msg>,
    ) -> WidgetIdResult {
        self.visit_single(anchor, 0)?;
        self.visit_single(content, 1)
    }

    fn visit_single<Msg>(&mut self, child: &Widget<'_, Msg>, index: usize) -> WidgetIdResult {
        self.path.push(index);
        let result = self.visit(child);
        self.path.pop();
        result
    }

    fn register_primary<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
    ) -> WidgetIdResult<Option<WidgetIdOrigin>> {
        let Some((raw_id, family, widget_type)) = widget.id_owner_metadata() else {
            self.register_accessibility_leaf(widget)?;
            return Ok(None);
        };
        let origin = validate_owner_origin(raw_id)?;
        let value = widget
            .resolved_id(&self.path)
            .or_else(|| widget.keyboard_focus_id(&self.path));
        let Some(value) = value else { return Ok(None) };
        self.snapshot.insert_owner(WidgetIdOwner {
            value,
            family,
            widget_type,
            path: self.path.clone(),
            origin,
        })?;
        Ok(Some(origin))
    }

    fn register_accessibility_leaf<Msg>(&mut self, widget: &Widget<'_, Msg>) -> WidgetIdResult {
        let widget_type = match widget {
            Widget::Text { .. } | Widget::RichText { .. } => "AccessibilityText",
            Widget::Image { .. } => "AccessibilityImage",
            _ => return Ok(()),
        };
        self.snapshot.insert_owner(WidgetIdOwner {
            value: resolve_accessibility_path_id(&self.path),
            family: WidgetIdTag::AccessibilityLeaf,
            widget_type,
            path: self.path.clone(),
            origin: WidgetIdOrigin::Automatic,
        })
    }

    fn register_subwidgets<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        origin: Option<WidgetIdOrigin>,
    ) -> WidgetIdResult {
        let Some(origin) = origin else { return Ok(()) };
        match widget {
            Widget::TabBar { tabs, .. } => self.register_tabs(widget, tabs.len(), origin),
            Widget::Dialog { .. } => {
                self.register_dialog(widget, DialogAction::Confirm, origin)?;
                self.register_dialog(widget, DialogAction::Cancel, origin)
            }
            Widget::DropdownMenu { .. } => self.register_dropdown_menu(widget, origin),
            _ => Ok(()),
        }
    }

    fn register_tabs<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        count: usize,
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        for index in 0..count {
            if let Some(value) = widget.tab_focus_id(&self.path, index) {
                self.insert_subwidget(value, WidgetIdTag::Tab, "Tab", &[index], origin)?;
            }
        }
        Ok(())
    }

    fn register_dialog<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        action: DialogAction,
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        let Some(value) = widget.dialog_action_focus_id(&self.path, action) else {
            return Ok(());
        };
        let (family, widget_type, slot) = match action {
            DialogAction::Confirm => (WidgetIdTag::DialogConfirm, "DialogConfirm", 0),
            DialogAction::Cancel => (WidgetIdTag::DialogCancel, "DialogCancel", 1),
        };
        self.insert_subwidget(value, family, widget_type, &[slot], origin)
    }

    fn register_dropdown_menu<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        let value = widget.dropdown_menu_popup_id(&self.path).unwrap();
        self.insert_subwidget(
            value,
            WidgetIdTag::DropdownMenuPopup,
            "DropdownMenuPopup",
            &[],
            origin,
        )?;
        self.register_dropdown_items(widget, origin)
    }

    fn register_dropdown_items<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        for entry_path in widget.dropdown_menu_item_paths() {
            self.register_submenu_popup(widget, &entry_path, origin)?;
            let value = widget
                .dropdown_menu_item_focus_id(&self.path, &entry_path)
                .unwrap();
            self.insert_subwidget(
                value,
                WidgetIdTag::DropdownMenuItem,
                "DropdownMenuItem",
                &entry_path,
                origin,
            )?;
        }
        Ok(())
    }

    fn register_submenu_popup<Msg>(
        &mut self,
        widget: &Widget<'_, Msg>,
        entry_path: &[usize],
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        let Widget::DropdownMenu { entries, .. } = widget else {
            return Ok(());
        };
        if entry_at_path(entries, entry_path).map(|entry| entry.kind())
            != Some(DropdownMenuEntryKind::Submenu)
        {
            return Ok(());
        }
        let value = widget
            .dropdown_menu_submenu_popup_id(&self.path, entry_path)
            .unwrap();
        self.insert_subwidget(
            value,
            WidgetIdTag::DropdownMenuPopup,
            "DropdownMenuPopup",
            entry_path,
            origin,
        )
    }

    fn insert_subwidget(
        &mut self,
        value: u64,
        family: WidgetIdTag,
        widget_type: &'static str,
        subwidget_path: &[usize],
        origin: WidgetIdOrigin,
    ) -> WidgetIdResult {
        let mut path = self.path.clone();
        path.extend_from_slice(subwidget_path);
        self.snapshot.insert_owner(WidgetIdOwner {
            value,
            family,
            widget_type,
            path,
            origin,
        })
    }
}

pub(crate) fn validate_widget_id_snapshot<Msg>(
    root: &Widget<'_, Msg>,
) -> WidgetIdResult<WidgetIdSnapshot> {
    WidgetIdSnapshot::capture(root)
}

pub(crate) fn resolve_accessibility_path_id(path: &[usize]) -> u64 {
    let mut hash = ACCESSIBILITY_PATH_HASH_OFFSET ^ ACCESSIBILITY_PATH_TAG;
    for &segment in path {
        hash = hash.wrapping_mul(ACCESSIBILITY_PATH_HASH_PRIME);
        hash ^= (segment as u64).wrapping_add(1);
    }
    if hash == 0 {
        AUTOMATIC_ID_NAMESPACE_BIT
    } else {
        hash
    }
}

fn validate_owner_origin(raw: Option<u64>) -> WidgetIdResult<WidgetIdOrigin> {
    match raw {
        None | Some(AUTO_ID) => Ok(WidgetIdOrigin::Automatic),
        Some(raw) => {
            WidgetId::manual(raw)?;
            Ok(WidgetIdOrigin::Manual)
        }
    }
}

fn validate_owner_transition(
    value: u64,
    previous: &WidgetIdOwner,
    next: &WidgetIdOwner,
) -> WidgetIdResult {
    let automatic_moved =
        previous.origin == WidgetIdOrigin::Automatic && previous.path != next.path;
    if previous.family == next.family
        && previous.widget_type == next.widget_type
        && previous.origin == next.origin
        && !automatic_moved
    {
        return Ok(());
    }
    Err(WidgetIdError::IncompatibleReuse {
        value,
        previous_type: previous.widget_type,
        previous_path: previous.path.clone(),
        next_type: next.widget_type,
        next_path: next.path.clone(),
    })
}

fn validate_structure(
    validated: &[(usize, WidgetStructureKind)],
    rebuilt: &[(usize, WidgetStructureKind)],
) -> WidgetIdResult {
    let mismatch = (0..validated.len().max(rebuilt.len()))
        .find(|index| validated.get(*index) != rebuilt.get(*index));
    let Some(index) = mismatch else { return Ok(()) };
    Err(WidgetIdError::InconsistentStructure {
        index,
        validated_type: validated.get(index).map(|(_, kind)| kind.label()),
        rebuilt_type: rebuilt.get(index).map(|(_, kind)| kind.label()),
    })
}

fn duplicate_owner_error(first: &WidgetIdOwner, second: &WidgetIdOwner) -> WidgetIdError {
    WidgetIdError::Duplicate {
        value: first.value,
        first_type: first.widget_type,
        first_path: first.path.clone(),
        second_type: second.widget_type,
        second_path: second.path.clone(),
    }
}

fn inconsistent_tree_error(
    value: u64,
    validated: &WidgetIdSnapshot,
    rebuilt: &WidgetIdSnapshot,
) -> WidgetIdError {
    WidgetIdError::InconsistentTree {
        value,
        validated_owner: validated
            .owners
            .get(&value)
            .map(|owner| format!("{owner:?}")),
        rebuilt_owner: rebuilt.owners.get(&value).map(|owner| format!("{owner:?}")),
    }
}

#[cfg(test)]
#[path = "../tests/unit/dropdown_menu_id_unit_tests.rs"]
mod dropdown_menu_tests;
