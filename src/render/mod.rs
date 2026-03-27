// ============================================================
// Rutter Framework — render/mod.rs  (Fase 4 + fixes v6.2)
//
// FIXES v6.2:
//   FIX-1  draw_text_input: cursor desenhado usando o mesmo buffer
//          de renderização (métricas corretas). Corrigida dupla-
//          subtração de scroll_x (canvas já estava traduzido).
//   FIX-2  draw_widgets/TabBar: anim_x calculado de active * tab_w
//          em tempo de render, ignorando underline_x incorreto.
//   FIX-5  draw_progress_bar: fill clampado corretamente nos dois
//          lados para que a barra "cresça" ao entrar e "encolha"
//          ao sair (sem aparecer instantaneamente em tamanho total).
// ============================================================

pub mod hit_test;
pub mod pipeline;
pub mod text;

use std::collections::HashMap;

use cosmic_text::{Attrs, FontSystem, Metrics, Shaping, SwashCache};
use skia_safe::{
    Color as SkiaColor, Contains, Font, Paint, Point, RRect, Rect as SkiaRect, canvas::Canvas,
    paint,
};
use taffy::prelude::{NodeId, TaffyTree};

use self::text::{draw_text, get_cached_font};
use crate::engine::widget_state::WidgetState;
use crate::input_state::InputWidgetState;
use crate::layout::{OPTION_HEIGHT, RutterContext, SCROLLBAR_W};
use crate::theme::Theme;
use crate::widget::{ButtonVariant, InputState, Orientation, ToastKind, Widget};

