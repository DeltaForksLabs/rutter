// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — render/mod.rs
// ============================================================

pub mod hit_test;
pub mod pipeline;
pub mod text;

use std::{collections::HashMap, time::Instant};

use cosmic_text::{Cursor, Edit, FontSystem, LayoutRun, SwashCache, Wrap};
use skia_safe::{
    Color as SkiaColor, Contains, Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas,
    paint,
};
use taffy::prelude::{NodeId, TaffyTree};

use self::text::{TextBufferCache, TextShapeRequest, draw_text, get_cached_font};
use crate::engine::widget_state::{
    WidgetState, normalize_virtual_grid_columns, virtual_grid_cell_left, virtual_grid_cell_width,
    virtual_grid_row_count,
};
use crate::input_state::{InputWidgetState, TextSelection, cursor_x_in_run};
use crate::layout::{OPTION_HEIGHT, RutterContext, SCROLLBAR_W, VIRTUAL_GRID_GAP};
use crate::render::hit_test::{context_menu_rect, dialog_card_rect, popover_rect};
use crate::theme::Theme;
use crate::widget::{
    ButtonVariant, CONTEXT_MENU_ITEM_H, CONTEXT_MENU_PAD_Y, CONTEXT_MENU_SEPARATOR_H,
    ContextMenuEntry, DialogAction, DialogPosition, InputState, Orientation, ToastKind,
    ToastPosition, Widget,
};

const ACCORDION_HEADER_H: f32 = 44.0;

#[derive(Clone, Copy)]
struct ToastOverlay<'a> {
    message: &'a str,
    kind: ToastKind,
    position: ToastPosition,
    progress: f32,
    created_at: Instant,
}

#[derive(Clone, Copy)]
struct ContextMenuOverlay<'a, Msg> {
    id: u64,
    entries: &'a [ContextMenuEntry<'a, Msg>],
    anchor: Point,
}

fn color_luminance(color: SkiaColor) -> f32 {
    let channel = |value: u8| {
        let srgb = value as f32 / 255.0;
        if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    };

    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

fn outset_rect(rect: SkiaRect, amount: f32) -> SkiaRect {
    SkiaRect::from_xywh(
        rect.left - amount,
        rect.top - amount,
        rect.width() + amount * 2.0,
        rect.height() + amount * 2.0,
    )
}

fn draw_focus_outline_with_colors(
    canvas: &Canvas,
    rect: SkiaRect,
    radius: f32,
    accent: SkiaColor,
    background: SkiaColor,
) {
    let outer_color = if color_luminance(background) > 0.5 {
        Theme::alpha(SkiaColor::BLACK, 210)
    } else {
        Theme::alpha(SkiaColor::WHITE, 230)
    };

    let mut outer = Paint::default();
    outer.set_style(paint::Style::Stroke);
    outer.set_stroke_width(4.0);
    outer.set_color(outer_color);
    outer.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(outset_rect(rect, 1.5), radius + 1.5, radius + 1.5),
        &outer,
    );

    let mut inner = Paint::default();
    inner.set_style(paint::Style::Stroke);
    inner.set_stroke_width(2.0);
    inner.set_color(Theme::alpha(accent, 235));
    inner.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, radius, radius), &inner);
}

fn draw_focus_outline(canvas: &Canvas, rect: SkiaRect, radius: f32, theme: &Theme) {
    draw_focus_outline_with_colors(canvas, rect, radius, theme.primary, theme.surface);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_widgets<'w, Msg>(
    canvas: &Canvas,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    widget: &Widget<'w, Msg>,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    mouse_pos: Point,
    focused_id: Option<u64>,
    input_states: &HashMap<u64, InputWidgetState>,
    widget_states: &HashMap<u64, WidgetState>,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
) {
    let mut path = Vec::new();
    draw_widgets_impl(
        canvas,
        taffy,
        node,
        widget,
        fs,
        swash,
        mouse_pos,
        focused_id,
        input_states,
        widget_states,
        font_cache,
        text_cache,
        cursor_visible,
        theme,
        scale,
        &mut path,
    );
    let mut path = Vec::new();
    draw_popover_overlays(
        canvas,
        taffy,
        node,
        widget,
        fs,
        swash,
        mouse_pos,
        focused_id,
        input_states,
        widget_states,
        font_cache,
        text_cache,
        cursor_visible,
        theme,
        scale,
        &mut path,
        Point::new(0.0, 0.0),
    );
    draw_toast_overlays(canvas, widget, widget_states, font_cache, theme, scale);
    draw_context_menu_overlays(
        canvas,
        widget,
        widget_states,
        mouse_pos,
        font_cache,
        theme,
        scale,
    );
}

fn draw_toast_overlays<'w, Msg>(
    canvas: &Canvas,
    widget: &Widget<'w, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
    scale: f32,
) {
    let mut toasts = Vec::new();
    let mut path = Vec::new();
    collect_visible_toasts(widget, widget_states, &mut path, &mut toasts);
    if toasts.is_empty() {
        return;
    }

    toasts.sort_by_key(|toast| toast.created_at);

    let dims = canvas.image_info().dimensions();
    let viewport_size = (
        dims.width as f32 / scale.max(f32::EPSILON),
        dims.height as f32 / scale.max(f32::EPSILON),
    );

    let mut top_left = 0;
    let mut top_right = 0;
    let mut bottom_right = 0;
    let mut bottom_left = 0;

    canvas.save();
    canvas.reset_matrix();
    canvas.scale((scale, scale));
    for toast in toasts {
        let index = match toast.position {
            ToastPosition::TopLeft => {
                let index = top_left;
                top_left += 1;
                index
            }
            ToastPosition::TopRight => {
                let index = top_right;
                top_right += 1;
                index
            }
            ToastPosition::BottomRight => {
                let index = bottom_right;
                bottom_right += 1;
                index
            }
            ToastPosition::BottomLeft => {
                let index = bottom_left;
                bottom_left += 1;
                index
            }
        };
        draw_toast(
            canvas,
            toast.message,
            toast.kind,
            toast.position,
            toast.progress,
            viewport_size,
            index,
            font_cache,
            theme,
        );
    }
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_popover_overlays<'w, Msg>(
    canvas: &Canvas,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    widget: &Widget<'w, Msg>,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    mouse_pos: Point,
    focused_id: Option<u64>,
    input_states: &HashMap<u64, InputWidgetState>,
    widget_states: &HashMap<u64, WidgetState>,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
    path: &mut Vec<usize>,
    abs: Point,
) {
    let Ok(layout) = taffy.layout(node) else {
        return;
    };
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    match widget {
        Widget::Popover {
            anchor, content, ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let Ok(node_children) = taffy.children(node) else {
                return;
            };

            if let Some(anchor_node) = node_children.first().copied() {
                path.push(0);
                draw_popover_overlays(
                    canvas,
                    taffy,
                    anchor_node,
                    anchor,
                    fs,
                    swash,
                    mouse_pos,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                    abs_pos,
                );
                path.pop();
            }

            let Some(popover) = widget_states
                .get(&resolved_id)
                .and_then(|state| state.as_popover())
            else {
                return;
            };
            if !popover.is_open {
                return;
            }

            let Some(popup_node) = node_children.get(1).copied() else {
                return;
            };
            let Ok(popup_layout) = taffy.layout(popup_node) else {
                return;
            };
            let Some(content_node) = taffy
                .children(popup_node)
                .ok()
                .and_then(|ids| ids.first().copied())
            else {
                return;
            };

            let dims = canvas.image_info().dimensions();
            let viewport_size = (
                dims.width as f32 / scale.max(f32::EPSILON),
                dims.height as f32 / scale.max(f32::EPSILON),
            );
            let anchor_rect = SkiaRect::from_xywh(
                popover.anchor_x,
                popover.anchor_y,
                popover.anchor_w,
                popover.anchor_h,
            );
            let rect = popover_rect(
                anchor_rect,
                (popup_layout.size.width, popup_layout.size.height),
                viewport_size,
            );

            draw_popover_surface(canvas, rect, theme);
            canvas.save();
            canvas.clip_rect(rect, None, true);
            canvas.translate((rect.left, rect.top));
            path.push(1);
            draw_widgets_impl(
                canvas,
                taffy,
                content_node,
                content,
                fs,
                swash,
                Point::new(mouse_pos.x - rect.left, mouse_pos.y - rect.top),
                focused_id,
                input_states,
                widget_states,
                font_cache,
                text_cache,
                cursor_visible,
                theme,
                scale,
                path,
            );
            draw_popover_overlays(
                canvas,
                taffy,
                content_node,
                content,
                fs,
                swash,
                Point::new(mouse_pos.x - rect.left, mouse_pos.y - rect.top),
                focused_id,
                input_states,
                widget_states,
                font_cache,
                text_cache,
                cursor_visible,
                theme,
                scale,
                path,
                Point::new(rect.left, rect.top),
            );
            path.pop();
            canvas.restore();
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let Ok(ids) = taffy.children(node) else {
                return;
            };
            for (index, child) in children.iter().enumerate() {
                if let Some(child_node) = ids.get(index).copied() {
                    path.push(index);
                    draw_popover_overlays(
                        canvas,
                        taffy,
                        child_node,
                        child,
                        fs,
                        swash,
                        mouse_pos,
                        focused_id,
                        input_states,
                        widget_states,
                        font_cache,
                        text_cache,
                        cursor_visible,
                        theme,
                        scale,
                        path,
                        abs_pos,
                    );
                    path.pop();
                }
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. } => {
            if let Some(child_node) = taffy
                .children(node)
                .ok()
                .and_then(|ids| ids.first().copied())
            {
                path.push(0);
                draw_popover_overlays(
                    canvas,
                    taffy,
                    child_node,
                    child,
                    fs,
                    swash,
                    mouse_pos,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                    abs_pos,
                );
                path.pop();
            }
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if !expanded {
                return;
            }
            if let Some(child_node) = taffy
                .children(node)
                .ok()
                .and_then(|ids| ids.first().copied())
            {
                path.push(0);
                draw_popover_overlays(
                    canvas,
                    taffy,
                    child_node,
                    child,
                    fs,
                    swash,
                    mouse_pos,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                    abs_pos,
                );
                path.pop();
            }
        }
        Widget::Modal { visible, child, .. } | Widget::Dialog { visible, child, .. } => {
            if !visible {
                return;
            }
            if let Some(child_node) = taffy
                .children(node)
                .ok()
                .and_then(|ids| ids.first().copied())
            {
                path.push(0);
                draw_popover_overlays(
                    canvas,
                    taffy,
                    child_node,
                    child,
                    fs,
                    swash,
                    mouse_pos,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                    abs_pos,
                );
                path.pop();
            }
        }
        _ => {}
    }
}

fn draw_context_menu_overlays<'w, Msg>(
    canvas: &Canvas,
    widget: &Widget<'w, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
    mouse_pos: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
    scale: f32,
) {
    let mut overlays = Vec::new();
    let mut path = Vec::new();
    collect_open_context_menus(widget, widget_states, &mut path, &mut overlays);
    if overlays.is_empty() {
        return;
    }

    let dims = canvas.image_info().dimensions();
    let viewport_size = (
        dims.width as f32 / scale.max(f32::EPSILON),
        dims.height as f32 / scale.max(f32::EPSILON),
    );

    canvas.save();
    canvas.reset_matrix();
    canvas.scale((scale, scale));
    for overlay in overlays {
        draw_context_menu(
            canvas,
            overlay.id,
            overlay.entries,
            overlay.anchor,
            viewport_size,
            mouse_pos,
            font_cache,
            theme,
        );
    }
    canvas.restore();
}

