// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use accesskit::{
    Action, ActivationHandler, DeactivationHandler, Node, NodeId, Orientation as AccessOrientation,
    Rect, Role, Toggled, Tree, TreeId, TreeUpdate,
};
use cosmic_text::FontSystem;
use skia_safe::{Point, Rect as SkiaRect};
use taffy::prelude::{NodeId as TaffyNodeId, TaffyTree};
use winit::dpi::PhysicalSize;

use crate::engine::widget_state::WidgetState;
use crate::engine::widget_state::{
    normalize_virtual_grid_columns, virtual_grid_cell_left, virtual_grid_cell_width,
};
use crate::i18n::LayoutDirection;
use crate::input_state::InputWidgetState;
use crate::layout::{
    RutterContext, SCROLLBAR_W, VIRTUAL_GRID_GAP, build_taffy_tree_with_direction, compute_layout,
};
use crate::render::RichTextRenderer;
use crate::render::select_overlay::collector::{
    collect_dropdown_triggers, collect_open_dropdown_overlays,
};
use crate::widget::id::resolve_accessibility_path_id;
use crate::widget::{
    CustomAccessibility, CustomAccessibilityActions, CustomAccessibilityRole,
    CustomAccessibilityState, DialogAction, KeyedVirtualItems, VirtualSelection, Widget,
    pop_interactive_virtual_item_path, push_interactive_virtual_item_path,
    resolve_table_of_contents_accordion_id, resolve_table_of_contents_navigation_id,
    resolve_table_of_contents_viewport_id,
};
use crate::widgets::carousel::geometry::carousel_item_frames;
use crate::widgets::table_of_contents::{collect_entries, entry_rects, layout_nodes, title_rect};
use crate::widgets::time::{ClockFormat, TimeZone, current_clock_text};

mod action_queue;
mod dropdown_menu;
mod search;
mod table;

pub(crate) use action_queue::AccessibilityActionInbox;

#[cfg(test)]
#[path = "table_tests.rs"]
mod table_tests;

const ROOT_ACCESSIBILITY_ID: u64 = 0;

#[derive(Debug, Default)]
pub(crate) struct LazyActivationHandler;

impl ActivationHandler for LazyActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        None
    }
}

#[derive(Debug, Default)]
pub(crate) struct IgnoredDeactivationHandler;

impl DeactivationHandler for IgnoredDeactivationHandler {
    fn deactivate_accessibility(&mut self) {}
}

#[derive(Clone, Copy)]
pub(crate) struct AccessibilityInputs<'a> {
    pub input_states: &'a HashMap<u64, InputWidgetState>,
    pub widget_states: &'a HashMap<u64, WidgetState>,
    pub focused_widget_id: Option<u64>,
    pub viewport: (f32, f32),
    pub direction: LayoutDirection,
}

#[derive(Clone, Copy)]
struct DropdownAccessibilityGeometry {
    anchor: skia_safe::Rect,
    visible_anchor: skia_safe::Rect,
}

pub(crate) fn build_accessibility_update<Msg>(
    taffy: &TaffyTree<RutterContext>,
    widget: &Widget<Msg>,
    root_node: TaffyNodeId,
    inputs: AccessibilityInputs<'_>,
) -> TreeUpdate {
    let dropdown_geometries = collect_dropdown_triggers(
        widget,
        taffy,
        root_node,
        inputs.widget_states,
        inputs.viewport,
    )
    .into_iter()
    .map(|trigger| {
        (
            trigger.id,
            DropdownAccessibilityGeometry {
                anchor: trigger.anchor,
                visible_anchor: trigger.visible_anchor,
            },
        )
    })
    .collect();
    let visible_dropdowns = collect_open_dropdown_overlays(
        widget,
        taffy,
        root_node,
        inputs.widget_states,
        inputs.viewport,
    )
    .into_iter()
    .map(|overlay| overlay.id)
    .collect();
    let search_popups = search::collect_search_accessibility_popups(
        widget,
        taffy,
        root_node,
        inputs.widget_states,
        inputs.input_states,
        inputs.focused_widget_id,
        inputs.viewport,
    );
    let mut builder = AccessibilityBuilder::new(
        taffy,
        inputs,
        dropdown_geometries,
        visible_dropdowns,
        search_popups,
    );
    let children = builder.collect(
        widget,
        Some(root_node),
        Point::new(0.0, 0.0),
        &mut Vec::new(),
    );
    builder.finish(children)
}

struct AccessibilityBuilder<'layout, 'input> {
    taffy: &'layout TaffyTree<RutterContext>,
    inputs: AccessibilityInputs<'input>,
    nodes: Vec<(NodeId, Node)>,
    dropdown_geometries: HashMap<u64, DropdownAccessibilityGeometry>,
    visible_dropdowns: HashSet<u64>,
    search_popups: HashMap<u64, search::SearchAccessibilityPopup>,
}

impl<'layout, 'input> AccessibilityBuilder<'layout, 'input> {
    fn new(
        taffy: &'layout TaffyTree<RutterContext>,
        inputs: AccessibilityInputs<'input>,
        dropdown_geometries: HashMap<u64, DropdownAccessibilityGeometry>,
        visible_dropdowns: HashSet<u64>,
        search_popups: HashMap<u64, search::SearchAccessibilityPopup>,
    ) -> Self {
        Self {
            taffy,
            inputs,
            nodes: Vec::new(),
            dropdown_geometries,
            visible_dropdowns,
            search_popups,
        }
    }

