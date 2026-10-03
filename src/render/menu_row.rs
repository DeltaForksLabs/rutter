// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Shared single-line command/shortcut columns and bounded context-menu icons.

use skia_safe::{Canvas, Font, Paint, Rect};

use super::ImageRenderCache;
use super::text::{draw_single_line_text, measure_single_line_text};
use crate::i18n::LayoutDirection;
use crate::theme::Theme;
use crate::widgets::dropdown_menu::{DropdownMenuEntryAccess, DropdownMenuEntryKind};

const LABEL_START: f32 = 30.0;
const LABEL_END: f32 = 10.0;
const SUBMENU_END: f32 = 24.0;
const COLUMN_GAP: f32 = 16.0;
const ICON_SIZE: f32 = 16.0;

#[derive(Clone, Copy)]
enum TextAlignment {
    InlineStart,
    InlineEnd,
}

#[derive(Clone, Copy)]
pub(crate) struct MenuColumns {
    command_width: f32,
    shortcut_width: f32,
    end_padding: f32,
}

impl MenuColumns {
    pub(crate) fn measure<Entry: DropdownMenuEntryAccess>(entries: &[Entry], font: &Font) -> Self {
        let paint = Paint::default();
        let mut columns = Self {
            command_width: 0.0,
            shortcut_width: 0.0,
            end_padding: LABEL_END,
        };
        for entry in entries {
            if let Some(label) = entry.entry_label() {
                columns.command_width = columns
                    .command_width
                    .max(measure_single_line_text(font, label, &paint));
                if let Some(shortcut) = entry.entry_shortcut_label() {
                    columns.shortcut_width = columns
                        .shortcut_width
                        .max(measure_single_line_text(font, shortcut, &paint));
                }
                if entry.entry_kind() == DropdownMenuEntryKind::Submenu {
                    columns.end_padding = SUBMENU_END;
                }
            }
        }
        columns
    }

    fn width(self) -> f32 {
        let gap = if self.shortcut_width > 0.0 {
            COLUMN_GAP
        } else {
            0.0
        };
        // The extra padding leaves room for scrollbars without changing columns.
        LABEL_START + self.command_width + gap + self.shortcut_width + self.end_padding + 8.0
    }

    fn clips(self, rect: Rect, direction: LayoutDirection, submenu: bool) -> (Rect, Option<Rect>) {
        if self.shortcut_width == 0.0 && rect.width() >= LABEL_START + SUBMENU_END {
            return (dropdown_entry_label_clip(rect, direction, submenu), None);
        }
        let end = if self.shortcut_width > 0.0 {
            self.end_padding
        } else if submenu {
            SUBMENU_END
        } else {
            LABEL_END
        };
        let available = (rect.width() - LABEL_START - end).max(0.0);
        // In constrained panels preserve a usable command column. Very narrow
        // panels suppress shortcuts rather than overlaying the label or icon.
        let shortcut_width = if available >= self.command_width + COLUMN_GAP + self.shortcut_width {
            self.shortcut_width
        } else if available >= 48.0 {
            self.shortcut_width.min((available - COLUMN_GAP) * 0.5)
        } else {
            0.0
        };
        let gap = if shortcut_width > 0.0 {
            COLUMN_GAP
        } else {
            0.0
        };
        let command_width = (available - shortcut_width - gap).max(0.0);
        let (command, shortcut) = match direction {
            LayoutDirection::Ltr => {
                let start = (rect.left + LABEL_START).min(rect.right);
                let command = Rect::from_xywh(start, rect.top, command_width, rect.height());
                let shortcut =
                    Rect::from_xywh(command.right + gap, rect.top, shortcut_width, rect.height());
                (command, shortcut)
            }
            LayoutDirection::Rtl => {
                let start = (rect.right - LABEL_START).max(rect.left);
                let command = Rect::from_xywh(
                    start - command_width,
                    rect.top,
                    command_width,
                    rect.height(),
                );
                let shortcut = Rect::from_xywh(
                    command.left - gap - shortcut_width,
                    rect.top,
                    shortcut_width,
                    rect.height(),
                );
                (command, shortcut)
            }
        };
        (command, (shortcut_width > 0.0).then_some(shortcut))
    }
}

