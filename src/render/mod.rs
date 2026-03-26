// ============================================================
// Rutter Framework — render/mod.rs  (Fase 3)
//
// Novos draw functions:
//   draw_checkbox, draw_switch, draw_radio, draw_slider,
//   draw_progress_bar, draw_spinner, draw_divider,
//   draw_scroll_view, draw_select, draw_tooltip, draw_image
// ============================================================

pub mod hit_test;
pub mod pipeline;
pub mod text;

use std::collections::HashMap;

use cosmic_text::{Attrs, FontSystem, Metrics, Shaping, SwashCache};
use skia_safe::{
    Color as SkiaColor, Contains, Font, Matrix, Paint, Point, RRect, Rect as SkiaRect,
    canvas::Canvas, paint,
};
use taffy::prelude::{NodeId, TaffyTree};

use self::text::{draw_text, get_cached_font};
use crate::engine::widget_state::WidgetState;
use crate::input_state::InputWidgetState;
use crate::layout::{OPTION_HEIGHT, RutterContext, SCROLLBAR_W};
use crate::theme::Theme;
use crate::widget::{ButtonVariant, InputState, Orientation, Widget};

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
                    cursor_visible,
                    theme,
                    scale,
                );
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
            draw_widgets(
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
                cursor_visible,
                theme,
                scale,
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
                cursor_visible,
                theme,
                scale,
            );
            canvas.restore();

            // Scrollbar
            if content_h > size.1 {
                draw_scrollbar(canvas, size, scroll_state, theme);
            }
        }

        Widget::Tooltip { child, text, .. } => {
            let ids = taffy.children(node).unwrap();
            draw_widgets(
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
                cursor_visible,
                theme,
                scale,
            );

            // Mostrar tooltip se mouse dentro do bounding box
            let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
            if rect.contains(local_mouse) {
                draw_tooltip_popup(canvas, text, local_mouse, font_cache, theme);
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
            size,
            local_mouse,
            font_cache,
            theme,
        ),

        Widget::TextInput {
            id,
            label,
            placeholder,
            state,
            error_msg,
            is_password,
            ..
        } => {
            draw_text_input(
                canvas,
                fs,
                swash,
                font_cache,
                theme,
                scale,
                size,
                focused_id == Some(*id),
                label,
                placeholder,
                *state,
                error_msg.as_deref(),
                *is_password,
                input_states.get(id),
                cursor_visible,
            );
        }

        Widget::Checkbox { checked, label, .. } => draw_checkbox(
            canvas,
            *checked,
            label,
            size,
            local_mouse,
            font_cache,
            theme,
        ),

        Widget::Switch { checked, .. } => draw_switch(canvas, *checked, size, local_mouse, theme),

        Widget::Radio {
            selected, label, ..
        } => draw_radio(
            canvas,
            *selected,
            label,
            size,
            local_mouse,
            font_cache,
            theme,
        ),

        Widget::Slider {
            id,
            value,
            min,
            max,
            ..
        } => {
            let is_dragging = widget_states
                .get(id)
                .and_then(|s| s.as_slider())
                .map(|s| s.dragging)
                .unwrap_or(false);
            draw_slider(
                canvas,
                *value,
                *min,
                *max,
                size,
                local_mouse,
                is_dragging,
                theme,
            );
        }

        Widget::ProgressBar {
            value,
            indeterminate,
            ..
        } => {
            let anim_offset = if *indeterminate {
                widget_states
                    .values()
                    .find_map(|s| s.as_anim())
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

        Widget::Spacer { .. } => {} // Apenas ocupa espaço no layout

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
            id,
            options,
            selected_index,
            label,
            placeholder,
            ..
        } => {
            let sel_state = widget_states.get(id).and_then(|s| s.as_select());
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
                size,
                local_mouse,
                font_cache,
                theme,
            );
        }
    }

    canvas.restore();
}

// ── Button ────────────────────────────────────────────────────

