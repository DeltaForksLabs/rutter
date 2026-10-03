// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Pointer-anchored context menus share dropdown surface geometry and navigation.

use skia_safe::{Contains, Font, Point, Rect};

use super::ContextMenuOverlay;
use super::dropdown_menu_overlay::{DropdownMenuOverlayHit, DropdownMenuScrollTarget};
use crate::i18n::LayoutDirection;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntryAccess, DropdownMenuSurface, build_context_menu_surfaces, entries_at_level,
    point_to_entry,
};

pub(crate) fn context_surfaces<Msg>(
    menu: &ContextMenuOverlay<'_, Msg>,
    viewport: (f32, f32),
    direction: LayoutDirection,
    font: &Font,
) -> Vec<DropdownMenuSurface> {
    build_context_menu_surfaces(
        menu.anchor,
        menu.entries,
        &menu.state,
        Rect::from_xywh(0.0, 0.0, viewport.0, viewport.1),
        direction,
        font,
    )
}

pub(crate) fn context_entry_hit<Msg>(
    menu: &ContextMenuOverlay<'_, Msg>,
    surface: &DropdownMenuSurface,
    point: Point,
) -> Option<DropdownMenuOverlayHit> {
    let entries = entries_at_level(menu.entries, &surface.level_path)?;
    let index = point_to_entry(surface, entries, point)?;
    let entry = &entries[index];
    let mut path = surface.level_path.clone();
    path.push(index);
    Some(DropdownMenuOverlayHit::Entry {
        id: menu.id,
        path,
        kind: entry.entry_kind(),
        disabled: entry.entry_is_disabled(),
    })
}

pub(crate) fn context_cursor_targets<Msg>(
    menus: &[ContextMenuOverlay<'_, Msg>],
    point: Point,
    viewport: (f32, f32),
    direction: LayoutDirection,
    font: &Font,
) -> (
    Option<DropdownMenuOverlayHit>,
    Option<DropdownMenuScrollTarget>,
) {
    for menu in menus.iter().rev() {
        let surfaces = context_surfaces(menu, viewport, direction, font);
        if let Some(surface) = surfaces
            .iter()
            .rev()
            .find(|surface| surface.rect.contains(point))
        {
            return (
                context_entry_hit(menu, surface, point),
                Some(DropdownMenuScrollTarget {
                    id: menu.id,
                    level: surface.level_path.len(),
                    current_scroll: surface.scroll_y,
                    max_scroll: surface.max_scroll,
                }),
            );
        }
    }
    (None, None)
}

#[cfg(test)]
#[path = "../../tests/unit/context_menu_overlay_unit_tests.rs"]
mod tests;
