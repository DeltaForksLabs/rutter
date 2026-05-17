// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use accesskit::{
    Action, ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node, NodeId,
    Rect, Role, Toggled, Tree, TreeId, TreeUpdate,
};
use skia_safe::Point;
use taffy::prelude::{NodeId as TaffyNodeId, TaffyTree};

use crate::input_state::InputWidgetState;
use crate::layout::RutterContext;
use crate::widget::{DialogAction, Widget};

const ROOT_ACCESSIBILITY_ID: u64 = 1;
const PATH_HASH_OFFSET: u64 = 0x6a09e667f3bcc909;
const PATH_HASH_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Default)]
pub(crate) struct LazyActivationHandler;

impl ActivationHandler for LazyActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        None
    }
}

#[derive(Debug, Default)]
pub(crate) struct IgnoredActionHandler;

impl ActionHandler for IgnoredActionHandler {
    fn do_action(&mut self, _: ActionRequest) {}
}

#[derive(Debug, Default)]
pub(crate) struct IgnoredDeactivationHandler;

impl DeactivationHandler for IgnoredDeactivationHandler {
    fn deactivate_accessibility(&mut self) {}
}

#[derive(Clone, Copy)]
pub(crate) struct AccessibilityInputs<'a> {
    pub input_states: &'a HashMap<u64, InputWidgetState>,
    pub focused_widget_id: Option<u64>,
}

pub(crate) fn build_accessibility_update<Msg>(
    taffy: &TaffyTree<RutterContext>,
    widget: &Widget<Msg>,
    root_node: TaffyNodeId,
    inputs: AccessibilityInputs<'_>,
) -> TreeUpdate {
    let mut builder = AccessibilityBuilder::new(taffy, inputs);
    let children = builder.collect(
        widget,
        Some(root_node),
        Point::new(0.0, 0.0),
        &mut Vec::new(),
    );
    builder.finish(children)
}

struct AccessibilityBuilder<'a> {
    taffy: &'a TaffyTree<RutterContext>,
    inputs: AccessibilityInputs<'a>,
    nodes: Vec<(NodeId, Node)>,
}

impl<'a> AccessibilityBuilder<'a> {
    fn new(taffy: &'a TaffyTree<RutterContext>, inputs: AccessibilityInputs<'a>) -> Self {
        Self {
            taffy,
            inputs,
            nodes: Vec::new(),
        }
    }

    fn finish(mut self, children: Vec<NodeId>) -> TreeUpdate {
        let root = NodeId(ROOT_ACCESSIBILITY_ID);
        let focus = self.focus_node_id(root);
        let mut root_node = Node::new(Role::Window);
        root_node.set_children(children);
        self.nodes.push((root, root_node));
        TreeUpdate {
            nodes: self.nodes,
            tree: Some(Tree::new(root)),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    fn focus_node_id(&self, root: NodeId) -> NodeId {
        self.inputs
            .focused_widget_id
            .map(access_node_id)
            .unwrap_or(root)
    }

    fn collect<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        node: Option<TaffyNodeId>,
        abs: Point,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let frame = LayoutFrame::from_taffy(self.taffy, node, abs);
        match widget {
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                self.collect_children(children, node, frame.origin, path)
            }
            Widget::Container { child, .. } | Widget::Tooltip { child, .. } => {
                self.collect_single_child(child, node, frame.origin, path)
            }
            Widget::ScrollView { child, .. } => {
                self.collect_scroll_view(widget, child, node, frame, path)
            }
            Widget::Popover {
                anchor,
                content,
                open,
                ..
            } => self.collect_popover(anchor, content, *open, node, frame.origin, path),
            Widget::Accordion {
                child, expanded, ..
            } => self.collect_accordion(widget, child, *expanded, node, frame, path),
            Widget::Modal { child, visible, .. } => {
                self.collect_modal(widget, child, *visible, node, frame, path)
            }
            Widget::Dialog { child, visible, .. } => {
                self.collect_dialog(widget, child, *visible, node, frame, path)
            }
            _ => self.collect_leaf(widget, frame, path),
        }
    }

    fn collect_children<Msg>(
        &mut self,
        children: &[Widget<Msg>],
        node: Option<TaffyNodeId>,
        abs: Point,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let node_children = children_for(self.taffy, node);
        let mut ids = Vec::new();
        for (index, child) in children.iter().enumerate() {
            path.push(index);
            ids.extend(self.collect(child, node_children.get(index).copied(), abs, path));
            path.pop();
        }
        ids
    }