fn collect_visible_toasts<'w, Msg>(
    widget: &Widget<'w, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
    out: &mut Vec<ToastOverlay<'w>>,
) {
    match widget {
        Widget::Toast {
            visible,
            message,
            kind,
            position,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            let Some(toast) = widget_states
                .get(&resolved_id)
                .and_then(|state| state.as_toast())
            else {
                return;
            };
            if *visible && toast.visible && !toast.is_expired() {
                out.push(ToastOverlay {
                    message,
                    kind: *kind,
                    position: *position,
                    progress: toast.progress(),
                    created_at: toast.created_at,
                });
            }
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_visible_toasts(child, widget_states, path, out);
                path.pop();
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. } => {
            path.push(0);
            collect_visible_toasts(child, widget_states, path, out);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            collect_visible_toasts(anchor, widget_states, path, out);
            path.pop();
            if *open {
                path.push(1);
                collect_visible_toasts(content, widget_states, path, out);
                path.pop();
            }
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if *expanded {
                path.push(0);
                collect_visible_toasts(child, widget_states, path, out);
                path.pop();
            }
        }
        Widget::Modal { visible, child, .. } => {
            if *visible {
                path.push(0);
                collect_visible_toasts(child, widget_states, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

fn collect_open_context_menus<'w, Msg>(
    widget: &Widget<'w, Msg>,
    widget_states: &HashMap<u64, WidgetState>,
    path: &mut Vec<usize>,
    out: &mut Vec<ContextMenuOverlay<'w, Msg>>,
) {
    match widget {
        Widget::ContextMenu { child, entries, .. } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            if let Some(menu) = widget_states
                .get(&resolved_id)
                .and_then(|state| state.as_context_menu())
            {
                if menu.is_open {
                    out.push(ContextMenuOverlay {
                        id: resolved_id,
                        entries,
                        anchor: Point::new(menu.anchor_x, menu.anchor_y),
                    });
                }
            }
            path.push(0);
            collect_open_context_menus(child, widget_states, path, out);
            path.pop();
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_open_context_menus(child, widget_states, path, out);
                path.pop();
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. } => {
            path.push(0);
            collect_open_context_menus(child, widget_states, path, out);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            collect_open_context_menus(anchor, widget_states, path, out);
            path.pop();
            if *open {
                path.push(1);
                collect_open_context_menus(content, widget_states, path, out);
                path.pop();
            }
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if *expanded {
                path.push(0);
                collect_open_context_menus(child, widget_states, path, out);
                path.pop();
            }
        }
        Widget::Modal { visible, child, .. } | Widget::Dialog { visible, child, .. } => {
            if *visible {
                path.push(0);
                collect_open_context_menus(child, widget_states, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_widgets_impl<'w, Msg>(
    canvas: &Canvas,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    widget: &Widget<'w, Msg>,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    mouse_pos: Point,
    focused_id: Option<u64>,
    input_states: &HashMap<u64, InputWidgetState>,
    widget_states: &HashMap<u64, WidgetState>,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
    path: &mut Vec<usize>,
) {
    let layout = taffy.layout(node).unwrap();
    let pos = Point::new(layout.location.x, layout.location.y);
    let size = (layout.size.width, layout.size.height);
    let local_mouse = Point::new(mouse_pos.x - pos.x, mouse_pos.y - pos.y);
    let resolved_id = widget.resolved_id(path);
    let is_focused = focused_id == widget.keyboard_focus_id(path);

    canvas.save();
    canvas.translate((pos.x, pos.y));

    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node).unwrap();
            for (i, child) in children.iter().enumerate() {
                path.push(i);
                draw_widgets_impl(
                    canvas,
                    taffy,
                    ids[i],
                    child,
                    fs,
                    swash,
                    local_mouse,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                );
                path.pop();
            }
        }
        Widget::Container {
            child,
            color,
            radius,
            ..
        } => {
            if let Some(c) = color {
                let mut p = Paint::default();
                p.set_color(*c);
                p.set_anti_alias(true);
                canvas.draw_rrect(rrect(size, *radius), &p);
            }
            let ids = taffy.children(node).unwrap();
            path.push(0);
            draw_widgets_impl(
                canvas,
                taffy,
                ids[0],
                child,
                fs,
                swash,
                local_mouse,
                focused_id,
                input_states,
                widget_states,
                font_cache,
                text_cache,
                cursor_visible,
                theme,
                scale,
                path,
            );
            path.pop();
        }
        Widget::ScrollView { child, .. } => {
            let resolved_id = resolved_id.unwrap();
            let scroll_state = widget_states.get(&resolved_id).and_then(|s| s.as_scroll());
            let offset_y = scroll_state.map(|s| s.offset_y).unwrap_or(0.0);
            let content_h = scroll_state.map(|s| s.content_height).unwrap_or(0.0);
            canvas.save();
            canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
            canvas.translate((0.0, -offset_y));
            let ids = taffy.children(node).unwrap();
            path.push(0);
            draw_widgets_impl(
                canvas,
                taffy,
                ids[0],
                child,
                fs,
                swash,
                Point::new(local_mouse.x, local_mouse.y + offset_y),
                focused_id,
                input_states,
                widget_states,
                font_cache,
                text_cache,
                cursor_visible,
                theme,
                scale,
                path,
            );
            path.pop();
            canvas.restore();
            if content_h > size.1 {
                draw_scrollbar(canvas, size, scroll_state, theme);
            }
        }
        Widget::Tooltip { child, text, .. } => {
            let ids = taffy.children(node).unwrap();
            path.push(0);
            draw_widgets_impl(
                canvas,
                taffy,
                ids[0],
                child,
                fs,
                swash,
                local_mouse,
                focused_id,
                input_states,
                widget_states,
                font_cache,
                text_cache,
                cursor_visible,
                theme,
                scale,
                path,
            );
            path.pop();
            let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
            if rect.contains(local_mouse) {
                draw_tooltip_popup(canvas, text, local_mouse, font_cache, theme);
            }
        }
        Widget::ContextMenu { child, .. } => {
            let ids = taffy.children(node).unwrap();
            if !ids.is_empty() {
                path.push(0);
                draw_widgets_impl(
                    canvas,
                    taffy,
                    ids[0],
                    child,
                    fs,
                    swash,
                    local_mouse,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                );
                path.pop();
            }
        }
        Widget::Popover { anchor, .. } => {
            let ids = taffy.children(node).unwrap();
            if let Some(anchor_node) = ids.first().copied() {
                path.push(0);
                draw_widgets_impl(
                    canvas,
                    taffy,
                    anchor_node,
                    anchor,
                    fs,
                    swash,
                    local_mouse,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                );
                path.pop();
            }
        }
        Widget::Button {
            text,
            color,
            variant,
            ..
        } => draw_button(
            canvas,
            text,
            *color,
            *variant,
            is_focused,
            size,
            local_mouse,
            font_cache,
            theme,
        ),
        Widget::TextInput {
            label,
            placeholder,
            state,
            error_msg,
            is_password,
            ..
        } => draw_text_input(
            canvas,
            fs,
            swash,
            font_cache,
            text_cache,
            theme,
            scale,
            size,
            is_focused,
            label,
            placeholder,
            *state,
            error_msg.as_deref(),
            *is_password,
            input_states.get(&resolved_id.unwrap()),
            cursor_visible,
            false,
        ),
        Widget::TextArea {
            label,
            placeholder,
            state,
            error_msg,
            ..
        } => draw_text_input(
            canvas,
            fs,
            swash,
            font_cache,
            text_cache,
            theme,
            scale,
            size,
            is_focused,
            label,
            placeholder,
            *state,
            error_msg.as_deref(),
            false,
            input_states.get(&resolved_id.unwrap()),
            cursor_visible,
            true,
        ),
        Widget::SearchBar { placeholder, .. } => draw_search_bar(
            canvas,
            fs,
            swash,
            font_cache,
            text_cache,
            theme,
            scale,
            size,
            is_focused,
            placeholder,
            input_states.get(&resolved_id.unwrap()),
            cursor_visible,
        ),
        Widget::Checkbox { checked, label, .. } => draw_checkbox(
            canvas,
            *checked,
            label,
            is_focused,
            size,
            local_mouse,
            font_cache,
            theme,
        ),
        Widget::Switch { checked, .. } => {
            draw_switch(canvas, *checked, is_focused, size, local_mouse, theme)
        }
        Widget::Radio {
            selected, label, ..
        } => draw_radio(
            canvas,
            *selected,
            label,
            is_focused,
            size,
            local_mouse,
            font_cache,
            theme,
        ),
        Widget::Slider {
            value, min, max, ..
        } => {
            let resolved_id = resolved_id.unwrap();
            let dragging = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_slider())
                .map(|s| s.dragging)
                .unwrap_or(false);
            draw_slider(
                canvas,
                *value,
                *min,
                *max,
                is_focused,
                size,
                local_mouse,
                dragging,
                theme,
            );
        }
        Widget::ProgressBar {
            value,
            indeterminate,
            ..
        } => {
            let resolved_id = resolved_id.unwrap();
            let anim_offset = if *indeterminate {
                widget_states
                    .get(&resolved_id)
                    .and_then(|s| s.as_anim())
                    .map(|a| a.anim_offset)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            draw_progress_bar(canvas, *value, *indeterminate, anim_offset, size, theme);
        }
        Widget::Spinner { .. } => {
            let resolved_id = resolved_id.unwrap();
            let angle = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_anim())
                .map(|a| a.angle)
                .unwrap_or(0.0);
            draw_spinner(canvas, angle, size, theme);
        }
        Widget::Image { data, radius, .. } => draw_image(canvas, data, size, *radius),
        Widget::Divider { orientation, .. } => draw_divider(canvas, *orientation, size, theme),
        Widget::Spacer { .. } => {}
        Widget::Text {
            content,
            color,
            size: font_size,
            ..
        } => {
            let c = color.unwrap_or(theme.on_surface);
            draw_text(
                canvas,
                content,
                (0.0, 0.0).into(),
                size,
                c,
                *font_size,
                font_cache,
                false,
            );
        }
        Widget::Select {
            options,
            selected_index,
            label,
            placeholder,
            ..
        } => {
            let resolved_id = resolved_id.unwrap();
            let sel_state = widget_states.get(&resolved_id).and_then(|s| s.as_select());
            let is_open = sel_state.map(|s| s.is_open).unwrap_or(false);
            let hovered = sel_state.and_then(|s| s.hovered_option);
            draw_select(
                canvas,
                options,
                *selected_index,
                is_open,
                hovered,
                label,
                placeholder,
                is_focused,
                size,
                local_mouse,
                font_cache,
                theme,
            );
        }
        Widget::TabBar { tabs, active, .. } => {
            let tab_w = size.0 / tabs.len().max(1) as f32;
            let anim_x = *active as f32 * tab_w;
            let focused_tab =
                (0..tabs.len()).find(|index| focused_id == widget.tab_focus_id(path, *index));
            draw_tabbar(
                canvas,
                tabs,
                *active,
                anim_x,
                focused_tab,
                size,
                local_mouse,
                font_cache,
                theme,
            );
        }
        Widget::Accordion {
            title,
            expanded,
            child,
            ..
        } => {
            draw_accordion_header(
                canvas,
                title,
                *expanded,
                is_focused,
                size,
                local_mouse,
                font_cache,
                theme,
            );
            if *expanded {
                let ids = taffy.children(node).unwrap();
                canvas.save();
                canvas.translate((0.0, ACCORDION_HEADER_H));
                path.push(0);
                draw_widgets_impl(
                    canvas,
                    taffy,
                    ids[0],
                    child,
                    fs,
                    swash,
                    Point::new(local_mouse.x, local_mouse.y - ACCORDION_HEADER_H),
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    cursor_visible,
                    theme,
                    scale,
                    path,
                );
                path.pop();
                canvas.restore();
            }
        }
        Widget::Modal { visible, child, .. } => {
            if !*visible {
                canvas.restore();
                return;
            }
            let resolved_id = resolved_id.unwrap();
            let alpha = widget_states
                .get(&resolved_id)
                .and_then(|s| s.as_modal())
                .map(|m| m.backdrop_alpha)
                .unwrap_or(180);
            draw_modal(
                canvas,
                taffy,
                node,
                child,
                fs,
                swash,
                mouse_pos,
                focused_id,
                input_states,
                widget_states,
                font_cache,
                text_cache,
                cursor_visible,
                theme,
                scale,
                size,
                alpha,
                path,
            );
        }
        Widget::Dialog {
            title,
            message,
            confirm_label,
            cancel_label,
            visible,
            position,
            ..
        } => {
            if !*visible {
                canvas.restore();
                return;
            }
            draw_dialog(
                canvas,
                title,
                message,
                confirm_label,
                cancel_label,
                *position,
                size,
                focused_id,
                widget.dialog_action_focus_id(path, DialogAction::Confirm),
                widget.dialog_action_focus_id(path, DialogAction::Cancel),
                font_cache,
                text_cache,
                theme,
                fs,
                swash,
                scale,
            );
        }
        Widget::Toast { .. } => {}
        Widget::VirtualList {
            item_height,
            item_count,
            items,
            ..
        } => {
            let resolved_id = resolved_id.unwrap();
            let vstate = widget_states.get(&resolved_id).and_then(|s| s.as_vlist());
            let scroll_y = vstate.map(|v| v.scroll_y).unwrap_or(0.0);
            let selected = vstate.and_then(|v| v.selected_row);
            let hovered = vstate.and_then(|v| v.hovered_row);
            draw_virtual_list(
                canvas,
                item_height,
                item_count,
                items,
                scroll_y,
                selected,
                hovered,
                size,
                local_mouse,
                font_cache,
                theme,
            );
        }
        Widget::VirtualGrid {
            columns,
            item_height,
            item_count,
            items,
            ..
        } => {
            let resolved_id = resolved_id.unwrap();
            let gstate = widget_states.get(&resolved_id).and_then(|s| s.as_vgrid());
            let scroll_y = gstate.map(|g| g.scroll_y).unwrap_or(0.0);
            let selected = gstate.and_then(|g| g.selected_item);
            let hovered = gstate.and_then(|g| g.hovered_item);
            draw_virtual_grid(
                canvas,
                columns,
                item_height,
                item_count,
                items,
                scroll_y,
                selected,
                hovered,
                size,
                local_mouse,
                is_focused,
                font_cache,
                theme,
            );
        }
    }

    canvas.restore();
}

fn draw_tabbar(
    canvas: &Canvas,
    tabs: &[&str],
    active: usize,
    anim_x: f32,
    focused_tab: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    if tabs.is_empty() {
        return;
    }
    let tab_w = size.0 / tabs.len() as f32;
    let bar_h = 2.0_f32;

    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), &bg);

    let mut border = Paint::default();
    border.set_color(Theme::alpha(theme.on_surface, 30));
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    canvas.draw_line((0.0, size.1 - 0.5), (size.0, size.1 - 0.5), &border);

    for (i, tab) in tabs.iter().enumerate() {
        let tx = i as f32 * tab_w;
        let hov = SkiaRect::from_xywh(tx, 0.0, tab_w, size.1).contains(mouse);
        if focused_tab == Some(i) {
            draw_focus_outline(
                canvas,
                SkiaRect::from_xywh(
                    tx + 3.0,
                    3.0,
                    (tab_w - 6.0).max(0.0),
                    (size.1 - 7.0).max(0.0),
                ),
                theme.radius_sm,
                theme,
            );
        }
        let tc = if i == active {
            theme.primary
        } else if hov {
            theme.on_surface
        } else {
            Theme::alpha(theme.on_surface, 140)
        };
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(tc);
        p.set_anti_alias(true);
        let tw = f.measure_str(tab, Some(&p)).0;
        let x = tx + (tab_w - tw) / 2.0;
        let y = size.1 / 2.0 + theme.font_body / 3.0;
        canvas.draw_str(tab, (x, y), &f, &p);
    }

    let mut up = Paint::default();
    up.set_color(theme.primary);
    up.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(anim_x + 4.0, size.1 - bar_h, tab_w - 8.0, bar_h),
            1.0,
            1.0,
        ),
        &up,
    );
}

