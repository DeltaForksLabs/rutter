// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Semantic headings and layout rules for the document table of contents.

use skia_safe::Rect as SkiaRect;
use taffy::prelude::{
    Dimension, FlexDirection, LengthPercentage, NodeId, Rect, Size, Style, TaffyTree,
};

use crate::layout::RutterContext;
use crate::widget::Widget;

pub(crate) const TABLE_OF_CONTENTS_TITLE_SIZE: f32 = 18.0;
pub(crate) const TABLE_OF_CONTENTS_LINK_SIZE: f32 = 14.0;
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
}

#[derive(Clone, Copy)]
pub(crate) struct TableOfContentsLayoutNodes {
    pub(crate) table: NodeId,
    pub(crate) navigation: NodeId,
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
    let content = taffy.children(viewport).ok()?.first().copied()?;
    Some(TableOfContentsLayoutNodes {
        table: table_node,
        navigation,
        viewport,
        content,
    })
}

pub(crate) fn entry_rect(
    taffy: &TaffyTree<RutterContext>,
    table_node: NodeId,
    index: usize,
) -> Option<SkiaRect> {
    navigation_child_rect(taffy, table_node, index + 1)
}

pub(crate) fn navigation_content_height(
    taffy: &TaffyTree<RutterContext>,
    table_node: NodeId,
) -> Option<f32> {
    let navigation = layout_nodes(taffy, table_node)?.navigation;
    let content_height = taffy
        .children(navigation)
        .ok()?
        .iter()
        .filter_map(|node| taffy.layout(*node).ok())
        .map(|layout| layout.location.y + layout.size.height)
        .fold(0.0_f32, f32::max);
    Some(content_height + TABLE_OF_CONTENTS_NAVIGATION_PADDING)
}

pub(crate) fn title_rect(taffy: &TaffyTree<RutterContext>, table_node: NodeId) -> Option<SkiaRect> {
    navigation_child_rect(taffy, table_node, 0)
}

pub(crate) fn entry_text_inset(level: HeadingLevel) -> f32 {
    8.0 + level.index() as f32 * 16.0
}

fn navigation_child_rect(
    taffy: &TaffyTree<RutterContext>,
    table_node: NodeId,
    child_index: usize,
) -> Option<SkiaRect> {
    let nodes = layout_nodes(taffy, table_node)?;
    let navigation = taffy.layout(nodes.navigation).ok()?;
    let entry_node = taffy
        .children(nodes.navigation)
        .ok()?
        .get(child_index)
        .copied()?;
    let entry = taffy.layout(entry_node).ok()?;
    Some(SkiaRect::from_xywh(
        navigation.location.x + entry.location.x,
        navigation.location.y + entry.location.y,
        entry.size.width,
        entry.size.height,
    ))
}

pub(crate) fn collect_entries<Msg>(document: &Widget<'_, Msg>) -> Vec<TableOfContentsEntry> {
    let mut entries = Vec::new();
    collect_entries_impl(document, &mut entries);
    entries
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

pub(crate) fn root_style(style: &Style) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        ..style.clone()
    }
}

pub(crate) fn navigation_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        min_size: Size::zero(),
        padding: Rect::length(TABLE_OF_CONTENTS_NAVIGATION_PADDING),
        gap: Size {
            width: LengthPercentage::length(0.0),
            height: LengthPercentage::length(4.0),
        },
        flex_shrink: 1.0,
        ..Style::default()
    }
}

pub(crate) fn navigation_title_style() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

pub(crate) fn entry_style(level: HeadingLevel) -> Style {
    let mut style = navigation_title_style();
    style.padding = Rect::length(4.0_f32);
    style.padding.left = LengthPercentage::length(8.0 + level.index() as f32 * 16.0);
    style
}

pub(crate) fn viewport_style() -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        min_size: Size::zero(),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use cosmic_text::FontSystem;
    use taffy::prelude::{Dimension, Size};
    use winit::dpi::PhysicalSize;

    use super::*;
    use crate::engine::widget_state::{ScrollState, WidgetState};
    use crate::layout::{build_taffy_tree, compute_layout};

    fn fixed_style(width: f32, height: f32) -> Style {
        Style {
            size: Size {
                width: Dimension::length(width),
                height: Dimension::length(height),
            },
            ..Style::default()
        }
    }

    fn document() -> Widget<'static, ()> {
        Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::heading(HeadingLevel::H1, "Overview", fixed_style(320.0, 36.0)),
                Widget::Spacer {
                    style: fixed_style(320.0, 96.0),
                },
                Widget::heading(HeadingLevel::H2, "Installation", fixed_style(320.0, 32.0)),
            ],
        }
    }

    #[test]
    fn discovers_headings_and_resolves_their_document_offsets() {
        let document = document();
        let table =
            Widget::table_of_contents("Contents", document, fixed_style(320.0, 200.0)).with_id(42);
        let states = HashMap::from([(42, WidgetState::Scroll(ScrollState::default()))]);
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &table, fonts.clone(), &states);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(320, 200),
            fonts,
            &crate::render::RichTextRenderer::default(),
        );

        let Widget::TableOfContents { child, .. } = &table else {
            panic!("expected a TableOfContents widget");
        };
        let entries = collect_entries(child);
        let nodes = layout_nodes(&taffy, root).unwrap();
        let first_offset = entry_offset_y(child, &taffy, nodes.content, 0).unwrap();
        let second_offset = entry_offset_y(child, &taffy, nodes.content, 1).unwrap();
        let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;
        let content_height = taffy.layout(nodes.content).unwrap().size.height;

        assert_eq!(entries[0].title, "Overview");
        assert_eq!(entries[1].level, HeadingLevel::H2);
        assert!(second_offset > first_offset);
        assert!(content_height > viewport_height);
        assert!(
            entry_rect(&taffy, root, 1).unwrap().top > entry_rect(&taffy, root, 0).unwrap().top
        );
    }

    #[test]
    fn long_navigation_keeps_a_document_viewport() {
        let document: Widget<'_, ()> = Widget::Column {
            style: Style::default(),
            children: (0..16)
                .map(|index| -> Widget<'_, ()> {
                    Widget::heading(
                        HeadingLevel::H2,
                        format!("Section {index}"),
                        fixed_style(320.0, 28.0),
                    )
                })
                .collect(),
        };
        let table =
            Widget::table_of_contents("Contents", document, fixed_style(320.0, 220.0)).with_id(43);
        let states = HashMap::from([(43, WidgetState::Scroll(ScrollState::default()))]);
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &table, fonts.clone(), &states);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(320, 220),
            fonts,
            &crate::render::RichTextRenderer::default(),
        );

        let nodes = layout_nodes(&taffy, root).unwrap();
        let navigation_height = taffy.layout(nodes.navigation).unwrap().size.height;
        let viewport_height = taffy.layout(nodes.viewport).unwrap().size.height;

        assert!(navigation_content_height(&taffy, root).unwrap() > navigation_height);
        assert!(viewport_height > 0.0);
    }
}
