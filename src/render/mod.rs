// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — render/mod.rs
// ============================================================

pub mod hit_test;
pub mod image;
mod image_cache;
mod image_headers;
pub mod pipeline;
pub(crate) mod rich_text;
mod svg;
pub mod text;
mod text_cache;

use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    rc::Rc,
    time::Instant,
};

use cosmic_text::{Cursor, Edit, FontSystem, LayoutRun, SwashCache, Wrap};
use skia_safe::{
    Color as SkiaColor, Contains, Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas,
    paint,
};
use taffy::Direction;
use taffy::prelude::{NodeId, TaffyTree};

pub use self::image_cache::ImageRenderCache;
use self::rich_text::RichTextDirection;
pub use self::rich_text::RichTextRenderer;
use self::text::{TextBufferCache, TextShapeRequest, draw_text, get_cached_font};
use self::{
    image::{MAX_ENCODED_IMAGE_BYTES, decode_rutter_image},
    image_cache::SvgImageCacheKey,
    svg::{checked_svg_raster_size, validate_svg_source},
};
use crate::carousel::geometry::{CarouselItemFrame, carousel_item_frames};
use crate::engine::widget_state::{
    VirtualGridState, WidgetState, normalize_virtual_grid_columns, virtual_grid_cell_left,
    virtual_grid_cell_width, virtual_grid_row_count,
};
use crate::i18n::LayoutDirection;
use crate::input_state::{InputWidgetState, cursor_x_in_run};
use crate::layout::{
    OPTION_HEIGHT, RutterContext, SCROLLBAR_W, VIRTUAL_GRID_GAP, build_taffy_tree_with_direction,
    compute_layout,
};
use crate::render::hit_test::{context_menu_rect, dialog_card_rect, modal_card_rect, popover_rect};
use crate::rich_text::OwnedRichTextSpec;
use crate::theme::Theme;
use crate::widget::{
    ButtonVariant, CONTEXT_MENU_ITEM_H, CONTEXT_MENU_PAD_Y, CONTEXT_MENU_SEPARATOR_H,
    ContextMenuEntry, DialogAction, DialogPosition, InputState, Orientation, ToastKind,
    ToastPosition, Widget,
};
use winit::dpi::PhysicalSize;

const ACCORDION_HEADER_H: f32 = 44.0;

