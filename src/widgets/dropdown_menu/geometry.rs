// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::{Contains, Point, Rect};

use super::DropdownMenuState;
use super::runtime::{DropdownMenuEntryAccess, entries_at_level};
use crate::i18n::LayoutDirection;

pub(crate) const ITEM_ROW_HEIGHT: f32 = 32.0;
pub(crate) const SEPARATOR_HEIGHT: f32 = 9.0;
pub(crate) const MENU_PADDING: f32 = 4.0;
pub(crate) const VIEWPORT_MARGIN: f32 = 8.0;
pub(crate) const ROOT_GAP: f32 = 4.0;
pub(crate) const SUBMENU_OVERLAP: f32 = 4.0;
pub(crate) const MIN_WIDTH: f32 = 160.0;
pub(crate) const MAX_HEIGHT: f32 = 320.0;

const LABEL_GLYPH_WIDTH: f32 = 8.0;
const ITEM_INLINE_SPACE: f32 = 64.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DropdownMenuSurface {
    pub(crate) level_path: Vec<usize>,
    pub(crate) rect: Rect,
    pub(crate) content_height: f32,
    pub(crate) scroll_y: f32,
    pub(crate) max_scroll: f32,
}

pub(crate) fn estimate_level_width<Entry: DropdownMenuEntryAccess>(entries: &[Entry]) -> f32 {
    let label_width = entries
        .iter()
        .filter_map(DropdownMenuEntryAccess::entry_label)
        .map(estimated_label_width)
        .fold(0.0_f32, f32::max);
    (label_width + ITEM_INLINE_SPACE).max(MIN_WIDTH)
}

fn estimated_label_width(label: &str) -> f32 {
    label.chars().count() as f32 * LABEL_GLYPH_WIDTH
}

pub(crate) fn estimate_content_height<Entry: DropdownMenuEntryAccess>(entries: &[Entry]) -> f32 {
    let rows = entries.iter().map(entry_height).sum::<f32>();
    rows + MENU_PADDING * 2.0
}

fn entry_height<Entry: DropdownMenuEntryAccess>(entry: &Entry) -> f32 {
    if entry.entry_is_focusable() {
        return ITEM_ROW_HEIGHT;
    }
    SEPARATOR_HEIGHT
}

pub(crate) fn place_root_surface(
    anchor: Rect,
    requested_width: f32,
    requested_height: f32,
    viewport: Rect,
    direction: LayoutDirection,
) -> Rect {
    let width = effective_width(requested_width, viewport);
    let horizontal_start = root_horizontal_start(anchor, width, direction);
    let x = clamp_axis(horizontal_start, width, viewport.left, viewport.right);
    let (y, height) = root_vertical_axis(anchor, requested_height, viewport);
    Rect::from_xywh(x, y, width, height)
}

fn root_horizontal_start(anchor: Rect, width: f32, direction: LayoutDirection) -> f32 {
    match direction {
        LayoutDirection::Ltr => anchor.left,
        LayoutDirection::Rtl => anchor.right - width,
    }
}

fn root_vertical_axis(anchor: Rect, height: f32, viewport: Rect) -> (f32, f32) {
    let top = viewport.top + VIEWPORT_MARGIN;
    let bottom = viewport.bottom - VIEWPORT_MARGIN;
    let below = (bottom - anchor.bottom - ROOT_GAP).max(0.0);
    let above = (anchor.top - ROOT_GAP - top).max(0.0);
    if height <= below || below >= above && height > above {
        return (anchor.bottom + ROOT_GAP, height.min(below));
    }
    let effective_height = height.min(above);
    (anchor.top - ROOT_GAP - effective_height, effective_height)
}

pub(crate) fn place_submenu_surface(
    parent_surface: Rect,
    parent_row: Rect,
    requested_width: f32,
    requested_height: f32,
    viewport: Rect,
    direction: LayoutDirection,
) -> Rect {
    let width = effective_width(requested_width, viewport);
    let x = submenu_horizontal_start(parent_surface, width, viewport, direction);
    let height = effective_height(requested_height, viewport);
    let y = clamp_axis(
        parent_row.top - MENU_PADDING,
        height,
        viewport.top,
        viewport.bottom,
    );
    Rect::from_xywh(x, y, width, height)
}

fn submenu_horizontal_start(
    parent: Rect,
    width: f32,
    viewport: Rect,
    direction: LayoutDirection,
) -> f32 {
    let (preferred, fallback) = match direction {
        LayoutDirection::Ltr => (
            parent.right - SUBMENU_OVERLAP,
            parent.left - width + SUBMENU_OVERLAP,
        ),
        LayoutDirection::Rtl => (
            parent.left - width + SUBMENU_OVERLAP,
            parent.right - SUBMENU_OVERLAP,
        ),
    };
    choose_inline_side(preferred, fallback, width, viewport)
}