    fn collect_single_child<Msg>(
        &mut self,
        child: &Widget<Msg>,
        node: Option<TaffyNodeId>,
        abs: Point,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        path.push(0);
        let ids = self.collect(child, first_child(self.taffy, node), abs, path);
        path.pop();
        ids
    }

    fn collect_scroll_view<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        child: &Widget<Msg>,
        node: Option<TaffyNodeId>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let children = self.collect_single_child(child, node, frame.origin, path);
        self.push_node(
            widget.resolved_id(path).unwrap(),
            Role::ScrollView,
            frame.rect,
            children,
        );
        vec![access_node_id(widget.resolved_id(path).unwrap())]
    }

    fn collect_popover<Msg>(
        &mut self,
        anchor: &Widget<Msg>,
        content: &Widget<Msg>,
        open: bool,
        node: Option<TaffyNodeId>,
        abs: Point,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let node_children = children_for(self.taffy, node);
        let mut ids =
            collect_indexed_child(self, anchor, node_children.first().copied(), abs, path, 0);
        if open {
            ids.extend(collect_indexed_child(
                self,
                content,
                node_children.get(1).copied(),
                abs,
                path,
                1,
            ));
        }
        ids
    }

    fn collect_accordion<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        child: &Widget<Msg>,
        expanded: bool,
        node: Option<TaffyNodeId>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let mut access_node = self.widget_node(widget, Role::DisclosureTriangle, frame.rect, path);
        apply_accordion_props(&mut access_node, widget, expanded);
        let children = expanded.then(|| self.collect_single_child(child, node, frame.origin, path));
        access_node.set_children(children.unwrap_or_default());
        let id = access_node_id(widget.resolved_id(path).unwrap());
        self.nodes.push((id, access_node));
        vec![id]
    }

    fn collect_modal<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        child: &Widget<Msg>,
        visible: bool,
        node: Option<TaffyNodeId>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        if !visible {
            return Vec::new();
        }
        let children = self.collect_single_child(child, node, frame.origin, path);
        self.push_node(
            widget.resolved_id(path).unwrap(),
            Role::Dialog,
            frame.rect,
            children,
        );
        vec![access_node_id(widget.resolved_id(path).unwrap())]
    }

    fn collect_dialog<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        child: &Widget<Msg>,
        visible: bool,
        node: Option<TaffyNodeId>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        if !visible {
            return Vec::new();
        }
        let mut children = self.collect_single_child(child, node, frame.origin, path);
        children.extend(self.push_dialog_actions(widget, frame.rect, path));
        self.push_node(
            widget.resolved_id(path).unwrap(),
            Role::AlertDialog,
            frame.rect,
            children,
        );
        vec![access_node_id(widget.resolved_id(path).unwrap())]
    }

    fn collect_leaf<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        frame: LayoutFrame,
        path: &[usize],
    ) -> Vec<NodeId> {
        let Some((id, node)) = self.leaf_node(widget, frame.rect, path) else {
            return Vec::new();
        };
        self.nodes.push((id, node));
        vec![id]
    }

    fn leaf_node<Msg>(
        &self,
        widget: &Widget<Msg>,
        rect: Rect,
        path: &[usize],
    ) -> Option<(NodeId, Node)> {
        let mut node = self.widget_node(widget, leaf_role(widget)?, rect, path);
        apply_leaf_props(&mut node, widget, self.inputs, path);
        Some((leaf_access_id(widget, path)?, node))
    }

    fn widget_node<Msg>(
        &self,
        widget: &Widget<Msg>,
        role: Role,
        rect: Rect,
        path: &[usize],
    ) -> Node {
        let mut node = Node::new(role);
        node.set_bounds(rect);
        set_widget_label(&mut node, widget, path, self.inputs.input_states);
        node
    }

    fn push_node(&mut self, raw_id: u64, role: Role, rect: Rect, children: Vec<NodeId>) {
        let id = access_node_id(raw_id);
        let mut node = Node::new(role);
        node.set_bounds(rect);
        node.set_children(children);
        self.nodes.push((id, node));
    }

    fn push_dialog_actions<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        rect: Rect,
        path: &[usize],
    ) -> Vec<NodeId> {
        let mut ids = Vec::new();
        if let Some(cancel) = dialog_action_node(widget, rect, path, DialogAction::Cancel) {
            ids.push(cancel.0);
            self.nodes.push(cancel);
        }
        if let Some(confirm) = dialog_action_node(widget, rect, path, DialogAction::Confirm) {
            ids.push(confirm.0);
            self.nodes.push(confirm);
        }
        ids
    }
}