    fn finish(mut self, children: Vec<NodeId>) -> TreeUpdate {
        let root = NodeId(ROOT_ACCESSIBILITY_ID);
        let mut root_node = Node::new(Role::Window);
        root_node.set_children(children);
        self.nodes.push((root, root_node));
        let focus = self.focus_node_id(root);
        TreeUpdate {
            nodes: self.nodes,
            tree: Some(Tree::new(root)),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    fn focus_node_id(&self, root: NodeId) -> NodeId {
        let candidate = self
            .inputs
            .focused_widget_id
            .map(access_node_id)
            .unwrap_or(root);
        if self.nodes.iter().any(|(id, _)| *id == candidate) {
            candidate
        } else {
            root
        }
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
            Widget::Container { child, .. }
            | Widget::PointerRegion { child, .. }
            | Widget::Tooltip { child, .. } => {
                self.collect_single_child(child, node, frame.origin, path)
            }
            Widget::ScrollView { child, .. } => {
                self.collect_scroll_view(widget, child, node, frame, path)
            }
            Widget::TableOfContents { child, .. } => {
                self.collect_table_of_contents(widget, child, node, frame, path)
            }
            Widget::Table { model, options, .. } => {
                table::collect(self, widget, model, options, frame, path)
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
            Widget::DropdownMenu { label, entries, .. } => {
                dropdown_menu::collect(self, widget, label, entries, frame, path)
            }
            Widget::SearchBar {
                suggestions: Some(_),
                ..
            } => self.collect_search_bar(widget, frame, path),
            Widget::InteractiveVirtualListContent {
                item_height, items, ..
            } => self.collect_interactive_virtual_list(widget, *item_height, items, frame, path),
            Widget::InteractiveVirtualGridContent {
                columns,
                item_height,
                items,
                ..
            } => self.collect_interactive_virtual_grid(
                widget,
                *columns,
                *item_height,
                items,
                frame,
                path,
            ),
            Widget::InteractiveCarouselView { items, config, .. } => {
                self.collect_interactive_carousel(widget, items, config, frame, path)
            }
            Widget::Custom { .. } => self.collect_custom_leaf(widget, frame, path),
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

    fn collect_table_of_contents<Msg>(
        &mut self,
        table: &Widget<Msg>,
        document: &Widget<Msg>,
        node: Option<TaffyNodeId>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let Some(table_node) = node else {
            return self.collect_single_child(document, None, frame.origin, path);
        };
        let Some(nodes) = layout_nodes(self.taffy, table_node) else {
            return self.collect_single_child(document, None, frame.origin, path);
        };
        let table_id = table.resolved_id(path).unwrap();
        let navigation =
            self.table_of_contents_navigation(table, document, table_node, frame.origin, path);
        let viewport = LayoutFrame::from_taffy(self.taffy, Some(nodes.viewport), frame.origin);
        let offset_y = self
            .inputs
            .widget_states
            .get(&table_id)
            .and_then(WidgetState::as_scroll)
            .map(|state| state.offset_y)
            .unwrap_or(0.0);
        let document_origin = Point::new(viewport.origin.x, viewport.origin.y - offset_y);
        path.push(0);
        let document_children = self.collect(document, Some(nodes.content), document_origin, path);
        path.pop();
        self.push_viewport_node(table_id, viewport.rect, document_children);
        self.push_table_of_contents_node(table_id, frame.rect, navigation);
        vec![access_node_id(table_id)]
    }

    fn table_of_contents_navigation<Msg>(
        &mut self,
        table: &Widget<Msg>,
        document: &Widget<Msg>,
        table_node: TaffyNodeId,
        table_origin: Point,
        path: &[usize],
    ) -> NodeId {
        let table_id = table.resolved_id(path).unwrap();
        let navigation = table_of_contents_navigation_frame(self.taffy, table_node, table_origin);
        if table.table_of_contents_accordion().is_some() {
            return self.table_of_contents_accordion_navigation(
                table,
                document,
                table_node,
                table_origin,
                path,
            );
        }
        let children = self.table_of_contents_link_nodes(
            table,
            table_node,
            table_origin,
            path,
            &collect_entries(document),
        );
        self.push_table_of_contents_navigation(table, table_id, navigation, children)
    }

    fn table_of_contents_accordion_navigation<Msg>(
        &mut self,
        table: &Widget<Msg>,
        document: &Widget<Msg>,
        table_node: TaffyNodeId,
        table_origin: Point,
        path: &[usize],
    ) -> NodeId {
        let table_id = table.resolved_id(path).unwrap();
        let expanded = table.table_of_contents_entries_visible();
        let entries = collect_entries(document);
        let navigation_id = (expanded && !entries.is_empty()).then(|| {
            let navigation = table_of_contents_entries_frame(self.taffy, table_node, table_origin);
            let links =
                self.table_of_contents_link_nodes(table, table_node, table_origin, path, &entries);
            self.push_table_of_contents_navigation(table, table_id, navigation, links)
        });
        self.push_table_of_contents_accordion(
            table,
            table_node,
            table_origin,
            path,
            expanded,
            navigation_id,
        )
    }

    fn push_table_of_contents_navigation<Msg>(
        &mut self,
        table: &Widget<Msg>,
        table_id: u64,
        navigation: LayoutFrame,
        children: Vec<NodeId>,
    ) -> NodeId {
        let mut node = Node::new(Role::Navigation);
        node.set_bounds(navigation.rect);
        if let Widget::TableOfContents { title, .. } = table {
            node.set_label(*title);
        }
        node.set_children(children);
        let id = access_node_id(resolve_table_of_contents_navigation_id(table_id));
        self.nodes.push((id, node));
        id
    }

    fn push_table_of_contents_accordion<Msg>(
        &mut self,
        table: &Widget<Msg>,
        table_node: TaffyNodeId,
        table_origin: Point,
        path: &[usize],
        expanded: bool,
        navigation_id: Option<NodeId>,
    ) -> NodeId {
        let table_id = table.resolved_id(path).unwrap();
        let header = table_of_contents_header_frame(self.taffy, table_node, table_origin);
        let mut node = Node::new(Role::DisclosureTriangle);
        node.set_bounds(header.rect);
        apply_table_of_contents_accordion_props(&mut node, table, expanded);
        node.set_children(navigation_id.into_iter().collect::<Vec<_>>());
        let id = access_node_id(resolve_table_of_contents_accordion_id(table_id));
        self.nodes.push((id, node));
        id
    }

    fn table_of_contents_link_nodes<Msg>(
        &mut self,
        table: &Widget<Msg>,
        table_node: TaffyNodeId,
        table_origin: Point,
        path: &[usize],
        entries: &[crate::widgets::table_of_contents::TableOfContentsEntry],
    ) -> Vec<NodeId> {
        let rects = entry_rects(self.taffy, table_node);
        entries
            .iter()
            .zip(rects)
            .enumerate()
            .filter_map(|(index, (entry, rect))| {
                let id = table.table_of_contents_entry_focus_id(path, index)?;
                let mut node = Node::new(Role::Link);
                node.set_bounds(access_rect(SkiaRect::from_xywh(
                    table_origin.x + rect.left,
                    table_origin.y + rect.top,
                    rect.width(),
                    rect.height(),
                )));
                node.set_label(entry.title.clone());
                node.add_action(Action::Focus);
                node.add_action(Action::Click);
                let node_id = access_node_id(id);
                self.nodes.push((node_id, node));
                Some(node_id)
            })
            .collect()
    }

    fn push_viewport_node(&mut self, table_id: u64, rect: Rect, children: Vec<NodeId>) {
        let mut node = Node::new(Role::Group);
        node.set_bounds(rect);
        node.set_children(children);
        let id = access_node_id(resolve_table_of_contents_viewport_id(table_id));
        self.nodes.push((id, node));
    }

    fn push_table_of_contents_node(&mut self, table_id: u64, rect: Rect, children: NodeId) {
        let mut node = Node::new(Role::ScrollView);
        node.set_bounds(rect);
        node.set_children(vec![
            children,
            access_node_id(resolve_table_of_contents_viewport_id(table_id)),
        ]);
        self.nodes.push((access_node_id(table_id), node));
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
        path.push(0);
        let anchor_id = leaf_access_id(anchor, path);
        path.pop();
        let mut ids =
            collect_indexed_child(self, anchor, node_children.first().copied(), abs, path, 0);
        self.set_expanded_state(anchor_id, open);
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

    fn set_expanded_state(&mut self, id: Option<NodeId>, expanded: bool) {
        let Some(id) = id else { return };
        if let Some((_, node)) = self.nodes.iter_mut().find(|(node_id, _)| *node_id == id) {
            node.set_expanded(expanded);
        }
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

    fn collect_interactive_virtual_list<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        item_height: f32,
        items: &KeyedVirtualItems<'_, Msg>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let id = widget.resolved_id(path).unwrap();
        let state = self
            .inputs
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_vlist);
        let scroll_y = state.map(|state| state.scroll_y).unwrap_or(0.0);
        let (first, last) = state
            .map(|state| state.visible_range(item_height, items.len()))
            .unwrap_or((0, 0));
        let width = (frame_width(frame.rect) - SCROLLBAR_W - 4.0).max(0.0);
        let children = (first..last)
            .flat_map(|index| {
                self.collect_interactive_virtual_item(
                    items,
                    index,
                    Point::new(
                        frame.origin.x,
                        frame.origin.y + index as f32 * item_height - scroll_y,
                    ),
                    (width, item_height),
                    path,
                )
            })
            .collect();
        self.push_interactive_collection(widget, frame.rect, path, children);
        vec![access_node_id(id)]
    }

    fn collect_interactive_virtual_grid<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        columns: usize,
        item_height: f32,
        items: &KeyedVirtualItems<'_, Msg>,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let id = widget.resolved_id(path).unwrap();
        let state = self
            .inputs
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_vgrid);
        let (first, last) = state
            .map(|state| state.visible_row_range(item_height, items.len(), columns))
            .unwrap_or((0, 0));
        let columns = normalize_virtual_grid_columns(columns);
        let scroll_y = state.map(|state| state.scroll_y).unwrap_or(0.0);
        let cell_w = virtual_grid_cell_width(frame_width(frame.rect), columns);
        let cell_h = (item_height - VIRTUAL_GRID_GAP).max(12.0);
        let children = (first * columns..last * columns)
            .filter(|index| *index < items.len())
            .flat_map(|index| {
                let row = index / columns;
                let origin = Point::new(
                    frame.origin.x
                        + virtual_grid_cell_left(index % columns, frame_width(frame.rect), columns),
                    frame.origin.y + row as f32 * item_height - scroll_y + VIRTUAL_GRID_GAP * 0.5,
                );
                self.collect_interactive_virtual_item(items, index, origin, (cell_w, cell_h), path)
            })
            .collect();
        self.push_interactive_collection(widget, frame.rect, path, children);
        vec![access_node_id(id)]
    }

    fn collect_interactive_carousel<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        items: &KeyedVirtualItems<'_, Msg>,
        config: &crate::widgets::carousel::CarouselConfig,
        frame: LayoutFrame,
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let id = widget.resolved_id(path).unwrap();
        let state = self
            .inputs
            .widget_states
            .get(&id)
            .and_then(WidgetState::as_carousel);
        let position = state.map(|state| state.position).unwrap_or_default();
        let frames = carousel_item_frames(
            config,
            position,
            frame_width(frame.rect),
            items.len(),
            self.inputs.direction,
        );
        let children = frames
            .into_iter()
            .flat_map(|item_frame| {
                let rect = accessibility_carousel_card_rect(item_frame, frame_height(frame.rect));
                self.collect_interactive_virtual_item(
                    items,
                    item_frame.index,
                    Point::new(frame.origin.x + rect.left, frame.origin.y + rect.top),
                    (rect.width(), rect.height()),
                    path,
                )
            })
            .collect();
        self.push_interactive_collection(widget, frame.rect, path, children);
        vec![access_node_id(id)]
    }

    fn collect_interactive_virtual_item<Msg>(
        &mut self,
        items: &KeyedVirtualItems<'_, Msg>,
        index: usize,
        origin: Point,
        size: (f32, f32),
        path: &mut Vec<usize>,
    ) -> Vec<NodeId> {
        let Some(key) = items.key_at(index) else {
            return Vec::new();
        };
        let Some(item) = items.build_item(index) else {
            return Vec::new();
        };
        let mut item_taffy = TaffyTree::new();
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let root = build_taffy_tree_with_direction(
            &mut item_taffy,
            &item,
            fonts.clone(),
            self.inputs.widget_states,
            self.inputs.direction,
        );
        compute_layout(
            &mut item_taffy,
            root,
            virtual_item_size(size),
            fonts,
            &RichTextRenderer::default(),
        );
        push_interactive_virtual_item_path(path, key);
        let mut item_builder = AccessibilityBuilder::new(
            &item_taffy,
            self.inputs,
            HashMap::new(),
            HashSet::new(),
            HashMap::new(),
        );
        let children = item_builder.collect(&item, Some(root), origin, path);
        self.nodes.append(&mut item_builder.nodes);
        pop_interactive_virtual_item_path(path);
        children
    }

    fn push_interactive_collection<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        rect: Rect,
        path: &[usize],
        children: Vec<NodeId>,
    ) {
        let id = widget.resolved_id(path).unwrap();
        let role = leaf_role(widget).unwrap_or(Role::ListBox);
        let mut node = self.widget_node(widget, role, rect, path);
        node.set_children(children);
        self.nodes.push((access_node_id(id), node));
    }

