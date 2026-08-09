// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Contains, Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas, paint};
use taffy::prelude::{NodeId, TaffyTree};

use super::text::get_cached_font;
use crate::engine::widget_state::WidgetState;
use crate::layout::{OPTION_HEIGHT, RutterContext};
use crate::theme::Theme;
use crate::widget::Widget;

mod collector;

use collector::{SelectOverlay, collect_open_select_overlays};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectOptionOverlayHit {
    pub(crate) id: u64,
    pub(crate) index: usize,
}

#[derive(Clone, Copy)]
struct SelectPopupLayout {
    rect: SkiaRect,
    first_option: usize,
    visible_options: usize,
}

pub(crate) fn draw_select_overlays<'a, Msg>(
    canvas: &Canvas,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget: &Widget<'a, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
    scale: f32,
) {
    let viewport = logical_canvas_size(canvas, scale);
    let overlays = collect_open_select_overlays(widget, taffy, root, widget_states, viewport);
    if overlays.is_empty() {
        return;
    }
    canvas.save();
    canvas.reset_matrix();
    canvas.scale((scale, scale));
    for overlay in overlays {
        draw_select_popup(canvas, overlay, viewport, mouse, font_cache, theme);
    }
    canvas.restore();
}

pub(crate) fn hit_test_select_overlay<'a, Msg>(
    widget: &Widget<'a, Msg>,
    taffy: &TaffyTree<RutterContext>,
    root: NodeId,
    widget_states: &HashMap<u64, WidgetState>,
    mouse: Point,
    viewport: (f32, f32),
) -> Option<SelectOptionOverlayHit> {
    let overlays = collect_open_select_overlays(widget, taffy, root, widget_states, viewport);
    overlays.iter().rev().find_map(|overlay| {
        let popup = select_popup_layout(*overlay, viewport);
        select_option_at(popup.rect, mouse, popup.visible_options).map(|row| {
            SelectOptionOverlayHit {
                id: overlay.id,
                index: popup.first_option + row,
            }
        })
    })
}

#[cfg(test)]
fn select_popup_rect(anchor: SkiaRect, option_count: usize, viewport: (f32, f32)) -> SkiaRect {
    popup_layout_for_focus(anchor, option_count, 0, viewport).rect
}

fn select_popup_layout(overlay: SelectOverlay<'_>, viewport: (f32, f32)) -> SelectPopupLayout {
    let focus = overlay.hovered_option.unwrap_or(overlay.selected_index);
    popup_layout_for_focus(overlay.anchor, overlay.options.len(), focus, viewport)
}

fn popup_layout_for_focus(
    anchor: SkiaRect,
    option_count: usize,
    focus: usize,
    viewport: (f32, f32),
) -> SelectPopupLayout {
    let width = anchor.width().min(viewport.0.max(0.0));
    let max_x = (viewport.0 - width).max(0.0);
    let x = anchor.left.clamp(0.0, max_x);
    let (below, available_height) = popup_side(anchor, option_count, viewport.1);
    let visible_options = visible_option_count(option_count, available_height);
    let height = visible_options as f32 * OPTION_HEIGHT;
    let y = if below {
        anchor.bottom
    } else {
        anchor.top - height
    };
    let first_option = first_visible_option(option_count, visible_options, focus);
    SelectPopupLayout {
        rect: SkiaRect::from_xywh(x, y, width, height),
        first_option,
        visible_options,
    }
}

fn popup_side(anchor: SkiaRect, option_count: usize, viewport_height: f32) -> (bool, f32) {
    let full_height = option_count as f32 * OPTION_HEIGHT;
    let space_below = (viewport_height - anchor.bottom).max(0.0);
    let space_above = anchor.top.max(0.0);
    let below =
        full_height <= space_below || full_height > space_above && space_below >= space_above;
    (below, if below { space_below } else { space_above })
}

fn visible_option_count(option_count: usize, available_height: f32) -> usize {
    let available_rows = (available_height / OPTION_HEIGHT).floor() as usize;
    option_count.min(available_rows)
}

fn first_visible_option(option_count: usize, visible_options: usize, focus: usize) -> usize {
    let maximum = option_count.saturating_sub(visible_options);
    focus.saturating_sub(visible_options / 2).min(maximum)
}