// ── Entry point ───────────────────────────────────────────────

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
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
) {
    let layout = taffy.layout(node).unwrap();
    let pos = Point::new(layout.location.x, layout.location.y);
    let size = (layout.size.width, layout.size.height);
    let local_mouse = Point::new(mouse_pos.x - pos.x, mouse_pos.y - pos.y);

    canvas.save();
    canvas.translate((pos.x, pos.y));

    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node).unwrap();
            for (i, child) in children.iter().enumerate() {
                draw_widgets(
                    canvas, taffy, ids[i], child, fs, swash, local_mouse,
                    focused_id, input_states, widget_states, font_cache,
                    cursor_visible, theme, scale,
                );
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
            draw_widgets(
                canvas, taffy, ids[0], child, fs, swash, local_mouse,
                focused_id, input_states, widget_states, font_cache,
                cursor_visible, theme, scale,
            );
        }

        Widget::ScrollView { id, child, .. } => {
            let scroll_state = widget_states.get(id).and_then(|s| s.as_scroll());
            let offset_y = scroll_state.map(|s| s.offset_y).unwrap_or(0.0);
            let content_h = scroll_state.map(|s| s.content_height).unwrap_or(0.0);
            canvas.save();
            canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), None, true);
            canvas.translate((0.0, -offset_y));
            let ids = taffy.children(node).unwrap();
            draw_widgets(
                canvas, taffy, ids[0], child, fs, swash,
                Point::new(local_mouse.x, local_mouse.y + offset_y),
                focused_id, input_states, widget_states, font_cache,
                cursor_visible, theme, scale,
            );
            canvas.restore();
            if content_h > size.1 {
                draw_scrollbar(canvas, size, scroll_state, theme);
            }
        }

        Widget::Tooltip { child, text, .. } => {
            let ids = taffy.children(node).unwrap();
            draw_widgets(
                canvas, taffy, ids[0], child, fs, swash, local_mouse,
                focused_id, input_states, widget_states, font_cache,
                cursor_visible, theme, scale,
            );
            let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
            if rect.contains(local_mouse) {
                draw_tooltip_popup(canvas, text, local_mouse, font_cache, theme);
            }
        }

        Widget::Button { text, color, variant, .. } => draw_button(
            canvas, text, *color, *variant, size, local_mouse, font_cache, theme,
        ),

        Widget::TextInput { id, label, placeholder, state, error_msg, is_password, .. } => {
            draw_text_input(
                canvas, fs, swash, font_cache, theme, scale, size,
                focused_id == Some(*id), label, placeholder, *state,
                error_msg.as_deref(), *is_password, input_states.get(id),
                cursor_visible,
            )
        }

        Widget::Checkbox { checked, label, .. } => draw_checkbox(
            canvas, *checked, label, size, local_mouse, font_cache, theme,
        ),

        Widget::Switch { checked, .. } => draw_switch(canvas, *checked, size, local_mouse, theme),

        Widget::Radio { selected, label, .. } => draw_radio(
            canvas, *selected, label, size, local_mouse, font_cache, theme,
        ),

        Widget::Slider { id, value, min, max, .. } => {
            let dragging = widget_states
                .get(id)
                .and_then(|s| s.as_slider())
                .map(|s| s.dragging)
                .unwrap_or(false);
            draw_slider(canvas, *value, *min, *max, size, local_mouse, dragging, theme);
        }

        Widget::ProgressBar { id, value, indeterminate, .. } => {
            let anim_offset = if *indeterminate {
                widget_states
                    .get(id)
                    .and_then(|s| s.as_anim())
                    .map(|a| a.anim_offset)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            draw_progress_bar(canvas, *value, *indeterminate, anim_offset, size, theme);
        }

        Widget::Spinner { id, .. } => {
            let angle = widget_states
                .get(id)
                .and_then(|s| s.as_anim())
                .map(|a| a.angle)
                .unwrap_or(0.0);
            draw_spinner(canvas, angle, size, theme);
        }

        Widget::Image { data, radius, .. } => draw_image(canvas, data, size, *radius),

        Widget::Divider { orientation, .. } => draw_divider(canvas, *orientation, size, theme),

        Widget::Spacer { .. } => {}

        Widget::Text { content, color, size: font_size, .. } => {
            let c = color.unwrap_or(theme.on_surface);
            draw_text(canvas, content, (0.0, 0.0).into(), size, c, *font_size, font_cache, false);
        }

        Widget::Select { id, options, selected_index, label, placeholder, .. } => {
            let sel_state = widget_states.get(id).and_then(|s| s.as_select());
            let is_open = sel_state.map(|s| s.is_open).unwrap_or(false);
            let hovered = sel_state.and_then(|s| s.hovered_option);
            draw_select(
                canvas, options, *selected_index, is_open, hovered, label, placeholder,
                size, local_mouse, font_cache, theme,
            );
        }

        // ── FIX-2: TabBar — underline sempre alinhado ao nome ────────
        //
        // Problema original: `anim_x` vinha de `TabState::underline_x`,
        // que era setado em `runner.rs` com `size_ref / 4.0` como
        // estimativa de tab_width — errada sempre que num_tabs ≠ 4.
        //
        // Solução: calcular `anim_x = active * tab_w` em tempo de
        // render, quando `size.0` (largura real da TabBar) está
        // disponível. O `TabState::underline_x` continua existindo para
        // a animação suave da Fase 5; por ora ignoramos ele aqui.
        Widget::TabBar { id: _, tabs, active, .. } => {
            let tab_w = size.0 / tabs.len().max(1) as f32;
            let anim_x = *active as f32 * tab_w; // FIX-2: base no active real
            draw_tabbar(canvas, tabs, *active, anim_x, size, local_mouse, font_cache, theme);
        }

        Widget::Modal { id, visible, child, .. } => {
            if !*visible {
                canvas.restore();
                return;
            }
            let alpha = widget_states
                .get(id)
                .and_then(|s| s.as_modal())
                .map(|m| m.backdrop_alpha)
                .unwrap_or(180);
            draw_modal(
                canvas, taffy, node, child, fs, swash, mouse_pos,
                focused_id, input_states, widget_states, font_cache,
                cursor_visible, theme, scale, size, alpha,
            );
        }

        Widget::Toast { id, message, kind, .. } => {
            let visible = widget_states
                .get(id)
                .and_then(|s| s.as_toast())
                .map(|t| t.visible && !t.is_expired())
                .unwrap_or(false);
            let progress = widget_states
                .get(id)
                .and_then(|s| s.as_toast())
                .map(|t| t.progress())
                .unwrap_or(0.0);
            if visible {
                draw_toast(canvas, message, *kind, progress, size, font_cache, theme);
            }
        }

        Widget::VirtualList { id, item_height, item_count, items, .. } => {
            let vstate = widget_states.get(id).and_then(|s| s.as_vlist());
            let scroll_y = vstate.map(|v| v.scroll_y).unwrap_or(0.0);
            let selected = vstate.and_then(|v| v.selected_row);
            let hovered = vstate.and_then(|v| v.hovered_row);
            draw_virtual_list(
                canvas, item_height, item_count, items, scroll_y,
                selected, hovered, size, local_mouse, font_cache, theme,
            );
        }
    }

    canvas.restore();
}