fn draw_text_input(
    canvas: &Canvas,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    theme: &Theme,
    scale: f32,
    size: (f32, f32),
    is_focused: bool,
    label: &str,
    placeholder: &str,
    state: InputState,
    error_msg: Option<&str>,
    is_password: bool,
    istate: Option<&InputWidgetState>,
    cursor_visible: bool,
    is_multiline: bool,
) {
    let line_height = theme.font_body * 1.3;
    let border_c = theme.input_border(state, is_focused);
    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &bg);
    let mut brd = Paint::default();
    brd.set_style(paint::Style::Stroke);
    brd.set_stroke_width(if is_focused { 1.5 } else { 1.0 });
    brd.set_color(border_c);
    brd.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &brd);

    if !label.is_empty() {
        let lf = get_cached_font(font_cache, "sans-serif", theme.font_label);
        let mut p = Paint::default();
        p.set_color(if is_focused {
            theme.primary
        } else {
            Theme::alpha(theme.on_surface, 180)
        });
        p.set_anti_alias(true);
        canvas.draw_str(label, (4.0, -4.0), &lf, &p);
    }

    let pad_x = theme.spacing * 2.0;
    let pad_y = theme.spacing;
    let text_width = if is_multiline {
        (size.0 - pad_x * 2.0).max(1.0)
    } else {
        10_000.0
    };
    canvas.save();
    canvas.translate((pad_x, pad_y));
    canvas.clip_rect(
        SkiaRect::from_xywh(0.0, 0.0, size.0 - pad_x * 2.0, size.1 - pad_y * 2.0),
        None,
        true,
    );

    let Some(s) = istate else {
        if !placeholder.is_empty() {
            let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let mut p = Paint::default();
            p.set_color(Theme::alpha(theme.on_surface, 120));
            p.set_anti_alias(true);
            let y = if is_multiline {
                theme.font_body
            } else {
                size.1 / 2.0 + theme.font_body / 3.0 - pad_y
            };
            canvas.draw_str(placeholder, (0.0, y), &f, &p);
        }
        canvas.restore();
        return;
    };

    canvas.translate((-s.scroll_x, -s.scroll_y));

    if s.text_is_empty() && !placeholder.is_empty() && !is_focused {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(Theme::alpha(theme.on_surface, 120));
        p.set_anti_alias(true);
        let y = if is_multiline {
            theme.font_body
        } else {
            size.1 / 2.0 + theme.font_body / 3.0 - pad_y
        };
        canvas.draw_str(placeholder, (0.0, y), &f, &p);
        canvas.restore();
        if let Some(msg) = error_msg {
            let ef = get_cached_font(font_cache, "sans-serif", theme.font_small);
            let mut p = Paint::default();
            p.set_color(theme.error);
            p.set_anti_alias(true);
            canvas.draw_str(msg, (4.0, size.1 + theme.spacing * 3.0), &ef, &p);
        }
        return;
    }

    let cursor = s.editor.cursor();
    let mapped_cursor = if is_password {
        Cursor::new(cursor.line, s.password_display_index(cursor.index))
    } else {
        cursor
    };
    let mapped_selection = s
        .selection
        .filter(|selection| !selection.is_empty())
        .map(|selection| map_selection_for_display(selection, s, is_password));

    let display = s.display_text(is_password);

    let buffer = text_cache.get_or_shape(
        fs,
        TextShapeRequest::new(&display, theme.font_body, line_height)
            .with_bounds(
                Some(text_width),
                if is_multiline { None } else { Some(size.1) },
            )
            .with_wrap(if is_multiline {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            }),
    );

    let runs = buffer.layout_runs().collect::<Vec<_>>();

    draw_text_input_runs(
        canvas,
        fs,
        swash,
        theme,
        scale,
        size,
        pad_y,
        line_height,
        is_focused,
        cursor_visible,
        is_multiline,
        mapped_cursor,
        mapped_selection,
        runs,
    );

    canvas.restore();

    if let Some(msg) = error_msg {
        let ef = get_cached_font(font_cache, "sans-serif", theme.font_small);
        let mut p = Paint::default();
        p.set_color(theme.error);
        p.set_anti_alias(true);
        canvas.draw_str(msg, (4.0, size.1 + theme.spacing * 3.0), &ef, &p);
    }
}