fn stable_bytes_hash(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

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
    let mut image_cache = ImageRenderCache::default();
    draw_widgets_with_cache(
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
        &mut image_cache,
        cursor_visible,
        theme,
        scale,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn draw_widgets_with_cache<'w, Msg>(
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
    image_cache: &mut ImageRenderCache,
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
) {
    let mut path = Vec::new();
    let layout_fs = image_cache.layout_font_system();
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
        image_cache,
        layout_fs.clone(),
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
        image_cache,
        layout_fs.clone(),
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
    text_cache.clear_transient_buffer();
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
    image_cache: &mut ImageRenderCache,
    layout_fs: Rc<RefCell<FontSystem>>,
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
                    image_cache,
                    layout_fs.clone(),
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
                image_cache,
                layout_fs.clone(),
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
                image_cache,
                layout_fs.clone(),
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
                        image_cache,
                        layout_fs.clone(),
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
                    image_cache,
                    layout_fs.clone(),
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
                    image_cache,
                    layout_fs.clone(),
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
                    image_cache,
                    layout_fs.clone(),
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
    image_cache: &mut ImageRenderCache,
    layout_fs: Rc<RefCell<FontSystem>>,
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
                    image_cache,
                    layout_fs.clone(),
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
            let clips_child = begin_rounded_container_clip(canvas, size, *radius);
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
                image_cache,
                layout_fs.clone(),
                cursor_visible,
                theme,
                scale,
                path,
            );
            path.pop();
            if clips_child {
                canvas.restore();
            }
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
                image_cache,
                layout_fs.clone(),
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
                image_cache,
                layout_fs.clone(),
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
                    image_cache,
                    layout_fs.clone(),
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
                    image_cache,
                    layout_fs.clone(),
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
        } => draw_text_button(
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
        Widget::ButtonContent {
            child,
            color,
            variant,
            ..
        } => {
            draw_button_frame(
                canvas,
                *color,
                *variant,
                is_focused,
                size,
                local_mouse,
                theme,
            );
            let ids = taffy.children(node).unwrap();
            if let Some(child_node) = ids.first().copied() {
                path.push(0);
                draw_widgets_impl(
                    canvas,
                    taffy,
                    child_node,
                    child,
                    fs,
                    swash,
                    local_mouse,
                    focused_id,
                    input_states,
                    widget_states,
                    font_cache,
                    text_cache,
                    image_cache,
                    layout_fs.clone(),
                    cursor_visible,
                    theme,
                    scale,
                    path,
                );
                path.pop();
            }
        }
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
        Widget::Image { data, radius, .. } => {
            draw_image(canvas, data, size, *radius, scale, image_cache)
        }
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
        Widget::RichText { .. } => {
            if let Some(RutterContext::RichText(content)) = taffy.get_node_context(node) {
                let direction = rich_text_node_direction(taffy, node);
                draw_rich_text_content(
                    canvas,
                    layout,
                    content,
                    theme.on_surface,
                    direction,
                    image_cache.rich_text_renderer(),
                );
            }
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
                    image_cache,
                    layout_fs.clone(),
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
                image_cache,
                layout_fs.clone(),
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
        Widget::CarouselView {
            item_count,
            items,
            config,
            ..
        } => {
            let state = widget_states
                .get(&resolved_id.unwrap())
                .and_then(WidgetState::as_carousel);
            let direction = node_layout_direction(taffy, node);
            let position = state.map(|state| state.position).unwrap_or_default();
            let selected = state.and_then(|state| state.selected_item);
            let frames = carousel_item_frames(config, position, size.0, *item_count, direction);
            draw_carousel_view(
                canvas,
                items.as_ref(),
                &frames,
                CarouselViewPaintState(selected, size, local_mouse, is_focused, theme),
                &mut VirtualItemDrawContext {
                    fs,
                    swash,
                    font_cache,
                    text_cache,
                    image_cache,
                    layout_fs: layout_fs.clone(),
                    layout_direction: direction,
                    cursor_visible,
                    scale,
                },
                path,
            );
        }
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
        Widget::VirtualListContent {
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
            draw_virtual_list_content(
                canvas,
                item_height,
                item_count,
                items,
                scroll_y,
                selected,
                hovered,
                size,
                local_mouse,
                theme,
                &mut VirtualItemDrawContext {
                    fs,
                    swash,
                    font_cache,
                    text_cache,
                    image_cache,
                    layout_fs: layout_fs.clone(),
                    layout_direction: node_layout_direction(taffy, node),
                    cursor_visible,
                    scale,
                },
                path,
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
                gstate,
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
        Widget::VirtualGridContent {
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
            draw_virtual_grid_content(
                canvas,
                columns,
                item_height,
                item_count,
                items,
                gstate,
                scroll_y,
                selected,
                hovered,
                size,
                local_mouse,
                is_focused,
                theme,
                &mut VirtualItemDrawContext {
                    fs,
                    swash,
                    font_cache,
                    text_cache,
                    image_cache,
                    layout_fs: layout_fs.clone(),
                    layout_direction: node_layout_direction(taffy, node),
                    cursor_visible,
                    scale,
                },
                path,
            );
        }
    }

    canvas.restore();
}

fn begin_rounded_container_clip(canvas: &Canvas, size: (f32, f32), radius: f32) -> bool {
    if !radius.is_finite() || radius <= 0.0 {
        return false;
    }
    canvas.save();
    canvas.clip_rrect(rrect(size, radius), None, true);
    true
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
        Cursor::new(
            cursor.line,
            s.password_display_index_for_line(cursor.line, cursor.index),
        )
    } else {
        cursor
    };

    let display = s.display_text(is_password);

    text_cache.with_shaped(
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
        |buffer, font_system| {
            let runs = buffer.layout_runs().collect::<Vec<_>>();
            draw_text_input_runs(
                canvas,
                font_system,
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
                s,
                is_password,
                runs,
            );
        },
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
    selection_state: &InputWidgetState,
    is_password: bool,
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

    if let Some(selection) = selection_state
        .selection
        .filter(|selection| !selection.is_empty())
    {
        for run in runs.iter() {
            let Some(selection) =
                selection_state.selection_in_display_line(selection, run.line_i, is_password)
            else {
                continue;
            };
            let (a, b) = selection.normalized();
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
    image_cache: &mut ImageRenderCache,
    layout_fs: Rc<RefCell<FontSystem>>,
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

    let ids = taffy.children(node).unwrap();
    let child_layout = taffy.layout(ids[0]).unwrap();
    let card = modal_card_rect(child_layout.size.height, size);
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
        image_cache,
        layout_fs.clone(),
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

    text_cache.with_shaped(
        fs,
        TextShapeRequest::new(message, 14.0, 20.0)
            .with_bounds(Some(card_w - 48.0), None)
            .with_wrap(Wrap::WordOrGlyph),
        |buffer, font_system| {
            crate::render::pipeline::render_text_runs(
                canvas,
                buffer.layout_runs(),
                Point::new(card_x + 24.0, card_y + 60.0),
                Theme::alpha(theme.on_surface, 180),
                font_system,
                swash,
                scale,
            );
        },
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
        let (ratio, thumb_y) = fallback_scrollbar_metrics(scroll_y, total_h, size.1);
        draw_virtual_scrollbar(canvas, size, ratio, thumb_y, theme);
    }
}

struct VirtualItemDrawContext<'a> {
    fs: &'a mut FontSystem,
    swash: &'a mut SwashCache,
    font_cache: &'a mut HashMap<(String, u32), Font>,
    text_cache: &'a mut TextBufferCache,
    image_cache: &'a mut ImageRenderCache,
    layout_fs: Rc<RefCell<FontSystem>>,
    layout_direction: LayoutDirection,
    cursor_visible: bool,
    scale: f32,
}

struct CarouselViewPaintState<'a>(Option<usize>, (f32, f32), Point, bool, &'a Theme);

fn draw_carousel_view<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    frames: &[CarouselItemFrame],
    paint_state: CarouselViewPaintState<'_>,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let CarouselViewPaintState(selected, size, mouse, is_focused, theme) = paint_state;
    let visible_selection = visible_carousel_selection(selected, frames, size);
    draw_virtual_background(canvas, size, theme);
    canvas.save();
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
    draw_carousel_items(
        canvas, items, frames, selected, mouse, is_focused, size.1, theme, ctx, path,
    );
    canvas.restore();
    draw_unselected_carousel_focus(canvas, visible_selection, is_focused, size, theme);
}

fn visible_carousel_selection(
    selected: Option<usize>,
    frames: &[CarouselItemFrame],
    viewport_size: (f32, f32),
) -> Option<usize> {
    selected.filter(|selected| {
        frames
            .iter()
            .find(|frame| frame.index == *selected)
            .is_some_and(|frame| carousel_focus_intersects_viewport(*frame, viewport_size))
    })
}

fn carousel_focus_intersects_viewport(frame: CarouselItemFrame, viewport_size: (f32, f32)) -> bool {
    let focus = inset_rect(carousel_card_rect(frame, viewport_size.1), 1.0);
    focus.left < viewport_size.0
        && focus.right > 0.0
        && focus.top < viewport_size.1
        && focus.bottom > 0.0
}

#[allow(clippy::too_many_arguments)]
fn draw_carousel_items<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    frames: &[CarouselItemFrame],
    selected: Option<usize>,
    mouse: Point,
    is_focused: bool,
    height: f32,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    for frame in frames.iter().copied() {
        draw_carousel_item(
            canvas, items, frame, selected, mouse, is_focused, height, theme, ctx, path,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_carousel_item<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    frame: CarouselItemFrame,
    selected: Option<usize>,
    mouse: Point,
    is_focused: bool,
    viewport_height: f32,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let rect = carousel_card_rect(frame, viewport_height);
    let is_selected = selected == Some(frame.index);
    draw_virtual_grid_cell_frame(canvas, rect, is_selected, rect.contains(mouse), theme);
    draw_carousel_item_focus(canvas, rect, is_selected && is_focused, theme);
    draw_carousel_item_content(canvas, items, frame.index, rect, mouse, theme, ctx, path);
}

#[allow(clippy::too_many_arguments)]
fn draw_carousel_item_content<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    index: usize,
    rect: SkiaRect,
    mouse: Point,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    if let Some(item) = items(index) {
        let origin = (rect.left, rect.top);
        let item_size = rect_size(rect);
        path.push(index);
        draw_virtual_item_widget(canvas, &item, origin, item_size, mouse, theme, ctx, path);
        path.pop();
    }
}

fn node_layout_direction(taffy: &TaffyTree<RutterContext>, node: NodeId) -> LayoutDirection {
    match taffy.style(node).map(|style| style.direction) {
        Ok(Direction::Rtl) => LayoutDirection::Rtl,
        _ => LayoutDirection::Ltr,
    }
}

fn rich_text_node_direction(taffy: &TaffyTree<RutterContext>, node: NodeId) -> RichTextDirection {
    match taffy.style(node).map(|style| style.direction) {
        Ok(Direction::Rtl) => RichTextDirection::RightToLeft,
        Ok(Direction::Ltr) | Err(_) => RichTextDirection::LeftToRight,
    }
}

fn draw_rich_text_content(
    canvas: &Canvas,
    layout: &taffy::Layout,
    content: &OwnedRichTextSpec,
    fallback_color: SkiaColor,
    direction: RichTextDirection,
    renderer: &RichTextRenderer,
) {
    let size = (layout.content_box_width(), layout.content_box_height());
    let origin = (
        layout.border.left + layout.padding.left,
        layout.border.top + layout.padding.top,
    );
    canvas.save();
    canvas.translate(origin);
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
    renderer.draw(canvas, content, size, fallback_color, direction);
    canvas.restore();
}

fn carousel_card_rect(frame: CarouselItemFrame, viewport_height: f32) -> SkiaRect {
    const GAP: f32 = 8.0;
    let horizontal = (GAP * 0.5).min(frame.width * 0.2);
    let vertical = (GAP * 0.5).min(viewport_height * 0.2);
    SkiaRect::from_xywh(
        frame.x + horizontal,
        vertical,
        (frame.width - horizontal * 2.0).max(1.0),
        (viewport_height - vertical * 2.0).max(1.0),
    )
}

fn draw_carousel_item_focus(canvas: &Canvas, rect: SkiaRect, focused: bool, theme: &Theme) {
    if !focused {
        return;
    }
    draw_focus_outline(canvas, inset_rect(rect, 1.0), theme.radius_sm, theme);
}

fn draw_unselected_carousel_focus(
    canvas: &Canvas,
    selected: Option<usize>,
    focused: bool,
    size: (f32, f32),
    theme: &Theme,
) {
    if !focused || selected.is_some() {
        return;
    }
    let rect = SkiaRect::from_xywh(2.0, 2.0, (size.0 - 4.0).max(0.0), (size.1 - 4.0).max(0.0));
    draw_focus_outline(canvas, rect, theme.radius_sm, theme);
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_list_content<'w, Msg>(
    canvas: &Canvas,
    item_height: &f32,
    item_count: &usize,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    scroll_y: f32,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let ih = *item_height;
    let count = *item_count;
    draw_virtual_background(canvas, size, theme);
    canvas.save();
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
    for i in visible_virtual_rows(scroll_y, ih, size.1, count) {
        draw_virtual_list_content_row(
            canvas, items, i, ih, scroll_y, selected, hovered, size, mouse, theme, ctx, path,
        );
    }
    canvas.restore();
    draw_list_scrollbar_if_needed(canvas, scroll_y, ih * count as f32, size, theme);
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_list_content_row<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    index: usize,
    item_height: f32,
    scroll_y: f32,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let y = index as f32 * item_height - scroll_y;
    let rect = SkiaRect::from_xywh(0.0, y, size.0 - SCROLLBAR_W - 4.0, item_height);
    let is_hovered =
        hovered == Some(index) || SkiaRect::from_xywh(0.0, y, size.0, item_height).contains(mouse);
    draw_virtual_item_highlight(canvas, rect, selected == Some(index), is_hovered, theme);
    if let Some(item) = items(index) {
        path.push(index);
        draw_virtual_item_widget(
            canvas,
            &item,
            (0.0, y),
            rect_size(rect),
            mouse,
            theme,
            ctx,
            path,
        );
        path.pop();
    }
    draw_virtual_row_separator(canvas, y, item_height, size.0, theme);
}

fn visible_virtual_rows(
    scroll_y: f32,
    item_height: f32,
    viewport_h: f32,
    count: usize,
) -> std::ops::Range<usize> {
    let first = (scroll_y / item_height).floor() as usize;
    let visible = (viewport_h / item_height).ceil() as usize + 1;
    first..(first + visible).min(count)
}

fn draw_list_scrollbar_if_needed(
    canvas: &Canvas,
    scroll_y: f32,
    total_h: f32,
    size: (f32, f32),
    theme: &Theme,
) {
    if total_h > size.1 {
        let (ratio, thumb_y) = fallback_scrollbar_metrics(scroll_y, total_h, size.1);
        draw_virtual_scrollbar(canvas, size, ratio, thumb_y, theme);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_grid(
    canvas: &Canvas,
    columns: &usize,
    item_height: &f32,
    item_count: &usize,
    items: &dyn Fn(usize) -> Option<String>,
    state: Option<&VirtualGridState>,
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

    if let Some((ratio, thumb_y)) =
        virtual_grid_scrollbar_metrics(state, scroll_y, row_h, count, columns, row_count, size.1)
    {
        draw_virtual_scrollbar(canvas, size, ratio, thumb_y, theme);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_grid_content<'w, Msg>(
    canvas: &Canvas,
    columns: &usize,
    item_height: &f32,
    item_count: &usize,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    state: Option<&VirtualGridState>,
    scroll_y: f32,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    is_focused: bool,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let columns = normalize_virtual_grid_columns(*columns);
    let row_h = *item_height;
    let count = *item_count;
    let row_count = virtual_grid_row_count(count, columns);
    draw_virtual_background(canvas, size, theme);
    canvas.save();
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
    for row in visible_virtual_rows(scroll_y, row_h, size.1, row_count) {
        draw_virtual_grid_content_row(
            canvas, items, row, columns, row_h, count, scroll_y, selected, hovered, size, mouse,
            is_focused, theme, ctx, path,
        );
    }
    canvas.restore();
    if let Some((ratio, thumb_y)) =
        virtual_grid_scrollbar_metrics(state, scroll_y, row_h, count, columns, row_count, size.1)
    {
        draw_virtual_scrollbar(canvas, size, ratio, thumb_y, theme);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_grid_content_row<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    row: usize,
    columns: usize,
    row_h: f32,
    count: usize,
    scroll_y: f32,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    is_focused: bool,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let y = row as f32 * row_h - scroll_y;
    for col in 0..columns {
        let index = row * columns + col;
        if index >= count {
            break;
        }
        draw_virtual_grid_content_cell(
            canvas, items, index, col, y, row_h, columns, selected, hovered, size, mouse,
            is_focused, theme, ctx, path,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_grid_content_cell<'w, Msg>(
    canvas: &Canvas,
    items: &dyn Fn(usize) -> Option<Widget<'w, Msg>>,
    index: usize,
    col: usize,
    y: f32,
    row_h: f32,
    columns: usize,
    selected: Option<usize>,
    hovered: Option<usize>,
    size: (f32, f32),
    mouse: Point,
    is_focused: bool,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let rect = virtual_grid_cell_rect(col, y, row_h, size.0, columns);
    let is_hovered = hovered == Some(index) || rect.contains(mouse);
    draw_virtual_grid_cell_frame(canvas, rect, selected == Some(index), is_hovered, theme);
    if is_focused && selected == Some(index) {
        draw_focus_outline(canvas, inset_rect(rect, 1.0), theme.radius_sm, theme);
    }
    if let Some(item) = items(index) {
        path.push(index);
        draw_virtual_item_widget(
            canvas,
            &item,
            (rect.left, rect.top),
            rect_size(rect),
            mouse,
            theme,
            ctx,
            path,
        );
        path.pop();
    }
}

fn virtual_grid_scrollbar_metrics(
    state: Option<&VirtualGridState>,
    scroll_y: f32,
    item_height: f32,
    item_count: usize,
    columns: usize,
    row_count: usize,
    viewport_h: f32,
) -> Option<(f32, f32)> {
    let total_h = item_height * row_count as f32;
    if total_h <= viewport_h {
        return None;
    }
    Some(match state.filter(|s| s.viewport_h > 0.0) {
        Some(s) => (
            s.thumb_ratio(item_height, item_count, columns),
            s.thumb_y(item_height, item_count, columns),
        ),
        None => fallback_scrollbar_metrics(scroll_y, total_h, viewport_h),
    })
}

fn fallback_scrollbar_metrics(scroll_y: f32, total_h: f32, viewport_h: f32) -> (f32, f32) {
    let max_scroll = (total_h - viewport_h).max(1.0);
    let ratio = (viewport_h / total_h).clamp(0.0, 1.0);
    let thumb_h = (viewport_h * ratio).max(20.0);
    (ratio, (scroll_y / max_scroll) * (viewport_h - thumb_h))
}

#[allow(clippy::too_many_arguments)]
fn draw_virtual_item_widget<'w, Msg>(
    canvas: &Canvas,
    item: &Widget<'w, Msg>,
    origin: (f32, f32),
    size: (f32, f32),
    mouse: Point,
    theme: &Theme,
    ctx: &mut VirtualItemDrawContext<'_>,
    path: &mut Vec<usize>,
) {
    let mut taffy = TaffyTree::new();
    let fs_rc = ctx.layout_fs.clone();
    // Virtual item widgets are visual-only, so global state must not alias IDs materialized on demand.
    let isolated_input_states = HashMap::new();
    let isolated_widget_states = HashMap::new();
    let root = build_taffy_tree_with_direction(
        &mut taffy,
        item,
        fs_rc.clone(),
        &isolated_widget_states,
        ctx.layout_direction,
    );
    compute_layout(
        &mut taffy,
        root,
        physical_size(size),
        fs_rc,
        ctx.image_cache.rich_text_renderer(),
    );
    canvas.save();
    canvas.translate(origin);
    canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
    draw_widgets_impl(
        canvas,
        &taffy,
        root,
        item,
        ctx.fs,
        ctx.swash,
        Point::new(mouse.x - origin.0, mouse.y - origin.1),
        None,
        &isolated_input_states,
        &isolated_widget_states,
        ctx.font_cache,
        ctx.text_cache,
        ctx.image_cache,
        ctx.layout_fs.clone(),
        ctx.cursor_visible,
        theme,
        ctx.scale,
        path,
    );
    canvas.restore();
}

fn physical_size(size: (f32, f32)) -> PhysicalSize<u32> {
    PhysicalSize::new(size.0.max(1.0).ceil() as u32, size.1.max(1.0).ceil() as u32)
}

fn draw_virtual_background(canvas: &Canvas, size: (f32, f32), theme: &Theme) {
    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), &bg);
}

fn draw_virtual_item_highlight(
    canvas: &Canvas,
    rect: SkiaRect,
    selected: bool,
    hovered: bool,
    theme: &Theme,
) {
    if !selected && !hovered {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(virtual_item_fill(selected, theme));
    paint.set_anti_alias(true);
    canvas.draw_rect(rect, &paint);
}

fn virtual_item_fill(selected: bool, theme: &Theme) -> SkiaColor {
    if selected {
        Theme::alpha(theme.primary, 40)
    } else {
        Theme::alpha(theme.on_surface, 12)
    }
}

fn draw_virtual_row_separator(
    canvas: &Canvas,
    y: f32,
    item_height: f32,
    width: f32,
    theme: &Theme,
) {
    let mut sep = Paint::default();
    sep.set_color(Theme::alpha(theme.on_surface, 15));
    sep.set_style(paint::Style::Stroke);
    sep.set_stroke_width(0.5);
    canvas.draw_line(
        (0.0, y + item_height - 0.5),
        (width, y + item_height - 0.5),
        &sep,
    );
}

fn virtual_grid_cell_rect(col: usize, y: f32, row_h: f32, width: f32, columns: usize) -> SkiaRect {
    let cell_h = (row_h - VIRTUAL_GRID_GAP).max(12.0);
    SkiaRect::from_xywh(
        virtual_grid_cell_left(col, width, columns),
        y + VIRTUAL_GRID_GAP * 0.5,
        virtual_grid_cell_width(width, columns),
        cell_h,
    )
}

fn draw_virtual_grid_cell_frame(
    canvas: &Canvas,
    rect: SkiaRect,
    selected: bool,
    hovered: bool,
    theme: &Theme,
) {
    let mut cell_bg = Paint::default();
    cell_bg.set_color(virtual_grid_cell_fill(selected, hovered, theme));
    cell_bg.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
        &cell_bg,
    );
    draw_virtual_grid_cell_border(canvas, rect, selected, theme);
}

fn draw_virtual_grid_cell_border(canvas: &Canvas, rect: SkiaRect, selected: bool, theme: &Theme) {
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(if selected {
        Theme::alpha(theme.primary, 130)
    } else {
        Theme::alpha(theme.on_surface, 22)
    });
    border.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(rect, theme.radius_sm, theme.radius_sm),
        &border,
    );
}

fn virtual_grid_cell_fill(selected: bool, hovered: bool, theme: &Theme) -> SkiaColor {
    if selected {
        Theme::alpha(theme.primary, 36)
    } else if hovered {
        Theme::alpha(theme.on_surface, 12)
    } else {
        Theme::alpha(theme.on_surface, 6)
    }
}

fn inset_rect(rect: SkiaRect, inset: f32) -> SkiaRect {
    SkiaRect::from_xywh(
        rect.left + inset,
        rect.top + inset,
        (rect.width() - inset * 2.0).max(0.0),
        (rect.height() - inset * 2.0).max(0.0),
    )
}

fn rect_size(rect: SkiaRect) -> (f32, f32) {
    (rect.width(), rect.height())
}

fn draw_virtual_scrollbar(
    canvas: &Canvas,
    size: (f32, f32),
    thumb_ratio: f32,
    thumb_y: f32,
    theme: &Theme,
) {
    let thumb_h = (size.1 * thumb_ratio).max(20.0);
    let sb_x = size.0 - SCROLLBAR_W - 2.0;
    draw_virtual_scrollbar_track(canvas, sb_x, size.1, theme);
    draw_virtual_scrollbar_thumb(canvas, sb_x, thumb_y, thumb_h, theme);
}

fn draw_virtual_scrollbar_track(canvas: &Canvas, sb_x: f32, height: f32, theme: &Theme) {
    let mut track = Paint::default();
    track.set_color(Theme::alpha(theme.on_surface, 20));
    track.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(sb_x, 0.0, SCROLLBAR_W, height),
            4.0,
            4.0,
        ),
        &track,
    );
}

fn draw_virtual_scrollbar_thumb(
    canvas: &Canvas,
    sb_x: f32,
    thumb_y: f32,
    thumb_h: f32,
    theme: &Theme,
) {
    let mut thumb = Paint::default();
    thumb.set_color(Theme::alpha(theme.on_surface, 70));
    thumb.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h),
            4.0,
            4.0,
        ),
        &thumb,
    );
}

