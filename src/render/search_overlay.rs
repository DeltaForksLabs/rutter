// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Floating suggestions popup for focused search bars with integrated
//! suggestions.
//!
//! Mirrors the Select popup: geometry comes from the anchor rect, rows scroll
//! in a window around the hovered option, and an empty result set renders a
//! single non-clickable "no matches" row so users get explicit feedback.

use std::collections::HashMap;

use skia_safe::{Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas};
use taffy::prelude::{NodeId, TaffyTree};

use super::overlay_canvas::logical_canvas_size;
use super::select_overlay::collector::{SearchOverlay, collect_open_search_overlays};
use super::select_overlay::{
    SelectPopupLayout, draw_select_popup_border, draw_select_popup_surface, popup_layout_for_focus,
    select_option_at,
};
use super::text::{draw_single_line_text, get_cached_font};
use crate::engine::widget_state::WidgetState;
use crate::input_state::InputWidgetState;
use crate::layout::{OPTION_HEIGHT, RutterContext};
use crate::theme::Theme;
use crate::widget::Widget;

/// A clicked suggestion row, carrying the ORIGINAL index of the item inside
/// the application-provided slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SearchOverlayHit {
    Suggestion { id: u64, index: usize },
    Consume { id: u64 },
}

pub(crate) struct SearchOverlayDrawInput<'render, 'widget, Msg> {
    pub(crate) canvas: &'render Canvas,
    pub(crate) taffy: &'render TaffyTree<RutterContext>,
    pub(crate) root: NodeId,
    pub(crate) widget: &'render Widget<'widget, Msg>,
    pub(crate) widget_states: &'render HashMap<u64, WidgetState>,
    pub(crate) input_states: &'render HashMap<u64, InputWidgetState>,
    pub(crate) focused_id: Option<u64>,
    pub(crate) mouse: Point,
    pub(crate) font_cache: &'render mut HashMap<(String, u32), Font>,
    pub(crate) theme: &'render Theme,
    pub(crate) scale: f32,
}

pub(crate) struct SearchOverlayHitInput<'render, 'widget, Msg> {
    pub(crate) widget: &'render Widget<'widget, Msg>,
    pub(crate) taffy: &'render TaffyTree<RutterContext>,
    pub(crate) root: NodeId,
    pub(crate) widget_states: &'render HashMap<u64, WidgetState>,
    pub(crate) input_states: &'render HashMap<u64, InputWidgetState>,
    pub(crate) focused_id: Option<u64>,
    pub(crate) mouse: Point,
    pub(crate) viewport: (f32, f32),
}

pub(crate) fn draw_search_overlays<'render, 'widget, Msg>(
    input: SearchOverlayDrawInput<'render, 'widget, Msg>,
) {
    let SearchOverlayDrawInput {
        canvas,
        taffy,
        root,
        widget,
        widget_states,
        input_states,
        focused_id,
        mouse,
        font_cache,
        theme,
        scale,
    } = input;
    let viewport = logical_canvas_size(canvas, scale);
    let overlays = collect_open_search_overlays(
        widget,
        taffy,
        root,
        widget_states,
        input_states,
        focused_id,
        viewport,
    );
    if overlays.is_empty() {
        return;
    }
    canvas.save();
    canvas.reset_matrix();
    canvas.scale((scale, scale));
    let font = get_cached_font(font_cache, "sans-serif", theme.font_body);
    for overlay in overlays {
        draw_search_popup(canvas, &overlay, viewport, mouse, &font, theme);
    }
    canvas.restore();
}

/// Maps a pointer position onto a suggestion, returning the original item
/// index. The no-results row is never clickable.
pub(crate) fn hit_test_search_overlay<Msg>(
    input: SearchOverlayHitInput<'_, '_, Msg>,
) -> Option<SearchOverlayHit> {
    let overlays = collect_open_search_overlays(
        input.widget,
        input.taffy,
        input.root,
        input.widget_states,
        input.input_states,
        input.focused_id,
        input.viewport,
    );
    overlays
        .iter()
        .rev()
        .find_map(|overlay| search_overlay_hit(overlay, input.mouse, input.viewport))
}