fn draw_search_bar(
    canvas: &Canvas,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    theme: &Theme,
    scale: f32,
    size: (f32, f32),
    is_focused: bool,
    placeholder: &str,
    istate: Option<&InputWidgetState>,
    cursor_visible: bool,
) {
    draw_text_input(
        canvas,
        fs,
        swash,
        font_cache,
        text_cache,
        theme,
        scale,
        size,
        is_focused,
        "",
        placeholder,
        InputState::Idle,
        None,
        false,
        istate,
        cursor_visible,
        false,
    );
    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut p = Paint::default();
    p.set_color(Theme::alpha(theme.on_surface, 160));
    p.set_anti_alias(true);
    canvas.draw_str("⌕", (10.0, size.1 / 2.0 + theme.font_body / 3.0), &f, &p);
}

fn map_selection_for_display(
    selection: TextSelection,
    state: &InputWidgetState,
    is_password: bool,
) -> TextSelection {
    if !is_password {
        return selection;
    }

    let (start, end) = selection.normalized();
    TextSelection {
        start: state.password_display_index(start),
        end: state.password_display_index(end),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text_input_runs<'a>(
    canvas: &Canvas,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    theme: &Theme,
    scale: f32,
    size: (f32, f32),
    pad_y: f32,
    line_height: f32,
    is_focused: bool,
    cursor_visible: bool,
    is_multiline: bool,
    cursor: Cursor,
    selection: Option<TextSelection>,
    runs: Vec<LayoutRun<'a>>,
) {
    let inner_height = (size.1 - pad_y * 2.0).max(0.0);
    let vertical_offset = if is_multiline {
        0.0
    } else {
        let run_height = runs
            .first()
            .map(|run| run.line_height)
            .unwrap_or(line_height);
        ((inner_height - run_height).max(0.0)) / 2.0
    };

    let mut cx = 0.0;
    let mut cy = vertical_offset;
    let mut cursor_h = line_height;

    for run in runs.iter() {
        if let Some(run_x) = cursor_x_in_run(cursor, run) {
            cx = run_x;
            cy = vertical_offset + run.line_top;
            cursor_h = run.line_height;
            break;
        }
    }

    if let Some(selection) = selection {
        let (a, b) = selection.normalized();
        for run in runs.iter() {
            let mut x_start = None;
            let mut x_end = None;
            for glyph in run.glyphs.iter() {
                if glyph.start >= a && glyph.start < b {
                    if x_start.is_none() {
                        x_start = Some(glyph.x);
                    }
                    x_end = Some(glyph.x + glyph.w);
                }
            }
            if let (Some(xs), Some(xe)) = (x_start, x_end) {
                let mut sp = Paint::default();
                sp.set_color(Theme::alpha(theme.primary, 60));
                sp.set_anti_alias(true);
                canvas.draw_rect(
                    SkiaRect::from_xywh(
                        xs,
                        vertical_offset + run.line_top,
                        (xe - xs).max(0.0),
                        run.line_height,
                    ),
                    &sp,
                );
            }
        }
    }

    if is_focused && cursor_visible {
        let mut cp = Paint::default();
        cp.set_color(theme.primary);
        cp.set_anti_alias(true);
        canvas.draw_rect(SkiaRect::from_xywh(cx, cy, 1.5, cursor_h), &cp);
    }

    crate::render::pipeline::render_text_runs(
        canvas,
        runs.into_iter(),
        Point::new(0.0, vertical_offset),
        theme.on_surface,
        fs,
        swash,
        scale,
    );
}

fn draw_accordion_header(
    canvas: &Canvas,
    title: &str,
    expanded: bool,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let header_h = ACCORDION_HEADER_H.min(size.1.max(ACCORDION_HEADER_H));
    let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, header_h);
    let hovered = rect.contains(mouse);

    let mut bg = Paint::default();
    bg.set_color(if hovered {
        Theme::alpha(theme.on_surface, 15)
    } else {
        Theme::alpha(theme.on_surface, 8)
    });
    bg.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
        &bg,
    );

    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(Theme::alpha(theme.on_surface, 28));
    border.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
        &border,
    );
    if is_focused {
        draw_focus_outline(
            canvas,
            SkiaRect::from_xywh(1.0, 1.0, (size.0 - 2.0).max(0.0), (header_h - 2.0).max(0.0)),
            theme.radius_sm,
            theme,
        );
    }

    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut tp = Paint::default();
    tp.set_color(theme.on_surface);
    tp.set_anti_alias(true);
    canvas.draw_str(
        title,
        (16.0, header_h / 2.0 + theme.font_body / 3.0),
        &f,
        &tp,
    );

    let caret = if expanded { "▾" } else { "▸" };
    canvas.draw_str(
        caret,
        (size.0 - 20.0, header_h / 2.0 + theme.font_body / 3.0),
        &f,
        &tp,
    );
}

fn draw_progress_bar(
    canvas: &Canvas,
    value: f32,
    indeterminate: bool,
    anim_offset: f32,
    size: (f32, f32),
    theme: &Theme,
) {
    let h = 4.0_f32;
    let y = (size.1 - h) / 2.0;
    let track = SkiaRect::from_xywh(0.0, y, size.0, h);
    let mut tp = Paint::default();
    tp.set_color(Theme::alpha(theme.primary, 30));
    tp.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(track, h / 2.0, h / 2.0), &tp);
    let mut fp = Paint::default();
    fp.set_color(theme.primary);
    fp.set_anti_alias(true);

    if indeterminate {
        let w = size.0 * 0.30;
        let start = anim_offset * (size.0 + w) - w;
        let clip_start = start.max(0.0);
        let clip_end = (start + w).min(size.0);
        let visible_w = (clip_end - clip_start).max(0.0);
        if visible_w > 0.0 {
            canvas.save();
            canvas.clip_rect(track, None, true);
            canvas.draw_rrect(
                RRect::new_rect_xy(
                    SkiaRect::from_xywh(clip_start, y, visible_w, h),
                    h / 2.0,
                    h / 2.0,
                ),
                &fp,
            );
            canvas.restore();
        }
    } else {
        let fw = (value.clamp(0.0, 1.0) * size.0).max(0.0);
        canvas.draw_rrect(
            RRect::new_rect_xy(SkiaRect::from_xywh(0.0, y, fw, h), h / 2.0, h / 2.0),
            &fp,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_modal<Msg>(
    canvas: &Canvas,
    taffy: &TaffyTree<RutterContext>,
    node: NodeId,
    child: &Widget<Msg>,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    mouse_pos: Point,
    focused_id: Option<u64>,
    input_states: &HashMap<u64, InputWidgetState>,
    widget_states: &HashMap<u64, WidgetState>,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
    size: (f32, f32),
    backdrop_alpha: u8,
    path: &mut Vec<usize>,
) {
    let mut bp = Paint::default();
    bp.set_color(Theme::alpha(SkiaColor::BLACK, backdrop_alpha));
    bp.set_anti_alias(true);
    canvas.draw_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), &bp);

    let card_w = (size.0 * 0.85).min(480.0);
    let ids = taffy.children(node).unwrap();
    let child_layout = taffy.layout(ids[0]).unwrap();
    let card_h = child_layout.size.height.max(200.0);
    let card_x = (size.0 - card_w) / 2.0;
    let card_y = (size.1 - card_h) / 2.0;

    let mut card_p = Paint::default();
    card_p.set_color(theme.surface);
    card_p.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(card_x, card_y, card_w, card_h),
            theme.radius_md,
            theme.radius_md,
        ),
        &card_p,
    );

    let mut shadow_p = Paint::default();
    shadow_p.set_style(paint::Style::Stroke);
    shadow_p.set_stroke_width(1.0);
    shadow_p.set_color(Theme::alpha(theme.on_surface, 20));
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(card_x, card_y, card_w, card_h),
            theme.radius_md,
            theme.radius_md,
        ),
        &shadow_p,
    );

    canvas.save();
    canvas.translate((card_x, card_y));
    path.push(0);
    draw_widgets_impl(
        canvas,
        taffy,
        ids[0],
        child,
        fs,
        swash,
        Point::new(mouse_pos.x - card_x, mouse_pos.y - card_y),
        focused_id,
        input_states,
        widget_states,
        font_cache,
        text_cache,
        cursor_visible,
        theme,
        scale,
        path,
    );
    path.pop();
    canvas.restore();
}