    fn collect_custom_leaf<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        frame: LayoutFrame,
        path: &[usize],
    ) -> Vec<NodeId> {
        let Widget::Custom {
            id, widget: custom, ..
        } = widget
        else {
            return Vec::new();
        };
        let CustomAccessibility::Node(descriptor) = custom.accessibility() else {
            return Vec::new();
        };
        let mut node = Node::new(custom_accessibility_role(descriptor.role));
        node.set_bounds(frame.rect);
        node.set_label(descriptor.name);
        if let Some(value) = descriptor.value {
            node.set_value(value);
        }
        apply_custom_accessibility_state(&mut node, descriptor.state);
        apply_custom_accessibility_actions(&mut node, descriptor.actions);
        if custom.interaction().supports_keyboard() {
            node.add_action(Action::Focus);
        }
        let node_id = access_node_id(widget.resolved_id(path).unwrap_or(id.get()));
        self.nodes.push((node_id, node));
        vec![node_id]
    }

    fn collect_search_bar<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        frame: LayoutFrame,
        path: &[usize],
    ) -> Vec<NodeId> {
        let Some(search_id) = widget.resolved_id(path) else {
            return Vec::new();
        };
        let mut node = self.widget_node(widget, Role::EditableComboBox, frame.rect, path);
        apply_leaf_props(&mut node, widget, self.inputs, path);
        let popup = self.search_popups.get(&search_id);
        let popup_id = popup.map(|popup| popup.listbox_id);
        self.nodes
            .extend(search::configure_search_combobox(&mut node, popup));
        let node_id = access_node_id(search_id);
        self.nodes.push((node_id, node));
        let mut ids = vec![node_id];
        if let Some(id) = popup_id {
            ids.push(id);
        }
        ids
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

fn table_of_contents_navigation_frame(
    taffy: &TaffyTree<RutterContext>,
    table_node: TaffyNodeId,
    table_origin: Point,
) -> LayoutFrame {
    let navigation = layout_nodes(taffy, table_node).map(|nodes| nodes.navigation);
    LayoutFrame::from_taffy(taffy, navigation, table_origin)
}

fn table_of_contents_entries_frame(
    taffy: &TaffyTree<RutterContext>,
    table_node: TaffyNodeId,
    table_origin: Point,
) -> LayoutFrame {
    let Some(nodes) = layout_nodes(taffy, table_node) else {
        return LayoutFrame::empty(table_origin);
    };
    let navigation = LayoutFrame::from_taffy(taffy, Some(nodes.navigation), table_origin);
    LayoutFrame::from_taffy(taffy, nodes.entries, navigation.origin)
}

fn table_of_contents_header_frame(
    taffy: &TaffyTree<RutterContext>,
    table_node: TaffyNodeId,
    table_origin: Point,
) -> LayoutFrame {
    let Some(rect) = title_rect(taffy, table_node) else {
        return LayoutFrame::empty(table_origin);
    };
    let origin = Point::new(table_origin.x + rect.left, table_origin.y + rect.top);
    LayoutFrame {
        origin,
        rect: access_rect(SkiaRect::from_xywh(
            origin.x,
            origin.y,
            rect.width(),
            rect.height(),
        )),
    }
}

fn apply_table_of_contents_accordion_props<Msg>(
    node: &mut Node,
    table: &Widget<Msg>,
    expanded: bool,
) {
    if let Widget::TableOfContents { title, .. } = table {
        node.set_label(*title);
    }
    node.set_expanded(expanded);
    node.add_action(if expanded {
        Action::Collapse
    } else {
        Action::Expand
    });
    node.add_action(Action::Focus);
    node.add_action(Action::Click);
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
    NodeId(raw_id)
}

fn rect_from_layout(origin: Point, width: f32, height: f32) -> Rect {
    Rect::new(
        origin.x as f64,
        origin.y as f64,
        (origin.x + width.max(0.0)) as f64,
        (origin.y + height.max(0.0)) as f64,
    )
}

fn access_rect(rect: skia_safe::Rect) -> Rect {
    Rect::new(
        rect.left as f64,
        rect.top as f64,
        rect.right as f64,
        rect.bottom as f64,
    )
}

fn frame_width(rect: Rect) -> f32 {
    (rect.x1 - rect.x0) as f32
}

fn frame_height(rect: Rect) -> f32 {
    (rect.y1 - rect.y0) as f32
}

fn virtual_item_size(size: (f32, f32)) -> PhysicalSize<u32> {
    PhysicalSize::new(size.0.max(1.0).ceil() as u32, size.1.max(1.0).ceil() as u32)
}

fn accessibility_carousel_card_rect(
    frame: crate::widgets::carousel::geometry::CarouselItemFrame,
    viewport_height: f32,
) -> SkiaRect {
    const GAP: f32 = 8.0;
    let horizontal = (GAP * 0.5).min(frame.width * 0.2);
    let vertical = (GAP * 0.5).min(viewport_height * 0.2);
    SkiaRect::from_xywh(
        frame.x + horizontal,
        vertical,
        (frame.width - horizontal * 2.0).max(1.0),
        (viewport_height - vertical * 2.0).max(1.0),
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
    builder: &mut AccessibilityBuilder<'_, '_>,
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
        Widget::Text { .. } | Widget::RichText { .. } => Role::TextRun,
        Widget::Heading { .. } => Role::Heading,
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
        Widget::Counter { .. } => Role::SpinButton,
        Widget::Clock { .. } => Role::TextRun,
        Widget::Select { .. } => Role::ComboBox,
        Widget::ProgressBar { .. } | Widget::Spinner { .. } => Role::ProgressIndicator,
        Widget::TabBar { .. } => Role::TabList,
        Widget::Toast { visible: true, .. } => Role::Status,
        Widget::CarouselView { .. } | Widget::InteractiveCarouselView { .. } => Role::ListBox,
        Widget::VirtualList { .. }
        | Widget::VirtualListContent { .. }
        | Widget::InteractiveVirtualListContent { .. }
        | Widget::VirtualListWithSelection { .. }
        | Widget::VirtualListContentWithSelection { .. } => Role::ListBox,
        Widget::VirtualGrid { .. }
        | Widget::VirtualGridContent { .. }
        | Widget::InteractiveVirtualGridContent { .. }
        | Widget::VirtualGridWithSelection { .. }
        | Widget::VirtualGridContentWithSelection { .. } => Role::Grid,
        _ => return None,
    })
}

