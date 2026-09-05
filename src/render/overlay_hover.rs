// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache-2.0.

//! Routes hover rendering to the visually topmost floating overlay.

use std::collections::HashMap;

use skia_safe::{Contains, Point, Rect as SkiaRect};
use taffy::prelude::{NodeId, TaffyTree};

use super::dropdown_menu_overlay::{DropdownMenuOverlayHit, hit_test_dropdown_menu_overlay};
use super::hit_test::{context_menu_rect, popover_rect};
use super::search_overlay::search_popup_layout;
use super::select_overlay::collector::{
    collect_open_dropdown_overlays, collect_open_search_overlays, collect_open_select_overlays,
};
use super::select_overlay::select_popup_surface_contains;
use crate::engine::widget_state::{PopoverState, WidgetState};
use crate::i18n::LayoutDirection;
use crate::input_state::InputWidgetState;
use crate::layout::RutterContext;
use crate::widget::Widget;

// A finite off-canvas point keeps downstream geometry calculations valid.
const HIDDEN_HOVER_COORDINATE: f32 = -1.0e20;

#[derive(Clone, Copy)]
pub(crate) struct OverlayHoverRoutes {
    pub(crate) base: OverlayVisualRoute,
    pub(crate) popover: OverlayVisualRoute,
    pub(crate) select: OverlayVisualRoute,
    pub(crate) search: OverlayVisualRoute,
    pub(crate) dropdown: OverlayVisualRoute,
    pub(crate) context_menu: OverlayVisualRoute,
}

/// Rendering inputs allowed for one visual layer.
#[derive(Clone, Copy)]
pub(crate) struct OverlayVisualRoute {
    pub(crate) mouse: Point,
    pub(crate) focused_id: Option<u64>,
    pub(crate) shows_interaction_effects: bool,
}

pub(crate) struct OverlayHoverInput<'render, 'widget, Msg>
where
    'widget: 'render,
{
    pub(crate) taffy: &'render TaffyTree<RutterContext>,
    pub(crate) root: NodeId,
    pub(crate) widget: &'render Widget<'widget, Msg>,
    pub(crate) widget_states: &'render HashMap<u64, WidgetState>,
    pub(crate) input_states: &'render HashMap<u64, InputWidgetState>,
    pub(crate) focused_id: Option<u64>,
    pub(crate) mouse: Point,
    pub(crate) viewport: (f32, f32),
    pub(crate) font_size: f32,
    pub(crate) direction: LayoutDirection,
}

#[derive(Clone, Copy, Default)]
struct OverlayHoverCoverage {
    context_menu: bool,
    dropdown: bool,
    search: bool,
    select: bool,
    popover: bool,
}

/// Resolves the mouse point each visual layer may use for hover affordances.
pub(crate) fn overlay_hover_routes<Msg>(
    input: OverlayHoverInput<'_, '_, Msg>,
) -> OverlayHoverRoutes {
    let coverage = OverlayHoverCoverage::collect(&input);
    OverlayHoverRoutes::from_coverage(input.mouse, input.focused_id, coverage)
}

impl OverlayHoverRoutes {
    fn from_coverage(
        mouse: Point,
        focused_id: Option<u64>,
        coverage: OverlayHoverCoverage,
    ) -> Self {
        let context_menu = coverage.context_menu;
        let dropdown = context_menu;
        let search = dropdown || coverage.dropdown;
        let select = search || coverage.search;
        let popover = select || coverage.select;
        Self {
            base: OverlayVisualRoute::new(mouse, focused_id, coverage.any()),
            popover: OverlayVisualRoute::new(mouse, focused_id, popover),
            select: OverlayVisualRoute::new(mouse, focused_id, select),
            search: OverlayVisualRoute::new(mouse, focused_id, search),
            dropdown: OverlayVisualRoute::new(mouse, focused_id, dropdown),
            context_menu: OverlayVisualRoute::new(mouse, focused_id, context_menu),
        }
    }
}