fn draw_dialog(
    canvas: &Canvas,
    title: &str,
    message: &str,
    confirm_label: &str,
    cancel_label: &str,
    position: DialogPosition,
    size: (f32, f32),
    focused_id: Option<u64>,
    confirm_focus_id: Option<u64>,
    cancel_focus_id: Option<u64>,
    font_cache: &mut HashMap<(String, u32), Font>,
    text_cache: &mut TextBufferCache,
    theme: &Theme,
    fs: &mut FontSystem,
    swash: &mut SwashCache,
    scale: f32,
) {
    let mut bp = Paint::default();
    bp.set_color(Theme::alpha(SkiaColor::BLACK, 180));
    bp.set_anti_alias(true);
    canvas.draw_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), &bp);

    let card = dialog_card_rect(position, size);
    let card_w = card.width();
    let card_h = card.height();
    let card_x = card.left;
    let card_y = card.top;

    let mut card_p = Paint::default();
    card_p.set_color(theme.surface);
    card_p.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(card_x, card_y, card_w, card_h),
            theme.radius_md,
            theme.radius_md,
        ),
        &card_p,
    );

    let tf = get_cached_font(font_cache, "sans-serif", 18.0);
    let mut tp = Paint::default();
    tp.set_color(theme.on_surface);
    tp.set_anti_alias(true);
    canvas.draw_str(title, (card_x + 24.0, card_y + 40.0), &tf, &tp);

    let buffer = text_cache.get_or_shape(
        fs,
        TextShapeRequest::new(message, 14.0, 20.0)
            .with_bounds(Some(card_w - 48.0), None)
            .with_wrap(Wrap::WordOrGlyph),
    );

    crate::render::pipeline::render_text_runs(
        canvas,
        buffer.layout_runs(),
        Point::new(card_x + 24.0, card_y + 60.0),
        Theme::alpha(theme.on_surface, 180),
        fs,
        swash,
        scale,
    );

    let mf = get_cached_font(font_cache, "sans-serif", 14.0);

    let cancel_w = 100.0;
    let confirm_w = 100.0;
    let btn_h = 36.0;
    let cancel_rect = SkiaRect::from_xywh(
        card_x + card_w - 24.0 - confirm_w - 12.0 - cancel_w,
        card_y + card_h - 24.0 - btn_h,
        cancel_w,
        btn_h,
    );
    let mut cancel_p = Paint::default();
    cancel_p.set_color(Theme::alpha(theme.on_surface, 20));
    cancel_p.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(cancel_rect, theme.radius_sm, theme.radius_sm),
        &cancel_p,
    );
    let mut cancel_tp = Paint::default();
    cancel_tp.set_color(theme.on_surface);
    cancel_tp.set_anti_alias(true);
    let cw = mf.measure_str(cancel_label, Some(&cancel_tp)).0;
    canvas.draw_str(
        cancel_label,
        (
            cancel_rect.left + (cancel_w - cw) / 2.0,
            cancel_rect.top + 24.0,
        ),
        &mf,
        &cancel_tp,
    );

    let confirm_rect = SkiaRect::from_xywh(
        card_x + card_w - 24.0 - confirm_w,
        card_y + card_h - 24.0 - btn_h,
        confirm_w,
        btn_h,
    );
    let mut confirm_p = Paint::default();
    confirm_p.set_color(theme.primary);
    confirm_p.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(confirm_rect, theme.radius_sm, theme.radius_sm),
        &confirm_p,
    );
    if focused_id == cancel_focus_id {
        draw_focus_outline_with_colors(
            canvas,
            cancel_rect,
            theme.radius_sm,
            Theme::alpha(theme.on_surface, 180),
            theme.surface,
        );
    }
    let mut confirm_tp = Paint::default();
    confirm_tp.set_color(theme.on_primary);
    confirm_tp.set_anti_alias(true);
    let cw2 = mf.measure_str(confirm_label, Some(&confirm_tp)).0;
    canvas.draw_str(
        confirm_label,
        (
            confirm_rect.left + (confirm_w - cw2) / 2.0,
            confirm_rect.top + 24.0,
        ),
        &mf,
        &confirm_tp,
    );
    if focused_id == confirm_focus_id {
        draw_focus_outline_with_colors(
            canvas,
            confirm_rect,
            theme.radius_sm,
            theme.primary,
            theme.surface,
        );
    }
}

fn draw_toast(
    canvas: &Canvas,
    message: &str,
    kind: ToastKind,
    position: crate::widget::ToastPosition,
    progress: f32,
    size: (f32, f32),
    index: usize,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let accent = match kind {
        ToastKind::Info => theme.primary,
        ToastKind::Success => theme.success,
        ToastKind::Warning => SkiaColor::from_rgb(204, 160, 0),
        ToastKind::Error => theme.error,
    };

    let pad = 16.0_f32;
    let h = 48.0_f32;
    let toast_w = 320.0_f32;

    let x = match position {
        crate::widget::ToastPosition::TopLeft | crate::widget::ToastPosition::BottomLeft => pad,
        crate::widget::ToastPosition::TopRight | crate::widget::ToastPosition::BottomRight => {
            size.0 - toast_w - pad
        }
    };

    let y = match position {
        crate::widget::ToastPosition::TopLeft | crate::widget::ToastPosition::TopRight => {
            pad + (h + pad) * index as f32
        }
        crate::widget::ToastPosition::BottomLeft | crate::widget::ToastPosition::BottomRight => {
            size.1 - (h + pad) * (index as f32 + 1.0)
        }
    };

    let rect = SkiaRect::from_xywh(x, y, toast_w, h);

    let mut bg = Paint::default();
    bg.set_color(Theme::alpha(SkiaColor::from_rgb(30, 30, 30), 240));
    bg.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 8.0, 8.0), &bg);

    let mut strip = Paint::default();
    strip.set_color(accent);
    strip.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(SkiaRect::from_xywh(x, y, 4.0, h), 2.0, 2.0),
        &strip,
    );

    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut tp = Paint::default();
    tp.set_color(SkiaColor::from_rgb(230, 230, 230));
    tp.set_anti_alias(true);
    let ty = y + h / 2.0 + theme.font_body / 3.0;
    canvas.draw_str(message, (x + 16.0, ty), &f, &tp);

    if progress > 0.0 && progress < 1.0 {
        let bar_w = toast_w * progress;
        let mut pp = Paint::default();
        pp.set_color(Theme::alpha(accent, 100));
        pp.set_anti_alias(true);
        canvas.draw_rect(SkiaRect::from_xywh(x, y + h - 3.0, bar_w, 3.0), &pp);
    }
}

fn draw_context_menu<Msg>(
    canvas: &Canvas,
    _id: u64,
    entries: &[ContextMenuEntry<'_, Msg>],
    anchor: Point,
    viewport_size: (f32, f32),
    mouse_pos: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let rect = context_menu_rect(entries, anchor, viewport_size, theme.font_body);

    let mut bg = Paint::default();
    bg.set_color(Theme::alpha(SkiaColor::from_rgb(28, 28, 28), 248));
    bg.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 8.0, 8.0), &bg);

    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(Theme::alpha(theme.primary, 70));
    border.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 8.0, 8.0), &border);

    let mut y = rect.top + CONTEXT_MENU_PAD_Y;
    for entry in entries {
        match entry {
            ContextMenuEntry::Separator => {
                let mut sep = Paint::default();
                sep.set_color(Theme::alpha(theme.on_surface, 28));
                sep.set_style(paint::Style::Stroke);
                sep.set_stroke_width(1.0);
                canvas.draw_line(
                    (rect.left + 8.0, y + CONTEXT_MENU_SEPARATOR_H / 2.0),
                    (rect.right - 8.0, y + CONTEXT_MENU_SEPARATOR_H / 2.0),
                    &sep,
                );
                y += CONTEXT_MENU_SEPARATOR_H;
            }
            ContextMenuEntry::Item { label, on_select } => {
                let item_rect =
                    SkiaRect::from_xywh(rect.left, y, rect.width(), CONTEXT_MENU_ITEM_H);
                let hovered = item_rect.contains(mouse_pos);
                if hovered {
                    let mut hp = Paint::default();
                    hp.set_color(Theme::alpha(theme.primary, 34));
                    hp.set_anti_alias(true);
                    canvas.draw_rrect(
                        RRect::new_rect_xy(
                            SkiaRect::from_xywh(
                                item_rect.left + 4.0,
                                item_rect.top + 2.0,
                                (item_rect.width() - 8.0).max(0.0),
                                (item_rect.height() - 4.0).max(0.0),
                            ),
                            6.0,
                            6.0,
                        ),
                        &hp,
                    );
                }

                let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
                let mut tp = Paint::default();
                tp.set_color(if on_select.is_some() {
                    if hovered {
                        theme.primary
                    } else {
                        theme.on_surface
                    }
                } else {
                    Theme::alpha(theme.on_surface, 100)
                });
                tp.set_anti_alias(true);
                let text_y = item_rect.top + item_rect.height() / 2.0 + theme.font_body / 3.0;
                canvas.draw_str(label, (rect.left + 12.0, text_y), &f, &tp);
                y += CONTEXT_MENU_ITEM_H;
            }
        }
    }
}

