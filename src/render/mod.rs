// ============================================================
// Rutter Framework — render/mod.rs
//
// FASE 2 — mudanças principais:
//   • Widget::TextInput não carrega mais Editor
//   • Engine passa &InputWidgetState para renderização
//   • Seleção visual (highlight azul)
//   • Pipeline cosmic-text disponível via render/pipeline.rs
//   • HiDPI: todos os tamanhos de texto passados como lógicos;
//     scale_factor aplicado pelo engine antes de chamar draw_widgets
// ============================================================

pub mod hit_test;
pub mod pipeline;
pub mod text;

use std::collections::HashMap;

use skia_safe::{
    canvas::Canvas,
    paint,
    Color as SkiaColor,
    Contains,
    Font,
    Paint,
    Point,
    RRect,
    Rect as SkiaRect,
};
use taffy::prelude::{NodeId, TaffyTree};
use cosmic_text::{Attrs, FontSystem, Metrics, Shaping, SwashCache};

use crate::input_state::InputWidgetState;
use crate::layout::RutterContext;
use crate::theme::Theme;
use crate::widget::{ButtonVariant, InputState, Widget};
use self::text::{draw_text, get_cached_font};

// ── Ponto de entrada ─────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn draw_widgets<'w, Msg>(
    canvas:         &Canvas,
    taffy:          &TaffyTree<RutterContext>,
    node:           NodeId,
    widget:         &Widget<'w, Msg>,
    fs:             &mut FontSystem,
    swash:          &mut SwashCache,
    mouse_pos:      Point,
    focused_id:     Option<u64>,
    input_states:   &HashMap<u64, InputWidgetState>,
    font_cache:     &mut HashMap<(String, u32), Font>,
    cursor_visible: bool,
    theme:          &Theme,
    scale:          f32, // HiDPI scale factor
) {
    let layout      = taffy.layout(node).unwrap();
    let pos         = Point::new(layout.location.x, layout.location.y);
    let size        = (layout.size.width, layout.size.height);
    let local_mouse = Point::new(mouse_pos.x - pos.x, mouse_pos.y - pos.y);

    canvas.save();
    canvas.translate((pos.x, pos.y));

    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node).unwrap();
            for (i, child) in children.iter().enumerate() {
                draw_widgets(canvas, taffy, ids[i], child, fs, swash,
                    local_mouse, focused_id, input_states,
                    font_cache, cursor_visible, theme, scale);
            }
        }

        Widget::Container { child, color, radius, .. } => {
            if let Some(c) = color {
                let mut p = Paint::default();
                p.set_color(*c);
                p.set_anti_alias(true);
                canvas.draw_rrect(rrect(size, *radius), &p);
            }
            let ids = taffy.children(node).unwrap();
            draw_widgets(canvas, taffy, ids[0], child, fs, swash,
                local_mouse, focused_id, input_states,
                font_cache, cursor_visible, theme, scale);
        }

        Widget::Button { text, color, variant, .. } => {
            draw_button(canvas, text, *color, *variant, size, local_mouse, font_cache, theme);
        }

        Widget::TextInput {
            id, label, placeholder, state, error_msg, is_password, ..
        } => {
            let is_focused = focused_id == Some(*id);
            let istate     = input_states.get(id);
            draw_text_input(
                canvas, fs, swash, font_cache, theme, scale,
                size, is_focused, label, placeholder,
                *state, error_msg.as_deref(), *is_password,
                istate, cursor_visible,
            );
        }

        Widget::Text { content, color, size: font_size, .. } => {
            let c = color.unwrap_or(theme.on_surface);
            draw_text(canvas, content, (0.0, 0.0).into(), size,
                c, *font_size, font_cache, false);
        }
    }

    canvas.restore();
}

// ── Button ────────────────────────────────────────────────────

fn draw_button(
    canvas:     &Canvas,
    text:       &str,
    color:      Option<SkiaColor>,
    variant:    ButtonVariant,
    size:       (f32, f32),
    mouse:      Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme:      &Theme,
) {
    let rect    = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
    let hovered = rect.contains(mouse);
    let accent  = color.unwrap_or(theme.primary);

    match variant {
        ButtonVariant::Primary => {
            let fill = if hovered { Theme::darken(accent, 0.15) } else { accent };
            let mut p = Paint::default();
            p.set_color(fill);
            p.set_anti_alias(true);
            canvas.draw_rrect(rrect(size, theme.radius_sm), &p);
            draw_text(canvas, text, (0.0, 0.0).into(), size,
                theme.on_primary, theme.font_body, font_cache, true);
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
            b.set_color(if hovered { accent } else { Theme::alpha(theme.on_surface, 100) });
            b.set_anti_alias(true);
            canvas.draw_rrect(rrect(size, theme.radius_sm), &b);
            let tc = if hovered { accent } else { theme.on_surface };
            draw_text(canvas, text, (0.0, 0.0).into(), size, tc,
                theme.font_body, font_cache, true);
        }
        ButtonVariant::Text => {
            let tc = if hovered { accent } else { Theme::alpha(theme.on_surface, 180) };
            draw_text(canvas, text, (0.0, 0.0).into(), size, tc,
                theme.font_body, font_cache, true);
        }
    }
}

