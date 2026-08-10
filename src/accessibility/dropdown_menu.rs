// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use accesskit::{
    Action, HasPopup, Node, NodeId, Orientation as AccessOrientation, Rect, Role, Toggled,
};
use skia_safe::Rect as SkiaRect;

use super::{AccessibilityBuilder, LayoutFrame, access_node_id};
use crate::widget::Widget;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntry, DropdownMenuEntryKind, DropdownMenuState, DropdownMenuSurface,
    build_open_menu_surfaces, entries_at_level, row_rect,
};

pub(super) fn collect<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    label: &str,
    entries: &[DropdownMenuEntry<'_, Msg>],
    frame: LayoutFrame,
    widget_path: &[usize],
) -> Vec<NodeId> {
    let raw_id = widget.resolved_id(widget_path).unwrap();
    if !builder.dropdown_geometries.contains_key(&raw_id) {
        return Vec::new();
    }
    let state = dropdown_state(builder, raw_id);
    let surfaces = menu_surfaces(builder, raw_id, frame, entries, &state);
    let menu_id = open_root_menu(
        builder,
        widget,
        label,
        entries,
        &state,
        &surfaces,
        widget_path,
    );
    let trigger_id = push_trigger(
        builder,
        widget,
        label,
        trigger_bounds(builder, raw_id, frame.rect),
        &state,
        menu_id,
        widget_path,
    );
    match menu_id {
        Some(menu_id) => vec![trigger_id, menu_id],
        None => vec![trigger_id],
    }
}

fn dropdown_state(builder: &AccessibilityBuilder<'_>, id: u64) -> DropdownMenuState {
    let mut state = builder
        .inputs
        .widget_states
        .get(&id)
        .and_then(|state| state.as_dropdown_menu())
        .cloned()
        .unwrap_or_default();
    if !builder.visible_dropdowns.contains(&id) {
        state.close();
    }
    state
}

fn menu_surfaces<Msg>(
    builder: &AccessibilityBuilder<'_>,
    id: u64,
    frame: LayoutFrame,
    entries: &[DropdownMenuEntry<'_, Msg>],
    state: &DropdownMenuState,
) -> Vec<DropdownMenuSurface> {
    let anchor = builder
        .dropdown_geometries
        .get(&id)
        .map(|geometry| geometry.anchor)
        .unwrap_or_else(|| skia_rect(frame.rect));
    let viewport = SkiaRect::from_xywh(
        0.0,
        0.0,
        builder.inputs.viewport.0,
        builder.inputs.viewport.1,
    );
    build_open_menu_surfaces(anchor, entries, state, viewport, builder.inputs.direction)
}

fn trigger_bounds(builder: &AccessibilityBuilder<'_>, id: u64, fallback: Rect) -> Rect {
    builder
        .dropdown_geometries
        .get(&id)
        .map(|geometry| geometry.visible_anchor)
        .map(access_rect)
        .unwrap_or(fallback)
}

fn open_root_menu<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    label: &str,
    entries: &[DropdownMenuEntry<'_, Msg>],
    state: &DropdownMenuState,
    surfaces: &[DropdownMenuSurface],
    widget_path: &[usize],
) -> Option<NodeId> {
    if !state.is_open() {
        return None;
    }
    collect_level(
        builder,
        widget,
        label,
        entries,
        state,
        surfaces,
        widget_path,
        &[],
    )
}

fn push_trigger<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    label: &str,
    bounds: Rect,
    state: &DropdownMenuState,
    menu_id: Option<NodeId>,
    widget_path: &[usize],
) -> NodeId {
    let id = access_node_id(widget.resolved_id(widget_path).unwrap());
    let mut node = Node::new(Role::Button);
    node.set_bounds(bounds);
    node.set_label(label);
    node.set_has_popup(HasPopup::Menu);
    node.set_expanded(state.is_open());
    node.add_action(Action::Focus);
    node.add_action(Action::Click);
    node.add_action(expansion_action(state.is_open()));
    if let Some(menu_id) = menu_id {
        node.set_controls(vec![menu_id]);
    }
    builder.nodes.push((id, node));
    id
}