#[derive(Clone, Copy)]
struct LayoutFrame {
    origin: Point,
    rect: Rect,
}

impl LayoutFrame {
    fn from_taffy(taffy: &TaffyTree<RutterContext>, node: Option<TaffyNodeId>, abs: Point) -> Self {
        let Some(layout) = node.and_then(|node| taffy.layout(node).ok()) else {
            return Self::empty(abs);
        };
        let origin = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
        let rect = rect_from_layout(origin, layout.size.width, layout.size.height);
        Self { origin, rect }
    }

    fn empty(origin: Point) -> Self {
        Self {
            origin,
            rect: rect_from_layout(origin, 0.0, 0.0),
        }
    }
}

fn access_node_id(raw_id: u64) -> NodeId {
    NodeId(raw_id.wrapping_mul(2).wrapping_add(2))
}

fn rect_from_layout(origin: Point, width: f32, height: f32) -> Rect {
    Rect::new(
        origin.x as f64,
        origin.y as f64,
        (origin.x + width.max(0.0)) as f64,
        (origin.y + height.max(0.0)) as f64,
    )
}

fn children_for(taffy: &TaffyTree<RutterContext>, node: Option<TaffyNodeId>) -> Vec<TaffyNodeId> {
    node.and_then(|node| taffy.children(node).ok())
        .unwrap_or_default()
}

fn first_child(taffy: &TaffyTree<RutterContext>, node: Option<TaffyNodeId>) -> Option<TaffyNodeId> {
    children_for(taffy, node).into_iter().next()
}

fn collect_indexed_child<Msg>(
    builder: &mut AccessibilityBuilder<'_>,
    widget: &Widget<Msg>,
    node: Option<TaffyNodeId>,
    abs: Point,
    path: &mut Vec<usize>,
    index: usize,
) -> Vec<NodeId> {
    path.push(index);
    let ids = builder.collect(widget, node, abs, path);
    path.pop();
    ids
}

fn leaf_role<Msg>(widget: &Widget<Msg>) -> Option<Role> {
    Some(match widget {
        Widget::Text { .. } => Role::TextRun,
        Widget::Image { .. } => Role::Image,
        Widget::Button { .. } | Widget::ButtonContent { .. } => Role::Button,
        Widget::TextInput { is_password, .. } if *is_password => Role::PasswordInput,
        Widget::TextInput { .. } => Role::TextInput,
        Widget::TextArea { .. } => Role::MultilineTextInput,
        Widget::SearchBar { .. } => Role::SearchInput,
        Widget::Checkbox { .. } => Role::CheckBox,
        Widget::Switch { .. } => Role::Switch,
        Widget::Radio { .. } => Role::RadioButton,
        Widget::Slider { .. } => Role::Slider,
        Widget::Select { .. } => Role::ComboBox,
        Widget::ProgressBar { .. } | Widget::Spinner { .. } => Role::ProgressIndicator,
        Widget::TabBar { .. } => Role::TabList,
        Widget::Toast { visible: true, .. } => Role::Status,
        Widget::VirtualList { .. } | Widget::VirtualListContent { .. } => Role::ListBox,
        Widget::VirtualGrid { .. } | Widget::VirtualGridContent { .. } => Role::Grid,
        _ => return None,
    })
}

fn leaf_access_id<Msg>(widget: &Widget<Msg>, path: &[usize]) -> Option<NodeId> {
    let raw_id = widget
        .keyboard_focus_id(path)
        .or_else(|| widget.resolved_id(path))
        .unwrap_or_else(|| path_access_id(path, 31));
    Some(access_node_id(raw_id))
}

fn path_access_id(path: &[usize], tag: u64) -> u64 {
    let mut hash = PATH_HASH_OFFSET ^ tag;
    for &segment in path {
        hash = hash.wrapping_mul(PATH_HASH_PRIME);
        hash ^= (segment as u64).wrapping_add(1);
    }
    hash
}

fn apply_leaf_props<Msg>(
    node: &mut Node,
    widget: &Widget<Msg>,
    inputs: AccessibilityInputs<'_>,
    path: &[usize],
) {
    apply_actions(node, widget);
    apply_toggle_props(node, widget);
    apply_numeric_props(node, widget);
    apply_input_props(node, widget, inputs.input_states, path);
    apply_collection_props(node, widget);
}

