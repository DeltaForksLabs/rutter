// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — layout.rs
// ============================================================

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use crate::engine::widget_state::WidgetState;
use crate::i18n::LayoutDirection;
use crate::widget::Widget;

const ACCORDION_HEADER_H: f32 = 44.0;

pub const OPTION_HEIGHT: f32 = 32.0;
pub const SCROLLBAR_W: f32 = 8.0;
pub const VIRTUAL_GRID_GAP: f32 = 8.0;
pub const VIRTUAL_GRID_PADDING: f32 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TextContext {
    pub content: String,
    pub font_size: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncedLayoutTree {
    node_id: NodeId,
    key: Option<u64>,
    style: Style,
    context: RutterContext,
    children: Vec<SyncedLayoutTree>,
}

impl SyncedLayoutTree {
    pub fn placeholder(node_id: NodeId) -> Self {
        Self {
            node_id,
            key: None,
            style: Style::default(),
            context: RutterContext::None,
            children: Vec::new(),
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RutterContext {
    #[default]
    None,
    Text(TextContext),
}

#[derive(Debug, Clone, PartialEq)]
struct LayoutBlueprint {
    key: Option<u64>,
    style: Style,
    context: RutterContext,
    children: Vec<LayoutBlueprint>,
}

impl LayoutBlueprint {
    fn leaf(key: Option<u64>, style: Style) -> Self {
        Self {
            key,
            style,
            context: RutterContext::None,
            children: Vec::new(),
        }
    }

    fn leaf_with_context(key: Option<u64>, style: Style, context: RutterContext) -> Self {
        Self {
            key,
            style,
            context,
            children: Vec::new(),
        }
    }

    fn with_children(key: Option<u64>, style: Style, children: Vec<Self>) -> Self {
        Self {
            key,
            style,
            context: RutterContext::None,
            children,
        }
    }

    fn from_widget<'a, Msg>(
        widget: &Widget<'a, Msg>,
        widget_states: &HashMap<u64, WidgetState>,
    ) -> Self {
        let mut path = Vec::new();
        Self::from_widget_with_path(widget, widget_states, &mut path)
    }

    fn from_widget_with_direction<'a, Msg>(
        widget: &Widget<'a, Msg>,
        widget_states: &HashMap<u64, WidgetState>,
        direction: LayoutDirection,
    ) -> Self {
        let mut blueprint = Self::from_widget(widget, widget_states);
        blueprint.apply_direction(direction);
        blueprint
    }

    fn apply_direction(&mut self, direction: LayoutDirection) {
        self.style.direction = direction.into();
        for child in &mut self.children {
            child.apply_direction(direction);
        }
    }

    fn from_widget_with_path<'a, Msg>(
        widget: &Widget<'a, Msg>,
        widget_states: &HashMap<u64, WidgetState>,
        path: &mut Vec<usize>,
    ) -> Self {
        match widget {
            Widget::Column { children, style } => {
                let style = Style {
                    flex_direction: FlexDirection::Column,
                    ..style.clone()
                };
                let children = children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        path.push(index);
                        let blueprint = Self::from_widget_with_path(child, widget_states, path);
                        path.pop();
                        blueprint
                    })
                    .collect();
                Self::with_children(None, style, children)
            }
            Widget::Row { children, style } => {
                let style = Style {
                    flex_direction: FlexDirection::Row,
                    ..style.clone()
                };
                let children = children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        path.push(index);
                        let blueprint = Self::from_widget_with_path(child, widget_states, path);
                        path.pop();
                        blueprint
                    })
                    .collect();
                Self::with_children(None, style, children)
            }
            Widget::Container { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, widget_states, path);
                path.pop();
                Self::with_children(None, style.clone(), vec![child])
            }
            Widget::ScrollView { child, style, .. } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                path.push(0);
                let child = Self::from_widget_with_path(child, widget_states, path);
                path.pop();
                Self::with_children(Some(resolved_id), style.clone(), vec![child])
            }
            Widget::Tooltip { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, widget_states, path);
                path.pop();
                Self::with_children(None, style.clone(), vec![child])
            }
            Widget::ContextMenu { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, widget_states, path);
                path.pop();
                Self::with_children(
                    Some(widget.resolved_id(path).unwrap()),
                    style.clone(),
                    vec![child],
                )
            }
            Widget::Popover {
                anchor,
                content,
                open,
                style,
                popup_style,
                ..
            } => {
                path.push(0);
                let anchor = Self::from_widget_with_path(anchor, widget_states, path);
                path.pop();

                let popup = if *open {
                    path.push(1);
                    let content = Self::from_widget_with_path(content, widget_states, path);
                    path.pop();
                    Self::with_children(
                        None,
                        Style {
                            position: Position::Absolute,
                            ..popup_style.clone()
                        },
                        vec![content],
                    )
                } else {
                    Self::leaf(
                        None,
                        Style {
                            position: Position::Absolute,
                            size: Size::zero(),
                            ..popup_style.clone()
                        },
                    )
                };

                Self::with_children(
                    Some(widget.resolved_id(path).unwrap()),
                    style.clone(),
                    vec![anchor, popup],
                )
            }
            Widget::Accordion {
                child,
                style,
                expanded,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                let mut style = style.clone();
                style.padding.top = LengthPercentage::length(ACCORDION_HEADER_H);
                if *expanded {
                    path.push(0);
                    let child = Self::from_widget_with_path(child, widget_states, path);
                    path.pop();
                    Self::with_children(Some(resolved_id), style, vec![child])
                } else {
                    style.size.height = Dimension::length(ACCORDION_HEADER_H);
                    Self::leaf(Some(resolved_id), style)
                }
            }
            Widget::Modal {
                child,
                style,
                visible,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                if *visible {
                    path.push(0);
                    let child = Self::from_widget_with_path(child, widget_states, path);
                    path.pop();
                    Self::with_children(Some(resolved_id), overlay_style(style), vec![child])
                } else {
                    Self::leaf(
                        Some(resolved_id),
                        Style {
                            size: Size::zero(),
                            ..style.clone()
                        },
                    )
                }
            }
            Widget::Dialog {
                child,
                style,
                visible,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                if *visible {
                    path.push(0);
                    let child = Self::from_widget_with_path(child, widget_states, path);
                    path.pop();
                    Self::with_children(Some(resolved_id), style.clone(), vec![child])
                } else {
                    Self::leaf(
                        Some(resolved_id),
                        Style {
                            size: Size::zero(),
                            ..style.clone()
                        },
                    )
                }
            }
            Widget::Select { options, style, .. } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                let is_open = widget_states
                    .get(&resolved_id)
                    .and_then(|s| s.as_select())
                    .map(|s| s.is_open)
                    .unwrap_or(false);
                let style = if is_open {
                    let closed_h = extract_height(style);
                    Style {
                        size: Size {
                            height: Dimension::length(
                                closed_h + options.len() as f32 * OPTION_HEIGHT,
                            ),
                            ..style.size
                        },
                        ..style.clone()
                    }
                } else {
                    style.clone()
                };
                Self::leaf(Some(resolved_id), style)
            }
            Widget::Text {
                content,
                style,
                size,
                ..
            } => Self::leaf_with_context(
                None,
                style.clone(),
                RutterContext::Text(TextContext {
                    content: content.clone(),
                    font_size: *size,
                }),
            ),
            Widget::Button { style, .. }
            | Widget::Checkbox { style, .. }
            | Widget::Divider { style, .. }
            | Widget::Image { style, .. }
            | Widget::Radio { style, .. }
            | Widget::Spacer { style, .. }
            | Widget::Switch { style, .. } => Self::leaf(None, style.clone()),
            Widget::ProgressBar { style, .. }
            | Widget::Spinner { style, .. }
            | Widget::TabBar { style, .. }
            | Widget::TextArea { style, .. }
            | Widget::TextInput { style, .. }
            | Widget::SearchBar { style, .. }
            | Widget::Slider { style, .. }
            | Widget::VirtualList { style, .. }
            | Widget::VirtualGrid { style, .. } => {
                Self::leaf(Some(widget.resolved_id(path).unwrap()), style.clone())
            }
            Widget::Toast { .. } => Self::leaf(
                Some(widget.resolved_id(path).unwrap()),
                Style {
                    size: Size::zero(),
                    ..Default::default()
                },
            ),
        }
    }
}