// ── TextInput ────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_text_input(
    canvas:         &Canvas,
    fs:             &mut FontSystem,
    _swash:         &mut SwashCache,
    font_cache:     &mut HashMap<(String, u32), Font>,
    theme:          &Theme,
    _scale:         f32,
    size:           (f32, f32),
    is_focused:     bool,
    label:          &str,
    placeholder:    &str,
    state:          InputState,
    error_msg:      Option<&str>,
    is_password:    bool,
    istate:         Option<&InputWidgetState>,
    cursor_visible: bool,
) {
    let border_c = theme.input_border(state, is_focused);
    let border_w = if is_focused { 1.5 } else { 1.0 };

    // ── Fundo ─────────────────────────────────────────────────
    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &bg);

    // ── Borda ─────────────────────────────────────────────────
    let mut brd = Paint::default();
    brd.set_style(paint::Style::Stroke);
    brd.set_stroke_width(border_w);
    brd.set_color(border_c);
    brd.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &brd);

    // ── Label flutuante ───────────────────────────────────────
    if !label.is_empty() {
        let lbl_font  = get_cached_font(font_cache, "sans-serif", theme.font_label);
        let lbl_color = if is_focused { theme.primary } else { Theme::alpha(theme.on_surface, 180) };
        let mut p = Paint::default();
        p.set_color(lbl_color);
        p.set_anti_alias(true);
        canvas.draw_str(label, (4.0, -4.0), &lbl_font, &p);
    }

    // ── Região de conteúdo com clip ───────────────────────────
    let pad_x = theme.spacing * 2.0;
    let pad_y = theme.spacing;

    // FIX #1: único par save/restore interno
    canvas.save();
    canvas.translate((pad_x, pad_y));
    canvas.clip_rect(
        SkiaRect::from_xywh(0.0, 0.0, size.0 - pad_x * 2.0, size.1),
        None, true,
    );

    let Some(s) = istate else {
        // Sem estado interno → só placeholder
        if !placeholder.is_empty() {
            draw_placeholder(canvas, placeholder, size, pad_y, font_cache, theme);
        }
        canvas.restore();
        return;
    };

    // Scroll horizontal
    canvas.translate((-s.scroll_x, 0.0));

    // ── Seleção visual ────────────────────────────────────────
    if let Some(sel) = s.selection {
        if !sel.is_empty() {
            draw_selection_highlight(canvas, s, sel.start, sel.end, size, pad_y, theme, font_cache, fs);
        }
    }

    let text = s.text();
    let is_empty = text.is_empty();

    if is_empty && !placeholder.is_empty() && !is_focused {
        draw_placeholder(canvas, placeholder, size, pad_y, font_cache, theme);
    } else {
        // ── Cursor ────────────────────────────────────────────
        if is_focused && cursor_visible {
            let cx = s.cursor_x();
            let mut cp = Paint::default();
            cp.set_color(theme.primary);
            canvas.draw_rect(
                SkiaRect::from_xywh(cx - s.scroll_x, 2.0, 1.5, theme.font_body + 4.0),
                &cp,
            );
        }

        // ── Texto ─────────────────────────────────────────────
        // Usar cosmic-text para shaping, Skia para blit
        let display_text = if is_password {
            "•".repeat(text.chars().count())
        } else {
            text
        };

        // Criar um Buffer temporário para renderização
        use cosmic_text::Buffer;
        let mut buf = Buffer::new(fs, Metrics::new(theme.font_body, theme.font_body * 1.3));
        buf.set_size(fs, Some(10_000.0), Some(size.1));
        buf.set_text(fs, &display_text, &Attrs::new(), Shaping::Advanced, None);
        buf.shape_until_scroll(fs, false);

        for run in buf.layout_runs() {
            let font = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let mut tp = Paint::default();
            tp.set_color(theme.on_surface);
            tp.set_anti_alias(true);
            canvas.draw_str(run.text, (0.0, run.line_y), &font, &tp);
        }
    }

    canvas.restore(); // único restore interno

    // ── Mensagem de erro ──────────────────────────────────────
    if let Some(msg) = error_msg {
        let ef   = get_cached_font(font_cache, "sans-serif", theme.font_small);
        let mut p = Paint::default();
        p.set_color(theme.error);
        p.set_anti_alias(true);
        canvas.draw_str(msg, (4.0, size.1 + theme.spacing * 3.0), &ef, &p);
    }
}

// ── Seleção visual ───────────────────────────────────────────

fn draw_selection_highlight(
    canvas:     &Canvas,
    _s:         &InputWidgetState,
    _start:     usize,
    _end:       usize,
    size:       (f32, f32),
    pad_y:      f32,
    theme:      &Theme,
    _cache:     &mut HashMap<(String, u32), Font>,
    _fs:        &mut FontSystem,
) {
    // Por enquanto: highlight full-width quando tem seleção
    // Fase 3: calcular o rect exato por glyph range
    let h   = theme.font_body + 4.0;
    let y   = (size.1 - h) / 2.0 - pad_y;
    let mut p = Paint::default();
    p.set_color(Theme::alpha(theme.primary, 60)); // accent com 24% alpha
    p.set_anti_alias(true);
    canvas.draw_rect(
        SkiaRect::from_xywh(0.0, y, size.0 - 8.0, h),
        &p,
    );
}

// ── Placeholder ──────────────────────────────────────────────

fn draw_placeholder(
    canvas:      &Canvas,
    placeholder: &str,
    size:        (f32, f32),
    pad_y:       f32,
    font_cache:  &mut HashMap<(String, u32), Font>,
    theme:       &Theme,
) {
    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut p = Paint::default();
    p.set_color(Theme::alpha(theme.on_surface, 120));
    p.set_anti_alias(true);
    let y = (size.1 / 2.0) + (theme.font_body / 3.0) - pad_y;
    canvas.draw_str(placeholder, (0.0, y), &f, &p);
}

// ── Helper ───────────────────────────────────────────────────

fn rrect(size: (f32, f32), r: f32) -> RRect {
    RRect::new_rect_xy(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), r, r)
}