#[allow(clippy::too_many_arguments)]
fn collect_level<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    label: &str,
    root_entries: &[DropdownMenuEntry<'_, Msg>],
    state: &DropdownMenuState,
    surfaces: &[DropdownMenuSurface],
    widget_path: &[usize],
    level_path: &[usize],
) -> Option<NodeId> {
    let surface = surfaces
        .iter()
        .find(|surface| surface.level_path == level_path)?;
    let entries = entries_at_level(root_entries, level_path)?;
    let children = collect_items(
        builder,
        widget,
        root_entries,
        entries,
        state,
        surfaces,
        widget_path,
        surface,
    );
    let id = menu_node_id(widget, widget_path, level_path)?;
    let mut node = Node::new(Role::Menu);
    node.set_bounds(access_rect(surface.rect));
    node.set_label(label);
    node.set_orientation(AccessOrientation::Vertical);
    node.set_children(children);
    builder.nodes.push((id, node));
    Some(id)
}

#[allow(clippy::too_many_arguments)]
fn collect_items<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    root_entries: &[DropdownMenuEntry<'_, Msg>],
    entries: &[DropdownMenuEntry<'_, Msg>],
    state: &DropdownMenuState,
    surfaces: &[DropdownMenuSurface],
    widget_path: &[usize],
    surface: &DropdownMenuSurface,
) -> Vec<NodeId> {
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            collect_item(
                builder,
                widget,
                root_entries,
                entries,
                entry,
                index,
                state,
                surfaces,
                widget_path,
                surface,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn collect_item<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    root_entries: &[DropdownMenuEntry<'_, Msg>],
    level_entries: &[DropdownMenuEntry<'_, Msg>],
    entry: &DropdownMenuEntry<'_, Msg>,
    index: usize,
    state: &DropdownMenuState,
    surfaces: &[DropdownMenuSurface],
    widget_path: &[usize],
    surface: &DropdownMenuSurface,
) -> Option<NodeId> {
    let role = entry_role(entry.kind())?;
    let path = appended_path(&surface.level_path, index);
    let id = access_node_id(widget.dropdown_menu_item_focus_id(widget_path, &path)?);
    let mut node = menu_item_node(
        entry,
        role,
        access_rect(clipped_row_rect(surface, level_entries, index)?),
    );
    apply_submenu_expansion(&mut node, entry, state, &path);
    if let Some(submenu_id) = collect_submenu(
        builder,
        widget,
        root_entries,
        entry,
        state,
        surfaces,
        widget_path,
        &path,
    ) {
        node.set_children(vec![submenu_id]);
    }
    builder.nodes.push((id, node));
    Some(id)
}

fn clipped_row_rect<Msg>(
    surface: &DropdownMenuSurface,
    entries: &[DropdownMenuEntry<'_, Msg>],
    index: usize,
) -> Option<SkiaRect> {
    let row = row_rect(surface, entries, index)?;
    if row.bottom <= surface.rect.top {
        return Some(SkiaRect::from_xywh(
            surface.rect.left,
            surface.rect.top,
            0.0,
            0.0,
        ));
    }
    if row.top >= surface.rect.bottom {
        return Some(SkiaRect::from_xywh(
            surface.rect.left,
            surface.rect.bottom,
            0.0,
            0.0,
        ));
    }
    Some(SkiaRect::new(
        row.left.max(surface.rect.left),
        row.top.max(surface.rect.top),
        row.right.min(surface.rect.right),
        row.bottom.min(surface.rect.bottom),
    ))
}

#[allow(clippy::too_many_arguments)]
fn collect_submenu<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<'_, Msg>,
    root_entries: &[DropdownMenuEntry<'_, Msg>],
    entry: &DropdownMenuEntry<'_, Msg>,
    state: &DropdownMenuState,
    surfaces: &[DropdownMenuSurface],
    widget_path: &[usize],
    path: &[usize],
) -> Option<NodeId> {
    if entry.kind() != DropdownMenuEntryKind::Submenu
        || entry.is_disabled()
        || !submenu_is_open(state, path)
    {
        return None;
    }
    collect_level(
        builder,
        widget,
        entry.label()?,
        root_entries,
        state,
        surfaces,
        widget_path,
        path,
    )
}

fn menu_item_node<Msg>(entry: &DropdownMenuEntry<'_, Msg>, role: Role, bounds: Rect) -> Node {
    let mut node = Node::new(role);
    node.set_bounds(bounds);
    node.set_label(entry.label().unwrap_or_default());
    node.add_action(Action::Focus);
    apply_item_state(&mut node, entry);
    apply_item_actions(&mut node, entry);
    node
}

fn apply_item_state<Msg>(node: &mut Node, entry: &DropdownMenuEntry<'_, Msg>) {
    if entry.is_disabled() {
        node.set_disabled();
    }
    if let Some(checked) = entry.checked() {
        node.set_toggled(Toggled::from(checked));
    }
    if let Some(selected) = entry.selected() {
        node.set_toggled(Toggled::from(selected));
    }
    if entry.kind() == DropdownMenuEntryKind::Submenu {
        node.set_has_popup(HasPopup::Menu);
    }
}

fn apply_item_actions<Msg>(node: &mut Node, entry: &DropdownMenuEntry<'_, Msg>) {
    if entry.is_disabled() {
        return;
    }
    node.add_action(Action::Click);
}

fn apply_submenu_expansion<Msg>(
    node: &mut Node,
    entry: &DropdownMenuEntry<'_, Msg>,
    state: &DropdownMenuState,
    path: &[usize],
) {
    if entry.kind() != DropdownMenuEntryKind::Submenu {
        return;
    }
    let expanded = !entry.is_disabled() && submenu_is_open(state, path);
    node.set_expanded(expanded);
    if !entry.is_disabled() {
        node.add_action(expansion_action(expanded));
    }
}

fn menu_node_id<Msg>(
    widget: &Widget<'_, Msg>,
    widget_path: &[usize],
    level_path: &[usize],
) -> Option<NodeId> {
    let raw = if level_path.is_empty() {
        widget.dropdown_menu_popup_id(widget_path)?
    } else {
        widget.dropdown_menu_submenu_popup_id(widget_path, level_path)?
    };
    Some(access_node_id(raw))
}

fn entry_role(kind: DropdownMenuEntryKind) -> Option<Role> {
    match kind {
        DropdownMenuEntryKind::Item | DropdownMenuEntryKind::Submenu => Some(Role::MenuItem),
        DropdownMenuEntryKind::Checkbox => Some(Role::MenuItemCheckBox),
        DropdownMenuEntryKind::Radio => Some(Role::MenuItemRadio),
        DropdownMenuEntryKind::Separator => None,
    }
}

fn submenu_is_open(state: &DropdownMenuState, path: &[usize]) -> bool {
    state.open_submenu_path().starts_with(path)
}

fn expansion_action(expanded: bool) -> Action {
    if expanded {
        Action::Collapse
    } else {
        Action::Expand
    }
}

fn appended_path(path: &[usize], index: usize) -> Vec<usize> {
    let mut result = path.to_vec();
    result.push(index);
    result
}

fn skia_rect(rect: Rect) -> SkiaRect {
    SkiaRect::new(
        rect.x0 as f32,
        rect.y0 as f32,
        rect.x1 as f32,
        rect.y1 as f32,
    )
}

fn access_rect(rect: SkiaRect) -> Rect {
    Rect::new(
        rect.left as f64,
        rect.top as f64,
        rect.right as f64,
        rect.bottom as f64,
    )
}

#[cfg(test)]
#[path = "../../tests/unit/dropdown_menu_accessibility_unit_tests.rs"]
mod tests;