fn choose_inline_side(preferred: f32, fallback: f32, width: f32, viewport: Rect) -> f32 {
    if axis_fits(preferred, width, viewport.left, viewport.right) {
        return preferred;
    }
    if axis_fits(fallback, width, viewport.left, viewport.right) {
        return fallback;
    }
    clamp_axis(preferred, width, viewport.left, viewport.right)
}

fn effective_width(requested: f32, viewport: Rect) -> f32 {
    finite_nonnegative(requested).min(available_extent(viewport.width()))
}

fn effective_height(requested: f32, viewport: Rect) -> f32 {
    finite_nonnegative(requested)
        .min(MAX_HEIGHT)
        .min(available_extent(viewport.height()))
}

fn available_extent(viewport_extent: f32) -> f32 {
    (finite_nonnegative(viewport_extent) - VIEWPORT_MARGIN * 2.0).max(0.0)
}

fn axis_fits(start: f32, extent: f32, minimum: f32, maximum: f32) -> bool {
    start >= minimum + VIEWPORT_MARGIN && start + extent <= maximum - VIEWPORT_MARGIN
}

fn clamp_axis(start: f32, extent: f32, minimum: f32, maximum: f32) -> f32 {
    let minimum = minimum + VIEWPORT_MARGIN;
    let maximum = (maximum - VIEWPORT_MARGIN - extent).max(minimum);
    finite_or(start, minimum).clamp(minimum, maximum)
}

pub(crate) fn maximum_scroll(content_height: f32, surface_height: f32) -> f32 {
    (finite_nonnegative(content_height) - finite_nonnegative(surface_height)).max(0.0)
}

pub(crate) fn clamp_scroll(scroll_y: f32, max_scroll: f32) -> f32 {
    finite_nonnegative(scroll_y).min(finite_nonnegative(max_scroll))
}

pub(crate) fn scroll_to_reveal(
    scroll_y: f32,
    row_top: f32,
    row_height: f32,
    surface_height: f32,
) -> f32 {
    let visible_height = (surface_height - MENU_PADDING * 2.0).max(0.0);
    let row_bottom = row_top + row_height;
    if row_top < scroll_y {
        return row_top;
    }
    if row_bottom > scroll_y + visible_height {
        return row_bottom - visible_height;
    }
    scroll_y
}

pub(crate) fn row_rect<Entry: DropdownMenuEntryAccess>(
    surface: &DropdownMenuSurface,
    entries: &[Entry],
    index: usize,
) -> Option<Rect> {
    let entry = entries.get(index)?;
    let y = surface.rect.top + MENU_PADDING + row_content_top(entries, index) - surface.scroll_y;
    Some(Rect::from_xywh(
        surface.rect.left,
        y,
        surface.rect.width(),
        entry_height(entry),
    ))
}

fn row_content_top<Entry: DropdownMenuEntryAccess>(entries: &[Entry], index: usize) -> f32 {
    entries.iter().take(index).map(entry_height).sum()
}

pub(crate) fn point_to_entry<Entry: DropdownMenuEntryAccess>(
    surface: &DropdownMenuSurface,
    entries: &[Entry],
    point: Point,
) -> Option<usize> {
    if !surface.rect.contains(point) {
        return None;
    }
    entries.iter().enumerate().find_map(|(index, entry)| {
        let row = row_rect(surface, entries, index)?;
        (entry.entry_is_focusable() && row.contains(point)).then_some(index)
    })
}

pub(crate) fn build_open_menu_surfaces<Entry: DropdownMenuEntryAccess>(
    anchor: Rect,
    entries: &[Entry],
    state: &DropdownMenuState,
    viewport: Rect,
    direction: LayoutDirection,
) -> Vec<DropdownMenuSurface> {
    if !state.is_open() {
        return Vec::new();
    }
    let root = build_root_surface(anchor, entries, state, viewport, direction);
    append_submenu_surfaces(vec![root], entries, state, viewport, direction)
}

fn build_root_surface<Entry: DropdownMenuEntryAccess>(
    anchor: Rect,
    entries: &[Entry],
    state: &DropdownMenuState,
    viewport: Rect,
    direction: LayoutDirection,
) -> DropdownMenuSurface {
    let content_height = estimate_content_height(entries);
    let height = effective_height(content_height, viewport);
    let rect = place_root_surface(
        anchor,
        estimate_level_width(entries),
        height,
        viewport,
        direction,
    );
    build_surface(Vec::new(), rect, content_height, entries, state)
}