fn apply_actions<Msg>(node: &mut Node, widget: &Widget<Msg>) {
    match widget {
        Widget::Button { .. }
        | Widget::ButtonContent { .. }
        | Widget::Checkbox { .. }
        | Widget::Switch { .. }
        | Widget::Radio { .. }
        | Widget::Select { .. } => node.add_action(Action::Click),
        Widget::Slider { .. } => {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
        }
        _ => {}
    }
}

fn apply_toggle_props<Msg>(node: &mut Node, widget: &Widget<Msg>) {
    match widget {
        Widget::Checkbox { checked, .. } | Widget::Switch { checked, .. } => {
            node.set_toggled(Toggled::from(*checked));
        }
        Widget::Radio { selected, .. } => node.set_selected(*selected),
        _ => {}
    }
}

fn apply_numeric_props<Msg>(node: &mut Node, widget: &Widget<Msg>) {
    match widget {
        Widget::Slider {
            value,
            min,
            max,
            step,
            ..
        } => {
            node.set_numeric_value(*value as f64);
            node.set_min_numeric_value(*min as f64);
            node.set_max_numeric_value(*max as f64);
            node.set_numeric_value_step(*step as f64);
        }
        Widget::ProgressBar {
            value,
            indeterminate,
            ..
        } if !indeterminate => {
            node.set_numeric_value(*value as f64);
        }
        Widget::ProgressBar {
            indeterminate: true,
            ..
        }
        | Widget::Spinner { .. } => node.set_busy(),
        _ => {}
    }
}

fn apply_input_props<Msg>(
    node: &mut Node,
    widget: &Widget<Msg>,
    input_states: &HashMap<u64, InputWidgetState>,
    path: &[usize],
) {
    match widget {
        Widget::TextInput { placeholder, .. }
        | Widget::TextArea { placeholder, .. }
        | Widget::SearchBar { placeholder, .. } => {
            set_nonempty_placeholder(node, placeholder);
            set_input_value(node, widget, input_states, path);
        }
        Widget::Select {
            selected_index,
            options,
            placeholder,
            ..
        } => {
            set_select_value(node, options, *selected_index, placeholder);
        }
        _ => {}
    }
}

fn apply_collection_props<Msg>(node: &mut Node, widget: &Widget<Msg>) {
    match widget {
        Widget::VirtualList { item_count, .. } | Widget::VirtualListContent { item_count, .. } => {
            node.set_size_of_set(*item_count)
        }
        Widget::VirtualGrid {
            item_count,
            columns,
            ..
        }
        | Widget::VirtualGridContent {
            item_count,
            columns,
            ..
        } => {
            node.set_row_count(item_count.div_ceil((*columns).max(1)));
            node.set_column_count((*columns).max(1));
        }
        _ => {}
    }
}

fn apply_accordion_props<Msg>(node: &mut Node, widget: &Widget<Msg>, expanded: bool) {
    if let Widget::Accordion { title, .. } = widget {
        node.set_label(*title);
    }
    node.set_expanded(expanded);
    node.add_action(if expanded {
        Action::Collapse
    } else {
        Action::Expand
    });
    node.add_action(Action::Click);
}

fn set_widget_label<Msg>(
    node: &mut Node,
    widget: &Widget<Msg>,
    path: &[usize],
    input_states: &HashMap<u64, InputWidgetState>,
) {
    match widget {
        Widget::Text { content, .. } => node.set_label(content.clone()),
        Widget::Button { text, .. } => node.set_label(*text),
        Widget::ButtonContent { label, .. } => node.set_label(*label),
        Widget::TextInput { label, .. } | Widget::TextArea { label, .. } => node.set_label(*label),
        Widget::SearchBar { .. } => node.set_label("Search"),
        Widget::Checkbox { label, .. } | Widget::Radio { label, .. } => node.set_label(*label),
        Widget::Slider { label, .. } | Widget::Select { label, .. } => node.set_label(*label),
        Widget::ProgressBar { .. } => node.set_label("Progress"),
        Widget::Spinner { .. } => node.set_label("Loading"),
        Widget::Toast { message, .. } => node.set_label(*message),
        Widget::VirtualList { .. } | Widget::VirtualListContent { .. } => {
            node.set_label("Virtual list")
        }
        Widget::VirtualGrid { .. } | Widget::VirtualGridContent { .. } => {
            node.set_label("Virtual grid")
        }
        _ => set_input_value(node, widget, input_states, path),
    }
}

fn set_nonempty_placeholder(node: &mut Node, placeholder: &str) {
    if !placeholder.is_empty() {
        node.set_placeholder(placeholder);
    }
}