// ── TabBar ────────────────────────────────────────────────────

fn draw_tabbar(
    canvas: &Canvas,
    tabs: &[&str],
    active: usize,
    anim_x: f32,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    if tabs.is_empty() { return; }
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

    // FIX-2: underline posicionado via anim_x que já está correto
    // (calculado como active * tab_w em draw_widgets).
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

// ── TextInput ─────────────────────────────────────────────────

fn draw_text_input(
    canvas: &Canvas,
    fs: &mut FontSystem,
    _swash: &mut SwashCache,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
    _scale: f32,
    size: (f32, f32),
    is_focused: bool,
    label: &str,
    placeholder: &str,
    state: InputState,
    error_msg: Option<&str>,
    is_password: bool,
    istate: Option<&InputWidgetState>,
    cursor_visible: bool,
) {
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
        p.set_color(if is_focused { theme.primary } else { Theme::alpha(theme.on_surface, 180) });
        p.set_anti_alias(true);
        canvas.draw_str(label, (4.0, -4.0), &lf, &p);
    }

    let pad_x = theme.spacing * 2.0;
    let pad_y = theme.spacing;
    canvas.save();
    canvas.translate((pad_x, pad_y));
    canvas.clip_rect(
        SkiaRect::from_xywh(0.0, 0.0, size.0 - pad_x * 2.0, size.1),
        None,
        true,
    );

    let Some(s) = istate else {
        if !placeholder.is_empty() {
            let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let mut p = Paint::default();
            p.set_color(Theme::alpha(theme.on_surface, 120));
            p.set_anti_alias(true);
            canvas.draw_str(
                placeholder,
                (0.0, size.1 / 2.0 + theme.font_body / 3.0 - pad_y),
                &f,
                &p,
            );
        }
        canvas.restore();
        return;
    };

    // O canvas já está traduzido por pad_x; aplica scroll horizontal.
    canvas.translate((-s.scroll_x, 0.0));

    // Highlight de seleção
    if let Some(sel) = s.selection.filter(|sel| !sel.is_empty()) {
        // Precisamos das posições X do início e fim da seleção.
        // Usamos o buffer de renderização (métricas corretas) abaixo.
        // Por ora: se há seleção, preenche a área visível com cor.
        // A posição exata é refinada após shape, mais abaixo.
        let _ = sel; // usado abaixo após buf.shape_until_scroll
    }

    let text = s.text();

    // ── Placeholder quando sem texto ─────────────────────────
    if text.is_empty() && !placeholder.is_empty() && !is_focused {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(Theme::alpha(theme.on_surface, 120));
        p.set_anti_alias(true);
        canvas.draw_str(
            placeholder,
            (0.0, size.1 / 2.0 + theme.font_body / 3.0 - pad_y),
            &f,
            &p,
        );
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

    // ── Buffer de renderização (métricas corretas = font_body) ──
    let display = if is_password {
        "•".repeat(text.chars().count())
    } else {
        text.clone()
    };

    let mut buf = cosmic_text::Buffer::new(fs, Metrics::new(theme.font_body, theme.font_body * 1.3));
    buf.set_size(fs, Some(10_000.0), Some(size.1));
    buf.set_text(fs, &display, &Attrs::new(), Shaping::Advanced, None);
    buf.shape_until_scroll(fs, false);

    // ── FIX-1: cursor_x calculado do buffer de renderização ──
    //
    // Problema original:
    //   • s.cursor_x() usa o editor interno (Metrics 14px) → posição errada.
    //   • cx era desenhado em `cx - s.scroll_x`, mas o canvas já estava
    //     traduzido por `-s.scroll_x`, causando dupla-subtração.
    //
    // Solução:
    //   • Iterar as glyph runs do `buf` (Metrics font_body = 16px) para
    //     obter a posição X correta.
    //   • Desenhar em `cx` diretamente (canvas já no espaço de texto).
    let cursor_idx = s.cursor_byte_index();
    let mut cx = 0.0_f32;
    for run in buf.layout_runs() {
        if run.line_i == 0 {
            for glyph in run.glyphs.iter() {
                if glyph.start >= cursor_idx {
                    cx = glyph.x;
                    break;
                }
                cx = glyph.x + glyph.w;
            }
        }
    }

    // ── Highlight de seleção (posições corretas) ──────────────
    if let Some(sel) = s.selection.filter(|sel| !sel.is_empty()) {
        let (a, b) = sel.normalized();
        let mut x_start = 0.0_f32;
        let mut x_end = 0.0_f32;
        for run in buf.layout_runs() {
            if run.line_i == 0 {
                for glyph in run.glyphs.iter() {
                    if glyph.start >= a && x_start == 0.0 && a > 0 {
                        x_start = glyph.x;
                    }
                    if glyph.start == 0 && a == 0 {
                        x_start = 0.0;
                    }
                    if glyph.start >= b {
                        x_end = glyph.x;
                        break;
                    }
                    x_end = glyph.x + glyph.w;
                }
            }
        }
        let sel_w = (x_end - x_start).max(0.0);
        let sel_h = theme.font_body + 4.0;
        let sel_y = (size.1 - sel_h) / 2.0 - pad_y;
        let mut sp = Paint::default();
        sp.set_color(Theme::alpha(theme.primary, 60));
        sp.set_anti_alias(true);
        canvas.draw_rect(SkiaRect::from_xywh(x_start, sel_y, sel_w, sel_h), &sp);
    }

    // ── Cursor visual ─────────────────────────────────────────
    if is_focused && cursor_visible {
        let mut cp = Paint::default();
        cp.set_color(theme.primary);
        cp.set_anti_alias(true);
        canvas.draw_rect(
            // FIX-1: cx sem subtrair scroll_x (canvas já traduzido)
            SkiaRect::from_xywh(cx, 2.0, 1.5, theme.font_body + 4.0),
            &cp,
        );
    }

    // ── Texto ─────────────────────────────────────────────────
    for run in buf.layout_runs() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(theme.on_surface);
        p.set_anti_alias(true);
        canvas.draw_str(run.text, (0.0, run.line_y), &f, &p);
    }

    canvas.restore();

    if let Some(msg) = error_msg {
        let ef = get_cached_font(font_cache, "sans-serif", theme.font_small);
        let mut p = Paint::default();
        p.set_color(theme.error);
        p.set_anti_alias(true);
        canvas.draw_str(msg, (4.0, size.1 + theme.spacing * 3.0), &ef, &p);
    }
}