fn draw_text_button(
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
    let hovered = button_hovered(size, mouse);
    let text_color = button_text_color(color, variant, hovered, theme);
    draw_button_frame(canvas, color, variant, is_focused, size, mouse, theme);
    draw_text(
        canvas,
        text,
        (0.0, 0.0).into(),
        size,
        text_color,
        theme.font_body,
        font_cache,
        true,
    );
}

fn draw_button_frame(
    canvas: &Canvas,
    color: Option<SkiaColor>,
    variant: ButtonVariant,
    is_focused: bool,
    size: (f32, f32),
    mouse: Point,
    theme: &Theme,
) {
    let hovered = button_hovered(size, mouse);
    let accent = color.unwrap_or(theme.primary);
    draw_button_surface(canvas, variant, hovered, accent, size, theme);
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

fn draw_button_surface(
    canvas: &Canvas,
    variant: ButtonVariant,
    hovered: bool,
    accent: SkiaColor,
    size: (f32, f32),
    theme: &Theme,
) {
    match variant {
        ButtonVariant::Primary => draw_primary_button_surface(canvas, hovered, accent, size, theme),
        ButtonVariant::Ghost => draw_ghost_button_surface(canvas, hovered, accent, size, theme),
        ButtonVariant::Text => {}
    }
}

fn draw_primary_button_surface(
    canvas: &Canvas,
    hovered: bool,
    accent: SkiaColor,
    size: (f32, f32),
    theme: &Theme,
) {
    let fill = if hovered {
        Theme::darken(accent, 0.15)
    } else {
        accent
    };
    let mut paint = Paint::default();
    paint.set_color(fill);
    paint.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &paint);
}