pub fn build_taffy_tree<'a, Msg>(
    taffy: &mut TaffyTree<RutterContext>,
    widget: &Widget<'a, Msg>,
    _fs: Rc<RefCell<FontSystem>>,
    widget_states: &HashMap<u64, WidgetState>,
) -> NodeId {
    build_taffy_tree_with_direction(
        taffy,
        widget,
        _fs,
        widget_states,
        LayoutDirection::default(),
    )
}

/// Builds a Taffy tree and applies one global layout direction.
///
/// # Example
/// ```rust
/// # use std::{cell::RefCell, collections::HashMap, rc::Rc};
/// # use cosmic_text::FontSystem;
/// # use rutter::{LayoutDirection, Widget};
/// # use rutter::layout::{RutterContext, build_taffy_tree_with_direction};
/// # use rutter::engine::widget_state::WidgetState;
/// # use taffy::prelude::{Style, TaffyTree};
/// let mut taffy = TaffyTree::<RutterContext>::new();
/// let states = HashMap::<u64, WidgetState>::new();
/// let widget: Widget<'static, ()> = Widget::Spacer { style: Style::default() };
/// let root = build_taffy_tree_with_direction(
///     &mut taffy,
///     &widget,
///     Rc::new(RefCell::new(FontSystem::new())),
///     &states,
///     LayoutDirection::Rtl,
/// );
/// assert_eq!(taffy.style(root).unwrap().direction, taffy::style::Direction::Rtl);
/// ```
pub fn build_taffy_tree_with_direction<'a, Msg>(
    taffy: &mut TaffyTree<RutterContext>,
    widget: &Widget<'a, Msg>,
    _fs: Rc<RefCell<FontSystem>>,
    widget_states: &HashMap<u64, WidgetState>,
    direction: LayoutDirection,
) -> NodeId {
    let blueprint = LayoutBlueprint::from_widget_with_direction(widget, widget_states, direction);
    mount_layout_blueprint(taffy, &blueprint).node_id
}