fn draw_button(
    canvas: &Canvas,
    text: &str,
    color: Option<SkiaColor>,
    variant: ButtonVariant,
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
            let tc = if hovered { accent } else { theme.on_surface };
            draw_text(
                canvas,
                text,
                (0.0, 0.0).into(),
                size,
                tc,
                theme.font_body,
                font_cache,
                true,
            );
        }
        ButtonVariant::Text => {
            let tc = if hovered {
                accent
            } else {
                Theme::alpha(theme.on_surface, 180)
            };
            draw_text(
                canvas,
                text,
                (0.0, 0.0).into(),
                size,
                tc,
                theme.font_body,
                font_cache,
                true,
            );
        }
    }
}

// ── Checkbox ─────────────────────────────────────────────────

fn draw_checkbox(
    canvas: &Canvas,
    checked: bool,
    label: &str,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let box_size = 18.0_f32;
    let box_rect = SkiaRect::from_xywh(0.0, (size.1 - box_size) / 2.0, box_size, box_size);
    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);

    // Fundo
    let fill = if checked {
        theme.primary
    } else {
        if hovered {
            Theme::alpha(theme.on_surface, 15)
        } else {
            SkiaColor::TRANSPARENT
        }
    };
    if fill != SkiaColor::TRANSPARENT {
        let mut p = Paint::default();
        p.set_color(fill);
        p.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(box_rect, 3.0, 3.0), &p);
    }

    // Borda
    let border_c = if checked {
        theme.primary
    } else {
        Theme::alpha(theme.on_surface, 120)
    };
    let mut border = Paint::default();
    border.set_style(paint::Style::Stroke);
    border.set_stroke_width(1.5);
    border.set_color(border_c);
    border.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(box_rect, 3.0, 3.0), &border);

    // Checkmark ✓
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

    // Label
    if !label.is_empty() {
        let lc = Theme::alpha(theme.on_surface, 220);
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(lc);
        p.set_anti_alias(true);
        let y = size.1 / 2.0 + theme.font_body / 3.0;
        canvas.draw_str(label, (box_size + 8.0, y), &f, &p);
    }
}

// ── Switch ───────────────────────────────────────────────────

fn draw_switch(canvas: &Canvas, checked: bool, size: (f32, f32), mouse: Point, theme: &Theme) {
    let track_w = 40.0_f32;
    let track_h = 22.0_f32;
    let thumb_r = 9.0_f32;
    let tx = 0.0_f32;
    let ty = (size.1 - track_h) / 2.0;
    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);

    // Track
    let track_col = if checked {
        theme.primary
    } else if hovered {
        Theme::alpha(theme.on_surface, 50)
    } else {
        Theme::alpha(theme.on_surface, 30)
    };
    let mut tp = Paint::default();
    tp.set_color(track_col);
    tp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(tx, ty, track_w, track_h),
            track_h / 2.0,
            track_h / 2.0,
        ),
        &tp,
    );

    // Thumb
    let thumb_x = if checked {
        tx + track_w - thumb_r - 3.0
    } else {
        tx + thumb_r + 3.0
    };
    let thumb_y = ty + track_h / 2.0;
    let thumb_col = if checked {
        theme.on_primary
    } else {
        theme.on_surface
    };
    let mut bp = Paint::default();
    bp.set_color(thumb_col);
    bp.set_anti_alias(true);
    canvas.draw_circle((thumb_x, thumb_y), thumb_r, &bp);
}

// ── Radio ─────────────────────────────────────────────────────

fn draw_radio(
    canvas: &Canvas,
    selected: bool,
    label: &str,
    size: (f32, f32),
    mouse: Point,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
    let r = 9.0_f32;
    let cx = r;
    let cy = size.1 / 2.0;
    let hov = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);

    // Outer circle
    let border_c = if selected {
        theme.primary
    } else {
        Theme::alpha(theme.on_surface, 120)
    };
    let mut bp = Paint::default();
    bp.set_style(paint::Style::Stroke);
    bp.set_stroke_width(2.0);
    bp.set_color(border_c);
    bp.set_anti_alias(true);
    canvas.draw_circle((cx, cy), r, &bp);

    // Inner dot
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

    // Label
    if !label.is_empty() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
        let mut p = Paint::default();
        p.set_color(theme.on_surface);
        p.set_anti_alias(true);
        let y = cy + theme.font_body / 3.0;
        canvas.draw_str(label, (r * 2.0 + 8.0, y), &f, &p);
    }
}