fn draw_ghost_button_surface(
    canvas: &Canvas,
    hovered: bool,
    accent: SkiaColor,
    size: (f32, f32),
    theme: &Theme,
) {
    if hovered {
        draw_button_hover_fill(canvas, size, theme);
    }
    draw_button_border(canvas, hovered, accent, size, theme);
}

fn draw_button_hover_fill(canvas: &Canvas, size: (f32, f32), theme: &Theme) {
    let mut bg = Paint::default();
    bg.set_color(Theme::alpha(theme.on_surface, 20));
    bg.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &bg);
}

fn draw_button_border(
    canvas: &Canvas,
    hovered: bool,
    accent: SkiaColor,
    size: (f32, f32),
    theme: &Theme,
) {
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.0);
    border.set_color(button_border_color(hovered, accent, theme));
    border.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &border);
}

fn button_hovered(size: (f32, f32), mouse: Point) -> bool {
    SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse)
}

fn button_text_color(
    color: Option<SkiaColor>,
    variant: ButtonVariant,
    hovered: bool,
    theme: &Theme,
) -> SkiaColor {
    let accent = color.unwrap_or(theme.primary);
    match variant {
        ButtonVariant::Primary => theme.on_primary,
        ButtonVariant::Ghost if hovered => accent,
        ButtonVariant::Ghost => theme.on_surface,
        ButtonVariant::Text if hovered => accent,
        ButtonVariant::Text => Theme::alpha(theme.on_surface, 180),
    }
}

