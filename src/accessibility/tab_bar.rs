// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use accesskit::{Action, Node, NodeId, Orientation, Rect, Role};

use super::{AccessibilityBuilder, LayoutFrame, access_node_id};
use crate::widget::Widget;

pub(super) fn collect<Msg>(
    builder: &mut AccessibilityBuilder<'_, '_>,
    widget: &Widget<'_, Msg>,
    tabs: &[&str],
    active: usize,
    frame: LayoutFrame,
    path: &[usize],
) -> Vec<NodeId> {
    let id = access_node_id(widget.resolved_id(path).unwrap());
    let mut children = Vec::with_capacity(tabs.len());
    let width = (frame.rect.x1 - frame.rect.x0) / tabs.len().max(1) as f64;
    for (index, label) in tabs.iter().enumerate() {
        let tab_id = access_node_id(widget.tab_focus_id(path, index).unwrap());
        let mut tab = Node::new(Role::Tab);
        tab.set_label(*label);
        tab.set_selected(index == active);
        // Match the physical tab order used by painting and pointer hit testing,
        // including when the surrounding layout direction is RTL.
        tab.set_bounds(Rect::new(
            frame.rect.x0 + index as f64 * width,
            frame.rect.y0,
            frame.rect.x0 + (index + 1) as f64 * width,
            frame.rect.y1,
        ));
        tab.add_action(Action::Focus);
        tab.add_action(Action::Click);
        children.push(tab_id);
        builder.nodes.push((tab_id, tab));
    }
    let mut list = Node::new(Role::TabList);
    list.set_bounds(frame.rect);
    list.set_orientation(Orientation::Horizontal);
    list.set_children(children);
    builder.nodes.push((id, list));
    vec![id]
}