// ── ProgressBar ───────────────────────────────────────────────

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
        // FIX-5: "corrida" da barra indeterminada
        //
        // Problema original:
        //   let fill = from_xywh(start.max(0.0), y, w.min(size.0 - start.max(0.0)), h)
        //   → quando start < 0 (entrando pela esquerda),
        //     start.max(0) = 0, mas a largura era `w` inteiro, logo a
        //     barra aparecia na largura completa logo ao entrar, em vez de
        //     crescer gradualmente.
        //
        // Solução: clampar AMBOS os extremos (início e fim),
        //   de modo que a barra "nasce" pequena à esquerda, cresce até
        //   `w` completo, e depois "encolhe" ao sair pela direita.
        let w = size.0 * 0.30;
        let start = anim_offset * (size.0 + w) - w;

        let clip_start = start.max(0.0);          // não ultrapassa borda esquerda
        let clip_end   = (start + w).min(size.0); // não ultrapassa borda direita
        let visible_w  = (clip_end - clip_start).max(0.0);

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

// ── Modal ─────────────────────────────────────────────────────

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
    cursor_visible: bool,
    theme: &Theme,
    scale: f32,
    size: (f32, f32),
    backdrop_alpha: u8,
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
            theme.radius_md, theme.radius_md,
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
            theme.radius_md, theme.radius_md,
        ),
        &shadow_p,
    );

    canvas.save();
    canvas.translate((card_x, card_y));
    draw_widgets(
        canvas, taffy, ids[0], child, fs, swash,
        Point::new(mouse_pos.x - card_x, mouse_pos.y - card_y),
        focused_id, input_states, widget_states, font_cache,
        cursor_visible, theme, scale,
    );
    canvas.restore();
}