fn button_border_color(hovered: bool, accent: SkiaColor, theme: &Theme) -> SkiaColor {
    if hovered {
        accent
    } else {
        Theme::alpha(theme.on_surface, 100)
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

fn draw_image(
    canvas: &Canvas,
    data: &[u8],
    size: (f32, f32),
    radius: f32,
    scale: f32,
    cache: &mut ImageRenderCache,
) {
    use skia_safe::Matrix;
    if data.len() > MAX_ENCODED_IMAGE_BYTES {
        return;
    }
    if is_svg_image(data) {
        draw_svg_image(canvas, data, size, radius, scale, cache);
        return;
    }

    let Some(decoded) = cached_raster_image(data, cache) else {
        return;
    };

    let iw = decoded.width;
    let ih = decoded.height;
    if iw <= 0 || ih <= 0 {
        return;
    }

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
    canvas.draw_image(&decoded.image, (0.0_f32, 0.0_f32), Some(&Paint::default()));
    canvas.restore();
    if radius > 0.0 {
        canvas.restore();
    }
}

fn cached_raster_image(
    data: &[u8],
    cache: &mut ImageRenderCache,
) -> Option<crate::render::image::RutterDecodedImage> {
    let key = stable_bytes_hash(data);
    if let Some(decoded) = cache.raster_image(key) {
        return Some(decoded);
    }
    let decoded = decode_rutter_image(data).ok()?;
    cache.insert_raster_image(key, decoded.clone());
    Some(decoded)
}

fn draw_svg_image(
    canvas: &Canvas,
    data: &[u8],
    size: (f32, f32),
    radius: f32,
    scale: f32,
    cache: &mut ImageRenderCache,
) {
    if !validate_svg_source(data) || checked_svg_raster_size(size, scale).is_none() {
        return;
    }
    let key = svg_cache_key(data, size, scale);
    let Some(image) = cached_svg_image(data, size, scale, key, cache) else {
        return;
    };

    clip_image_radius(canvas, size, radius);
    draw_cached_svg_image(canvas, &image, size);
    if radius > 0.0 {
        canvas.restore();
    }
}

fn cached_svg_image(
    data: &[u8],
    size: (f32, f32),
    scale: f32,
    key: SvgImageCacheKey,
    cache: &mut ImageRenderCache,
) -> Option<skia_safe::Image> {
    if let Some(image) = cache.svg_image(key) {
        return Some(image);
    }
    let image = rasterize_svg_image(data, size, scale)?;
    cache.insert_svg_image(key, image.clone());
    Some(image)
}

fn rasterize_svg_image(data: &[u8], size: (f32, f32), scale: f32) -> Option<skia_safe::Image> {
    use skia_safe::{Color, FontMgr, Size, svg::Dom};

    if !validate_svg_source(data) {
        return None;
    }
    let image_size = checked_svg_raster_size(size, scale)?;
    let Ok(mut dom) = Dom::from_bytes(data, FontMgr::empty()) else {
        return None;
    };
    let svg_size = svg_intrinsic_size(data).unwrap_or(size);
    let fit_scale = svg_fit_scale(svg_size, size);
    let offset = centered_svg_offset(svg_size, size, fit_scale);
    let mut surface = skia_safe::surfaces::raster_n32_premul(image_size)?;
    surface.canvas().clear(Color::TRANSPARENT);
    surface.canvas().scale((scale, scale));
    surface.canvas().translate(offset);
    surface.canvas().scale((fit_scale, fit_scale));

    dom.set_container_size(Size::new(svg_size.0, svg_size.1));
    dom.render(surface.canvas());
    Some(surface.image_snapshot())
}

fn svg_intrinsic_size(data: &[u8]) -> Option<(f32, f32)> {
    let source = std::str::from_utf8(data).ok()?;
    let tag = source.split('>').next()?;
    let width = svg_number_attr(tag, "width")?;
    let height = svg_number_attr(tag, "height")?;
    Some((width, height))
}

fn svg_number_attr(tag: &str, name: &str) -> Option<f32> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let rest = tag.get(start..)?;
    let value = rest.split('"').next()?;
    value.parse::<f32>().ok().filter(|size| *size > 0.0)
}

fn svg_fit_scale(svg_size: (f32, f32), target_size: (f32, f32)) -> f32 {
    let sx = target_size.0 / svg_size.0.max(1.0);
    let sy = target_size.1 / svg_size.1.max(1.0);
    sx.min(sy)
}

fn centered_svg_offset(svg_size: (f32, f32), target_size: (f32, f32), scale: f32) -> (f32, f32) {
    (
        (target_size.0 - svg_size.0 * scale) * 0.5,
        (target_size.1 - svg_size.1 * scale) * 0.5,
    )
}

fn draw_cached_svg_image(canvas: &Canvas, image: &skia_safe::Image, size: (f32, f32)) {
    let sx = size.0 / image.width().max(1) as f32;
    let sy = size.1 / image.height().max(1) as f32;
    canvas.save();
    canvas.scale((sx, sy));
    canvas.draw_image(image, (0.0_f32, 0.0_f32), Some(&Paint::default()));
    canvas.restore();
}

fn svg_cache_key(data: &[u8], size: (f32, f32), scale: f32) -> SvgImageCacheKey {
    SvgImageCacheKey {
        data_hash: stable_bytes_hash(data),
        width: size.0.ceil() as u32,
        height: size.1.ceil() as u32,
        scale_bits: scale.to_bits(),
    }
}

fn clip_image_radius(canvas: &Canvas, size: (f32, f32), radius: f32) {
    if radius <= 0.0 {
        return;
    }
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

fn is_svg_image(data: &[u8]) -> bool {
    let Ok(source) = std::str::from_utf8(data) else {
        return false;
    };
    let trimmed = source.trim_start();
    trimmed.starts_with("<svg") || trimmed.starts_with("<?xml") && trimmed.contains("<svg")
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
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use cosmic_text::{FontSystem, SwashCache};
    use skia_safe::{Canvas, Color, Font, Point, Surface, surfaces};
    use taffy::prelude::{Dimension, NodeId, Size, Style, TaffyTree};
    use winit::dpi::PhysicalSize;

    use super::{
        ImageRenderCache, RichTextRenderer, draw_image, draw_virtual_grid, draw_widgets,
        is_svg_image, svg_cache_key, virtual_grid_scrollbar_metrics, visible_carousel_selection,
    };
    use crate::carousel::geometry::CarouselItemFrame;
    use crate::engine::widget_state::VirtualGridState;
    use crate::layout::{SCROLLBAR_W, build_taffy_tree, compute_layout};
    use crate::render::text::TextBufferCache;
    use crate::theme::Theme;
    use crate::widget::Widget;

    fn grid_item(index: usize) -> Option<String> {
        Some(format!("Item {index}"))
    }

    #[test]
    fn offscreen_carousel_selection_uses_collection_focus_fallback() {
        let frames = [CarouselItemFrame {
            index: 4,
            x: 0.0,
            width: 200.0,
        }];
        assert_eq!(
            visible_carousel_selection(Some(4), &frames, (200.0, 100.0)),
            Some(4)
        );
        assert_eq!(
            visible_carousel_selection(Some(2), &frames, (200.0, 100.0)),
            None
        );
        let overscan = [CarouselItemFrame {
            index: 5,
            x: 200.0,
            width: 200.0,
        }];
        assert_eq!(
            visible_carousel_selection(Some(5), &overscan, (200.0, 100.0)),
            None
        );
        let leading_overscan = [CarouselItemFrame {
            index: 3,
            x: -200.0,
            width: 200.0,
        }];
        assert_eq!(
            visible_carousel_selection(Some(3), &leading_overscan, (200.0, 100.0)),
            None
        );
        let trailing_sliver = [CarouselItemFrame {
            index: 6,
            x: 199.0,
            width: 200.0,
        }];
        assert_eq!(
            visible_carousel_selection(Some(6), &trailing_sliver, (200.0, 100.0)),
            None
        );
        let leading_sliver = [CarouselItemFrame {
            index: 2,
            x: -199.0,
            width: 200.0,
        }];
        assert_eq!(
            visible_carousel_selection(Some(2), &leading_sliver, (200.0, 100.0)),
            None
        );
    }

    #[test]
    fn image_detection_accepts_svg_sources() {
        assert!(is_svg_image(
            br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#
        ));
        assert!(is_svg_image(
            br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"></svg>"#
        ));
        assert!(!is_svg_image(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn rounded_container_clip_keeps_child_out_of_corner_pixels() {
        let widget = rounded_container_widget();
        let widget_states = HashMap::new();
        let layout_fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &widget, layout_fonts.clone(), &widget_states);
        compute_layout(
            &mut taffy,
            root,
            PhysicalSize::new(20, 20),
            layout_fonts,
            &RichTextRenderer::default(),
        );
        let mut surface = surfaces::raster_n32_premul((20, 20)).unwrap();
        surface.canvas().clear(Color::TRANSPARENT);
        draw_container_test_tree(surface.canvas(), &taffy, root, &widget, &widget_states);

        assert_eq!(pixel_at(&mut surface, 0, 0), Color::TRANSPARENT);
        assert_eq!(pixel_at(&mut surface, 10, 10), Color::BLUE);
    }

    fn rounded_container_widget() -> Widget<'static, ()> {
        Widget::Container {
            child: Box::new(Widget::Container {
                child: Box::new(Widget::Spacer {
                    style: fixed_style(),
                }),
                style: fixed_style(),
                color: Some(Color::BLUE),
                radius: 0.0,
            }),
            style: fixed_style(),
            color: Some(Color::RED),
            radius: 8.0,
        }
    }

    fn fixed_style() -> Style {
        Style {
            size: Size {
                width: Dimension::length(20.0),
                height: Dimension::length(20.0),
            },
            ..Style::default()
        }
    }

    fn draw_container_test_tree(
        canvas: &Canvas,
        taffy: &TaffyTree<crate::layout::RutterContext>,
        root: NodeId,
        widget: &Widget<'_, ()>,
        widget_states: &HashMap<u64, crate::engine::widget_state::WidgetState>,
    ) {
        let mut fonts = FontSystem::new();
        let mut swash = SwashCache::new();
        let mut font_cache = HashMap::<(String, u32), Font>::new();
        let mut text_cache = TextBufferCache::default();
        draw_widgets(
            canvas,
            taffy,
            root,
            widget,
            &mut fonts,
            &mut swash,
            Point::new(-1.0, -1.0),
            None,
            &HashMap::new(),
            widget_states,
            &mut font_cache,
            &mut text_cache,
            true,
            &Theme::default(),
            1.0,
        );
    }

    #[test]
    fn draw_image_renders_svg_data() {
        let mut surface = surfaces::raster_n32_premul((12, 12)).unwrap();
        let mut image_cache = ImageRenderCache::default();
        draw_image(
            surface.canvas(),
            red_svg(),
            (12.0, 12.0),
            0.0,
            1.0,
            &mut image_cache,
        );

        assert_ne!(pixel_at(&mut surface, 6, 6), Color::TRANSPARENT);
    }

    #[test]
    fn draw_image_reuses_svg_cache_entry() {
        let mut surface = surfaces::raster_n32_premul((12, 12)).unwrap();
        let mut image_cache = ImageRenderCache::default();

        draw_test_svg(&mut surface, &mut image_cache);
        draw_test_svg(&mut surface, &mut image_cache);

        assert!(
            image_cache
                .svg_image(svg_cache_key(red_svg(), (12.0, 12.0), 1.0))
                .is_some()
        );
    }

    #[test]
    fn draw_image_scales_intrinsic_svg_size_without_clipping() {
        let mut surface = surfaces::raster_n32_premul((24, 24)).unwrap();
        let mut image_cache = ImageRenderCache::default();
        draw_image(
            surface.canvas(),
            large_red_svg(),
            (24.0, 24.0),
            0.0,
            1.0,
            &mut image_cache,
        );

        assert_ne!(pixel_at(&mut surface, 20, 20), Color::TRANSPARENT);
    }

    #[test]
    fn draw_image_rejects_svg_raster_output_above_pixel_budget() {
        let mut surface = surfaces::raster_n32_premul((12, 12)).unwrap();
        let mut image_cache = ImageRenderCache::default();

        draw_image(
            surface.canvas(),
            red_svg(),
            (4097.0, 4096.0),
            0.0,
            1.0,
            &mut image_cache,
        );

        assert!(
            image_cache
                .svg_image(svg_cache_key(red_svg(), (4097.0, 4096.0), 1.0))
                .is_none()
        );
    }

    #[test]
    fn virtual_grid_scrollbar_metrics_use_grid_state_methods() {
        let state = grid_state();
        let metrics = virtual_grid_scrollbar_metrics(Some(&state), 0.0, 20.0, 60, 3, 20, 80.0);

        assert_eq!(
            metrics,
            Some((state.thumb_ratio(20.0, 60, 3), state.thumb_y(20.0, 60, 3)))
        );
    }

    #[test]
    fn draw_virtual_grid_renders_scrollbar_thumb() {
        let theme = Theme::dark();
        let state = grid_state();
        let mut surface = surfaces::raster_n32_premul((120, 80)).unwrap();
        let mut font_cache = HashMap::new();

        draw_virtual_grid(
            surface.canvas(),
            &3,
            &20.0,
            &60,
            &grid_item,
            Some(&state),
            state.scroll_y,
            None,
            None,
            (120.0, 80.0),
            Point::new(-1.0, -1.0),
            false,
            &mut font_cache,
            &theme,
        );

        assert_ne!(
            pixel_at(&mut surface, scrollbar_x(), 18),
            pixel_at(&mut surface, scrollbar_x(), 70)
        );
    }

    fn grid_state() -> VirtualGridState {
        VirtualGridState {
            scroll_y: 40.0,
            viewport_w: 120.0,
            viewport_h: 80.0,
            selected_item: None,
            hovered_item: None,
        }
    }

    fn scrollbar_x() -> i32 {
        (120.0 - SCROLLBAR_W - 2.0 + SCROLLBAR_W / 2.0) as i32
    }

    fn red_svg() -> &'static [u8] {
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12">
<rect width="12" height="12" fill="#ff0000"/>
</svg>"##
    }

    fn large_red_svg() -> &'static [u8] {
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
<rect width="48" height="48" fill="#ff0000"/>
</svg>"##
    }

    fn draw_test_svg(surface: &mut Surface, image_cache: &mut ImageRenderCache) {
        draw_image(
            surface.canvas(),
            red_svg(),
            (12.0, 12.0),
            0.0,
            1.0,
            image_cache,
        );
    }

    fn pixel_at(surface: &mut Surface, x: i32, y: i32) -> Color {
        surface.peek_pixels().unwrap().get_color((x, y))
    }
}