fn set_input_value<Msg>(
    node: &mut Node,
    widget: &Widget<Msg>,
    input_states: &HashMap<u64, InputWidgetState>,
    path: &[usize],
) {
    let Some(id) = widget.resolved_id(path) else {
        return;
    };
    if let Some(input) = input_states.get(&id) {
        node.set_value(input.text());
    }
}

fn set_select_value(node: &mut Node, options: &[&str], selected_index: usize, placeholder: &str) {
    match options.get(selected_index) {
        Some(value) => node.set_value(*value),
        None if !placeholder.is_empty() => node.set_placeholder(placeholder),
        None => {}
    }
}

fn dialog_action_node<Msg>(
    widget: &Widget<Msg>,
    rect: Rect,
    path: &[usize],
    action: DialogAction,
) -> Option<(NodeId, Node)> {
    let Widget::Dialog {
        confirm_label,
        cancel_label,
        ..
    } = widget
    else {
        return None;
    };
    let raw_id = widget.dialog_action_focus_id(path, action)?;
    let mut node = Node::new(Role::Button);
    node.set_bounds(rect);
    node.set_label(match action {
        DialogAction::Confirm => *confirm_label,
        DialogAction::Cancel => *cancel_label,
    });
    node.add_action(Action::Click);
    Some((access_node_id(raw_id), node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use cosmic_text::FontSystem;
    use taffy::prelude::{Dimension, Size, Style};

    use crate::layout::{build_taffy_tree, compute_layout};
    use crate::widget::ButtonVariant;
    use winit::dpi::PhysicalSize;

    fn fs() -> Rc<RefCell<FontSystem>> {
        Rc::new(RefCell::new(FontSystem::new()))
    }

    fn base_style(width: f32, height: f32) -> Style {
        Style {
            size: Size {
                width: Dimension::length(width),
                height: Dimension::length(height),
            },
            ..Style::default()
        }
    }

    fn build_update(widget: &Widget<'_, ()>) -> TreeUpdate {
        let states = HashMap::new();
        let inputs = HashMap::new();
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, widget, fs(), &states);
        compute_layout(&mut taffy, root, PhysicalSize::new(300, 120), fs());
        build_accessibility_update(
            &taffy,
            widget,
            root,
            AccessibilityInputs {
                input_states: &inputs,
                focused_widget_id: None,
            },
        )
    }

    fn node_for(update: &TreeUpdate, role: Role) -> &Node {
        update
            .nodes
            .iter()
            .find_map(|(_, node)| (node.role() == role).then_some(node))
            .unwrap()
    }

    #[test]
    fn accessibility_update_exposes_button_label() {
        let widget = Widget::Button {
            text: "Save",
            on_press: (),
            style: base_style(100.0, 40.0),
            color: None,
            variant: ButtonVariant::Primary,
        };

        let update = build_update(&widget);
        let button = node_for(&update, Role::Button);

        assert_eq!(button.label(), Some("Save"));
        assert!(button.supports_action(Action::Click));
        assert_eq!(
            update.tree.as_ref().unwrap().root,
            NodeId(ROOT_ACCESSIBILITY_ID)
        );
    }

    #[test]
    fn accessibility_update_exposes_button_content_label() {
        let widget = Widget::button_content(
            "Upload image",
            Widget::Image {
                data: &[],
                style: base_style(16.0, 16.0),
                radius: 0.0,
            },
            (),
            base_style(100.0, 40.0),
            None,
            ButtonVariant::Primary,
        );

        let update = build_update(&widget);
        let button = node_for(&update, Role::Button);

        assert_eq!(button.label(), Some("Upload image"));
        assert!(button.supports_action(Action::Click));
    }

    #[test]
    fn accessibility_update_uses_focused_node() {
        let widget = Widget::Button {
            text: "Run",
            on_press: (),
            style: base_style(100.0, 40.0),
            color: None,
            variant: ButtonVariant::Primary,
        };
        let focus_id = widget.keyboard_focus_id(&[]).unwrap();
        let states = HashMap::new();
        let inputs = HashMap::new();
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &widget, fs(), &states);
        compute_layout(&mut taffy, root, PhysicalSize::new(300, 120), fs());

        let update = build_accessibility_update(
            &taffy,
            &widget,
            root,
            AccessibilityInputs {
                input_states: &inputs,
                focused_widget_id: Some(focus_id),
            },
        );

        assert_eq!(update.focus, access_node_id(focus_id));
    }
}