fn draw_popover_surface(canvas: &Canvas, rect: SkiaRect, theme: &Theme) {
    let shadow_rect = SkiaRect::from_xywh(rect.left, rect.top + 3.0, rect.width(), rect.height());
    let mut shadow = Paint::default();
    shadow.set_color(Theme::alpha(SkiaColor::BLACK, 42));
    shadow.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(shadow_rect, theme.radius_md, theme.radius_md),
        &shadow,
    );

    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_md, theme.radius_md),
        &bg,
    );

    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(Theme::alpha(theme.on_surface, 38));
    border.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_md, theme.radius_md),
        &border,
    );
}

fn draw_virtual_list(
    canvas: &Canvas,
    item_height: &f32,
    item_count: &usize,
    items: &dyn Fn(usize) -> Option<String>,
    scroll_y: f32,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let ih = *item_height;
    let count = *item_count;
    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), &bg);

    canvas.save();
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);

    let first = (scroll_y / ih).floor() as usize;
    let vis = (size.1 / ih).ceil() as usize + 1;
    let last = (first + vis).min(count);

    for i in first..last {
        let y = i as f32 * ih - scroll_y;
        let rect = SkiaRect::from_xywh(0.0, y, size.0 - SCROLLBAR_W - 4.0, ih);
        let is_sel = selected == Some(i);
        let is_hov = hovered == Some(i) || SkiaRect::from_xywh(0.0, y, size.0, ih).contains(mouse);

        if is_sel || is_hov {
            let bg_c = if is_sel {
                Theme::alpha(theme.primary, 40)
            } else {
                Theme::alpha(theme.on_surface, 12)
            };
            let mut ip = Paint::default();
            ip.set_color(bg_c);
            ip.set_anti_alias(true);
            canvas.draw_rect(rect, &ip);
        }

        if let Some(text) = items(i) {
            let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let tc = if is_sel {
                theme.primary
            } else {
                theme.on_surface
            };
            let mut tp = Paint::default();
            tp.set_color(tc);
            tp.set_anti_alias(true);
            let ty = y + ih / 2.0 + theme.font_body / 3.0;
            canvas.draw_str(&text, (12.0, ty), &f, &tp);
        }

        let mut sep = Paint::default();
        sep.set_color(Theme::alpha(theme.on_surface, 15));
        sep.set_style(paint::Style::Stroke);
        sep.set_stroke_width(0.5);
        canvas.draw_line((0.0, y + ih - 0.5), (size.0, y + ih - 0.5), &sep);
    }

    canvas.restore();

    let total_h = ih * count as f32;
    if total_h > size.1 {
        let max_s = (total_h - size.1).max(1.0);
        let ratio = (size.1 / total_h).clamp(0.0, 1.0);
        let thumb_h = (size.1 * ratio).max(20.0);
        let thumb_y = (scroll_y / max_s) * (size.1 - thumb_h);
        let sb_x = size.0 - SCROLLBAR_W - 2.0;

        let mut st = Paint::default();
        st.set_color(Theme::alpha(theme.on_surface, 20));
        st.set_anti_alias(true);
        canvas.draw_rrect(
            RRect::new_rect_xy(
                SkiaRect::from_xywh(sb_x, 0.0, SCROLLBAR_W, size.1),
                4.0,
                4.0,
            ),
            &st,
        );
        let mut sm = Paint::default();
        sm.set_color(Theme::alpha(theme.on_surface, 70));
        sm.set_anti_alias(true);
        canvas.draw_rrect(
            RRect::new_rect_xy(
                SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h),
                4.0,
                4.0,
            ),
            &sm,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_grid(
    canvas: &Canvas,
    columns: &usize,
    item_height: &f32,
    item_count: &usize,
    items: &dyn Fn(usize) -> Option<String>,
    scroll_y: f32,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    is_focused: bool,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let columns = normalize_virtual_grid_columns(*columns);
    let row_h = *item_height;
    let count = *item_count;
    let row_count = virtual_grid_row_count(count, columns);
    let cell_w = virtual_grid_cell_width(size.0, columns);

    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), &bg);

    canvas.save();
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);

    let first_row = (scroll_y / row_h).floor() as usize;
    let vis_rows = (size.1 / row_h).ceil() as usize + 1;
    let last_row = (first_row + vis_rows).min(row_count);

    for row in first_row..last_row {
        let y = row as f32 * row_h - scroll_y;
        let cell_h = (row_h - VIRTUAL_GRID_GAP).max(12.0);
        let cell_y = y + VIRTUAL_GRID_GAP * 0.5;

        for col in 0..columns {
            let index = row * columns + col;
            if index >= count {
                break;
            }

            let cell_x = virtual_grid_cell_left(col, size.0, columns);
            let rect = SkiaRect::from_xywh(cell_x, cell_y, cell_w, cell_h);
            let is_sel = selected == Some(index);
            let is_hov = hovered == Some(index) || rect.contains(mouse);

            let fill = if is_sel {
                Theme::alpha(theme.primary, 36)
            } else if is_hov {
                Theme::alpha(theme.on_surface, 12)
            } else {
                Theme::alpha(theme.on_surface, 6)
            };
            let border = if is_sel {
                Theme::alpha(theme.primary, 130)
            } else {
                Theme::alpha(theme.on_surface, 22)
            };

            let mut cell_bg = Paint::default();
            cell_bg.set_color(fill);
            cell_bg.set_anti_alias(true);
            canvas.draw_rrect(
                RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
                &cell_bg,
            );

            let mut cell_border = Paint::default();
            cell_border.set_style(paint::Style::Stroke);
            cell_border.set_stroke_width(1.0);
            cell_border.set_color(border);
            cell_border.set_anti_alias(true);
            canvas.draw_rrect(
                RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
                &cell_border,
            );

            if is_focused && is_sel {
                draw_focus_outline(
                    canvas,
                    SkiaRect::from_xywh(
                        rect.left + 1.0,
                        rect.top + 1.0,
                        (rect.width() - 2.0).max(0.0),
                        (rect.height() - 2.0).max(0.0),
                    ),
                    theme.radius_sm,
                    theme,
                );
            }

            if let Some(text) = items(index) {
                let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
                let tc = if is_sel {
                    theme.primary
                } else {
                    theme.on_surface
                };
                let mut tp = Paint::default();
                tp.set_color(tc);
                tp.set_anti_alias(true);
                let text_x = rect.left + 12.0;
                let text_y = rect.top + rect.height() / 2.0 + theme.font_body / 3.0;
                canvas.draw_str(&text, (text_x, text_y), &f, &tp);
            }
        }
    }

    canvas.restore();

    if is_focused && selected.is_none() {
        draw_focus_outline(
            canvas,
            SkiaRect::from_xywh(2.0, 2.0, (size.0 - 4.0).max(0.0), (size.1 - 4.0).max(0.0)),
            theme.radius_sm,
            theme,
        );
    }

    let total_h = row_h * row_count as f32;
    if total_h > size.1 {
        let max_s = (total_h - size.1).max(1.0);
        let ratio = (size.1 / total_h).clamp(0.0, 1.0);
        let thumb_h = (size.1 * ratio).max(20.0);
        let thumb_y = (scroll_y / max_s) * (size.1 - thumb_h);
        let sb_x = size.0 - SCROLLBAR_W - 2.0;

        let mut st = Paint::default();
        st.set_color(Theme::alpha(theme.on_surface, 20));
        st.set_anti_alias(true);
        canvas.draw_rrect(
            RRect::new_rect_xy(
                SkiaRect::from_xywh(sb_x, 0.0, SCROLLBAR_W, size.1),
                4.0,
                4.0,
            ),
            &st,
        );
        let mut sm = Paint::default();
        sm.set_color(Theme::alpha(theme.on_surface, 70));
        sm.set_anti_alias(true);
        canvas.draw_rrect(
            RRect::new_rect_xy(
                SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h),
                4.0,
                4.0,
            ),
            &sm,
        );
    }
}

fn draw_button(
    canvas: &Canvas,
    text: &str,
    color: Option<SkiaColor>,
    variant: ButtonVariant,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
    let hovered = rect.contains(mouse);
    let accent = color.unwrap_or(theme.primary);
    match variant {
        ButtonVariant::Primary => {
            let fill = if hovered {
                Theme::darken(accent, 0.15)
            } else {
                accent
            };
            let mut p = Paint::default();
            p.set_color(fill);
            p.set_anti_alias(true);
            canvas.draw_rrect(rrect(size, theme.radius_sm), &p);
            draw_text(
                canvas,
                text,
                (0.0, 0.0).into(),
                size,
                theme.on_primary,
                theme.font_body,
                font_cache,
                true,
            );
        }
        ButtonVariant::Ghost => {
            if hovered {
                let mut bg = Paint::default();
                bg.set_color(Theme::alpha(theme.on_surface, 20));
                bg.set_anti_alias(true);
                canvas.draw_rrect(rrect(size, theme.radius_sm), &bg);
            }
            let mut b = Paint::default();
            b.set_style(paint::Style::Stroke);
            b.set_stroke_width(1.0);
            b.set_color(if hovered {
                accent
            } else {
                Theme::alpha(theme.on_surface, 100)
            });
            b.set_anti_alias(true);
            canvas.draw_rrect(rrect(size, theme.radius_sm), &b);
            draw_text(
                canvas,
                text,
                (0.0, 0.0).into(),
                size,
                if hovered { accent } else { theme.on_surface },
                theme.font_body,
                font_cache,
                true,
            );
        }
        ButtonVariant::Text => {
            draw_text(
                canvas,
                text,
                (0.0, 0.0).into(),
                size,
                if hovered {
                    accent
                } else {
                    Theme::alpha(theme.on_surface, 180)
                },
                theme.font_body,
                font_cache,
                true,
            );
        }
    }
    if is_focused {
        draw_focus_outline_with_colors(
            canvas,
            SkiaRect::from_xywh(1.5, 1.5, (size.0 - 3.0).max(0.0), (size.1 - 3.0).max(0.0)),
            theme.radius_sm,
            accent,
            theme.surface,
        );
    }
}

