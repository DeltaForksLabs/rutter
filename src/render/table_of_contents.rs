// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::collections::HashMap;

use skia_safe::{Canvas, Contains, Font, Paint, Point, RRect, Rect as SkiaRect};
use taffy::prelude::{NodeId, TaffyTree};

use super::{
    AccordionHeaderRenderInput, TextDrawInput, draw_accordion_header, draw_focus_outline,
    draw_text_line,
};
use crate::layout::RutterContext;
use crate::text_controls::TextControlPolicy;
use crate::theme::Theme;
use crate::widget::Widget;
use crate::widgets::table_of_contents::{
    TABLE_OF_CONTENTS_LINK_SIZE, TABLE_OF_CONTENTS_TITLE_SIZE, TableOfContentsEntry,
    collect_entries, entry_rects, entry_text_inset, title_rect,
};

pub(super) struct TableOfContentsNavigationInput<'a, 'widget, Msg> {
    pub(super) canvas: &'a Canvas,
    pub(super) taffy: &'a TaffyTree<RutterContext>,
    pub(super) table_node: NodeId,
    pub(super) table: &'a Widget<'widget, Msg>,
    pub(super) content: &'a Widget<'widget, Msg>,
    pub(super) mouse: Point,
    pub(super) focused_id: Option<u64>,
    pub(super) shows_interaction_effects: bool,
    pub(super) font_cache: &'a mut HashMap<(String, u32), Font>,
    pub(super) theme: &'a Theme,
    pub(super) path: &'a [usize],
}

pub(super) fn draw_navigation<Msg>(mut input: TableOfContentsNavigationInput<'_, '_, Msg>) {
    let Widget::TableOfContents { title, .. } = input.table else {
        return;
    };
    draw_navigation_header(&mut input, title);
    if !input.table.table_of_contents_entries_visible() {
        return;
    }
    let entries = collect_entries(input.content);
    draw_entries(&mut input, &entries);
}

fn draw_navigation_header<Msg>(
    input: &mut TableOfContentsNavigationInput<'_, '_, Msg>,
    title: &str,
) {
    let Some(accordion) = input.table.table_of_contents_accordion() else {
        draw_title(input, title);
        return;
    };
    let Some(rect) = title_rect(input.taffy, input.table_node) else {
        return;
    };
    let expanded = accordion.expanded();
    let focus_id = input.table.table_of_contents_accordion_focus_id(input.path);
    input.canvas.save();
    input.canvas.translate((rect.left, rect.top));
    draw_accordion_header(AccordionHeaderRenderInput {
        canvas: input.canvas,
        title,
        expanded,
        is_focused: input.shows_interaction_effects && focus_id == input.focused_id,
        size: (rect.width(), rect.height()),
        mouse: Point::new(input.mouse.x - rect.left, input.mouse.y - rect.top),
        font_cache: input.font_cache,
        theme: input.theme,
    });
    input.canvas.restore();
}

fn draw_title<Msg>(input: &mut TableOfContentsNavigationInput<'_, '_, Msg>, title: &str) {
    let Some(rect) = title_rect(input.taffy, input.table_node) else {
        return;
    };
    draw_text_in_rect(
        input,
        title,
        rect,
        TABLE_OF_CONTENTS_TITLE_SIZE,
        input.theme.on_surface,
    );
}

fn draw_entries<Msg>(
    input: &mut TableOfContentsNavigationInput<'_, '_, Msg>,
    entries: &[TableOfContentsEntry],
) {
    let rects = entry_rects(input.taffy, input.table_node);
    for (index, (entry, rect)) in entries.iter().zip(rects).enumerate() {
        draw_entry(input, entry, index, rect);
    }
}

fn draw_entry<Msg>(
    input: &mut TableOfContentsNavigationInput<'_, '_, Msg>,
    entry: &TableOfContentsEntry,
    index: usize,
    rect: SkiaRect,
) {
    let focus_id = input
        .table
        .table_of_contents_entry_focus_id(input.path, index);
    let hovered = input.shows_interaction_effects && rect.contains(input.mouse);
    let focused = input.shows_interaction_effects && focus_id == input.focused_id;
    draw_entry_highlight(input.canvas, rect, hovered, focused, input.theme);
    let text_rect = inset_entry_rect(rect, entry_text_inset(entry.visual_depth()));
    let color = entry_color(hovered || focused, input.theme);
    let label = entry.display_title();
    draw_text_in_rect(input, &label, text_rect, TABLE_OF_CONTENTS_LINK_SIZE, color);
}

fn draw_entry_highlight(
    canvas: &Canvas,
    rect: SkiaRect,
    hovered: bool,
    focused: bool,
    theme: &Theme,
) {
    if hovered {
        let mut paint = Paint::default();
        paint.set_color(Theme::alpha(theme.primary, 32));
        paint.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(rect, 4.0, 4.0), &paint);
    }
    if focused {
        draw_focus_outline(canvas, rect, 4.0, theme);
    }
}

fn inset_entry_rect(rect: SkiaRect, inset: f32) -> SkiaRect {
    SkiaRect::from_xywh(
        rect.left + inset,
        rect.top,
        (rect.width() - inset).max(0.0),
        rect.height(),
    )
}

fn entry_color(active: bool, theme: &Theme) -> skia_safe::Color {
    if active {
        return theme.primary;
    }
    Theme::alpha(theme.on_surface, 210)
}

fn draw_text_in_rect<Msg>(
    input: &mut TableOfContentsNavigationInput<'_, '_, Msg>,
    text: &str,
    rect: SkiaRect,
    font_size: f32,
    color: skia_safe::Color,
) {
    input.canvas.save();
    input.canvas.translate((rect.left, rect.top));
    input.canvas.clip_rect(
        SkiaRect::from_xywh(0.0, 0.0, rect.width(), rect.height()),
        None,
        true,
    );
    draw_text_line(TextDrawInput {
        canvas: input.canvas,
        text,
        size: (rect.width(), rect.height()),
        color,
        font_size,
        font_cache: input.font_cache,
        center: false,
        control_policy: TextControlPolicy::FlattenLineBreaks,
    });
    input.canvas.restore();
}
