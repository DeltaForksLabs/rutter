// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — layout.rs
// ============================================================

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use taffy::Direction;
use taffy::prelude::*;
use winit::dpi::PhysicalSize;

use crate::engine::widget_state::WidgetState;
use crate::i18n::LayoutDirection;
use crate::render::RichTextRenderer;
use crate::render::rich_text::{RichTextDirection, RichTextMetrics, RichTextWidth};
use crate::text_controls::{TextControlPolicy, normalize_text_controls};
use crate::widget::Widget;
use crate::widgets::counter::{COUNTER_DEFAULT_HEIGHT, counter_preferred_width};
use crate::widgets::rich_text::OwnedRichTextSpec;
use crate::widgets::table_of_contents::{
    TABLE_OF_CONTENTS_LINK_SIZE, TABLE_OF_CONTENTS_TITLE_SIZE, TableOfContentsEntry,
    TableOfContentsOptions, collect_entries, entry_style, navigation_column_style,
    navigation_entries_style, navigation_header_style, navigation_style,
    root_style as table_of_contents_root_style, viewport_style,
};
use crate::widgets::time::clock_layout_text;

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
pub struct CounterContext {
    pub value: i64,
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
    Counter(CounterContext),
    RichText(OwnedRichTextSpec),
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

    fn from_widget<'a, Msg>(widget: &Widget<'a, Msg>) -> Self {
        let mut path = Vec::new();
        Self::from_widget_with_path(widget, &mut path)
    }

    fn from_widget_with_direction<'a, Msg>(
        widget: &Widget<'a, Msg>,
        direction: LayoutDirection,
    ) -> Self {
        let mut blueprint = Self::from_widget(widget);
        blueprint.apply_direction(direction);
        blueprint
    }

    fn apply_direction(&mut self, direction: LayoutDirection) {
        self.style.direction = direction.into();
        for child in &mut self.children {
            child.apply_direction(direction);
        }
    }

    fn from_widget_with_path<'a, Msg>(widget: &Widget<'a, Msg>, path: &mut Vec<usize>) -> Self {
        match widget {
            Widget::Disabled { child } => Self::from_widget_with_path(child, path),
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
                        let blueprint = Self::from_widget_with_path(child, path);
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
                        let blueprint = Self::from_widget_with_path(child, path);
                        path.pop();
                        blueprint
                    })
                    .collect();
                Self::with_children(None, style, children)
            }
            Widget::Container { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, path);
                path.pop();
                Self::with_children(None, style.clone(), vec![child])
            }
            Widget::PointerRegion { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, path);
                path.pop();
                Self::with_children(
                    Some(widget.resolved_id(path).unwrap()),
                    style.clone(),
                    vec![child],
                )
            }
            Widget::ScrollView { child, style, .. } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                path.push(0);
                let child = Self::from_widget_with_path(child, path);
                path.pop();
                Self::with_children(Some(resolved_id), style.clone(), vec![child])
            }
            Widget::TableOfContents {
                child,
                style,
                title,
                options,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                let navigation = table_of_contents_navigation(title, child, options);
                path.push(0);
                let mut content = Self::from_widget_with_path(child, path);
                path.pop();
                // Keep the document at its intrinsic height so the viewport can scroll it.
                content.style.flex_shrink = 0.0;
                let viewport = Self::with_children(None, viewport_style(style), vec![content]);
                Self::with_children(
                    Some(resolved_id),
                    table_of_contents_root_style(style),
                    vec![navigation, viewport],
                )
            }
            Widget::Tooltip { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, path);
                path.pop();
                Self::with_children(None, style.clone(), vec![child])
            }
            Widget::ContextMenu { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, path);
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
                let anchor = Self::from_widget_with_path(anchor, path);
                path.pop();

                let popup = if *open {
                    path.push(1);
                    let content = Self::from_widget_with_path(content, path);
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
                    let child = Self::from_widget_with_path(child, path);
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
                    let child = Self::from_widget_with_path(child, path);
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
                    let child = Self::from_widget_with_path(child, path);
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
            Widget::ButtonContent { child, style, .. } => {
                path.push(0);
                let child = Self::from_widget_with_path(child, path);
                path.pop();
                Self::with_children(None, style.clone(), vec![child])
            }
            Widget::Select { style, .. } => {
                Self::leaf(Some(widget.resolved_id(path).unwrap()), style.clone())
            }
            Widget::Custom { style, .. } => {
                Self::leaf(Some(widget.resolved_id(path).unwrap()), style.clone())
            }
            Widget::Counter { value, style, .. } => Self::leaf_with_context(
                Some(widget.resolved_id(path).unwrap()),
                style.clone(),
                RutterContext::Counter(CounterContext { value: *value }),
            ),
            Widget::Clock {
                time_zone,
                config,
                style,
                ..
            } => Self::leaf_with_context(
                Some(widget.resolved_id(path).unwrap()),
                style.clone(),
                RutterContext::Text(TextContext {
                    content: clock_layout_text(*time_zone, config.format()),
                    font_size: config.font_size(),
                }),
            ),
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
            Widget::Heading {
                content,
                level,
                style,
                ..
            } => Self::leaf_with_context(
                None,
                style.clone(),
                RutterContext::Text(TextContext {
                    content: content.clone(),
                    font_size: level.font_size(),
                }),
            ),
            Widget::RichText { content, style } => Self::leaf_with_context(
                None,
                style.clone(),
                RutterContext::RichText(content.to_owned_spec()),
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
            | Widget::DropdownMenu { style, .. }
            | Widget::TextArea { style, .. }
            | Widget::TextInput { style, .. }
            | Widget::SearchBar { style, .. }
            | Widget::Slider { style, .. }
            | Widget::CarouselView { style, .. }
            | Widget::InteractiveCarouselView { style, .. }
            | Widget::Table { style, .. }
            | Widget::VirtualList { style, .. }
            | Widget::VirtualListContent { style, .. }
            | Widget::InteractiveVirtualListContent { style, .. }
            | Widget::VirtualListWithSelection { style, .. }
            | Widget::VirtualListContentWithSelection { style, .. }
            | Widget::VirtualGrid { style, .. } => {
                Self::leaf(Some(widget.resolved_id(path).unwrap()), style.clone())
            }
            Widget::VirtualGridContent { style, .. }
            | Widget::InteractiveVirtualGridContent { style, .. }
            | Widget::VirtualGridWithSelection { style, .. }
            | Widget::VirtualGridContentWithSelection { style, .. } => {
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

fn table_of_contents_navigation<Msg>(
    title: &str,
    document: &Widget<'_, Msg>,
    options: &TableOfContentsOptions<Msg>,
) -> LayoutBlueprint {
    let has_accordion = options.is_accordion();
    let mut children = vec![table_of_contents_text(title, has_accordion)];
    if options.is_expanded() {
        let entries = collect_entries(document);
        if !entries.is_empty() {
            children.push(table_of_contents_entries(
                entries,
                options.columns(),
                has_accordion,
            ));
        }
    }
    LayoutBlueprint::with_children(None, navigation_style(has_accordion), children)
}

fn table_of_contents_text(content: &str, has_accordion: bool) -> LayoutBlueprint {
    LayoutBlueprint::leaf_with_context(
        None,
        navigation_header_style(has_accordion),
        RutterContext::Text(TextContext {
            content: content.to_owned(),
            font_size: TABLE_OF_CONTENTS_TITLE_SIZE,
        }),
    )
}

fn table_of_contents_entries(
    entries: Vec<TableOfContentsEntry>,
    columns: usize,
    has_accordion: bool,
) -> LayoutBlueprint {
    let columns = table_of_contents_entry_columns(entries, columns);
    LayoutBlueprint::with_children(None, navigation_entries_style(has_accordion), columns)
}

fn table_of_contents_entry_columns(
    entries: Vec<TableOfContentsEntry>,
    columns: usize,
) -> Vec<LayoutBlueprint> {
    let column_count = columns.max(1).min(entries.len());
    let base_size = entries.len() / column_count;
    let larger_columns = entries.len() % column_count;
    let mut remaining = entries.as_slice();
    (0..column_count)
        .map(|index| {
            let size = base_size + usize::from(index < larger_columns);
            let (column, rest) = remaining.split_at(size);
            remaining = rest;
            table_of_contents_entry_column(column)
        })
        .collect()
}

fn table_of_contents_entry_column(entries: &[TableOfContentsEntry]) -> LayoutBlueprint {
    let children = entries
        .iter()
        .cloned()
        .map(table_of_contents_entry)
        .collect();
    LayoutBlueprint::with_children(None, navigation_column_style(), children)
}

fn table_of_contents_entry(entry: TableOfContentsEntry) -> LayoutBlueprint {
    let visual_depth = entry.visual_depth();
    LayoutBlueprint::leaf_with_context(
        None,
        entry_style(visual_depth),
        RutterContext::Text(TextContext {
            content: entry.display_title(),
            font_size: TABLE_OF_CONTENTS_LINK_SIZE,
        }),
    )
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
    _widget_states: &HashMap<u64, WidgetState>,
    direction: LayoutDirection,
) -> NodeId {
    let blueprint = LayoutBlueprint::from_widget_with_direction(widget, direction);
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
    _widget_states: &HashMap<u64, WidgetState>,
    direction: LayoutDirection,
) -> NodeId {
    let blueprint = LayoutBlueprint::from_widget_with_direction(widget, direction);
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

/// Computes the current Taffy tree while reusing rich-text font resources.
///
/// Example:
///
/// ```rust
/// use std::{cell::RefCell, rc::Rc};
/// use cosmic_text::FontSystem;
/// use rutter::layout::{RutterContext, compute_layout};
/// use rutter::render::RichTextRenderer;
/// use taffy::{Style, TaffyTree};
/// use winit::dpi::PhysicalSize;
///
/// let mut taffy = TaffyTree::<RutterContext>::new();
/// let root = taffy.new_leaf(Style::default())?;
/// let fonts = Rc::new(RefCell::new(FontSystem::new()));
/// let rich_text_renderer = RichTextRenderer::default();
/// compute_layout(&mut taffy, root, PhysicalSize::new(320, 200), fonts, &rich_text_renderer);
/// # Ok::<(), taffy::TaffyError>(())
/// ```
pub fn compute_layout(
    taffy: &mut TaffyTree<RutterContext>,
    root: NodeId,
    size: PhysicalSize<u32>,
    fs_rc: Rc<RefCell<FontSystem>>,
    rich_text_renderer: &RichTextRenderer,
) {
    let available = Size {
        width: AvailableSpace::Definite(size.width as f32),
        height: AvailableSpace::Definite(size.height as f32),
    };
    taffy
        .compute_layout_with_measure(root, available, |known, available, _, context, style| {
            match context {
                Some(RutterContext::Text(text)) => {
                    measure_plain_text(text, known, available, &fs_rc)
                }
                Some(RutterContext::Counter(counter)) => measure_counter(counter, known),
                Some(RutterContext::RichText(content)) => measure_rich_text(
                    content,
                    known,
                    available,
                    style.direction,
                    rich_text_renderer,
                ),
                Some(RutterContext::None) | None => Size::ZERO,
            }
        })
        .unwrap();
}

fn measure_counter(counter: &CounterContext, known: Size<Option<f32>>) -> Size<f32> {
    let height = known.height.unwrap_or(COUNTER_DEFAULT_HEIGHT);
    Size {
        width: known
            .width
            .unwrap_or_else(|| counter_preferred_width(counter.value, height)),
        height,
    }
}

fn measure_plain_text(
    text: &TextContext,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
    font_system: &Rc<RefCell<FontSystem>>,
) -> Size<f32> {
    let content = normalize_text_controls(&text.content, TextControlPolicy::PreserveLineBreaks);
    let mut font_system = font_system.borrow_mut();
    let mut buffer = Buffer::new(
        &mut font_system,
        Metrics::new(text.font_size, text.font_size * 1.2),
    );
    configure_plain_text_width(&mut buffer, &mut font_system, available.width);
    buffer.set_text(
        &mut font_system,
        content.as_ref(),
        &Attrs::new(),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, true);
    let measured = plain_text_layout_size(&buffer);
    Size {
        width: known.width.unwrap_or(measured.width),
        height: known.height.unwrap_or(measured.height),
    }
}

fn plain_text_layout_size(buffer: &Buffer) -> Size<f32> {
    buffer.layout_runs().fold(Size::ZERO, |size, run| Size {
        width: size.width.max(run.line_w),
        height: size.height.max(run.line_top + run.line_height),
    })
}

fn configure_plain_text_width(
    buffer: &mut Buffer,
    font_system: &mut FontSystem,
    available: AvailableSpace,
) {
    match available {
        AvailableSpace::Definite(width) => buffer.set_size(font_system, Some(width), None),
        AvailableSpace::MaxContent => buffer.set_size(font_system, None, None),
        AvailableSpace::MinContent => buffer.set_size(font_system, Some(0.0), None),
    }
}

fn measure_rich_text(
    content: &OwnedRichTextSpec,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
    direction: Direction,
    renderer: &RichTextRenderer,
) -> Size<f32> {
    let metrics = renderer.measure(
        content,
        rich_text_width(available.width),
        rich_text_direction(direction),
    );
    apply_known_rich_text_size(known, metrics)
}

fn rich_text_width(available: AvailableSpace) -> RichTextWidth {
    match available {
        AvailableSpace::Definite(width) => RichTextWidth::Definite(width),
        AvailableSpace::MinContent => RichTextWidth::MinContent,
        AvailableSpace::MaxContent => RichTextWidth::MaxContent,
    }
}

fn rich_text_direction(direction: Direction) -> RichTextDirection {
    match direction {
        Direction::Rtl => RichTextDirection::RightToLeft,
        Direction::Ltr => RichTextDirection::LeftToRight,
    }
}

fn apply_known_rich_text_size(known: Size<Option<f32>>, measured: RichTextMetrics) -> Size<f32> {
    Size {
        width: known.width.unwrap_or(measured.width),
        height: known.height.unwrap_or(measured.height),
    }
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

    #[test]
    fn plain_text_measurement_treats_carriage_return_as_a_line_break() {
        let font_system = fs();
        let raw = TextContext {
            content: "first\r\nsecond".to_string(),
            font_size: 16.0,
        };
        let canonical = TextContext {
            content: "first\nsecond".to_string(),
            font_size: 16.0,
        };
        let known = Size::NONE;
        let available = Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        };

        let raw_size = measure_plain_text(&raw, known, available, &font_system);
        let canonical_size = measure_plain_text(&canonical, known, available, &font_system);

        assert_eq!(raw_size, canonical_size);
        assert!(raw_size.height > raw.font_size * 1.2);
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

    fn button_content(width: f32) -> Widget<'static, ()> {
        Widget::button_content(
            "Run",
            text("Run", 14.0),
            (),
            Style {
                size: Size {
                    width: Dimension::length(width),
                    height: Dimension::length(36.0),
                },
                ..Style::default()
            },
            None,
            crate::widget::ButtonVariant::Primary,
        )
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
    fn dropdown_menu_is_a_keyed_leaf() {
        let menu = Widget::dropdown_menu(
            "File",
            vec![crate::DropdownMenuEntry::item("Save", ())],
            Style {
                size: Size {
                    width: Dimension::length(96.0),
                    height: Dimension::length(36.0),
                },
                ..Style::default()
            },
        )
        .with_id(77);
        let blueprint = LayoutBlueprint::from_widget(&menu);

        assert_eq!(blueprint.key, Some(77));
        assert!(blueprint.children.is_empty());
        assert_eq!(blueprint.style.size.width, Dimension::length(96.0));
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
    fn button_content_creates_child_layout_node() {
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &button_content(120.0), fs(), &empty_states());
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(200, 100),
            fs(),
            &RichTextRenderer::default(),
        );
        let children = taffy.children(root).unwrap();

        assert_eq!(children.len(), 1);
        assert_eq!(taffy.layout(root).unwrap().size.width, 120.0);
        assert_eq!(
            taffy.get_node_context(children[0]),
            Some(&RutterContext::Text(TextContext {
                content: "Run".to_string(),
                font_size: 14.0,
            }))
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
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(400, 300),
            fs(),
            &RichTextRenderer::default(),
        );
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
    fn visible_dialog_is_absolute_overlay_and_does_not_shift_siblings() {
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
                Widget::Dialog {
                    id: 12,
                    title: "Confirm",
                    message: "Continue?",
                    confirm_label: "Yes",
                    cancel_label: "No",
                    visible: true,
                    on_confirm: (),
                    on_cancel: (),
                    on_dismiss: None,
                    position: crate::widget::DialogPosition::Center,
                    style: Style::default(),
                    child: Box::new(button(200.0)),
                },
                button(120.0),
            ],
        };

        let root = build_taffy_tree(&mut taffy, &widget, fs(), &empty_states());
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(400, 300),
            fs(),
            &RichTextRenderer::default(),
        );
        let children = taffy.children(root).unwrap();
        let dialog = children
            .iter()
            .copied()
            .find(|node| taffy.style(*node).unwrap().position == Position::Absolute)
            .unwrap();
        let first = taffy.layout(children[0]).unwrap();
        let second = children
            .iter()
            .copied()
            .filter(|node| taffy.style(*node).unwrap().position != Position::Absolute)
            .nth(1)
            .and_then(|node| taffy.layout(node).ok())
            .unwrap();

        assert_eq!(taffy.layout(dialog).unwrap().size.width, 400.0);
        assert_eq!(taffy.layout(dialog).unwrap().size.height, 300.0);
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
