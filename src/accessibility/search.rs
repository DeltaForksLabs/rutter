// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use accesskit::{Action, AutoComplete, HasPopup, Node, NodeId, Orientation, Rect, Role};
use taffy::prelude::{NodeId as TaffyNodeId, TaffyTree};

use super::access_node_id;
use crate::engine::widget_state::WidgetState;
use crate::input_state::InputWidgetState;
use crate::layout::RutterContext;
use crate::render::search_overlay::{search_option_rect, search_popup_layout};
use crate::render::select_overlay::collector::collect_open_search_overlays;
use crate::widget::{Widget, resolve_search_popup_id, resolve_search_suggestion_id};

pub(super) struct SearchAccessibilityPopup {
    pub(super) listbox_id: NodeId,
    bounds: Rect,
    options: Vec<SearchAccessibilityOption>,
    active_id: Option<NodeId>,
    empty_label: String,
}

struct SearchAccessibilityOption {
    id: NodeId,
    label: String,
    bounds: Option<Rect>,
    selectable: bool,
    selected: bool,
}

pub(super) fn collect_search_accessibility_popups<Msg>(
    widget: &Widget<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    root: TaffyNodeId,
    widget_states: &HashMap<u64, WidgetState>,
    input_states: &HashMap<u64, InputWidgetState>,
    focused_id: Option<u64>,
    viewport: (f32, f32),
) -> HashMap<u64, SearchAccessibilityPopup> {
    collect_open_search_overlays(
        widget,
        taffy,
        root,
        widget_states,
        input_states,
        focused_id,
        viewport,
    )
    .into_iter()
    .map(|overlay| (overlay.id, popup_from_overlay(&overlay, viewport)))
    .collect()
}

pub(super) fn configure_search_combobox(
    node: &mut Node,
    popup: Option<&SearchAccessibilityPopup>,
) -> Vec<(NodeId, Node)> {
    node.set_has_popup(HasPopup::Listbox);
    node.set_expanded(popup.is_some());
    node.set_auto_complete(AutoComplete::List);
    node.add_action(Action::Focus);
    node.add_action(if popup.is_some() {
        Action::Collapse
    } else {
        Action::Expand
    });
    let Some(popup) = popup else {
        return Vec::new();
    };
    node.set_controls(vec![popup.listbox_id]);
    if let Some(active_id) = popup.active_id {
        node.set_active_descendant(active_id);
    }
    popup_nodes(popup)
}

fn popup_from_overlay(
    overlay: &crate::render::select_overlay::collector::SearchOverlay<'_>,
    viewport: (f32, f32),
) -> SearchAccessibilityPopup {
    let popup = search_popup_layout(overlay, viewport);
    let options = overlay
        .matches
        .iter()
        .enumerate()
        .map(|(row, matched)| SearchAccessibilityOption {
            id: access_node_id(resolve_search_suggestion_id(overlay.id, matched.index)),
            label: overlay.items[matched.index].to_owned(),
            bounds: visible_option_bounds(popup, row),
            selectable: overlay.selectable,
            selected: overlay.hovered_option == Some(row),
        })
        .collect();
    SearchAccessibilityPopup {
        listbox_id: access_node_id(resolve_search_popup_id(overlay.id)),
        bounds: super::access_rect(popup.rect),
        options,
        active_id: active_option_id(overlay),
        empty_label: overlay.empty_label.to_owned(),
    }
}

fn visible_option_bounds(
    popup: crate::render::select_overlay::SelectPopupLayout,
    row: usize,
) -> Option<Rect> {
    let visible_row = row.checked_sub(popup.first_option)?;
    (visible_row < popup.visible_options)
        .then(|| super::access_rect(search_option_rect(popup, visible_row)))
}

fn active_option_id(
    overlay: &crate::render::select_overlay::collector::SearchOverlay<'_>,
) -> Option<NodeId> {
    let row = overlay.hovered_option?;
    let matched = overlay.matches.get(row)?;
    Some(access_node_id(resolve_search_suggestion_id(
        overlay.id,
        matched.index,
    )))
}

fn popup_nodes(popup: &SearchAccessibilityPopup) -> Vec<(NodeId, Node)> {
    let option_ids: Vec<NodeId> = popup.options.iter().map(|option| option.id).collect();
    let mut nodes = popup
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| option_node(option, index, popup.options.len()))
        .collect::<Vec<_>>();
    let mut listbox = Node::new(Role::ListBox);
    listbox.set_bounds(popup.bounds);
    listbox.set_orientation(Orientation::Vertical);
    listbox.set_size_of_set(popup.options.len());
    listbox.set_children(option_ids);
    if popup.options.is_empty() {
        listbox.set_label(popup.empty_label.as_str());
    }
    nodes.push((popup.listbox_id, listbox));
    nodes
}

fn option_node(
    option: &SearchAccessibilityOption,
    index: usize,
    option_count: usize,
) -> (NodeId, Node) {
    let mut node = Node::new(Role::ListBoxOption);
    node.set_label(option.label.as_str());
    node.set_position_in_set(index);
    node.set_size_of_set(option_count);
    node.set_selected(option.selected);
    node.add_action(Action::Focus);
    if option.selectable {
        node.add_action(Action::Click);
    }
    if let Some(bounds) = option.bounds {
        node.set_bounds(bounds);
    }
    (option.id, node)
}