fn draw_checkbox(
    canvas: &Canvas,
    checked: bool,
    label: &str,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let box_size = 18.0_f32;
    let box_rect = SkiaRect::from_xywh(0.0, (size.1 - box_size) / 2.0, box_size, box_size);
    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);
    let fill = if checked {
        theme.primary
    } else if hovered {
        Theme::alpha(theme.on_surface, 15)
    } else {
        SkiaColor::TRANSPARENT
    };
    if fill != SkiaColor::TRANSPARENT {
        let mut p = Paint::default();
        p.set_color(fill);
        p.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(box_rect, 3.0, 3.0), &p);
    }
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.5);
    border.set_color(if checked {
        theme.primary
    } else {
        Theme::alpha(theme.on_surface, 120)
    });
    border.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(box_rect, 3.0, 3.0), &border);
    if checked {
        let cx = box_rect.left + box_size / 2.0;
        let cy = box_rect.top + box_size / 2.0;
        let mut p = Paint::default();
        p.set_color(theme.on_primary);
        p.set_style(paint::Style::Stroke);
        p.set_stroke_width(2.0);
        p.set_anti_alias(true);
        p.set_stroke_cap(paint::Cap::Round);
        p.set_stroke_join(paint::Join::Round);
        canvas.draw_line((cx - 4.5, cy), (cx - 1.5, cy + 3.5), &p);
        canvas.draw_line((cx - 1.5, cy + 3.5), (cx + 4.5, cy - 3.5), &p);
    }
    if !label.is_empty() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(Theme::alpha(theme.on_surface, 220));
        p.set_anti_alias(true);
        canvas.draw_str(
            label,
            (box_size + 8.0, size.1 / 2.0 + theme.font_body / 3.0),
            &f,
            &p,
        );
    }
    if is_focused {
        let focus_w = if label.is_empty() {
            box_size
        } else {
            let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let mut p = Paint::default();
            p.set_anti_alias(true);
            box_size + 8.0 + f.measure_str(label, Some(&p)).0
        };
        let focus_h = box_size.max(theme.font_body);
        draw_focus_outline(
            canvas,
            SkiaRect::from_xywh(
                -4.0,
                (size.1 - focus_h) / 2.0 - 4.0,
                focus_w + 8.0,
                focus_h + 8.0,
            ),
            theme.radius_sm,
            theme,
        );
    }
}

fn draw_switch(
    canvas: &Canvas,
    checked: bool,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    theme: &Theme,
) {
    let track_w = 40.0_f32;
    let track_h = 22.0_f32;
    let thumb_r = 9.0_f32;
    let ty = (size.1 - track_h) / 2.0;
    let hov = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);
    let mut tp = Paint::default();
    tp.set_color(if checked {
        theme.primary
    } else if hov {
        Theme::alpha(theme.on_surface, 50)
    } else {
        Theme::alpha(theme.on_surface, 30)
    });
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(0.0, ty, track_w, track_h),
            track_h / 2.0,
            track_h / 2.0,
        ),
        &tp,
    );
    let thumb_x = if checked {
        track_w - thumb_r - 3.0
    } else {
        thumb_r + 3.0
    };
    let mut bp = Paint::default();
    bp.set_color(if checked {
        theme.on_primary
    } else {
        theme.on_surface
    });
    bp.set_anti_alias(true);
    canvas.draw_circle((thumb_x, ty + track_h / 2.0), thumb_r, &bp);
    if is_focused {
        draw_focus_outline_with_colors(
            canvas,
            SkiaRect::from_xywh(-3.0, ty - 3.0, track_w + 6.0, track_h + 6.0),
            track_h / 2.0,
            if checked {
                theme.primary
            } else {
                Theme::alpha(theme.on_surface, 150)
            },
            theme.surface,
        );
    }
}

fn draw_radio(
    canvas: &Canvas,
    selected: bool,
    label: &str,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let r = 9.0_f32;
    let cx = r;
    let cy = size.1 / 2.0;
    let hov = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);
    let mut bp = Paint::default();
    bp.set_style(paint::Style::Stroke);
    bp.set_stroke_width(2.0);
    bp.set_color(if selected {
        theme.primary
    } else {
        Theme::alpha(theme.on_surface, 120)
    });
    bp.set_anti_alias(true);
    canvas.draw_circle((cx, cy), r, &bp);
    if selected {
        let mut dp = Paint::default();
        dp.set_color(theme.primary);
        dp.set_anti_alias(true);
        canvas.draw_circle((cx, cy), r * 0.5, &dp);
    } else if hov {
        let mut hp = Paint::default();
        hp.set_color(Theme::alpha(theme.primary, 40));
        hp.set_anti_alias(true);
        canvas.draw_circle((cx, cy), r * 0.5, &hp);
    }
    if !label.is_empty() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(theme.on_surface);
        p.set_anti_alias(true);
        canvas.draw_str(label, (r * 2.0 + 8.0, cy + theme.font_body / 3.0), &f, &p);
    }
    if is_focused {
        let focus_w = if label.is_empty() {
            r * 2.0
        } else {
            let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let mut p = Paint::default();
            p.set_anti_alias(true);
            r * 2.0 + 8.0 + f.measure_str(label, Some(&p)).0
        };
        let focus_h = (r * 2.0).max(theme.font_body);
        draw_focus_outline(
            canvas,
            SkiaRect::from_xywh(
                -4.0,
                (size.1 - focus_h) / 2.0 - 4.0,
                focus_w + 8.0,
                focus_h + 8.0,
            ),
            theme.radius_sm,
            theme,
        );
    }
}

fn draw_slider(
    canvas: &Canvas,
    value: f32,
    min: f32,
    max: f32,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    is_dragging: bool,
    theme: &Theme,
) {
    let pad = 16.0_f32;
    let track_y = size.1 / 2.0;
    let track_h = 4.0_f32;
    let thumb_r = 8.0_f32;
    let norm = ((value - min) / (max - min).max(f32::EPSILON)).clamp(0.0, 1.0);
    let track_w = size.0 - pad * 2.0;
    let thumb_x = pad + norm * track_w;
    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);

    let mut tp = Paint::default();
    tp.set_color(Theme::alpha(theme.on_surface, 40));
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(
                thumb_x,
                track_y - track_h / 2.0,
                size.0 - pad - thumb_x,
                track_h,
            ),
            track_h / 2.0,
            track_h / 2.0,
        ),
        &tp,
    );
    let mut ap = Paint::default();
    ap.set_color(theme.primary);
    ap.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(pad, track_y - track_h / 2.0, thumb_x - pad, track_h),
            track_h / 2.0,
            track_h / 2.0,
        ),
        &ap,
    );
    let tr = if hovered || is_dragging {
        thumb_r + 2.0
    } else {
        thumb_r
    };
    let tc = if is_dragging {
        Theme::darken(theme.primary, 0.15)
    } else {
        theme.primary
    };
    let mut thp = Paint::default();
    thp.set_color(tc);
    thp.set_anti_alias(true);
    canvas.draw_circle((thumb_x, track_y), tr, &thp);
    if hovered || is_dragging {
        let mut hp = Paint::default();
        hp.set_color(Theme::alpha(theme.primary, 30));
        hp.set_anti_alias(true);
        canvas.draw_circle((thumb_x, track_y), tr + 4.0, &hp);
    }
    if is_focused {
        draw_focus_outline(
            canvas,
            SkiaRect::from_xywh(1.0, 1.0, (size.0 - 2.0).max(0.0), (size.1 - 2.0).max(0.0)),
            theme.radius_sm,
            theme,
        );
    }
}

fn draw_spinner(canvas: &Canvas, angle_deg: f32, size: (f32, f32), theme: &Theme) {
    let cx = size.0 / 2.0;
    let cy = size.1 / 2.0;
    let r = (size.0.min(size.1) / 2.0 - 3.0).max(4.0);
    let mut tp = Paint::default();
    tp.set_style(paint::Style::Stroke);
    tp.set_stroke_width(3.0);
    tp.set_color(Theme::alpha(theme.primary, 30));
    tp.set_anti_alias(true);
    canvas.draw_circle((cx, cy), r, &tp);
    let arc = SkiaRect::from_xywh(cx - r, cy - r, r * 2.0, r * 2.0);
    let mut ap = Paint::default();
    ap.set_style(paint::Style::Stroke);
    ap.set_stroke_width(3.0);
    ap.set_color(theme.primary);
    ap.set_anti_alias(true);
    ap.set_stroke_cap(paint::Cap::Round);
    canvas.draw_arc(arc, angle_deg - 90.0, 270.0, false, &ap);
}

fn draw_divider(canvas: &Canvas, orientation: Orientation, size: (f32, f32), theme: &Theme) {
    let mut p = Paint::default();
    p.set_color(Theme::alpha(theme.on_surface, 30));
    p.set_stroke_width(1.0);
    p.set_style(paint::Style::Stroke);
    match orientation {
        Orientation::Horizontal => {
            canvas.draw_line((0.0, size.1 / 2.0), (size.0, size.1 / 2.0), &p)
        }
        Orientation::Vertical => canvas.draw_line((size.0 / 2.0, 0.0), (size.0 / 2.0, size.1), &p),
    };
}