fn search_overlay_hit(
    overlay: &SearchOverlay<'_>,
    mouse: Point,
    viewport: (f32, f32),
) -> Option<SearchOverlayHit> {
    let popup = search_popup_layout(overlay, viewport);
    let row = select_option_at(popup.rect, mouse, popup.visible_options)?;
    if overlay.matches.is_empty() || !overlay.selectable {
        return Some(SearchOverlayHit::Consume { id: overlay.id });
    }
    let matched = &overlay.matches[popup.first_option + row];
    Some(SearchOverlayHit::Suggestion {
        id: overlay.id,
        index: matched.index,
    })
}

pub(crate) fn search_popup_layout(
    overlay: &SearchOverlay<'_>,
    viewport: (f32, f32),
) -> SelectPopupLayout {
    let focus = overlay
        .hovered_option
        .filter(|hovered| *hovered < overlay.matches.len())
        .unwrap_or(0);
    let row_count = overlay.matches.len().max(1);
    popup_layout_for_focus(overlay.anchor, row_count, focus, viewport)
}

pub(crate) fn search_option_rect(popup: SelectPopupLayout, visible_row: usize) -> SkiaRect {
    SkiaRect::from_xywh(
        popup.rect.left,
        popup.rect.top + visible_row as f32 * OPTION_HEIGHT,
        popup.rect.width(),
        OPTION_HEIGHT,
    )
}

fn draw_search_popup(
    canvas: &Canvas,
    overlay: &SearchOverlay<'_>,
    viewport: (f32, f32),
    mouse: Point,
    font: &Font,
    theme: &Theme,
) {
    let popup = search_popup_layout(overlay, viewport);
    draw_select_popup_surface(canvas, popup.rect, theme);
    canvas.save();
    canvas.clip_rrect(
        RRect::new_rect_xy(popup.rect, 0.0, theme.radius_sm),
        None,
        true,
    );
    if overlay.matches.is_empty() {
        draw_no_results_row(canvas, popup.rect, overlay.empty_label, font, theme);
    } else {
        let mouse_row = select_option_at(popup.rect, mouse, popup.visible_options);
        let hovered = mouse_row.map(|row| popup.first_option + row).or(overlay
            .hovered_option
            .filter(|hovered| hovered < &overlay.matches.len()));
        for (row, matched) in overlay
            .matches
            .iter()
            .skip(popup.first_option)
            .take(popup.visible_options)
            .enumerate()
        {
            let absolute_row = popup.first_option + row;
            draw_suggestion_row(
                canvas,
                popup.rect,
                row,
                overlay.items[matched.index],
                hovered == Some(absolute_row),
                font,
                theme,
            );
        }
    }
    canvas.restore();
    draw_select_popup_border(canvas, popup.rect, theme);
}

fn draw_suggestion_row(
    canvas: &Canvas,
    popup: SkiaRect,
    row: usize,
    item: &str,
    hovered: bool,
    font: &Font,
    theme: &Theme,
) {
    let rect = SkiaRect::from_xywh(
        popup.left + 1.0,
        popup.top + row as f32 * OPTION_HEIGHT,
        (popup.width() - 2.0).max(0.0),
        OPTION_HEIGHT,
    );
    if hovered {
        let mut paint = Paint::default();
        paint.set_color(Theme::alpha(theme.on_surface, 10));
        paint.set_anti_alias(true);
        canvas.draw_rect(rect, &paint);
    }
    let mut text = Paint::default();
    text.set_color(theme.on_surface);
    text.set_anti_alias(true);
    let baseline = rect.top + OPTION_HEIGHT / 2.0 + theme.font_body / 3.0;
    draw_single_line_text(canvas, item, (rect.left + 8.0, baseline), font, &text);
}

fn draw_no_results_row(canvas: &Canvas, popup: SkiaRect, label: &str, font: &Font, theme: &Theme) {
    let mut text = Paint::default();
    text.set_color(Theme::alpha(theme.on_surface, 120));
    text.set_anti_alias(true);
    let baseline = popup.top + OPTION_HEIGHT / 2.0 + theme.font_body / 3.0;
    draw_single_line_text(canvas, label, (popup.left + 8.0, baseline), font, &text);
}

#[cfg(test)]
#[path = "../../tests/unit/search_overlay_unit_tests.rs"]
mod tests;