pub fn sync_taffy_tree<'a, Msg>(
    taffy: &mut TaffyTree<RutterContext>,
    tree: &mut SyncedLayoutTree,
    widget: &Widget<'a, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
) -> NodeId {
    sync_taffy_tree_with_direction(
        taffy,
        tree,
        widget,
        widget_states,
        LayoutDirection::default(),
    )
}

/// Syncs a reusable Taffy tree and applies one global layout direction.
///
/// # Example
/// ```rust
/// # use std::collections::HashMap;
/// # use rutter::{LayoutDirection, Widget};
/// # use rutter::layout::{RutterContext, SyncedLayoutTree, sync_taffy_tree_with_direction};
/// # use rutter::engine::widget_state::WidgetState;
/// # use taffy::prelude::{Style, TaffyTree};
/// let mut taffy = TaffyTree::<RutterContext>::new();
/// let root = taffy.new_leaf(Style::default()).unwrap();
/// let mut tree = SyncedLayoutTree::placeholder(root);
/// let states = HashMap::<u64, WidgetState>::new();
/// let widget: Widget<'static, ()> = Widget::Spacer { style: Style::default() };
/// sync_taffy_tree_with_direction(&mut taffy, &mut tree, &widget, &states, LayoutDirection::Rtl);
/// assert_eq!(taffy.style(root).unwrap().direction, taffy::style::Direction::Rtl);
/// ```
pub fn sync_taffy_tree_with_direction<'a, Msg>(
    taffy: &mut TaffyTree<RutterContext>,
    tree: &mut SyncedLayoutTree,
    widget: &Widget<'a, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
    direction: LayoutDirection,
) -> NodeId {
    let blueprint = LayoutBlueprint::from_widget_with_direction(widget, widget_states, direction);
    sync_layout_blueprint(taffy, tree, &blueprint);
    tree.node_id()
}

fn mount_layout_blueprint(
    taffy: &mut TaffyTree<RutterContext>,
    blueprint: &LayoutBlueprint,
) -> SyncedLayoutTree {
    let node_id = match &blueprint.context {
        RutterContext::None => taffy.new_leaf(blueprint.style.clone()).unwrap(),
        _ => taffy
            .new_leaf_with_context(blueprint.style.clone(), blueprint.context.clone())
            .unwrap(),
    };

    let children: Vec<_> = blueprint
        .children
        .iter()
        .map(|child| mount_layout_blueprint(taffy, child))
        .collect();

    if !children.is_empty() {
        let child_ids: Vec<_> = children.iter().map(|child| child.node_id).collect();
        taffy.set_children(node_id, &child_ids).unwrap();
    }

    SyncedLayoutTree {
        node_id,
        key: blueprint.key,
        style: blueprint.style.clone(),
        context: blueprint.context.clone(),
        children,
    }
}