fn draw_image(canvas: &Canvas, data: &[u8], size: (f32, f32), radius: f32) {
    use skia_safe::{AlphaType, Bitmap, ColorType, ImageInfo, Matrix, images};
    let Ok(dyn_img) = decode_image_with_default_limits(data) else {
        return;
    };

    let rgba = dyn_img.to_rgba8();
    let (iw, ih) = (rgba.width() as i32, rgba.height() as i32);
    if iw <= 0 || ih <= 0 {
        return;
    }

    let raw = rgba.into_raw();
    let mut bmp = Bitmap::new();
    if !bmp.set_info(
        &ImageInfo::new((iw, ih), ColorType::RGBA8888, AlphaType::Premul, None),
        None,
    ) {
        return;
    }
    bmp.alloc_pixels();
    let pixels = bmp.pixels();
    if !pixels.is_null() {
        unsafe {
            std::ptr::copy_nonoverlapping(raw.as_ptr(), pixels as *mut u8, raw.len());
        }
    }
    let Some(sk_img) = images::raster_from_bitmap(&bmp) else {
        return;
    };
    if radius > 0.0 {
        canvas.save();
        canvas.clip_rrect(
            RRect::new_rect_xy(
                SkiaRect::from_xywh(0.0, 0.0, size.0, size.1),
                radius,
                radius,
            ),
            None,
            true,
        );
    }
    let m = Matrix::scale((size.0 / iw as f32, size.1 / ih as f32));
    canvas.save();
    canvas.concat(&m);
    canvas.draw_image(&sk_img, (0.0_f32, 0.0_f32), Some(&Paint::default()));
    canvas.restore();
    if radius > 0.0 {
        canvas.restore();
    }
}

const MAX_IMAGE_DECODE_WIDTH: u32 = 8192;
const MAX_IMAGE_DECODE_HEIGHT: u32 = 8192;
const MAX_IMAGE_DECODE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;

fn decode_image_with_default_limits(data: &[u8]) -> image::ImageResult<image::DynamicImage> {
    decode_image_with_limits(
        data,
        MAX_IMAGE_DECODE_WIDTH,
        MAX_IMAGE_DECODE_HEIGHT,
        MAX_IMAGE_DECODE_ALLOC_BYTES,
    )
}

fn decode_image_with_limits(
    data: &[u8],
    max_width: u32,
    max_height: u32,
    max_alloc_bytes: u64,
) -> image::ImageResult<image::DynamicImage> {
    use image::{ImageReader, Limits};
    use std::io::Cursor;

    let mut reader = ImageReader::new(Cursor::new(data));
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_width);
    limits.max_image_height = Some(max_height);
    limits.max_alloc = Some(max_alloc_bytes);
    reader.limits(limits);

    let image = reader.with_guessed_format()?.decode()?;
    let rgba_bytes = u64::from(image.width())
        .saturating_mul(u64::from(image.height()))
        .saturating_mul(4);

    if rgba_bytes > max_alloc_bytes {
        return Err(image::ImageError::Limits(
            image::error::LimitError::from_kind(image::error::LimitErrorKind::InsufficientMemory),
        ));
    }

    Ok(image)
}

fn draw_select(
    canvas: &Canvas,
    options: &[&str],
    selected_index: usize,
    is_open: bool,
    hovered_option: Option<usize>,
    label: &str,
    placeholder: &str,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let closed_h = size.1
        - if is_open {
            options.len() as f32 * OPTION_HEIGHT
        } else {
            0.0
        };
    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, closed_h).contains(mouse);
    let mut bg = Paint::default();
    bg.set_color(if hovered {
        Theme::alpha(theme.on_surface, 10)
    } else {
        theme.surface
    });
    bg.set_anti_alias(true);
    canvas.draw_rrect(rrect((size.0, closed_h), theme.radius_sm), &bg);
    let mut brd = Paint::default();
    brd.set_style(paint::Style::Stroke);
    brd.set_stroke_width(1.0);
    brd.set_color(if is_open || is_focused {
        theme.primary
    } else {
        Theme::alpha(theme.on_surface, 80)
    });
    brd.set_anti_alias(true);
    canvas.draw_rrect(rrect((size.0, closed_h), theme.radius_sm), &brd);
    if !label.is_empty() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_label);
        let mut p = Paint::default();
        p.set_color(if is_open || is_focused {
            theme.primary
        } else {
            Theme::alpha(theme.on_surface, 160)
        });
        p.set_anti_alias(true);
        canvas.draw_str(label, (6.0, -4.0), &f, &p);
    }
    let display = options.get(selected_index).copied().unwrap_or(placeholder);
    let tc = if display == placeholder {
        Theme::alpha(theme.on_surface, 100)
    } else {
        theme.on_surface
    };
    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut tp = Paint::default();
    tp.set_color(tc);
    tp.set_anti_alias(true);
    canvas.draw_str(
        display,
        (8.0, closed_h / 2.0 + theme.font_body / 3.0),
        &f,
        &tp,
    );
    let chevron = if is_open { "▲" } else { "▼" };
    let cf = get_cached_font(font_cache, "sans-serif", 11.0);
    let mut cp = Paint::default();
    cp.set_color(Theme::alpha(theme.on_surface, 160));
    cp.set_anti_alias(true);
    let cw = cf.measure_str(chevron, Some(&cp)).0;
    canvas.draw_str(
        chevron,
        (size.0 - cw - 8.0, closed_h / 2.0 + theme.font_body / 3.0),
        &cf,
        &cp,
    );
    if is_open {
        let dd = SkiaRect::from_xywh(0.0, closed_h, size.0, size.1 - closed_h);
        let mut dbp = Paint::default();
        dbp.set_color(theme.surface);
        dbp.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(dd, 0.0, theme.radius_sm), &dbp);
        let mut dbrd = Paint::default();
        dbrd.set_style(paint::Style::Stroke);
        dbrd.set_stroke_width(1.0);
        dbrd.set_color(Theme::alpha(theme.on_surface, 80));
        dbrd.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(dd, 0.0, theme.radius_sm), &dbrd);
        for (i, opt) in options.iter().enumerate() {
            let oy = closed_h + i as f32 * OPTION_HEIGHT;
            if i == selected_index || hovered_option == Some(i) {
                let mut ip = Paint::default();
                ip.set_color(if i == selected_index {
                    Theme::alpha(theme.primary, 30)
                } else {
                    Theme::alpha(theme.on_surface, 10)
                });
                ip.set_anti_alias(true);
                canvas.draw_rect(
                    SkiaRect::from_xywh(1.0, oy, size.0 - 2.0, OPTION_HEIGHT),
                    &ip,
                );
            }
            let ot = if i == selected_index {
                theme.primary
            } else {
                theme.on_surface
            };
            let mut op = Paint::default();
            op.set_color(ot);
            op.set_anti_alias(true);
            canvas.draw_str(
                opt,
                (8.0, oy + OPTION_HEIGHT / 2.0 + theme.font_body / 3.0),
                &f,
                &op,
            );
        }
    }
    if is_focused {
        draw_focus_outline(
            canvas,
            SkiaRect::from_xywh(1.0, 1.0, (size.0 - 2.0).max(0.0), (closed_h - 2.0).max(0.0)),
            theme.radius_sm,
            theme,
        );
    }
}

fn draw_tooltip_popup(
    canvas: &Canvas,
    text: &str,
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let f = get_cached_font(font_cache, "sans-serif", theme.font_small);
    let mut tp = Paint::default();
    tp.set_color(theme.on_primary);
    tp.set_anti_alias(true);
    let tw = f.measure_str(text, Some(&tp)).0;
    let pad = 6.0_f32;
    let tt_w = tw + pad * 2.0;
    let tt_h = theme.font_small + pad * 2.0;
    let mut bg = Paint::default();
    bg.set_color(Theme::alpha(theme.on_surface, 220));
    bg.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(mouse.x + 12.0, mouse.y - tt_h - 4.0, tt_w, tt_h),
            3.0,
            3.0,
        ),
        &bg,
    );
    canvas.draw_str(text, (mouse.x + 12.0 + pad, mouse.y - 4.0 - pad), &f, &tp);
}

fn draw_scrollbar(
    canvas: &Canvas,
    size: (f32, f32),
    state: Option<&crate::engine::widget_state::ScrollState>,
    theme: &Theme,
) {
    let Some(s) = state else {
        return;
    };
    if s.content_height <= s.viewport_h {
        return;
    }
    let ratio = s.thumb_ratio();
    let thumb_h = (size.1 * ratio).max(20.0);
    let sb_x = size.0 - SCROLLBAR_W - 2.0;
    let mut tp = Paint::default();
    tp.set_color(Theme::alpha(theme.on_surface, 20));
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(sb_x, 0.0, SCROLLBAR_W, size.1),
            SCROLLBAR_W / 2.0,
            SCROLLBAR_W / 2.0,
        ),
        &tp,
    );
    let mut sp = Paint::default();
    sp.set_color(Theme::alpha(theme.on_surface, 80));
    sp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(sb_x, s.thumb_y(), SCROLLBAR_W, thumb_h),
            SCROLLBAR_W / 2.0,
            SCROLLBAR_W / 2.0,
        ),
        &sp,
    );
}

fn rrect(size: (f32, f32), r: f32) -> RRect {
    RRect::new_rect_xy(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), r, r)
}

#[cfg(test)]
mod tests {
    use super::{decode_image_with_default_limits, decode_image_with_limits};

    fn tiny_png() -> Vec<u8> {
        use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;

        let image = RgbaImage::from_pixel(1, 1, Rgba([0x12, 0x34, 0x56, 0xff]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn decode_image_accepts_small_png_with_default_limits() {
        let image = decode_image_with_default_limits(&tiny_png()).unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
    }

    #[test]
    fn decode_image_rejects_small_png_when_alloc_budget_is_too_low() {
        let result = decode_image_with_limits(&tiny_png(), 16, 16, 1);
        assert!(result.is_err());
    }
}
