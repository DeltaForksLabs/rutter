// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Semantic headings and layout rules for the document table of contents.

use skia_safe::Rect as SkiaRect;
use taffy::prelude::{NodeId, TaffyTree};

use crate::layout::RutterContext;
use crate::widget::Widget;

mod options;
mod styles;

pub(crate) use options::TableOfContentsAccordion;
pub use options::{TableOfContentsConfigError, TableOfContentsOptions};
pub(crate) use styles::{
    entry_style, navigation_column_style, navigation_entries_style, navigation_header_style,
    navigation_style, root_style, viewport_style,
};

pub(crate) const TABLE_OF_CONTENTS_TITLE_SIZE: f32 = 18.0;
pub(crate) const TABLE_OF_CONTENTS_LINK_SIZE: f32 = 14.0;
pub(crate) const TABLE_OF_CONTENTS_ACCORDION_HEADER_HEIGHT: f32 = 44.0;
const TABLE_OF_CONTENTS_NAVIGATION_PADDING: f32 = 12.0;

/// Semantic rank for a document heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingLevel {
    pub(crate) const fn font_size(self) -> f32 {
        match self {
            Self::H1 => 30.0,
            Self::H2 => 26.0,
            Self::H3 => 22.0,
            Self::H4 => 19.0,
            Self::H5 => 17.0,
            Self::H6 => 15.0,
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::H1 => 0,
            Self::H2 => 1,
            Self::H3 => 2,
            Self::H4 => 3,
            Self::H5 => 4,
            Self::H6 => 5,
        }
    }

    pub(crate) const fn access_level(self) -> u32 {
        self.index() as u32 + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableOfContentsEntry {
    pub(crate) title: String,
    pub(crate) level: HeadingLevel,
    pub(crate) depth: usize,
}

impl TableOfContentsEntry {
    pub(crate) fn display_title(&self) -> String {
        format!("· {}", self.title)
    }

    pub(crate) fn visual_depth(&self) -> usize {
        // Deep semantic ranks remain available to assistive technology, while
        // one visual child inset keeps narrow navigation columns readable.
        self.depth.min(1)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TableOfContentsLayoutNodes {
    pub(crate) navigation: NodeId,
    pub(crate) header: NodeId,
    pub(crate) entries: Option<NodeId>,
    pub(crate) viewport: NodeId,
    pub(crate) content: NodeId,
}

pub(crate) fn layout_nodes(
    taffy: &TaffyTree<RutterContext>,
    table_node: NodeId,
) -> Option<TableOfContentsLayoutNodes> {
    let table_children = taffy.children(table_node).ok()?;
    let navigation = table_children.first().copied()?;
    let viewport = table_children.get(1).copied()?;
    let navigation_children = taffy.children(navigation).ok()?;
    let header = navigation_children.first().copied()?;
    let content = taffy.children(viewport).ok()?.first().copied()?;
    Some(TableOfContentsLayoutNodes {
        navigation,
        header,
        entries: navigation_children.get(1).copied(),
        viewport,
        content,
    })
}

pub(crate) fn entry_rects(taffy: &TaffyTree<RutterContext>, table_node: NodeId) -> Vec<SkiaRect> {
    let Some((columns, origin)) = navigation_entry_columns(taffy, table_node) else {
        return Vec::new();
    };
    columns
        .into_iter()
        .flat_map(|column| column_entry_rects(taffy, column, origin))
        .collect()
}

fn navigation_entry_columns(
    taffy: &TaffyTree<RutterContext>,
    table_node: NodeId,
) -> Option<(Vec<NodeId>, (f32, f32))> {
    let nodes = layout_nodes(taffy, table_node)?;
    let entries_node = nodes.entries?;
    let navigation = taffy.layout(nodes.navigation).ok()?;
    let entries = taffy.layout(entries_node).ok()?;
    let origin = (
        navigation.location.x + entries.location.x,
        navigation.location.y + entries.location.y,
    );
    Some((taffy.children(entries_node).ok()?, origin))
}

pub(crate) fn title_rect(taffy: &TaffyTree<RutterContext>, table_node: NodeId) -> Option<SkiaRect> {
    let nodes = layout_nodes(taffy, table_node)?;
    let navigation = taffy.layout(nodes.navigation).ok()?;
    let header = taffy.layout(nodes.header).ok()?;
    Some(SkiaRect::from_xywh(
        navigation.location.x + header.location.x,
        navigation.location.y + header.location.y,
        header.size.width,
        header.size.height,
    ))
}

pub(crate) fn entry_text_inset(depth: usize) -> f32 {
    8.0 + depth as f32 * 16.0
}

fn column_entry_rects(
    taffy: &TaffyTree<RutterContext>,
    column_node: NodeId,
    parent_origin: (f32, f32),
) -> Vec<SkiaRect> {
    let Ok(column) = taffy.layout(column_node) else {
        return Vec::new();
    };
    let origin = (
        parent_origin.0 + column.location.x,
        parent_origin.1 + column.location.y,
    );
    let Ok(entries) = taffy.children(column_node) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter_map(|entry| entry_rect_at_origin(taffy, entry, origin))
        .collect()
}

fn entry_rect_at_origin(
    taffy: &TaffyTree<RutterContext>,
    entry_node: NodeId,
    origin: (f32, f32),
) -> Option<SkiaRect> {
    let entry = taffy.layout(entry_node).ok()?;
    Some(SkiaRect::from_xywh(
        origin.0 + entry.location.x,
        origin.1 + entry.location.y,
        entry.size.width,
        entry.size.height,
    ))
}

pub(crate) fn collect_entries<Msg>(document: &Widget<'_, Msg>) -> Vec<TableOfContentsEntry> {
    let mut entries = Vec::new();
    collect_entries_impl(document, &mut entries);
    assign_outline_depths(&mut entries);
    entries
}

fn assign_outline_depths(entries: &mut [TableOfContentsEntry]) {
    let mut ancestors = Vec::new();
    for entry in entries {
        while ancestors
            .last()
            .is_some_and(|ancestor: &HeadingLevel| ancestor.index() >= entry.level.index())
        {
            ancestors.pop();
        }
        entry.depth = ancestors.len();
        ancestors.push(entry.level);
    }
}

/// Returns a heading's vertical position relative to the document content node.
///
/// The same visibility rules as [`collect_entries`] are used so a generated
/// link always resolves to the section it represents.
pub(crate) fn entry_offset_y<Msg>(
    document: &Widget<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    document_node: NodeId,
    entry_index: usize,
) -> Option<f32> {
    let mut visited_entries = 0;
    find_heading_offset(
        document,
        taffy,
        document_node,
        0.0,
        entry_index,
        &mut visited_entries,
    )
}

fn find_heading_offset<Msg>(
    widget: &Widget<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    offset_y: f32,
    entry_index: usize,
    visited_entries: &mut usize,
) -> Option<f32> {
    if matches!(widget, Widget::Heading { .. }) {
        return matched_heading_offset(offset_y, entry_index, visited_entries);
    }
    find_nested_heading_offset(widget, taffy, node, offset_y, entry_index, visited_entries)
}

fn matched_heading_offset(
    offset_y: f32,
    entry_index: usize,
    visited_entries: &mut usize,
) -> Option<f32> {
    if *visited_entries == entry_index {
        return Some(offset_y);
    }
    *visited_entries += 1;
    None
}

fn find_nested_heading_offset<Msg>(
    widget: &Widget<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    offset_y: f32,
    entry_index: usize,
    visited_entries: &mut usize,
) -> Option<f32> {
    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            find_child_heading_offset(
                children,
                taffy,
                node,
                offset_y,
                entry_index,
                visited_entries,
            )
        }
        Widget::Container { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. } => {
            find_single_heading_offset(child, taffy, node, offset_y, entry_index, visited_entries)
        }
        Widget::Accordion {
            expanded: true,
            child,
            ..
        }
        | Widget::Modal {
            visible: true,
            child,
            ..
        }
        | Widget::Dialog {
            visible: true,
            child,
            ..
        } => find_single_heading_offset(child, taffy, node, offset_y, entry_index, visited_entries),
        Widget::Popover { anchor, .. } => {
            find_single_heading_offset(anchor, taffy, node, offset_y, entry_index, visited_entries)
        }
        _ => None,
    }
}

fn find_child_heading_offset<Msg>(
    children: &[Widget<'_, Msg>],
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    offset_y: f32,
    entry_index: usize,
    visited_entries: &mut usize,
) -> Option<f32> {
    let nodes = taffy.children(node).ok()?;
    for (index, child) in children.iter().enumerate() {
        let child_node = *nodes.get(index)?;
        let child_offset_y = child_offset_y(taffy, child_node, offset_y)?;
        if let Some(target) = find_heading_offset(
            child,
            taffy,
            child_node,
            child_offset_y,
            entry_index,
            visited_entries,
        ) {
            return Some(target);
        }
    }
    None
}

fn find_single_heading_offset<Msg>(
    child: &Widget<'_, Msg>,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    offset_y: f32,
    entry_index: usize,
    visited_entries: &mut usize,
) -> Option<f32> {
    let child_node = *taffy.children(node).ok()?.first()?;
    let child_offset_y = child_offset_y(taffy, child_node, offset_y)?;
    find_heading_offset(
        child,
        taffy,
        child_node,
        child_offset_y,
        entry_index,
        visited_entries,
    )
}

fn child_offset_y(
    taffy: &TaffyTree<RutterContext>,
    child_node: NodeId,
    parent_offset_y: f32,
) -> Option<f32> {
    let layout = taffy.layout(child_node).ok()?;
    Some(parent_offset_y + layout.location.y)
}

fn collect_entries_impl<Msg>(widget: &Widget<'_, Msg>, entries: &mut Vec<TableOfContentsEntry>) {
    if let Widget::Heading { content, level, .. } = widget {
        entries.push(TableOfContentsEntry {
            title: content.clone(),
            level: *level,
            depth: 0,
        });
        return;
    }
    collect_nested_entries(widget, entries);
}

fn collect_nested_entries<Msg>(widget: &Widget<'_, Msg>, entries: &mut Vec<TableOfContentsEntry>) {
    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                collect_entries_impl(child, entries);
            }
        }
        Widget::Container { child, .. }
        | Widget::ButtonContent { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. } => collect_entries_impl(child, entries),
        Widget::Accordion {
            expanded: true,
            child,
            ..
        }
        | Widget::Modal {
            visible: true,
            child,
            ..
        }
        | Widget::Dialog {
            visible: true,
            child,
            ..
        } => collect_entries_impl(child, entries),
        Widget::Popover { anchor, .. } => collect_entries_impl(anchor, entries),
        Widget::ScrollView { .. }
        | Widget::TableOfContents { .. }
        | Widget::Accordion { .. }
        | Widget::Modal { .. }
        | Widget::Dialog { .. }
        | Widget::Heading { .. }
        | Widget::Text { .. }
        | Widget::RichText { .. }
        | Widget::Image { .. }
        | Widget::Button { .. }
        | Widget::TextInput { .. }
        | Widget::TextArea { .. }
        | Widget::SearchBar { .. }
        | Widget::Checkbox { .. }
        | Widget::Switch { .. }
        | Widget::Radio { .. }
        | Widget::Slider { .. }
        | Widget::Counter { .. }
        | Widget::Clock { .. }
        | Widget::Select { .. }
        | Widget::ProgressBar { .. }
        | Widget::Spinner { .. }
        | Widget::Divider { .. }
        | Widget::Spacer { .. }
        | Widget::TabBar { .. }
        | Widget::Toast { .. }
        | Widget::DropdownMenu { .. }
        | Widget::CarouselView { .. }
        | Widget::VirtualList { .. }
        | Widget::VirtualListContent { .. }
        | Widget::VirtualGrid { .. }
        | Widget::VirtualGridContent { .. }
        | Widget::VirtualListWithSelection { .. }
        | Widget::VirtualListContentWithSelection { .. }
        | Widget::VirtualGridWithSelection { .. }
        | Widget::VirtualGridContentWithSelection { .. } => {}
    }
}

#[cfg(test)]
#[path = "table_of_contents/tests.rs"]
mod tests;