fn sync_layout_blueprint(
    taffy: &mut TaffyTree<RutterContext>,
    tree: &mut SyncedLayoutTree,
    blueprint: &LayoutBlueprint,
) {
    if tree.style != blueprint.style {
        taffy
            .set_style(tree.node_id, blueprint.style.clone())
            .unwrap();
        tree.style = blueprint.style.clone();
    }

    if tree.context != blueprint.context {
        taffy
            .set_node_context(tree.node_id, clone_context(&blueprint.context))
            .unwrap();
        tree.context = blueprint.context.clone();
    }

    tree.key = blueprint.key;
    sync_layout_children(taffy, tree, &blueprint.children);
}

fn sync_layout_children(
    taffy: &mut TaffyTree<RutterContext>,
    tree: &mut SyncedLayoutTree,
    blueprints: &[LayoutBlueprint],
) {
    let old_child_ids: Vec<_> = tree.children.iter().map(|child| child.node_id).collect();
    let old_children = std::mem::take(&mut tree.children);
    let mut keyed_children: HashMap<u64, VecDeque<SyncedLayoutTree>> = HashMap::new();
    let mut unkeyed_children = VecDeque::new();

    for child in old_children {
        if let Some(key) = child.key {
            keyed_children.entry(key).or_default().push_back(child);
        } else {
            unkeyed_children.push_back(child);
        }
    }

    let mut new_children = Vec::with_capacity(blueprints.len());
    for blueprint in blueprints {
        let existing_child = match blueprint.key {
            Some(key) => keyed_children
                .get_mut(&key)
                .and_then(|children| children.pop_front()),
            None => unkeyed_children.pop_front(),
        };

        let child = match existing_child {
            Some(mut child) => {
                sync_layout_blueprint(taffy, &mut child, blueprint);
                child
            }
            None => mount_layout_blueprint(taffy, blueprint),
        };
        new_children.push(child);
    }

    let new_child_ids: Vec<_> = new_children.iter().map(|child| child.node_id).collect();
    if old_child_ids != new_child_ids {
        taffy.set_children(tree.node_id, &new_child_ids).unwrap();
    }

    for queue in keyed_children.into_values() {
        for child in queue {
            remove_layout_subtree(taffy, child);
        }
    }
    for child in unkeyed_children {
        remove_layout_subtree(taffy, child);
    }

    tree.children = new_children;
}

fn remove_layout_subtree(taffy: &mut TaffyTree<RutterContext>, tree: SyncedLayoutTree) {
    for child in tree.children {
        remove_layout_subtree(taffy, child);
    }
    taffy.remove(tree.node_id).unwrap();
}

fn clone_context(context: &RutterContext) -> Option<RutterContext> {
    match context {
        RutterContext::None => None,
        _ => Some(context.clone()),
    }
}

pub fn compute_layout(
    taffy: &mut TaffyTree<RutterContext>,
    root: NodeId,
    size: PhysicalSize<u32>,
    fs_rc: Rc<RefCell<FontSystem>>,
) {
    let available = Size {
        width: AvailableSpace::Definite(size.width as f32),
        height: AvailableSpace::Definite(size.height as f32),
    };
    taffy
        .compute_layout_with_measure(root, available, |known, available, _, ctx, _| {
            let Some(RutterContext::Text(t)) = ctx else {
                return Size::ZERO;
            };
            let mut fs = fs_rc.borrow_mut();
            let mut buf = Buffer::new(&mut fs, Metrics::new(t.font_size, t.font_size * 1.2));
            match available.width {
                AvailableSpace::Definite(px) => buf.set_size(&mut fs, Some(px), None),
                AvailableSpace::MaxContent => buf.set_size(&mut fs, None, None),
                AvailableSpace::MinContent => buf.set_size(&mut fs, Some(0.0), None),
            }
            buf.set_text(&mut fs, &t.content, &Attrs::new(), Shaping::Advanced, None);
            buf.shape_until_scroll(&mut fs, true);
            let (w, h) = buf.size();
            Size {
                width: known.width.unwrap_or(w.unwrap_or(0.0)),
                height: known.height.unwrap_or(h.unwrap_or(0.0)),
            }
        })
        .unwrap();
}