impl OverlayVisualRoute {
    fn new(mouse: Point, focused_id: Option<u64>, covered: bool) -> Self {
        Self {
            mouse: hover_point(mouse, covered),
            focused_id: (!covered).then_some(focused_id).flatten(),
            shows_interaction_effects: !covered,
        }
    }
}

impl OverlayHoverCoverage {
    fn any(self) -> bool {
        self.context_menu || self.dropdown || self.search || self.select || self.popover
    }

    fn collect<Msg>(input: &OverlayHoverInput<'_, '_, Msg>) -> Self {
        Self {
            context_menu: context_menu_captures_hover(input),
            dropdown: dropdown_captures_hover(input),
            search: search_captures_hover(input),
            select: select_captures_hover(input),
            popover: popover_captures_hover(input),
        }
    }
}

fn hover_point(mouse: Point, covered: bool) -> Point {
    if covered {
        return Point::new(HIDDEN_HOVER_COORDINATE, HIDDEN_HOVER_COORDINATE);
    }
    mouse
}

fn context_menu_captures_hover<Msg>(input: &OverlayHoverInput<'_, '_, Msg>) -> bool {
    let mut menus = Vec::new();
    let mut path = Vec::new();
    super::collect_open_context_menus(input.widget, input.widget_states, &mut path, &mut menus);
    menus.into_iter().any(|menu| {
        context_menu_rect(menu.entries, menu.anchor, input.viewport, input.font_size)
            .contains(input.mouse)
    })
}

fn dropdown_captures_hover<Msg>(input: &OverlayHoverInput<'_, '_, Msg>) -> bool {
    let overlays = collect_open_dropdown_overlays(
        input.widget,
        input.taffy,
        input.root,
        input.widget_states,
        input.viewport,
    );
    matches!(
        hit_test_dropdown_menu_overlay(&overlays, input.mouse, input.viewport, input.direction),
        Some(DropdownMenuOverlayHit::Entry { .. } | DropdownMenuOverlayHit::Surface { .. })
    )
}

fn search_captures_hover<Msg>(input: &OverlayHoverInput<'_, '_, Msg>) -> bool {
    collect_open_search_overlays(
        input.widget,
        input.taffy,
        input.root,
        input.widget_states,
        input.input_states,
        input.focused_id,
        input.viewport,
    )
    .iter()
    .any(|overlay| {
        search_popup_layout(overlay, input.viewport)
            .rect
            .contains(input.mouse)
    })
}

fn select_captures_hover<Msg>(input: &OverlayHoverInput<'_, '_, Msg>) -> bool {
    collect_open_select_overlays(
        input.widget,
        input.taffy,
        input.root,
        input.widget_states,
        input.viewport,
    )
    .into_iter()
    .any(|overlay| select_popup_surface_contains(overlay, input.mouse, input.viewport))
}

fn popover_captures_hover<Msg>(input: &OverlayHoverInput<'_, '_, Msg>) -> bool {
    let mut visitor = PopoverHoverVisitor {
        taffy: input.taffy,
        widget_states: input.widget_states,
        viewport: input.viewport,
        mouse: input.mouse,
        path: Vec::new(),
    };
    visitor.captures_pointer_in_tree(input.widget, input.root)
}

struct PopoverHoverVisitor<'tree> {
    taffy: &'tree TaffyTree<RutterContext>,
    widget_states: &'tree HashMap<u64, WidgetState>,
    viewport: (f32, f32),
    mouse: Point,
    path: Vec<usize>,
}

impl<'tree> PopoverHoverVisitor<'tree> {
    fn captures_pointer_in_tree<Msg>(&mut self, widget: &Widget<Msg>, node: NodeId) -> bool {
        if let Widget::Popover {
            anchor, content, ..
        } = widget
        {
            return self.captures_pointer_in_popover(widget, anchor, content, node);
        }
        self.captures_pointer_in_nested_widget(widget, node)
    }