// ── Slider ────────────────────────────────────────────────────

fn draw_slider(
    canvas: &Canvas,
    value: f32,
    min: f32,
    max: f32,
    size: (f32, f32),
    mouse: Point,
    is_dragging: bool,
    theme: &Theme,
) {
    let pad = 16.0_f32;
    let track_y = size.1 / 2.0;
    let track_h = 4.0_f32;
    let thumb_r = 8.0_f32;

    let range = (max - min).max(f32::EPSILON);
    let norm = ((value - min) / range).clamp(0.0, 1.0);
    let track_w = size.0 - pad * 2.0;
    let thumb_x = pad + norm * track_w;

    let hovered = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1).contains(mouse);

    // Track inativo (direita do thumb)
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

    // Track ativo (esquerda do thumb)
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

    // Thumb
    let thumb_r_draw = if hovered || is_dragging {
        thumb_r + 2.0
    } else {
        thumb_r
    };
    let thumb_c = if is_dragging {
        Theme::darken(theme.primary, 0.15)
    } else {
        theme.primary
    };
    let mut thp = Paint::default();
    thp.set_color(thumb_c);
    thp.set_anti_alias(true);
    canvas.draw_circle((thumb_x, track_y), thumb_r_draw, &thp);

    // Halo de foco
    if hovered || is_dragging {
        let mut hp = Paint::default();
        hp.set_color(Theme::alpha(theme.primary, 30));
        hp.set_anti_alias(true);
        canvas.draw_circle((thumb_x, track_y), thumb_r_draw + 4.0, &hp);
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

    // Track de fundo
    let mut tp = Paint::default();
    tp.set_color(Theme::alpha(theme.primary, 30));
    tp.set_anti_alias(true);
    canvas.draw_rrect(RRect::new_rect_xy(track, h / 2.0, h / 2.0), &tp);

    // Barra preenchida
    let mut fp = Paint::default();
    fp.set_color(theme.primary);
    fp.set_anti_alias(true);

    if indeterminate {
        // Anima uma faixa de 30% percorrendo o track
        let w = size.0 * 0.30;
        let start = (anim_offset * (size.0 + w)) - w;
        let fill = SkiaRect::from_xywh(start.max(0.0), y, w.min(size.0 - start.max(0.0)), h);
        canvas.save();
        canvas.clip_rect(track, None, true);
        canvas.draw_rrect(RRect::new_rect_xy(fill, h / 2.0, h / 2.0), &fp);
        canvas.restore();
    } else {
        let fill_w = (value.clamp(0.0, 1.0) * size.0).max(0.0);
        let fill = SkiaRect::from_xywh(0.0, y, fill_w, h);
        canvas.draw_rrect(RRect::new_rect_xy(fill, h / 2.0, h / 2.0), &fp);
    }
}

// ── Spinner ───────────────────────────────────────────────────

fn draw_spinner(canvas: &Canvas, angle_deg: f32, size: (f32, f32), theme: &Theme) {
    let cx = size.0 / 2.0;
    let cy = size.1 / 2.0;
    let r = (size.0.min(size.1) / 2.0 - 3.0).max(4.0);

    // Track
    let mut tp = Paint::default();
    tp.set_style(paint::Style::Stroke);
    tp.set_stroke_width(3.0);
    tp.set_color(Theme::alpha(theme.primary, 30));
    tp.set_anti_alias(true);
    canvas.draw_circle((cx, cy), r, &tp);

    // Arco girante (~270° de abertura)
    let sweep = 270.0_f32;
    let start = angle_deg - 90.0; // começa de cima
    let arc = SkiaRect::from_xywh(cx - r, cy - r, r * 2.0, r * 2.0);

    let mut ap = Paint::default();
    ap.set_style(paint::Style::Stroke);
    ap.set_stroke_width(3.0);
    ap.set_color(theme.primary);
    ap.set_anti_alias(true);
    ap.set_stroke_cap(paint::Cap::Round);

    canvas.draw_arc(arc, start, sweep, false, &ap);
}