fn logical_canvas_size(canvas: &Canvas, scale: f32) -> (f32, f32) {
    let dimensions = canvas.image_info().dimensions();
    let logical_scale = scale.max(f32::EPSILON);
    (
        dimensions.width as f32 / logical_scale,
        dimensions.height as f32 / logical_scale,
    )
}

fn select_option_at(rect: SkiaRect, mouse: Point, option_count: usize) -> Option<usize> {
    if option_count == 0 || !rect.contains(mouse) {
        return None;
    }
    let index = ((mouse.y - rect.top) / OPTION_HEIGHT).floor() as usize;
    (index < option_count).then_some(index)
}

fn draw_select_popup(
    canvas: &Canvas,
    overlay: SelectOverlay<'_>,
    viewport: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let popup = select_popup_layout(overlay, viewport);
    draw_select_popup_surface(canvas, popup.rect, theme);
    canvas.save();
    canvas.clip_rrect(
        RRect::new_rect_xy(popup.rect, 0.0, theme.radius_sm),
        None,
        true,
    );
    let mouse_row = select_option_at(popup.rect, mouse, popup.visible_options);
    let hovered = mouse_row
        .map(|row| popup.first_option + row)
        .or(overlay.hovered_option);
    let font = get_cached_font(font_cache, "sans-serif", theme.font_body);
    for (row, (index, option)) in overlay
        .options
        .iter()
        .enumerate()
        .skip(popup.first_option)
        .take(popup.visible_options)
        .enumerate()
    {
        draw_select_popup_option(
            canvas,
            popup.rect,
            row,
            index,
            option,
            overlay.selected_index,
            hovered,
            &font,
            theme,
        );
    }
    canvas.restore();
    draw_select_popup_border(canvas, popup.rect, theme);
}

fn draw_select_popup_surface(canvas: &Canvas, rect: SkiaRect, theme: &Theme) {
    let mut background = Paint::default();
    background.set_color(theme.surface);
    background.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 0.0, theme.radius_sm), &background);
}

fn draw_select_popup_border(canvas: &Canvas, rect: SkiaRect, theme: &Theme) {
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(Theme::alpha(theme.on_surface, 80));
    border.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 0.0, theme.radius_sm), &border);
}

#[allow(clippy::too_many_arguments)]
fn draw_select_popup_option(
    canvas: &Canvas,
    popup: SkiaRect,
    row: usize,
    index: usize,
    option: &str,
    selected_index: usize,
    hovered_option: Option<usize>,
    font: &Font,
    theme: &Theme,
) {
    let rect = select_option_rect(popup, row);
    draw_select_option_background(canvas, rect, index, selected_index, hovered_option, theme);
    let mut text = Paint::default();
    text.set_color(if index == selected_index {
        theme.primary
    } else {
        theme.on_surface
    });
    text.set_anti_alias(true);
    let baseline = rect.top + OPTION_HEIGHT / 2.0 + theme.font_body / 3.0;
    canvas.draw_str(option, (rect.left + 8.0, baseline), font, &text);
}

fn select_option_rect(popup: SkiaRect, index: usize) -> SkiaRect {
    SkiaRect::from_xywh(
        popup.left + 1.0,
        popup.top + index as f32 * OPTION_HEIGHT,
        (popup.width() - 2.0).max(0.0),
        OPTION_HEIGHT,
    )
}

fn draw_select_option_background(
    canvas: &Canvas,
    rect: SkiaRect,
    index: usize,
    selected_index: usize,
    hovered_option: Option<usize>,
    theme: &Theme,
) {
    if index != selected_index && hovered_option != Some(index) {
        return;
    }
    let mut paint = Paint::default();
    let color = if index == selected_index {
        theme.primary
    } else {
        theme.on_surface
    };
    paint.set_color(Theme::alpha(
        color,
        if index == selected_index { 30 } else { 10 },
    ));
    paint.set_anti_alias(true);
    canvas.draw_rect(rect, &paint);
}

#[cfg(test)]
#[path = "../../tests/unit/select_overlay_unit_tests.rs"]
mod tests;