    fn captures_pointer_in_nested_widget<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        node: NodeId,
    ) -> bool {
        match widget {
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                self.captures_pointer_in_children(children, node)
            }
            Widget::Container { child, .. }
            | Widget::Tooltip { child, .. }
            | Widget::ContextMenu { child, .. }
            | Widget::ScrollView { child, .. } => self.captures_pointer_in_first_child(child, node),
            Widget::Accordion {
                expanded, child, ..
            } => *expanded && self.captures_pointer_in_first_child(child, node),
            Widget::Modal { visible, child, .. } | Widget::Dialog { visible, child, .. } => {
                *visible && self.captures_pointer_in_first_child(child, node)
            }
            _ => false,
        }
    }

    fn captures_pointer_in_popover<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        anchor: &Widget<Msg>,
        content: &Widget<Msg>,
        node: NodeId,
    ) -> bool {
        if self.popup_contains_pointer(widget, node) {
            return true;
        }
        self.captures_pointer_in_popover_children(widget, anchor, content, node)
    }

    fn captures_pointer_in_popover_children<Msg>(
        &mut self,
        widget: &Widget<Msg>,
        anchor: &Widget<Msg>,
        content: &Widget<Msg>,
        node: NodeId,
    ) -> bool {
        let Ok(nodes) = self.taffy.children(node) else {
            return false;
        };
        if let Some(anchor_node) = nodes.first().copied()
            && self.captures_pointer_in_child(anchor, anchor_node, 0)
        {
            return true;
        }
        if self.open_popover_state(widget).is_none() {
            return false;
        }
        let Some(content_node) = nodes.get(1).copied() else {
            return false;
        };
        self.captures_pointer_in_child(content, content_node, 1)
    }

    fn popup_contains_pointer<Msg>(&self, widget: &Widget<Msg>, node: NodeId) -> bool {
        let Some(state) = self.open_popover_state(widget) else {
            return false;
        };
        self.popup_rect_contains_pointer(node, state)
    }

    fn open_popover_state<Msg>(&self, widget: &Widget<Msg>) -> Option<&PopoverState> {
        let id = widget.resolved_id(&self.path)?;
        self.widget_states
            .get(&id)
            .and_then(WidgetState::as_popover)
            .filter(|state| state.is_open)
    }

    fn popup_rect_contains_pointer(&self, node: NodeId, state: &PopoverState) -> bool {
        let Some(popup_node) = self.popup_layout_node(node) else {
            return false;
        };
        let Ok(layout) = self.taffy.layout(popup_node) else {
            return false;
        };
        let anchor = SkiaRect::from_xywh(
            state.anchor_x,
            state.anchor_y,
            state.anchor_w,
            state.anchor_h,
        );
        popover_rect(
            anchor,
            (layout.size.width, layout.size.height),
            self.viewport,
        )
        .contains(self.mouse)
    }

    fn popup_layout_node(&self, node: NodeId) -> Option<NodeId> {
        self.taffy.children(node).ok()?.get(1).copied()
    }

    fn captures_pointer_in_children<Msg>(
        &mut self,
        children: &[Widget<Msg>],
        node: NodeId,
    ) -> bool {
        let Ok(nodes) = self.taffy.children(node) else {
            return false;
        };
        for (index, child) in children.iter().enumerate() {
            let Some(child_node) = nodes.get(index).copied() else {
                continue;
            };
            if self.captures_pointer_in_child(child, child_node, index) {
                return true;
            }
        }
        false
    }

    fn captures_pointer_in_first_child<Msg>(&mut self, child: &Widget<Msg>, node: NodeId) -> bool {
        let Ok(nodes) = self.taffy.children(node) else {
            return false;
        };
        let Some(child_node) = nodes.first().copied() else {
            return false;
        };
        self.captures_pointer_in_child(child, child_node, 0)
    }

    fn captures_pointer_in_child<Msg>(
        &mut self,
        child: &Widget<Msg>,
        node: NodeId,
        index: usize,
    ) -> bool {
        self.path.push(index);
        let captures_hover = self.captures_pointer_in_tree(child, node);
        self.path.pop();
        captures_hover
    }
}

#[cfg(test)]
#[path = "../../tests/unit/overlay_hover_unit_tests.rs"]
mod tests;
