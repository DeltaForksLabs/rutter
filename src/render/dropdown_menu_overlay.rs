// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Contains, Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas, paint};
use taffy::prelude::{NodeId, TaffyTree};

use super::ImageRenderCache;
use super::NativeMenuKind;
use super::control_icons::{ControlChevronDirection, draw_control_chevron};
#[cfg(test)]
use super::menu_row::dropdown_entry_label_clip;
use super::menu_row::{MenuColumns, MenuRowPainter};
use super::overlay_canvas::logical_canvas_size;
use super::select_overlay::collector::{DropdownOverlay, collect_open_dropdown_overlays};
use super::text::get_cached_font;
use crate::engine::widget_state::WidgetState;
use crate::i18n::LayoutDirection;
use crate::layout::RutterContext;
use crate::theme::Theme;
use crate::widget::Widget;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntryAccess, DropdownMenuEntryKind, DropdownMenuState, DropdownMenuSurface,
    MENU_PADDING, build_open_menu_surfaces, entries_at_level, point_to_entry, row_rect,
};

type FontCache = HashMap<(String, u32), Font>;

struct MenuContent<'a, Entry> {
    entries: &'a [Entry],
    state: &'a DropdownMenuState,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_context_menu<Msg>(
    canvas: &Canvas,
    menu: &super::ContextMenuOverlay<'_, Msg>,
    viewport: (f32, f32),
    mouse: Point,
    font: &Font,
    image_cache: &mut ImageRenderCache,
    theme: &Theme,
    scale: f32,
    direction: LayoutDirection,
) {
    let surfaces = super::context_menu_overlay::context_surfaces(menu, viewport, direction, font);
    let content = MenuContent {
        entries: menu.entries,
        state: &menu.state,
    };
    let hover = hover_path(content.entries, &surfaces, mouse);
    let mut painter = MenuPainter::new(canvas, font, theme, direction, true);
    painter.image_cache = Some(image_cache);
    painter.scale = scale;
    for surface in &surfaces {
        painter.draw_surface(&content, surface, hover.as_deref());
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DropdownMenuOverlayHit {
    Entry {
        id: u64,
        path: Vec<usize>,
        kind: DropdownMenuEntryKind,
        disabled: bool,
    },
    Surface {
        id: u64,
        level: usize,
        max_scroll: f32,
    },
    Trigger {
        id: u64,
    },
    Dismiss {
        id: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DropdownMenuScrollTarget {
    pub(crate) id: u64,
    pub(crate) level: usize,
    pub(crate) current_scroll: f32,
    pub(crate) max_scroll: f32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_dropdown_menu_overlays<'a, Msg>(
    canvas: &Canvas,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget: &Widget<'a, Msg>,
    states: &HashMap<u64, WidgetState>,
    mouse: Point,
    shows_interaction_effects: bool,
    fonts: &mut FontCache,
    theme: &Theme,
    scale: f32,
    direction: LayoutDirection,
    native_menu: Option<NativeMenuKind>,
) {
    let viewport = logical_canvas_size(canvas, scale);
    let overlays = collect_open_dropdown_overlays(widget, taffy, root, states, viewport);
    if overlays.is_empty() {
        return;
    }
    canvas.save();
    canvas.reset_matrix();
    canvas.scale((scale, scale));
    let overlays = overlays
        .into_iter()
        .filter(|overlay| native_menu != Some(NativeMenuKind::Dropdown(overlay.id)))
        .collect::<Vec<_>>();
    draw_collected_overlays(
        canvas,
        &overlays,
        viewport,
        mouse,
        shows_interaction_effects,
        fonts,
        theme,
        direction,
    );
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_native_dropdown<Msg>(
    canvas: &Canvas,
    overlays: &[DropdownOverlay<'_, Msg>],
    id: u64,
    viewport: (f32, f32),
    mouse: Point,
    fonts: &mut FontCache,
    theme: &Theme,
    direction: LayoutDirection,
) {
    if let Some(menu) = overlays.iter().find(|menu| menu.id == id) {
        let font = get_cached_font(fonts, "sans-serif", theme.font_body);
        MenuPainter::new(canvas, &font, theme, direction, true).draw_menu(menu, viewport, mouse);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_dropdown_menu_trigger(
    canvas: &Canvas,
    label: &str,
    is_open: bool,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    fonts: &mut FontCache,
    theme: &Theme,
    direction: LayoutDirection,
) {
    super::draw_select_trigger(super::SelectTriggerRenderInput {
        canvas,
        options: &[label],
        selected_index: 0,
        is_open,
        label: "",
        placeholder: "",
        is_focused,
        size,
        mouse,
        font_cache: fonts,
        theme,
        direction,
    });
}

pub(crate) fn hit_test_dropdown_menu_overlay<Msg>(
    overlays: &[DropdownOverlay<'_, Msg>],
    point: Point,
    viewport: (f32, f32),
    direction: LayoutDirection,
) -> Option<DropdownMenuOverlayHit> {
    if let Some(hit) = surface_hit_stack(overlays, point, viewport, direction) {
        return Some(hit);
    }
    if let Some(menu) = overlays
        .iter()
        .rev()
        .find(|menu| menu.visible_anchor.contains(point))
    {
        return Some(DropdownMenuOverlayHit::Trigger { id: menu.id });
    }
    overlays
        .last()
        .map(|menu| DropdownMenuOverlayHit::Dismiss { id: menu.id })
}

fn surface_hit_stack<Msg>(
    overlays: &[DropdownOverlay<'_, Msg>],
    point: Point,
    viewport: (f32, f32),
    direction: LayoutDirection,
) -> Option<DropdownMenuOverlayHit> {
    for menu in overlays.iter().rev() {
        let surfaces = overlay_surfaces(menu, viewport, direction);
        if let Some(surface) = surfaces
            .iter()
            .rev()
            .find(|surface| surface.rect.contains(point))
        {
            return Some(entry_hit(menu, surface, point).unwrap_or(
                DropdownMenuOverlayHit::Surface {
                    id: menu.id,
                    level: surface.level_path.len(),
                    max_scroll: surface.max_scroll,
                },
            ));
        }
    }
    None
}

pub(crate) fn dropdown_menu_entry_hover_at<Msg>(
    overlays: &[DropdownOverlay<'_, Msg>],
    point: Point,
    viewport: (f32, f32),
    direction: LayoutDirection,
) -> Option<DropdownMenuOverlayHit> {
    overlays.iter().rev().find_map(|menu| {
        overlay_surfaces(menu, viewport, direction)
            .iter()
            .rev()
            .find_map(|surface| entry_hit(menu, surface, point))
    })
}

fn entry_hit<Msg>(
    menu: &DropdownOverlay<'_, Msg>,
    surface: &DropdownMenuSurface,
    point: Point,
) -> Option<DropdownMenuOverlayHit> {
    let entries = entries_at_level(menu.entries, &surface.level_path)?;
    let index = point_to_entry(surface, entries, point)?;
    let entry = entries.get(index)?;
    let mut path = surface.level_path.clone();
    path.push(index);
    Some(DropdownMenuOverlayHit::Entry {
        id: menu.id,
        path,
        kind: entry.kind(),
        disabled: entry.is_disabled(),
    })
}

pub(crate) fn dropdown_menu_scroll_target_at<Msg>(
    overlays: &[DropdownOverlay<'_, Msg>],
    point: Point,
    viewport: (f32, f32),
    direction: LayoutDirection,
) -> Option<DropdownMenuScrollTarget> {
    overlays.iter().rev().find_map(|menu| {
        overlay_surfaces(menu, viewport, direction)
            .iter()
            .rev()
            .find(|surface| surface.rect.contains(point))
            .map(|surface| DropdownMenuScrollTarget {
                id: menu.id,
                level: surface.level_path.len(),
                current_scroll: surface.scroll_y,
                max_scroll: surface.max_scroll,
            })
    })
}

fn overlay_surfaces<Msg>(
    menu: &DropdownOverlay<'_, Msg>,
    viewport: (f32, f32),
    direction: LayoutDirection,
) -> Vec<DropdownMenuSurface> {
    let bounds = SkiaRect::from_xywh(0.0, 0.0, viewport.0, viewport.1);
    build_open_menu_surfaces(menu.anchor, menu.entries, &menu.state, bounds, direction)
}

#[allow(clippy::too_many_arguments)]
fn draw_collected_overlays<Msg>(
    canvas: &Canvas,
    overlays: &[DropdownOverlay<'_, Msg>],
    viewport: (f32, f32),
    mouse: Point,
    shows_interaction_effects: bool,
    fonts: &mut FontCache,
    theme: &Theme,
    direction: LayoutDirection,
) {
    let font = get_cached_font(fonts, "sans-serif", theme.font_body);
    let mut painter = MenuPainter::new(canvas, &font, theme, direction, shows_interaction_effects);
    for menu in overlays {
        painter.draw_menu(menu, viewport, mouse);
    }
}

struct MenuPainter<'a> {
    canvas: &'a Canvas,
    font: &'a Font,
    theme: &'a Theme,
    direction: LayoutDirection,
    shows_interaction_effects: bool,
    image_cache: Option<&'a mut ImageRenderCache>,
    scale: f32,
}

impl<'a> MenuPainter<'a> {
    fn new(
        canvas: &'a Canvas,
        font: &'a Font,
        theme: &'a Theme,
        direction: LayoutDirection,
        shows_interaction_effects: bool,
    ) -> Self {
        Self {
            canvas,
            font,
            theme,
            direction,
            shows_interaction_effects,
            image_cache: None,
            scale: 1.0,
        }
    }

    fn draw_menu<Msg>(
        &mut self,
        menu: &DropdownOverlay<'_, Msg>,
        viewport: (f32, f32),
        mouse: Point,
    ) {
        let surfaces = overlay_surfaces(menu, viewport, self.direction);
        let hover = hover_path(menu.entries, &surfaces, mouse);
        let content = MenuContent {
            entries: menu.entries,
            state: &menu.state,
        };
        for surface in &surfaces {
            self.draw_surface(&content, surface, hover.as_deref());
        }
    }

    fn draw_surface<Entry: DropdownMenuEntryAccess>(
        &mut self,
        menu: &MenuContent<'_, Entry>,
        surface: &DropdownMenuSurface,
        hover: Option<&[usize]>,
    ) {
        self.canvas.draw_rrect(
            self.rounded(surface.rect),
            &filled_paint(self.theme.surface),
        );
        self.canvas.save();
        self.canvas
            .clip_rrect(self.rounded(surface.rect), None, true);
        self.draw_entries(menu, surface, hover);
        self.draw_scrollbar(surface);
        self.canvas.restore();
        let border = stroked_paint(Theme::alpha(self.theme.on_surface, 75), 1.0);
        self.canvas.draw_rrect(self.rounded(surface.rect), &border);
    }

    fn draw_entries<Entry: DropdownMenuEntryAccess>(
        &mut self,
        menu: &MenuContent<'_, Entry>,
        surface: &DropdownMenuSurface,
        hover: Option<&[usize]>,
    ) {
        let Some(entries) = entries_at_level(menu.entries, &surface.level_path) else {
            return;
        };
        let font = self.font;
        let columns = MenuColumns::measure(entries, font);
        for (index, entry) in entries.iter().enumerate() {
            let Some(rect) = row_rect(surface, entries, index) else {
                continue;
            };
            let mut path = surface.level_path.clone();
            path.push(index);
            if entry.entry_kind() != DropdownMenuEntryKind::Separator {
                self.draw_entry_background(rect, &path, hover, menu.state);
            }
            self.draw_entry(rect, entry, font, columns);
        }
    }

    fn draw_entry<Entry: DropdownMenuEntryAccess>(
        &mut self,
        rect: SkiaRect,
        entry: &Entry,
        font: &Font,
        columns: MenuColumns,
    ) {
        if entry.entry_kind() == DropdownMenuEntryKind::Separator {
            self.draw_separator(rect);
            return;
        }
        self.draw_entry_mark(rect, entry);
        MenuRowPainter {
            canvas: self.canvas,
            font,
            theme: self.theme,
            direction: self.direction,
            columns,
            image_cache: self.image_cache.as_deref_mut(),
            scale: self.scale,
        }
        .draw(rect, entry);
        if entry.entry_kind() == DropdownMenuEntryKind::Submenu && rect.width() >= 54.0 {
            self.draw_submenu_arrow(rect, entry.entry_is_disabled());
        }
    }

    fn draw_entry_background(
        &self,
        rect: SkiaRect,
        path: &[usize],
        hover: Option<&[usize]>,
        state: &DropdownMenuState,
    ) {
        let color = if self.shows_interaction_effects && state.active_path() == Some(path) {
            Some(Theme::alpha(self.theme.primary, 34))
        } else if self.shows_interaction_effects && state.open_submenu_path().starts_with(path) {
            Some(Theme::alpha(self.theme.primary, 22))
        } else if hover == Some(path) {
            Some(Theme::alpha(self.theme.on_surface, 14))
        } else {
            None
        };
        if let Some(color) = color {
            let width = (rect.width() - 6.0).max(0.0);
            let row = SkiaRect::from_xywh(rect.left + 3.0, rect.top, width, rect.height());
            self.canvas.draw_rect(row, &filled_paint(color));
        }
    }

    fn draw_entry_mark<Entry: DropdownMenuEntryAccess>(&self, rect: SkiaRect, entry: &Entry) {
        let x = match self.direction {
            LayoutDirection::Ltr => rect.left + 15.0,
            LayoutDirection::Rtl => rect.right - 15.0,
        };
        let center = Point::new(x, rect.center_y());
        let color = self.entry_color(entry.entry_is_disabled());
        if entry.entry_checked() == Some(true) {
            let paint = stroked_paint(color, 1.8);
            draw_segments(
                self.canvas,
                (center.x - 4.0, center.y),
                (center.x - 1.0, center.y + 3.0),
                (center.x + 5.0, center.y - 4.0),
                &paint,
            );
        }
        if let Some(selected) = entry.entry_selected() {
            self.canvas
                .draw_circle(center, 5.0, &stroked_paint(color, 1.5));
            if selected {
                self.canvas.draw_circle(center, 2.5, &filled_paint(color));
            }
        }
    }

    fn draw_submenu_arrow(&self, rect: SkiaRect, disabled: bool) {
        let direction = if self.direction == LayoutDirection::Ltr {
            ControlChevronDirection::Right
        } else {
            ControlChevronDirection::Left
        };
        let x = if self.direction == LayoutDirection::Ltr {
            rect.right - 14.0
        } else {
            rect.left + 14.0
        };
        draw_control_chevron(
            self.canvas,
            Point::new(x, rect.center_y()),
            4.0,
            direction,
            self.entry_color(disabled),
        );
    }

    fn draw_separator(&self, rect: SkiaRect) {
        let paint = stroked_paint(Theme::alpha(self.theme.on_surface, 45), 1.0);
        self.canvas.draw_line(
            (rect.left + 10.0, rect.center_y()),
            (rect.right - 10.0, rect.center_y()),
            &paint,
        );
    }

    fn draw_scrollbar(&self, surface: &DropdownMenuSurface) {
        if surface.max_scroll <= 0.0 || surface.content_height <= 0.0 {
            return;
        }
        let track = (surface.rect.height() - MENU_PADDING * 2.0).max(0.0);
        let thumb = (track * surface.rect.height() / surface.content_height)
            .max(18.0)
            .min(track);
        let travel = (track - thumb).max(0.0);
        let top = surface.rect.top + MENU_PADDING + travel * surface.scroll_y / surface.max_scroll;
        let x = match self.direction {
            LayoutDirection::Ltr => surface.rect.right - 7.0,
            LayoutDirection::Rtl => surface.rect.left + 3.0,
        };
        let rect = SkiaRect::from_xywh(x, top, 4.0, thumb);
        self.canvas.draw_rrect(
            RRect::new_rect_xy(rect, 2.0, 2.0),
            &filled_paint(Theme::alpha(self.theme.on_surface, 95)),
        );
    }

    fn rounded(&self, rect: SkiaRect) -> RRect {
        RRect::new_rect_xy(rect, self.theme.radius_sm, self.theme.radius_sm)
    }

    fn entry_color(&self, disabled: bool) -> skia_safe::Color {
        if disabled {
            Theme::alpha(self.theme.on_surface, 90)
        } else {
            self.theme.on_surface
        }
    }
}

fn hover_path<Entry: DropdownMenuEntryAccess>(
    root_entries: &[Entry],
    surfaces: &[DropdownMenuSurface],
    mouse: Point,
) -> Option<Vec<usize>> {
    surfaces.iter().rev().find_map(|surface| {
        let entries = entries_at_level(root_entries, &surface.level_path)?;
        let index = point_to_entry(surface, entries, mouse)?;
        let mut path = surface.level_path.clone();
        path.push(index);
        Some(path)
    })
}

fn draw_segments(
    canvas: &Canvas,
    first: (f32, f32),
    middle: (f32, f32),
    last: (f32, f32),
    paint: &Paint,
) {
    canvas.draw_line(first, middle, paint);
    canvas.draw_line(middle, last, paint);
}

fn filled_paint(color: skia_safe::Color) -> Paint {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.set_anti_alias(true);
    paint
}

fn stroked_paint(color: skia_safe::Color, width: f32) -> Paint {
    let mut paint = filled_paint(color);
    paint.set_style(paint::Style::Stroke);
    paint.set_stroke_width(width);
    paint
}

#[cfg(test)]
#[path = "../../tests/unit/dropdown_menu_overlay_unit_tests.rs"]
mod tests;