// ── Divider ───────────────────────────────────────────────────

fn draw_divider(canvas: &Canvas, orientation: Orientation, size: (f32, f32), theme: &Theme) {
    let mut p = Paint::default();
    p.set_color(Theme::alpha(theme.on_surface, 30));
    p.set_stroke_width(1.0);
    p.set_anti_alias(true);
    p.set_style(paint::Style::Stroke);
    match orientation {
        Orientation::Horizontal => {
            canvas.draw_line((0.0, size.1 / 2.0), (size.0, size.1 / 2.0), &p)
        }
        Orientation::Vertical => canvas.draw_line((size.0 / 2.0, 0.0), (size.0 / 2.0, size.1), &p),
    };
}

// ── Image ─────────────────────────────────────────────────────

fn draw_image(canvas: &Canvas, data: &[u8], size: (f32, f32), radius: f32) {
    // Decodificar via crate `image`
    use image::ImageReader;
    use skia_safe::{AlphaType, Bitmap, ColorType, ImageInfo, images};
    use std::io::Cursor;

    let Ok(dyn_img) = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .and_then(|r| Ok(r.decode().ok()))
    else {
        return;
    };
    let Some(img) = dyn_img else {
        return;
    };

    let rgba = img.to_rgba8();
    let (iw, ih) = (rgba.width() as i32, rgba.height() as i32);
    let raw = rgba.into_raw();

    let mut bmp = Bitmap::new();
    let info = ImageInfo::new((iw, ih), ColorType::RGBA8888, AlphaType::Premul, None);
    if !bmp.set_info(&info, None) {
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

    // Clipar com radius
    if radius > 0.0 {
        canvas.save();
        let rrect = RRect::new_rect_xy(
            SkiaRect::from_xywh(0.0, 0.0, size.0, size.1),
            radius, radius
        );
        canvas.clip_rrect(rrect, None, true);
    }

    // Escalar para preencher o widget
    let sx = size.0 / iw as f32;
    let sy = size.1 / ih as f32;
    let m = Matrix::scale((sx, sy));
    let p = Paint::default();
    canvas.save();
    canvas.concat(&m);
    canvas.draw_image(&sk_img, (0.0_f32, 0.0_f32), Some(&p));
    canvas.restore();

    if radius > 0.0 {
        canvas.restore();
    }
}

// ── Select ────────────────────────────────────────────────────

fn draw_select(
    canvas: &Canvas,
    options: &[&str],
    selected_index: usize,
    is_open: bool,
    hovered_option: Option<usize>,
    label: &str,
    placeholder: &str,
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
    let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, closed_h);
    let hovered = rect.contains(mouse);

    // Fundo do header
    let bg_c = if hovered {
        Theme::alpha(theme.on_surface, 10)
    } else {
        theme.surface
    };
    let mut bp = Paint::default();
    bp.set_color(bg_c);
    bp.set_anti_alias(true);
    canvas.draw_rrect(
        if is_open {
            RRect::new_rect_xy(
                SkiaRect::from_xywh(0.0, 0.0, size.0, closed_h),
                theme.radius_sm,
                theme.radius_sm,
            )
        } else {
            rrect((size.0, closed_h), theme.radius_sm)
        },
        &bp,
    );

    // Borda
    let mut brd = Paint::default();
    brd.set_style(paint::Style::Stroke);
    brd.set_stroke_width(1.0);
    brd.set_color(if is_open {
        theme.primary
    } else {
        Theme::alpha(theme.on_surface, 80)
    });
    brd.set_anti_alias(true);
    canvas.draw_rrect(rrect((size.0, closed_h), theme.radius_sm), &brd);

    // Label acima
    if !label.is_empty() {
        let f = get_cached_font(font_cache, "sans-serif", theme.font_label);
        let lc = if is_open {
            theme.primary
        } else {
            Theme::alpha(theme.on_surface, 160)
        };
        let mut p = Paint::default();
        p.set_color(lc);
        p.set_anti_alias(true);
        canvas.draw_str(label, (6.0, -4.0), &f, &p);
    }

    // Texto selecionado ou placeholder
    let display = if options.is_empty() {
        placeholder
    } else {
        options.get(selected_index).copied().unwrap_or(placeholder)
    };
    let tc = if display == placeholder {
        Theme::alpha(theme.on_surface, 100)
    } else {
        theme.on_surface
    };
    let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
    let mut tp = Paint::default();
    tp.set_color(tc);
    tp.set_anti_alias(true);
    let ty = closed_h / 2.0 + theme.font_body / 3.0;
    canvas.draw_str(display, (8.0, ty), &f, &tp);

    // Chevron ▼ / ▲
    let chevron = if is_open { "▲" } else { "▼" };
    let mut cp = Paint::default();
    cp.set_color(Theme::alpha(theme.on_surface, 160));
    cp.set_anti_alias(true);
    let cf = get_cached_font(font_cache, "sans-serif", 11.0);
    let cw = cf.measure_str(chevron, Some(&cp)).0;
    canvas.draw_str(chevron, (size.0 - cw - 8.0, ty), &cf, &cp);

    // Opções quando aberto
    if is_open {
        // Fundo do dropdown
        let dd_rect = SkiaRect::from_xywh(0.0, closed_h, size.0, size.1 - closed_h);
        let mut dbp = Paint::default();
        dbp.set_color(theme.surface);
        dbp.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(dd_rect, 0.0, theme.radius_sm), &dbp);

        let mut dbrd = Paint::default();
        dbrd.set_style(paint::Style::Stroke);
        dbrd.set_stroke_width(1.0);
        dbrd.set_color(Theme::alpha(theme.on_surface, 80));
        dbrd.set_anti_alias(true);
        canvas.draw_rrect(RRect::new_rect_xy(dd_rect, 0.0, theme.radius_sm), &dbrd);

        for (i, opt) in options.iter().enumerate() {
            let oy = closed_h + i as f32 * OPTION_HEIGHT;
            let is_sel = i == selected_index;
            let is_hov = hovered_option == Some(i);

            // Fundo do item
            if is_sel || is_hov {
                let item_c = if is_sel {
                    Theme::alpha(theme.primary, 30)
                } else {
                    Theme::alpha(theme.on_surface, 10)
                };
                let mut ip = Paint::default();
                ip.set_color(item_c);
                ip.set_anti_alias(true);
                canvas.draw_rect(
                    SkiaRect::from_xywh(1.0, oy, size.0 - 2.0, OPTION_HEIGHT),
                    &ip,
                );
            }

            let ot = if is_sel {
                theme.primary
            } else {
                theme.on_surface
            };
            let mut op = Paint::default();
            op.set_color(ot);
            op.set_anti_alias(true);
            let iy = oy + OPTION_HEIGHT / 2.0 + theme.font_body / 3.0;
            canvas.draw_str(opt, (8.0, iy), &f, &op);
        }
    }
}