// ── Toast ─────────────────────────────────────────────────────

fn draw_toast(
    canvas: &Canvas,
    message: &str,
    kind: ToastKind,
    progress: f32,
    size: (f32, f32),
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let accent = match kind {
        ToastKind::Info    => theme.primary,
        ToastKind::Success => theme.success,
        ToastKind::Warning => SkiaColor::from_rgb(204, 160, 0),
        ToastKind::Error   => theme.error,
    };

    let pad = 16.0_f32;
    let h   = 48.0_f32;
    let y   = size.1 - h - pad;
    let rect = SkiaRect::from_xywh(pad, y, size.0 - pad * 2.0, h);

    let mut bg = Paint::default();
    bg.set_color(Theme::alpha(SkiaColor::from_rgb(30, 30, 30), 240));
    bg.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(rect, 8.0, 8.0), &bg);

    let mut strip = Paint::default();
    strip.set_color(accent);
    strip.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(SkiaRect::from_xywh(pad, y, 4.0, h), 2.0, 2.0),
        &strip,
    );

    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut tp = Paint::default();
    tp.set_color(SkiaColor::from_rgb(230, 230, 230));
    tp.set_anti_alias(true);
    let ty = y + h / 2.0 + theme.font_body / 3.0;
    canvas.draw_str(message, (pad + 12.0, ty), &f, &tp);

    if progress > 0.0 && progress < 1.0 {
        let bar_w = (size.0 - pad * 2.0) * progress;
        let mut pp = Paint::default();
        pp.set_color(Theme::alpha(accent, 100));
        pp.set_anti_alias(true);
        canvas.draw_rect(SkiaRect::from_xywh(pad, y + h - 3.0, bar_w, 3.0), &pp);
    }
}

// ── VirtualList ───────────────────────────────────────────────

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
    let vis   = (size.1 / ih).ceil() as usize + 1;
    let last  = (first + vis).min(count);

    for i in first..last {
        let y = i as f32 * ih - scroll_y;
        let rect = SkiaRect::from_xywh(0.0, y, size.0 - SCROLLBAR_W - 4.0, ih);
        let is_sel = selected == Some(i);
        let is_hov = hovered == Some(i)
            || SkiaRect::from_xywh(0.0, y, size.0, ih).contains(mouse);

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
            let tc = if is_sel { theme.primary } else { theme.on_surface };
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
        let max_s   = (total_h - size.1).max(1.0);
        let ratio   = (size.1 / total_h).clamp(0.0, 1.0);
        let thumb_h = (size.1 * ratio).max(20.0);
        let thumb_y = (scroll_y / max_s) * (size.1 - thumb_h);
        let sb_x    = size.0 - SCROLLBAR_W - 2.0;

        let mut st = Paint::default();
        st.set_color(Theme::alpha(theme.on_surface, 20));
        st.set_anti_alias(true);
        canvas.draw_rrect(
            RRect::new_rect_xy(SkiaRect::from_xywh(sb_x, 0.0, SCROLLBAR_W, size.1), 4.0, 4.0),
            &st,
        );
        let mut sm = Paint::default();
        sm.set_color(Theme::alpha(theme.on_surface, 70));
        sm.set_anti_alias(true);
        canvas.draw_rrect(
            RRect::new_rect_xy(SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h), 4.0, 4.0),
            &sm,
        );
    }
}

