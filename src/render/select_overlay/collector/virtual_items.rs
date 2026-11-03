//! Search popups portal out of visible keyed items into the actual window viewport.
use super::{SelectOverlayCollector, intersect_optional, rects_overlap};
use crate::engine::widget_state::{
    normalize_virtual_grid_columns, virtual_grid_cell_left, virtual_grid_cell_width,
};
use crate::layout::{
    SCROLLBAR_W, VIRTUAL_GRID_GAP, build_layout_tree, compute_layout_with_renderer,
};
use crate::render::RichTextRenderer;
use crate::render::hit_test::{interactive_carousel_card_rect, virtual_item_physical_size};
use crate::widget::{KeyedVirtualItems, Widget, push_interactive_virtual_item_path};
use crate::widgets::carousel::geometry::carousel_item_frames;
use skia_safe::{Point, Rect};
use taffy::prelude::{NodeId, TaffyTree};

impl<'tree, 'widget, 'entry: 'widget, Msg> SelectOverlayCollector<'tree, 'widget, 'entry, Msg> {
    pub(super) fn visit_virtual_searches(
        &mut self,
        widget: &Widget<'entry, Msg>,
        node: NodeId,
        absolute: Point,
        size: (f32, f32),
    ) {
        let id = widget.resolved_id(&self.path).unwrap();
        let direction = crate::render::node_layout_direction(self.taffy, node);
        let frame = Rect::from_xywh(absolute.x, absolute.y, size.0, size.1);
        let clip = intersect_optional(self.clip, frame);
        if clip.is_some_and(|clip| clip.is_empty()) {
            return;
        }
        // Match scroll_runtime::GeometryPass::virtual_collection and native item
        // painting: current scroll, grid gutters, and RTL carousel card allocations.
        let (items, frames): (_, Vec<_>) = match widget {
            Widget::InteractiveVirtualListContent {
                items, item_height, ..
            }
            | Widget::InteractiveVirtualListContentWith {
                items, item_height, ..
            } => {
                let Some(state) = self.widget_states.get(&id).and_then(|s| s.as_vlist()) else {
                    return;
                };
                let (first, last) = state.visible_range(*item_height, items.len());
                let frames = (first..last)
                    .map(|index| {
                        (
                            index,
                            Rect::from_xywh(
                                absolute.x,
                                absolute.y + index as f32 * item_height - state.scroll_y,
                                (size.0 - SCROLLBAR_W - 4.0).max(0.0),
                                *item_height,
                            ),
                        )
                    })
                    .collect();
                (items, frames)
            }
            Widget::InteractiveVirtualGridContent {
                items,
                item_height,
                columns,
                ..
            }
            | Widget::InteractiveVirtualGridContentWith {
                items,
                item_height,
                columns,
                ..
            } => {
                let Some(state) = self.widget_states.get(&id).and_then(|s| s.as_vgrid()) else {
                    return;
                };
                let (first, last) = state.visible_row_range(*item_height, items.len(), *columns);
                let columns = normalize_virtual_grid_columns(*columns);
                let frames = (first * columns..(last * columns).min(items.len()))
                    .map(|index| {
                        (
                            index,
                            Rect::from_xywh(
                                absolute.x
                                    + virtual_grid_cell_left(index % columns, size.0, columns),
                                absolute.y + (index / columns) as f32 * item_height
                                    - state.scroll_y
                                    + VIRTUAL_GRID_GAP * 0.5,
                                virtual_grid_cell_width(size.0, columns),
                                (*item_height - VIRTUAL_GRID_GAP).max(12.0),
                            ),
                        )
                    })
                    .collect();
                (items, frames)
            }
            Widget::InteractiveCarouselView { items, config, .. }
            | Widget::InteractiveCarouselViewWith { items, config, .. } => {
                let Some(state) = self.widget_states.get(&id).and_then(|s| s.as_carousel()) else {
                    return;
                };
                let frames =
                    carousel_item_frames(config, state.position, size.0, items.len(), direction)
                        .into_iter()
                        .map(|item| {
                            (
                                item.index,
                                interactive_carousel_card_rect(item, size.1)
                                    .with_offset((absolute.x, absolute.y)),
                            )
                        })
                        .collect();
                (items, frames)
            }
            _ => return,
        };
        let renderer = RichTextRenderer::new();
        for (index, frame) in frames {
            if clip.is_none_or(|clip| rects_overlap(frame, clip)) {
                self.visit_virtual_search_item(items, index, frame, clip, direction, &renderer);
            }
        }
    }

    fn visit_virtual_search_item(
        &mut self,
        items: &KeyedVirtualItems<'entry, Msg>,
        index: usize,
        frame: Rect,
        clip: Option<Rect>,
        direction: crate::LayoutDirection,
        renderer: &RichTextRenderer,
    ) {
        let Some(key) = items.key_at(index) else {
            return;
        };
        let Some(item) = items.build_item(index) else {
            return;
        };
        let mut tree = TaffyTree::new();
        let root = build_layout_tree(&mut tree, &item, direction);
        compute_layout_with_renderer(
            &mut tree,
            root,
            virtual_item_physical_size((frame.width(), frame.height())),
            renderer,
        );
        // The reconstructed widget/tree are temporary; SearchOverlay borrows only
        // the application's source strings, never the widget or its callbacks.
        // Keep global ownership/order so a local blocker cannot expose a popup
        // behind a later full-tree modal/popover (or create a second root owner).
        let mut child = SelectOverlayCollector::new(&tree, self.widget_states, self.viewport);
        child.search_context = self.search_context;
        child.path = self.path.clone();
        push_interactive_virtual_item_path(&mut child.path, key);
        child.clip = intersect_optional(clip, frame);
        child.active_owner = self.active_owner;
        child.top_owner = self.top_owner;
        child.next_order = self.next_order;
        child.visit(&item, root, Point::new(frame.left, frame.top));
        self.top_owner = child.top_owner;
        self.next_order = child.next_order;
        self.searches.append(&mut child.searches);
    }
}