fn custom_accessibility_role(role: CustomAccessibilityRole) -> Role {
    match role {
        CustomAccessibilityRole::Button => Role::Button,
        CustomAccessibilityRole::CheckBox => Role::CheckBox,
        CustomAccessibilityRole::Switch => Role::Switch,
        CustomAccessibilityRole::RadioButton => Role::RadioButton,
        CustomAccessibilityRole::Slider => Role::Slider,
        CustomAccessibilityRole::ProgressIndicator => Role::ProgressIndicator,
        CustomAccessibilityRole::Image => Role::Image,
        CustomAccessibilityRole::StaticText => Role::TextRun,
    }
}

fn apply_custom_accessibility_state(node: &mut Node, state: CustomAccessibilityState) {
    match state {
        CustomAccessibilityState::Default => {}
        CustomAccessibilityState::Toggled(value) => node.set_toggled(Toggled::from(value)),
        CustomAccessibilityState::Selected(value) => node.set_selected(value),
        CustomAccessibilityState::Expanded(value) => node.set_expanded(value),
        CustomAccessibilityState::Busy => node.set_busy(),
    }
}

fn apply_custom_accessibility_actions(node: &mut Node, actions: CustomAccessibilityActions) {
    if actions.click {
        node.add_action(Action::Click);
    }
    if actions.increment {
        node.add_action(Action::Increment);
    }
    if actions.decrement {
        node.add_action(Action::Decrement);
    }
}