// ── Widgets mantidos da Fase 3 ────────────────────────────────

fn draw_button(
    canvas: &Canvas, text: &str, color: Option<SkiaColor>, variant: ButtonVariant,
    size: (f32, f32), mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>, theme: &Theme,
) {
    let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
    let hovered = rect.contains(mouse);
    let accent  = color.unwrap_or(theme.primary);
    match variant {
        ButtonVariant::Primary => {
            let fill = if hovered { Theme::darken(accent, 0.15) } else { accent };
            let mut p = Paint::default();
            p.set_color(fill);
            p.set_anti_alias(true);
            canvas.draw_rrect(rrect(size, theme.radius_sm), &p);
            draw_text(canvas, text, (0.0, 0.0).into(), size, theme.on_primary,
                      theme.font_body, font_cache, true);
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
            draw_text(canvas, text, (0.0, 0.0).into(), size,
                      if hovered { accent } else { theme.on_surface },
                      theme.font_body, font_cache, true);
        }
        ButtonVariant::Text => {
            draw_text(canvas, text, (0.0, 0.0).into(), size,
                      if hovered { accent } else { Theme::alpha(theme.on_surface, 180) },
                      theme.font_body, font_cache, true);
        }
    }
}

fn draw_checkbox(
    canvas: &Canvas, checked: bool, label: &str, size: (f32, f32), mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>, theme: &Theme,
) {
    let box_size = 18.0_f32;
    let box_rect = SkiaRect::from_xywh(0.0, (size.1 - box_size) / 2.0, box_size, box_size);
    let hovered  = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);
    let fill = if checked { theme.primary }
               else if hovered { Theme::alpha(theme.on_surface, 15) }
               else { SkiaColor::TRANSPARENT };
    if fill != SkiaColor::TRANSPARENT {
        let mut p = Paint::default();
        p.set_color(fill);
        p.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(box_rect, 3.0, 3.0), &p);
    }
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.5);
    border.set_color(if checked { theme.primary } else { Theme::alpha(theme.on_surface, 120) });
    border.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(box_rect, 3.0, 3.0), &border);
    if checked {
        let cx = box_rect.left + box_size / 2.0;
        let cy = box_rect.top  + box_size / 2.0;
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
        canvas.draw_str(label, (box_size + 8.0, size.1 / 2.0 + theme.font_body / 3.0), &f, &p);
    }
}

fn draw_switch(canvas: &Canvas, checked: bool, size: (f32, f32), mouse: Point, theme: &Theme) {
    let track_w = 40.0_f32;
    let track_h = 22.0_f32;
    let thumb_r = 9.0_f32;
    let ty  = (size.1 - track_h) / 2.0;
    let hov = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);
    let mut tp = Paint::default();
    tp.set_color(if checked { theme.primary }
                 else if hov { Theme::alpha(theme.on_surface, 50) }
                 else { Theme::alpha(theme.on_surface, 30) });
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(SkiaRect::from_xywh(0.0, ty, track_w, track_h),
                           track_h / 2.0, track_h / 2.0),
        &tp,
    );
    let thumb_x = if checked { track_w - thumb_r - 3.0 } else { thumb_r + 3.0 };
    let mut bp = Paint::default();
    bp.set_color(if checked { theme.on_primary } else { theme.on_surface });
    bp.set_anti_alias(true);
    canvas.draw_circle((thumb_x, ty + track_h / 2.0), thumb_r, &bp);
}

fn draw_radio(
    canvas: &Canvas, selected: bool, label: &str, size: (f32, f32), mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>, theme: &Theme,
) {
    let r  = 9.0_f32;
    let cx = r;
    let cy = size.1 / 2.0;
    let hov = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);
    let mut bp = Paint::default();
    bp.set_style(paint::Style::Stroke);
    bp.set_stroke_width(2.0);
    bp.set_color(if selected { theme.primary } else { Theme::alpha(theme.on_surface, 120) });
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
}