pub(crate) fn dropdown_entry_label_clip(
    rect: Rect,
    direction: LayoutDirection,
    has_submenu: bool,
) -> Rect {
    let end = if has_submenu { SUBMENU_END } else { LABEL_END };
    let (left, right) = match direction {
        LayoutDirection::Ltr => (LABEL_START, end),
        LayoutDirection::Rtl => (end, LABEL_START),
    };
    Rect::from_ltrb(rect.left + left, rect.top, rect.right - right, rect.bottom)
}

pub(crate) fn context_panel_width<Msg>(
    entries: &[crate::ContextMenuEntry<'_, Msg>],
    font: &Font,
) -> f32 {
    MenuColumns::measure(entries, font).width()
}

pub(crate) struct MenuRowPainter<'a> {
    pub(crate) canvas: &'a Canvas,
    pub(crate) font: &'a Font,
    pub(crate) theme: &'a Theme,
    pub(crate) direction: LayoutDirection,
    pub(crate) columns: MenuColumns,
    pub(crate) image_cache: Option<&'a mut ImageRenderCache>,
    pub(crate) scale: f32,
}

impl MenuRowPainter<'_> {
    pub(crate) fn draw<Entry: DropdownMenuEntryAccess>(&mut self, rect: Rect, entry: &Entry) {
        let disabled = entry.entry_is_disabled();
        let color = if disabled {
            Theme::alpha(self.theme.on_surface, 90)
        } else {
            self.theme.on_surface
        };
        let (command, shortcut) = self.columns.clips(
            rect,
            self.direction,
            entry.entry_kind() == DropdownMenuEntryKind::Submenu,
        );
        if let Some(label) = entry.entry_label() {
            self.draw_text(label, command, color, TextAlignment::InlineStart);
        }
        if let (Some(label), Some(clip)) = (entry.entry_shortcut_label(), shortcut) {
            self.draw_text(
                label,
                clip,
                Theme::alpha(self.theme.on_surface, if disabled { 70 } else { 160 }),
                TextAlignment::InlineEnd,
            );
        }
        self.draw_icon(rect, entry);
    }

    fn draw_text(&self, text: &str, clip: Rect, color: skia_safe::Color, alignment: TextAlignment) {
        if clip.width() <= 0.0 {
            return;
        }
        let mut paint = Paint::default();
        paint.set_color(color).set_anti_alias(true);
        let width = measure_single_line_text(self.font, text, &paint);
        let x = match (self.direction, alignment) {
            (LayoutDirection::Ltr, TextAlignment::InlineStart)
            | (LayoutDirection::Rtl, TextAlignment::InlineEnd) => clip.left,
            (LayoutDirection::Rtl, TextAlignment::InlineStart)
            | (LayoutDirection::Ltr, TextAlignment::InlineEnd) => clip.right - width,
        };
        self.canvas.save();
        self.canvas.clip_rect(clip, None, true);
        draw_single_line_text(
            self.canvas,
            text,
            (x, clip.center_y() + self.theme.font_body / 3.0),
            self.font,
            &paint,
        );
        self.canvas.restore();
    }

    fn draw_icon<Entry: DropdownMenuEntryAccess>(&mut self, rect: Rect, entry: &Entry) {
        // Full-size icons need both the start gutter and the trailing controls.
        if rect.width() < LABEL_START + SUBMENU_END {
            return;
        }
        let (Some(svg), Some(cache)) = (entry.entry_svg_icon(), self.image_cache.as_deref_mut())
        else {
            return;
        };
        let x = match self.direction {
            LayoutDirection::Ltr => rect.left + 7.0,
            LayoutDirection::Rtl => rect.right - 7.0 - ICON_SIZE,
        };
        self.canvas.save();
        self.canvas
            .translate((x, rect.center_y() - ICON_SIZE / 2.0));
        if entry.entry_is_disabled() {
            self.canvas
                .save_layer_alpha(None, self.theme.disabled_alpha().into());
        }
        super::draw_svg_image(
            self.canvas,
            svg,
            (ICON_SIZE, ICON_SIZE),
            0.0,
            self.scale,
            cache,
        );
        if entry.entry_is_disabled() {
            self.canvas.restore();
        }
        self.canvas.restore();
    }
}