fn leaf_access_id<Msg>(widget: &Widget<Msg>, path: &[usize]) -> Option<NodeId> {
    let raw_id = widget
        .keyboard_focus_id(path)
        .or_else(|| widget.resolved_id(path))
        .unwrap_or_else(|| resolve_accessibility_path_id(path));
    Some(access_node_id(raw_id))
}

fn apply_leaf_props<Msg>(
    node: &mut Node,
    widget: &Widget<Msg>,
    inputs: AccessibilityInputs<'_>,
    path: &[usize],
) {
    apply_actions(node, widget);
    apply_heading_props(node, widget);
    apply_toggle_props(node, widget);
    apply_numeric_props(node, widget);
    apply_input_props(node, widget, inputs.input_states, path);
    apply_collection_props(node, widget);
    if widget.keyboard_focus_id(path).is_some() {
        node.add_action(Action::Focus);
    }
}

fn apply_heading_props<Msg>(node: &mut Node, widget: &Widget<Msg>) {
    let Widget::Heading { level, .. } = widget else {
        return;
    };
    node.set_level(level.access_level() as usize);
}

fn apply_actions<Msg>(node: &mut Node, widget: &Widget<Msg>) {
    match widget {
        Widget::Button { .. }
        | Widget::ButtonContent { .. }
        | Widget::Checkbox { .. }
        | Widget::Switch { .. }
        | Widget::Radio { .. }
        | Widget::Select { .. } => node.add_action(Action::Click),
        Widget::Slider { .. } | Widget::Counter { .. } => {
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
        Widget::Counter {
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
        Widget::CarouselView { item_count, .. } => {
            node.set_size_of_set(*item_count);
            node.set_orientation(AccessOrientation::Horizontal);
        }
        Widget::InteractiveCarouselView { items, .. } => {
            node.set_size_of_set(items.len());
            node.set_orientation(AccessOrientation::Horizontal);
        }
        Widget::VirtualList { item_count, .. } | Widget::VirtualListContent { item_count, .. } => {
            node.set_size_of_set(*item_count)
        }
        Widget::InteractiveVirtualListContent { items, .. } => node.set_size_of_set(items.len()),
        Widget::VirtualListWithSelection {
            item_count,
            selection,
            ..
        }
        | Widget::VirtualListContentWithSelection {
            item_count,
            selection,
            ..
        } => {
            node.set_size_of_set(*item_count);
            set_multiselectable_if_needed(node, selection);
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
        Widget::InteractiveVirtualGridContent { items, columns, .. } => {
            node.set_row_count(items.len().div_ceil((*columns).max(1)));
            node.set_column_count((*columns).max(1));
        }
        Widget::VirtualGridWithSelection {
            item_count,
            columns,
            selection,
            ..
        }
        | Widget::VirtualGridContentWithSelection {
            item_count,
            columns,
            selection,
            ..
        } => {
            node.set_row_count(item_count.div_ceil((*columns).max(1)));
            node.set_column_count((*columns).max(1));
            set_multiselectable_if_needed(node, selection);
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
        Widget::Heading { content, .. } => node.set_label(content.clone()),
        Widget::RichText { content, .. } => node.set_label(content.plain_text()),
        Widget::Button { text, .. } => node.set_label(*text),
        Widget::ButtonContent { label, .. } => node.set_label(*label),
        Widget::TextInput { label, .. } | Widget::TextArea { label, .. } => node.set_label(*label),
        Widget::SearchBar { .. } => node.set_label("Search"),
        Widget::Checkbox { label, .. } | Widget::Radio { label, .. } => node.set_label(*label),
        Widget::Slider { label, .. }
        | Widget::Counter { label, .. }
        | Widget::Select { label, .. } => node.set_label(*label),
        Widget::Clock {
            label,
            time_zone,
            config,
            ..
        } => set_clock_label(node, label, *time_zone, config.format()),
        Widget::ProgressBar { .. } => node.set_label("Progress"),
        Widget::Spinner { .. } => node.set_label("Loading"),
        Widget::Toast { message, .. } => node.set_label(*message),
        Widget::CarouselView { config, .. } => node.set_label(config.accessibility_label.clone()),
        Widget::InteractiveCarouselView { config, .. } => {
            node.set_label(config.accessibility_label.clone())
        }
        Widget::VirtualList { .. } | Widget::VirtualListContent { .. } => {
            node.set_label("Virtual list")
        }
        Widget::InteractiveVirtualListContent { .. } => node.set_label("Interactive virtual list"),
        Widget::VirtualListWithSelection { selection, .. }
        | Widget::VirtualListContentWithSelection { selection, .. } => node.set_label(
            virtual_selection_label(selection, "Virtual list", "Virtual multiselect list"),
        ),
        Widget::VirtualGrid { .. } | Widget::VirtualGridContent { .. } => {
            node.set_label("Virtual grid")
        }
        Widget::InteractiveVirtualGridContent { .. } => node.set_label("Interactive virtual grid"),
        Widget::VirtualGridWithSelection { selection, .. }
        | Widget::VirtualGridContentWithSelection { selection, .. } => node.set_label(
            virtual_selection_label(selection, "Virtual grid", "Virtual multiselect grid"),
        ),
        _ => set_input_value(node, widget, input_states, path),
    }
}

fn set_clock_label(node: &mut Node, label: &str, time_zone: TimeZone, format: ClockFormat) {
    let clock_text = current_clock_text(time_zone, format);
    if label.is_empty() {
        node.set_label(clock_text);
        return;
    }
    node.set_label(format!("{label}: {clock_text}"));
}

fn set_multiselectable_if_needed<Msg>(node: &mut Node, selection: &VirtualSelection<'_, Msg>) {
    if matches!(selection, VirtualSelection::Multiple { .. }) {
        node.set_multiselectable();
    }
}

fn virtual_selection_label<Msg>(
    selection: &VirtualSelection<'_, Msg>,
    single_label: &'static str,
    multiple_label: &'static str,
) -> &'static str {
    if matches!(selection, VirtualSelection::Multiple { .. }) {
        return multiple_label;
    }
    single_label
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
    if matches!(
        widget,
        Widget::TextInput {
            is_password: true,
            ..
        }
    ) {
        return;
    }

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

    use crate::WidgetId;
    use crate::layout::{build_taffy_tree, compute_layout};
    use crate::widget::{
        ButtonVariant, CustomAccessibility, CustomAccessibilityActions, CustomAccessibilityNode,
        CustomAccessibilityRole, CustomAccessibilityState, CustomInteraction, CustomPaintContext,
        CustomWidgetV1,
    };
    use crate::widgets::calendar::{CalendarDate, CalendarMonth};
    use crate::widgets::table_of_contents::{HeadingLevel, TableOfContentsOptions};
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

    fn build_update_with_inputs(
        widget: &Widget<'_, ()>,
        inputs: &HashMap<u64, InputWidgetState>,
    ) -> TreeUpdate {
        let states = HashMap::new();
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, widget, fs(), &states);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(300, 120),
            fs(),
            &crate::render::RichTextRenderer::default(),
        );
        build_accessibility_update(
            &taffy,
            widget,
            root,
            AccessibilityInputs {
                input_states: inputs,
                widget_states: &states,
                focused_widget_id: None,
                viewport: (300.0, 120.0),
                direction: LayoutDirection::Ltr,
            },
        )
    }

    fn build_update(widget: &Widget<'_, ()>) -> TreeUpdate {
        build_update_with_inputs(widget, &HashMap::new())
    }

    const SEARCH_ID: u64 = 91;
    const SEARCH_ITEMS: &[&str] = &["The Matrix", "O Senhor dos Anéis", "Star Wars"];

    fn suggestion_search_widget() -> Widget<'static, ()> {
        let suggestions = crate::SearchSuggestions::new(
            SEARCH_ITEMS,
            crate::SearchMatcher::Fuzzy,
            5,
            Some(|_| ()),
        )
        .unwrap();
        Widget::search_bar_with_suggestions(
            |_| (),
            None,
            None,
            None,
            "Search movies",
            suggestions,
            base_style(240.0, 40.0),
        )
        .with_id(SEARCH_ID)
    }

    fn build_search_accessibility_update(
        query: &str,
        hovered: Option<usize>,
        focused: bool,
    ) -> TreeUpdate {
        let widget = suggestion_search_widget();
        let mut font_system = FontSystem::new();
        let mut input = InputWidgetState::new(&mut font_system);
        input.set_text(&mut font_system, query);
        let inputs = HashMap::from([(SEARCH_ID, input)]);
        let states = HashMap::from([(
            SEARCH_ID,
            WidgetState::Search(crate::engine::widget_state::SearchState {
                hovered_option: hovered,
                dismissed: false,
            }),
        )]);
        build_search_update(&widget, &inputs, &states, focused.then_some(SEARCH_ID))
    }

    fn build_search_update(
        widget: &Widget<'_, ()>,
        inputs: &HashMap<u64, InputWidgetState>,
        states: &HashMap<u64, WidgetState>,
        focused_widget_id: Option<u64>,
    ) -> TreeUpdate {
        let (taffy, root) = layout_search_widget(widget, states);
        build_accessibility_update(
            &taffy,
            widget,
            root,
            AccessibilityInputs {
                input_states: inputs,
                widget_states: states,
                focused_widget_id,
                viewport: (300.0, 120.0),
                direction: LayoutDirection::Ltr,
            },
        )
    }

    fn layout_search_widget(
        widget: &Widget<'_, ()>,
        states: &HashMap<u64, WidgetState>,
    ) -> (TaffyTree<RutterContext>, TaffyNodeId) {
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, widget, fs(), states);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(300, 120),
            fs(),
            &crate::render::RichTextRenderer::default(),
        );
        (taffy, root)
    }

    fn node_for(update: &TreeUpdate, role: Role) -> &Node {
        update
            .nodes
            .iter()
            .find_map(|(_, node)| (node.role() == role).then_some(node))
            .unwrap()
    }

    struct AccessibleCustom;

    impl CustomWidgetV1<()> for AccessibleCustom {
        fn paint(&self, _: CustomPaintContext<'_>) {}

        fn interaction(&self) -> CustomInteraction {
            CustomInteraction::PointerAndKeyboard
        }

        fn accessibility(&self) -> CustomAccessibility {
            CustomAccessibility::Node(CustomAccessibilityNode {
                role: CustomAccessibilityRole::Button,
                name: String::from("Refresh graph"),
                value: Some(String::from("Ready")),
                state: CustomAccessibilityState::Default,
                actions: CustomAccessibilityActions {
                    click: true,
                    ..Default::default()
                },
            })
        }
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
    fn accessible_custom_node_emits_its_declared_role_value_and_actions() {
        let widget = Widget::custom(
            WidgetId::manual(411).unwrap(),
            AccessibleCustom,
            base_style(120.0, 40.0),
        );

        let update = build_update(&widget);
        let node = update
            .nodes
            .iter()
            .find_map(|(id, node)| (*id == NodeId(411)).then_some(node))
            .unwrap();

        assert_eq!(node.role(), Role::Button);
        assert_eq!(node.label(), Some("Refresh graph"));
        assert_eq!(node.value(), Some("Ready"));
        assert!(node.supports_action(Action::Click));
        assert!(node.supports_action(Action::Focus));
    }

    #[test]
    fn accessibility_update_exposes_flattened_rich_text_label() {
        let content = crate::RichText::from_spans([
            crate::RichTextSpan::new("20").bold(),
            crate::RichTextSpan::new("26").italic(),
        ]);
        let widget: Widget<'_, ()> = Widget::rich_text(content, base_style(80.0, 24.0));

        let update = build_update(&widget);
        let text = node_for(&update, Role::TextRun);

        assert_eq!(text.label(), Some("2026"));
    }

    #[test]
    fn accessibility_update_exposes_heading_levels_and_navigation_links() {
        let document = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::heading(HeadingLevel::H1, "Overview", base_style(260.0, 32.0)),
                Widget::heading(HeadingLevel::H2, "Install", base_style(260.0, 28.0)),
            ],
        };
        let widget =
            Widget::table_of_contents("Contents", document, base_style(280.0, 180.0)).with_id(31);

        let update = build_update(&widget);
        let heading = node_for(&update, Role::Heading);
        let navigation = node_for(&update, Role::Navigation);
        let links = update
            .nodes
            .iter()
            .filter_map(|(_, node)| (node.role() == Role::Link).then_some(node))
            .collect::<Vec<_>>();

        assert_eq!(heading.label(), Some("Overview"));
        assert_eq!(heading.level(), Some(1));
        assert_eq!(navigation.label(), Some("Contents"));
        assert_eq!(links.len(), 2);
        assert_eq!(links[1].label(), Some("Install"));
        assert!(links[0].supports_action(Action::Focus));
        assert!(links[0].supports_action(Action::Click));
    }

    #[test]
    fn collapsed_table_of_contents_exposes_a_disclosure_without_hidden_links() {
        let options = TableOfContentsOptions::new(2)
            .unwrap()
            .with_accordion_state(false, ());
        let widget = Widget::table_of_contents_with_options(
            "On this page",
            Widget::Column {
                style: Style::default(),
                children: vec![Widget::heading(
                    HeadingLevel::H2,
                    "Install",
                    base_style(260.0, 28.0),
                )],
            },
            base_style(280.0, 180.0),
            options,
        )
        .with_id(32);

        let update = build_update(&widget);
        let disclosure = node_for(&update, Role::DisclosureTriangle);
        let has_navigation = update
            .nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Navigation);
        let has_link = update
            .nodes
            .iter()
            .any(|(_, node)| node.role() == Role::Link);

        assert_eq!(disclosure.label(), Some("On this page"));
        assert_eq!(disclosure.is_expanded(), Some(false));
        assert!(disclosure.supports_action(Action::Expand));
        assert!(disclosure.supports_action(Action::Focus));
        assert!(disclosure.supports_action(Action::Click));
        assert!(!has_navigation);
        assert!(!has_link);
    }

    #[test]
    fn expanded_table_of_contents_accordion_exposes_its_navigation_links() {
        let options = TableOfContentsOptions::default().with_accordion(());
        let widget = Widget::table_of_contents_with_options(
            "On this page",
            Widget::heading(HeadingLevel::H2, "Install", base_style(260.0, 28.0)),
            base_style(280.0, 180.0),
            options,
        )
        .with_id(33);

        let update = build_update(&widget);
        let disclosure = node_for(&update, Role::DisclosureTriangle);
        let navigation = node_for(&update, Role::Navigation);
        let link = node_for(&update, Role::Link);
        let disclosure_bounds = disclosure.bounds().unwrap();
        let navigation_bounds = navigation.bounds().unwrap();
        let link_bounds = link.bounds().unwrap();

        assert_eq!(disclosure.is_expanded(), Some(true));
        assert!(disclosure.supports_action(Action::Collapse));
        assert_eq!(navigation.label(), Some("On this page"));
        assert_eq!(link.label(), Some("Install"));
        assert!(navigation_bounds.y0 >= disclosure_bounds.y1);
        assert!(link_bounds.y0 >= navigation_bounds.y0);
        assert!(link_bounds.y1 <= navigation_bounds.y1);
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
    fn accessibility_update_exposes_counter_spin_button_metadata() {
        let widget =
            Widget::counter(1, 0, 5, 1, |_| (), base_style(160.0, 40.0), "Quantity").with_id(17);

        let update = build_update(&widget);
        let counter = node_for(&update, Role::SpinButton);

        assert_eq!(counter.label(), Some("Quantity"));
        assert_eq!(counter.numeric_value(), Some(1.0));
        assert_eq!(counter.min_numeric_value(), Some(0.0));
        assert_eq!(counter.max_numeric_value(), Some(5.0));
        assert_eq!(counter.numeric_value_step(), Some(1.0));
        assert!(counter.supports_action(Action::Increment));
        assert!(counter.supports_action(Action::Decrement));
    }

    #[test]
    fn accessibility_update_exposes_search_suggestions_as_an_editable_combobox() {
        let update = build_search_accessibility_update("r", Some(2), true);
        let combo = node_for(&update, Role::EditableComboBox);
        let listbox = node_for(&update, Role::ListBox);
        let options: Vec<&Node> = update
            .nodes
            .iter()
            .filter_map(|(_, node)| (node.role() == Role::ListBoxOption).then_some(node))
            .collect();

        assert_eq!(combo.is_expanded(), Some(true));
        assert_eq!(combo.has_popup(), Some(accesskit::HasPopup::Listbox));
        assert_eq!(combo.auto_complete(), Some(accesskit::AutoComplete::List));
        assert!(combo.supports_action(Action::Focus));
        assert!(combo.supports_action(Action::Collapse));
        assert_eq!(combo.controls().len(), 1);
        assert_eq!(listbox.size_of_set(), Some(3));
        assert_eq!(options.len(), 3);
        assert_eq!(options[2].label(), Some("Star Wars"));
        assert_eq!(options[2].position_in_set(), Some(2));
        assert_eq!(options[2].is_selected(), Some(true));
        assert!(options[2].supports_action(Action::Focus));
        assert!(options[2].supports_action(Action::Click));
        assert!(combo.active_descendant().is_some());
    }

    #[test]
    fn accessibility_update_labels_an_empty_search_result_list() {
        let update = build_search_accessibility_update("zzz", None, true);
        let listbox = node_for(&update, Role::ListBox);

        assert_eq!(listbox.label(), Some("No matches"));
        assert_eq!(listbox.size_of_set(), Some(0));
        assert!(listbox.children().is_empty());
    }

    #[test]
    fn accessibility_update_collapses_an_unfocused_search_popup() {
        let update = build_search_accessibility_update("r", Some(0), false);
        let combo = node_for(&update, Role::EditableComboBox);

        assert_eq!(combo.is_expanded(), Some(false));
        assert!(combo.supports_action(Action::Expand));
        assert!(
            update
                .nodes
                .iter()
                .all(|(_, node)| { !matches!(node.role(), Role::ListBox | Role::ListBoxOption) })
        );
    }

    #[test]
    fn accessibility_update_exposes_current_clock_text() {
        let widget = Widget::clock(
            crate::TimeZone::UTC,
            base_style(240.0, 40.0),
            "Current UTC time",
        )
        .with_id(18);

        let update = build_update(&widget);
        let clock = node_for(&update, Role::TextRun);
        let label = clock.label().unwrap();

        assert!(label.starts_with("Current UTC time: "));
        assert!(label.ends_with(" UTC"));
    }

    #[test]
    fn accessibility_update_exposes_horizontal_carousel_metadata() {
        let cards = |index| {
            Some(Widget::Text {
                content: format!("Card {index}"),
                style: Style::default(),
                color: None,
                size: 14.0,
            })
        };
        let config = crate::CarouselConfig::weighted([1, 6, 1])
            .unwrap()
            .with_accessibility_label("Featured cards");
        let widget = Widget::carousel_view(24, cards, |_| (), config, base_style(300.0, 120.0));

        let update = build_update(&widget);
        let carousel = node_for(&update, Role::ListBox);

        assert_eq!(carousel.label(), Some("Featured cards"));
        assert_eq!(carousel.size_of_set(), Some(24));
        assert_eq!(carousel.orientation(), Some(AccessOrientation::Horizontal));
        assert!(!carousel.supports_action(Action::ScrollLeft));
        assert!(!carousel.supports_action(Action::ScrollRight));
    }

    fn interactive_accessibility_button<'a>(index: usize) -> Option<Widget<'a, ()>> {
        Some(Widget::Button {
            text: if index == 0 { "Launch" } else { "Favorite" },
            on_press: (),
            style: base_style(120.0, 40.0),
            color: None,
            variant: ButtonVariant::Primary,
        })
    }

    #[test]
    fn accessibility_exposes_only_visible_keyed_virtual_descendants() {
        let keys = [
            crate::VirtualItemKey::new(11).unwrap(),
            crate::VirtualItemKey::new(12).unwrap(),
            crate::VirtualItemKey::new(13).unwrap(),
            crate::VirtualItemKey::new(14).unwrap(),
        ];
        let items =
            crate::KeyedVirtualItems::try_new(&keys, &interactive_accessibility_button).unwrap();
        let widget =
            Widget::interactive_virtual_list_content(40.0, items, |_| (), base_style(120.0, 80.0))
                .with_id(802);
        let states = HashMap::from([(
            802,
            WidgetState::VList(crate::engine::widget_state::VirtualListState {
                viewport_h: 80.0,
                ..Default::default()
            }),
        )]);
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &widget, fs(), &states);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(120, 80),
            fs(),
            &crate::render::RichTextRenderer::default(),
        );
        let update = build_accessibility_update(
            &taffy,
            &widget,
            root,
            AccessibilityInputs {
                input_states: &HashMap::new(),
                widget_states: &states,
                focused_widget_id: None,
                viewport: (120.0, 80.0),
                direction: LayoutDirection::Ltr,
            },
        );
        let buttons = update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Button)
            .map(|(_, node)| node)
            .collect::<Vec<_>>();

        assert_eq!(buttons.len(), 3);
        assert!(
            buttons
                .iter()
                .all(|node| node.supports_action(Action::Click))
        );
        assert!(
            buttons
                .iter()
                .all(|node| node.supports_action(Action::Focus))
        );
    }

    #[test]
    fn accessibility_update_exposes_date_picker_label_and_expanded_state() {
        let selected = CalendarDate::new(2026, 7, 31).unwrap();
        let widget = Widget::date_picker(
            true,
            CalendarMonth::from(selected),
            Some(selected),
            (),
            (),
            |_| (),
            |_| (),
            "Date: 2026-07-31",
            "YYYY-MM-DD",
            base_style(180.0, 40.0),
            base_style(280.0, 320.0),
        );

        let update = build_update(&widget);
        let picker = update
            .nodes
            .iter()
            .find_map(|(_, node)| (node.label() == Some("Date: 2026-07-31")).then_some(node))
            .unwrap();

        assert_eq!(picker.is_expanded(), Some(true));
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
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(300, 120),
            fs(),
            &crate::render::RichTextRenderer::default(),
        );

        let update = build_accessibility_update(
            &taffy,
            &widget,
            root,
            AccessibilityInputs {
                input_states: &inputs,
                widget_states: &states,
                focused_widget_id: Some(focus_id),
                viewport: (300.0, 120.0),
                direction: LayoutDirection::Ltr,
            },
        );

        assert_eq!(update.focus, access_node_id(focus_id));
    }

    #[test]
    fn accessibility_update_omits_password_value() {
        const INPUT_ID: u64 = 42;
        const PASSWORD: &str = "correct horse battery staple";
        let widget = Widget::TextInput {
            on_change: |_| (),
            on_submit: None,
            style: base_style(180.0, 40.0),
            id: INPUT_ID,
            label: "Password",
            placeholder: "Enter password",
            state: crate::widget::InputState::Idle,
            error_msg: None,
            is_password: true,
        };
        let mut font_system = FontSystem::new();
        let mut input = InputWidgetState::new(&mut font_system);
        input.set_sensitive(true);
        input.set_text(&mut font_system, PASSWORD);

        let update = build_update_with_inputs(&widget, &HashMap::from([(INPUT_ID, input)]));
        let password_input = node_for(&update, Role::PasswordInput);

        assert_eq!(password_input.role(), Role::PasswordInput);
        assert_eq!(password_input.value(), None);
        assert!(update.nodes.iter().all(|(_, node)| {
            node.value()
                .map(|value| !value.contains(PASSWORD))
                .unwrap_or(true)
        }));
    }

    #[test]
    fn accessibility_ids_preserve_manual_and_automatic_namespaces() {
        let manual_id = 42;
        let automatic_id = crate::widget::id::AUTOMATIC_ID_NAMESPACE_BIT | manual_id;

        assert_ne!(access_node_id(manual_id), access_node_id(automatic_id));
        assert_ne!(access_node_id(manual_id), NodeId(ROOT_ACCESSIBILITY_ID));
    }
}