fn draw_slider(
    canvas: &Canvas, value: f32, min: f32, max: f32, size: (f32, f32),
    mouse: Point, is_dragging: bool, theme: &Theme,
) {
    let pad     = 16.0_f32;
    let track_y = size.1 / 2.0;
    let track_h = 4.0_f32;
    let thumb_r = 8.0_f32;
    let norm    = ((value - min) / (max - min).max(f32::EPSILON)).clamp(0.0, 1.0);
    let track_w = size.0 - pad * 2.0;
    let thumb_x = pad + norm * track_w;
    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);

    let mut tp = Paint::default();
    tp.set_color(Theme::alpha(theme.on_surface, 40));
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(thumb_x, track_y - track_h / 2.0, size.0 - pad - thumb_x, track_h),
            track_h / 2.0, track_h / 2.0,
        ),
        &tp,
    );
    let mut ap = Paint::default();
    ap.set_color(theme.primary);
    ap.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(pad, track_y - track_h / 2.0, thumb_x - pad, track_h),
            track_h / 2.0, track_h / 2.0,
        ),
        &ap,
    );
    let tr = if hovered || is_dragging { thumb_r + 2.0 } else { thumb_r };
    let tc = if is_dragging { Theme::darken(theme.primary, 0.15) } else { theme.primary };
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
}

fn draw_spinner(canvas: &Canvas, angle_deg: f32, size: (f32, f32), theme: &Theme) {
    let cx = size.0 / 2.0;
    let cy = size.1 / 2.0;
    let r  = (size.0.min(size.1) / 2.0 - 3.0).max(4.0);
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
        Orientation::Horizontal =>
            canvas.draw_line((0.0, size.1 / 2.0), (size.0, size.1 / 2.0), &p),
        Orientation::Vertical =>
            canvas.draw_line((size.0 / 2.0, 0.0), (size.0 / 2.0, size.1), &p),
    };
}

fn draw_image(canvas: &Canvas, data: &[u8], size: (f32, f32), radius: f32) {
    use image::ImageReader;
    use skia_safe::{AlphaType, Bitmap, ColorType, ImageInfo, Matrix, images};
    use std::io::Cursor;
    let Ok(dyn_img) = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .and_then(|r| Ok(r.decode().ok()))
    else { return; };
    let Some(img) = dyn_img else { return; };
    let rgba = img.to_rgba8();
    let (iw, ih) = (rgba.width() as i32, rgba.height() as i32);
    let raw = rgba.into_raw();
    let mut bmp = Bitmap::new();
    if !bmp.set_info(&ImageInfo::new((iw, ih), ColorType::RGBA8888, AlphaType::Premul, None), None) {
        return;
    }
    bmp.alloc_pixels();
    let pixels = bmp.pixels();
    if !pixels.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(raw.as_ptr(), pixels as *mut u8, raw.len()); }
    }
    let Some(sk_img) = images::raster_from_bitmap(&bmp) else { return; };
    if radius > 0.0 {
        canvas.save();
        canvas.clip_rrect(
            RRect::new_rect_xy(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), radius, radius),
            None, true,
        );
    }
    let m = Matrix::scale((size.0 / iw as f32, size.1 / ih as f32));
    canvas.save();
    canvas.concat(&m);
    canvas.draw_image(&sk_img, (0.0_f32, 0.0_f32), Some(&Paint::default()));
    canvas.restore();
    if radius > 0.0 { canvas.restore(); }
}