// ── Tooltip popup ────────────────────────────────────────────

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
    let tt_x = mouse.x + 12.0;
    let tt_y = mouse.y - tt_h - 4.0;

    // Fundo
    let mut bg = Paint::default();
    bg.set_color(Theme::alpha(theme.on_surface, 220));
    bg.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(SkiaRect::from_xywh(tt_x, tt_y, tt_w, tt_h), 3.0, 3.0),
        &bg,
    );

    // Texto
    canvas.draw_str(
        text,
        (tt_x + pad, tt_y + pad + theme.font_small * 0.8),
        &f,
        &tp,
    );
}

// ── Scrollbar ────────────────────────────────────────────────

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

    let sb_x = size.0 - SCROLLBAR_W - 2.0;
    let ratio = s.thumb_ratio();
    let thumb_h = (size.1 * ratio).max(20.0);
    let thumb_y = s.thumb_y();

    // Track
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

    // Thumb
    let mut sp = Paint::default();
    sp.set_color(Theme::alpha(theme.on_surface, 80));
    sp.set_anti_alias(true);
    canvas.draw_rrect(
        RRect::new_rect_xy(
            SkiaRect::from_xywh(sb_x, thumb_y, SCROLLBAR_W, thumb_h),
            SCROLLBAR_W / 2.0,
            SCROLLBAR_W / 2.0,
        ),
        &sp,
    );
}