fn extract_height(style: &Style) -> f32 {
    style.size.height.into_option().unwrap_or(40.0)
}

fn overlay_style(style: &Style) -> Style {
    let mut overlay = style.clone();
    overlay.position = Position::Absolute;
    overlay.inset = Rect {
        left: LengthPercentageAuto::length(0.0),
        right: LengthPercentageAuto::length(0.0),
        top: LengthPercentageAuto::length(0.0),
        bottom: LengthPercentageAuto::length(0.0),
    };
    overlay.size = Size {
        width: Dimension::percent(1.0),
        height: Dimension::percent(1.0),
    };
    overlay
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::style::Direction;

    fn empty_states() -> HashMap<u64, WidgetState> {
        HashMap::new()
    }

    fn fs() -> Rc<RefCell<FontSystem>> {
        Rc::new(RefCell::new(FontSystem::new()))
    }

    fn sync_tree<'a>(
        taffy: &mut TaffyTree<RutterContext>,
        tree: &mut SyncedLayoutTree,
        widget: &Widget<'a, ()>,
    ) {
        sync_taffy_tree(taffy, tree, widget, &empty_states());
    }

    fn sync_tree_with_direction<'a>(
        taffy: &mut TaffyTree<RutterContext>,
        tree: &mut SyncedLayoutTree,
        widget: &Widget<'a, ()>,
        direction: LayoutDirection,
    ) {
        sync_taffy_tree_with_direction(taffy, tree, widget, &empty_states(), direction);
    }

    fn text(content: &str, size: f32) -> Widget<'static, ()> {
        Widget::Text {
            content: content.to_string(),
            style: Style::default(),
            color: None,
            size,
        }
    }

    fn button(width: f32) -> Widget<'static, ()> {
        Widget::Button {
            text: "Run",
            on_press: (),
            style: Style {
                size: Size {
                    width: Dimension::length(width),
                    height: Dimension::length(36.0),
                },
                ..Style::default()
            },
            color: None,
            variant: crate::widget::ButtonVariant::Primary,
        }
    }

    fn slider(id: u64, width: f32) -> Widget<'static, ()> {
        Widget::Slider {
            id,
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            on_change: |_| (),
            style: Style {
                size: Size {
                    width: Dimension::length(width),
                    height: Dimension::length(20.0),
                },
                ..Style::default()
            },
            label: "Slider",
        }
    }

    fn accordion(expanded: bool) -> Widget<'static, ()> {
        Widget::Accordion {
            id: 7,
            title: "Section",
            expanded,
            on_toggle: (),
            child: Box::new(text("Inner", 14.0)),
            style: Style::default(),
        }
    }

    #[test]
    fn sync_taffy_tree_reuses_nodes_for_style_and_text_updates() {
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        let mut tree = SyncedLayoutTree::placeholder(root);

        let initial = Widget::Column {
            children: vec![text("hello", 16.0), button(96.0)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &initial);

        let text_id = tree.children[0].node_id;
        let button_id = tree.children[1].node_id;
        assert_eq!(taffy.total_node_count(), 3);

        let updated = Widget::Column {
            children: vec![text("updated", 18.0), button(144.0)],
            style: Style {
                gap: Size {
                    width: LengthPercentage::length(12.0),
                    height: LengthPercentage::length(12.0),
                },
                ..Style::default()
            },
        };
        sync_tree(&mut taffy, &mut tree, &updated);

        assert_eq!(tree.node_id, root);
        assert_eq!(tree.children[0].node_id, text_id);
        assert_eq!(tree.children[1].node_id, button_id);
        assert_eq!(taffy.total_node_count(), 3);
        assert_eq!(
            taffy.style(button_id).unwrap().size.width,
            Dimension::length(144.0)
        );
        assert_eq!(
            taffy.style(root).unwrap().gap.width,
            LengthPercentage::length(12.0)
        );
        assert_eq!(
            taffy.get_node_context(text_id),
            Some(&RutterContext::Text(TextContext {
                content: "updated".to_string(),
                font_size: 18.0,
            }))
        );
    }

    #[test]
    fn sync_taffy_tree_applies_rtl_direction_to_descendants() {
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        let mut tree = SyncedLayoutTree::placeholder(root);
        let widget = Widget::Row {
            children: vec![text("مرحبا", 16.0), button(96.0)],
            style: Style::default(),
        };

        sync_tree_with_direction(&mut taffy, &mut tree, &widget, LayoutDirection::Rtl);

        assert_eq!(taffy.style(root).unwrap().direction, Direction::Rtl);
        assert_eq!(
            taffy.style(tree.children[0].node_id).unwrap().direction,
            Direction::Rtl
        );
        assert_eq!(
            taffy.style(tree.children[1].node_id).unwrap().direction,
            Direction::Rtl
        );
    }

    #[test]
    fn visible_modal_is_absolute_overlay_and_does_not_shift_siblings() {
        let mut taffy = TaffyTree::new();
        let widget = Widget::Column {
            style: Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::percent(1.0),
                },
                ..Style::default()
            },
            children: vec![
                button(100.0),
                Widget::Modal {
                    id: 11,
                    visible: true,
                    child: Box::new(button(200.0)),
                    on_dismiss: None,
                    style: Style::default(),
                },
                button(120.0),
            ],
        };

        let root = build_taffy_tree(&mut taffy, &widget, fs(), &empty_states());
        compute_layout(&mut taffy, root, PhysicalSize::new(400, 300), fs());
        let children = taffy.children(root).unwrap();
        let modal = children
            .iter()
            .copied()
            .find(|node| taffy.style(*node).unwrap().position == Position::Absolute)
            .unwrap();
        let modal_style = taffy.style(modal).unwrap();
        let first = taffy.layout(children[0]).unwrap();
        let second = children
            .iter()
            .copied()
            .filter(|node| taffy.style(*node).unwrap().position != Position::Absolute)
            .nth(1)
            .and_then(|node| taffy.layout(node).ok())
            .unwrap();

        assert_eq!(modal_style.position, Position::Absolute);
        assert_eq!(taffy.layout(modal).unwrap().size.width, 400.0);
        assert_eq!(taffy.layout(modal).unwrap().size.height, 300.0);
        assert_eq!(second.location.y, first.size.height);
    }

    #[test]
    fn sync_taffy_tree_reuses_keyed_children_across_reorder() {
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        let mut tree = SyncedLayoutTree::placeholder(root);

        let initial = Widget::Column {
            children: vec![slider(1, 100.0), slider(2, 120.0)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &initial);

        let first_id = tree.children[0].node_id;
        let second_id = tree.children[1].node_id;

        let reordered = Widget::Column {
            children: vec![slider(2, 180.0), slider(1, 100.0), slider(3, 80.0)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &reordered);

        assert_eq!(tree.children[0].node_id, second_id);
        assert_eq!(tree.children[1].node_id, first_id);
        assert_eq!(taffy.total_node_count(), 4);
        assert_eq!(
            taffy.style(tree.children[0].node_id).unwrap().size.width,
            Dimension::length(180.0)
        );
    }

    #[test]
    fn sync_taffy_tree_preserves_unkeyed_siblings_when_keyed_nodes_are_inserted() {
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        let mut tree = SyncedLayoutTree::placeholder(root);

        let initial = Widget::Column {
            children: vec![text("anchor", 14.0)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &initial);

        let text_id = tree.children[0].node_id;

        let updated = Widget::Column {
            children: vec![slider(10, 90.0), text("anchor", 14.0)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &updated);

        assert_eq!(tree.children[1].node_id, text_id);
        assert_eq!(taffy.total_node_count(), 3);
    }

    #[test]
    fn sync_taffy_tree_removes_orphaned_subtrees() {
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        let mut tree = SyncedLayoutTree::placeholder(root);

        let initial = Widget::Column {
            children: vec![accordion(true)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &initial);

        let accordion_id = tree.children[0].node_id;
        assert_eq!(taffy.total_node_count(), 3);
        assert_eq!(taffy.child_count(accordion_id), 1);

        let collapsed = Widget::Column {
            children: vec![accordion(false)],
            style: Style::default(),
        };
        sync_tree(&mut taffy, &mut tree, &collapsed);

        assert_eq!(tree.children[0].node_id, accordion_id);
        assert!(tree.children[0].children.is_empty());
        assert_eq!(taffy.total_node_count(), 2);
        assert_eq!(taffy.child_count(accordion_id), 0);
    }
}