fn draw_select(
    canvas: &Canvas, options: &[&str], selected_index: usize, is_open: bool,
    hovered_option: Option<usize>, label: &str, placeholder: &str,
    size: (f32, f32), mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>, theme: &Theme,
) {
    let closed_h = size.1 - if is_open { options.len() as f32 * OPTION_HEIGHT } else { 0.0 };
    let hovered  = SkiaRect::from_xywh(0.0, 0.0, size.0, closed_h).contains(mouse);
    let mut bg = Paint::default();
    bg.set_color(if hovered { Theme::alpha(theme.on_surface, 10) } else { theme.surface });
    bg.set_anti_alias(true);
    canvas.draw_rrect(rrect((size.0, closed_h), theme.radius_sm), &bg);
    let mut brd = Paint::default();
    brd.set_style(paint::Style::Stroke);
    brd.set_stroke_width(1.0);
    brd.set_color(if is_open { theme.primary } else { Theme::alpha(theme.on_surface, 80) });
    brd.set_anti_alias(true);
    canvas.draw_rrect(rrect((size.0, closed_h), theme.radius_sm), &brd);
    if !label.is_empty() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_label);
        let mut p = Paint::default();
        p.set_color(if is_open { theme.primary } else { Theme::alpha(theme.on_surface, 160) });
        p.set_anti_alias(true);
        canvas.draw_str(label, (6.0, -4.0), &f, &p);
    }
    let display = options.get(selected_index).copied().unwrap_or(placeholder);
    let tc = if display == placeholder { Theme::alpha(theme.on_surface, 100) } else { theme.on_surface };
    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut tp = Paint::default();
    tp.set_color(tc);
    tp.set_anti_alias(true);
    canvas.draw_str(display, (8.0, closed_h / 2.0 + theme.font_body / 3.0), &f, &tp);
    let chevron = if is_open { "▲" } else { "▼" };
    let cf = get_cached_font(font_cache, "sans-serif", 11.0);
    let mut cp = Paint::default();
    cp.set_color(Theme::alpha(theme.on_surface, 160));
    cp.set_anti_alias(true);
    let cw = cf.measure_str(chevron, Some(&cp)).0;
    canvas.draw_str(chevron, (size.0 - cw - 8.0, closed_h / 2.0 + theme.font_body / 3.0), &cf, &cp);
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
                canvas.draw_rect(SkiaRect::from_xywh(1.0, oy, size.0 - 2.0, OPTION_HEIGHT), &ip);
            }
            let ot = if i == selected_index { theme.primary } else { theme.on_surface };
            let mut op = Paint::default();
            op.set_color(ot);
            op.set_anti_alias(true);
            canvas.draw_str(opt, (8.0, oy + OPTION_HEIGHT / 2.0 + theme.font_body / 3.0), &f, &op);
        }
    }
}

fn draw_tooltip_popup(
    canvas: &Canvas, text: &str, mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>, theme: &Theme,
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
            3.0, 3.0,
        ),
        &bg,
    );
    canvas.draw_str(text, (mouse.x + 12.0 + pad, mouse.y - 4.0 - pad), &f, &tp);
}

fn draw_scrollbar(
    canvas: &Canvas, size: (f32, f32),
    state: Option<&crate::engine::widget_state::ScrollState>, theme: &Theme,
) {
    let Some(s) = state else { return; };
    if s.content_height <= s.viewport_h { return; }
    let ratio   = s.thumb_ratio();
    let thumb_h = (size.1 * ratio).max(20.0);
    let sb_x    = size.0 - SCROLLBAR_W - 2.0;
    let mut tp = Paint::default();
    tp.set_color(Theme::alpha(theme.on_surface, 20));
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(SkiaRect::from_xywh(sb_x, 0.0, SCROLLBAR_W, size.1),
                           SCROLLBAR_W / 2.0, SCROLLBAR_W / 2.0),
        &tp,
    );
    let mut sp = Paint::default();
    sp.set_color(Theme::alpha(theme.on_surface, 80));
    sp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(SkiaRect::from_xywh(sb_x, s.thumb_y(), SCROLLBAR_W, thumb_h),
                           SCROLLBAR_W / 2.0, SCROLLBAR_W / 2.0),
        &sp,
    );
}

fn rrect(size: (f32, f32), r: f32) -> RRect {
    RRect::new_rect_xy(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), r, r)
}