// ── TextInput (mantido da Fase 2) ─────────────────────────────

#[allow(clippy::too_many_arguments)]
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
    let border_w = if is_focused { 1.5 } else { 1.0 };

    let mut bg = Paint::default();
    bg.set_color(theme.surface);
    bg.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &bg);

    let mut brd = Paint::default();
    brd.set_style(paint::Style::Stroke);
    brd.set_stroke_width(border_w);
    brd.set_color(border_c);
    brd.set_anti_alias(true);
    canvas.draw_rrect(rrect(size, theme.radius_sm), &brd);

    if !label.is_empty() {
        let lf = get_cached_font(font_cache, "sans-serif", theme.font_label);
        let lc = if is_focused {
            theme.primary
        } else {
            Theme::alpha(theme.on_surface, 180)
        };
        let mut p = Paint::default();
        p.set_color(lc);
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
            draw_placeholder(canvas, placeholder, size, pad_y, font_cache, theme);
        }
        canvas.restore();
        return;
    };

    canvas.translate((-s.scroll_x, 0.0));

    if s.selection.map(|sel| !sel.is_empty()).unwrap_or(false) {
        let h = theme.font_body + 4.0;
        let y = (size.1 - h) / 2.0 - pad_y;
        let mut sp = Paint::default();
        sp.set_color(Theme::alpha(theme.primary, 60));
        sp.set_anti_alias(true);
        canvas.draw_rect(SkiaRect::from_xywh(0.0, y, size.0 - 8.0, h), &sp);
    }

    let text = s.text();
    if text.is_empty() && !placeholder.is_empty() && !is_focused {
        draw_placeholder(canvas, placeholder, size, pad_y, font_cache, theme);
    } else {
        if is_focused && cursor_visible {
            let cx = s.cursor_x();
            let mut cp = Paint::default();
            cp.set_color(theme.primary);
            canvas.draw_rect(
                SkiaRect::from_xywh(cx - s.scroll_x, 2.0, 1.5, theme.font_body + 4.0),
                &cp,
            );
        }
        let display = if is_password {
            "•".repeat(text.chars().count())
        } else {
            text
        };
        let mut buf =
            cosmic_text::Buffer::new(fs, Metrics::new(theme.font_body, theme.font_body * 1.3));
        buf.set_size(fs, Some(10_000.0), Some(size.1));
        buf.set_text(fs, &display, &Attrs::new(), Shaping::Advanced, None);
        buf.shape_until_scroll(fs, false);
        for run in buf.layout_runs() {
            let f = get_cached_font(font_cache, "sans-serif", theme.font_body);
            let mut p = Paint::default();
            p.set_color(theme.on_surface);
            p.set_anti_alias(true);
            canvas.draw_str(run.text, (0.0, run.line_y), &f, &p);
        }
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

fn draw_placeholder(
    canvas: &Canvas,
    placeholder: &str,
    size: (f32, f32),
    pad_y: f32,
    font_cache: &mut HashMap<(String, u32), Font>,
    theme: &Theme,
) {
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

// ── Helpers ───────────────────────────────────────────────────

fn rrect(size: (f32, f32), r: f32) -> RRect {
    RRect::new_rect_xy(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), r, r)
}