fn append_submenu_surfaces<Entry: DropdownMenuEntryAccess>(
    mut surfaces: Vec<DropdownMenuSurface>,
    root_entries: &[Entry],
    state: &DropdownMenuState,
    viewport: Rect,
    direction: LayoutDirection,
) -> Vec<DropdownMenuSurface> {
    let mut level_path = Vec::new();
    for index in state.open_submenu_path() {
        let Some(surface) = build_child_surface(
            &surfaces,
            root_entries,
            &level_path,
            *index,
            state,
            viewport,
            direction,
        ) else {
            break;
        };
        level_path.push(*index);
        surfaces.push(surface);
    }
    surfaces
}

fn build_child_surface<Entry: DropdownMenuEntryAccess>(
    surfaces: &[DropdownMenuSurface],
    root_entries: &[Entry],
    level_path: &[usize],
    submenu_index: usize,
    state: &DropdownMenuState,
    viewport: Rect,
    direction: LayoutDirection,
) -> Option<DropdownMenuSurface> {
    let parent_entries = entries_at_level(root_entries, level_path)?;
    let submenu = parent_entries.get(submenu_index)?;
    let child_entries = enabled_children(submenu)?;
    let parent_row = row_rect(surfaces.last()?, parent_entries, submenu_index)?;
    let child_path = appended_path(level_path, submenu_index);
    let child_direction = submenu_chain_direction(surfaces, direction);
    Some(position_child_surface(
        surfaces.last()?.rect,
        parent_row,
        child_path,
        child_entries,
        state,
        viewport,
        child_direction,
    ))
}

fn submenu_chain_direction(
    surfaces: &[DropdownMenuSurface],
    fallback: LayoutDirection,
) -> LayoutDirection {
    let [.., grandparent, parent] = surfaces else {
        return fallback;
    };
    if parent.rect.right <= grandparent.rect.left + SUBMENU_OVERLAP {
        return LayoutDirection::Rtl;
    }
    if parent.rect.left >= grandparent.rect.right - SUBMENU_OVERLAP {
        return LayoutDirection::Ltr;
    }
    fallback
}

fn enabled_children<Entry: DropdownMenuEntryAccess>(entry: &Entry) -> Option<&[Entry]> {
    if !entry.entry_is_activatable() {
        return None;
    }
    entry.child_entries()
}

fn appended_path(level_path: &[usize], index: usize) -> Vec<usize> {
    let mut path = level_path.to_vec();
    path.push(index);
    path
}

fn position_child_surface<Entry: DropdownMenuEntryAccess>(
    parent_rect: Rect,
    parent_row: Rect,
    level_path: Vec<usize>,
    entries: &[Entry],
    state: &DropdownMenuState,
    viewport: Rect,
    direction: LayoutDirection,
) -> DropdownMenuSurface {
    let content_height = estimate_content_height(entries);
    let height = effective_height(content_height, viewport);
    let rect = place_submenu_surface(
        parent_rect,
        parent_row,
        estimate_level_width(entries),
        height,
        viewport,
        direction,
    );
    build_surface(level_path, rect, content_height, entries, state)
}

fn build_surface<Entry: DropdownMenuEntryAccess>(
    level_path: Vec<usize>,
    rect: Rect,
    content_height: f32,
    entries: &[Entry],
    state: &DropdownMenuState,
) -> DropdownMenuSurface {
    let max_scroll = maximum_scroll(content_height, rect.height());
    let retained = clamp_scroll(state.scroll_offset(level_path.len()), max_scroll);
    let scroll_y = effective_scroll(
        retained,
        &level_path,
        entries,
        state,
        rect.height(),
        max_scroll,
    );
    DropdownMenuSurface {
        level_path,
        rect,
        content_height,
        scroll_y,
        max_scroll,
    }
}

fn effective_scroll<Entry: DropdownMenuEntryAccess>(
    retained: f32,
    level_path: &[usize],
    entries: &[Entry],
    state: &DropdownMenuState,
    surface_height: f32,
    max_scroll: f32,
) -> f32 {
    if !state.should_reveal_active() {
        return retained;
    }
    let Some(index) = active_index_at_level(state.active_path(), level_path) else {
        return retained;
    };
    let Some(entry) = entries.get(index) else {
        return retained;
    };
    let target = scroll_to_reveal(
        retained,
        row_content_top(entries, index),
        entry_height(entry),
        surface_height,
    );
    clamp_scroll(target, max_scroll)
}

fn active_index_at_level(active_path: Option<&[usize]>, level_path: &[usize]) -> Option<usize> {
    let active_path = active_path?;
    if !active_path.starts_with(level_path) {
        return None;
    }
    active_path.get(level_path.len()).copied()
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        return value.max(0.0);
    }
    0.0
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
#[path = "../../../tests/unit/dropdown_menu_geometry_unit_tests.rs"]
mod tests;
