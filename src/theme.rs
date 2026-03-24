// ============================================================
// Rutter Framework — theme.rs
// Tokens de design centralizados. Futuramente será injetado
// via thread_local! para que widgets o leiam sem parâmetros
// extras. Por hora é passado explicitamente.
// ============================================================

use skia_safe::Color as SkiaColor;
use crate::widget::InputState;

/// Tokens de design do tema visual do framework.
/// Use `Theme::default()` para o tema Material-inspired padrão.
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Cores primárias ──────────────────────────────────────
    pub primary:    SkiaColor,
    pub on_primary: SkiaColor,

    // ── Superfície ───────────────────────────────────────────
    pub surface:    SkiaColor,
    pub on_surface: SkiaColor,

    // ── Semânticas ───────────────────────────────────────────
    pub error:      SkiaColor,
    pub success:    SkiaColor,

    // ── Tipografia ───────────────────────────────────────────
    pub font_body:  f32,
    pub font_label: f32,
    pub font_small: f32,

    // ── Forma ────────────────────────────────────────────────
    pub radius_sm:  f32,
    pub radius_md:  f32,

    // ── Espaçamento base (múltiplos de spacing) ──────────────
    pub spacing:    f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary:    SkiaColor::from_rgb(103, 80, 164), // Material Purple
            on_primary: SkiaColor::WHITE,
            surface:    SkiaColor::WHITE,
            on_surface: SkiaColor::BLACK,
            error:      SkiaColor::from_rgb(186, 26,  26),
            success:    SkiaColor::from_rgb( 52, 168,  83),
            font_body:  16.0,
            font_label: 12.0,
            font_small: 11.0,
            radius_sm:   6.0,
            radius_md:   8.0,
            spacing:     8.0,
        }
    }
}

impl Theme {
    pub fn darken(color: SkiaColor, amount: f32) -> SkiaColor {
        let r = (color.r() as f32 * (1.0 - amount)).clamp(0.0, 255.0) as u8;
        let g = (color.g() as f32 * (1.0 - amount)).clamp(0.0, 255.0) as u8;
        let b = (color.b() as f32 * (1.0 - amount)).clamp(0.0, 255.0) as u8;
        SkiaColor::from_argb(color.a(), r, g, b)
    }

    pub fn alpha(color: SkiaColor, alpha: u8) -> SkiaColor {
        SkiaColor::from_argb(alpha, color.r(), color.g(), color.b())
    }

    pub fn input_border(&self, state: InputState, is_focused: bool) -> SkiaColor {
        match state {
            InputState::Error => self.error,
            InputState::Success => self.success,
            _ if is_focused => self.primary,
            _ => Self::alpha(self.on_surface, 100),
        }
    }

    pub fn dark() -> Self {
        Self {
            primary:    SkiaColor::from_rgb(168, 199, 250),
            on_primary: SkiaColor::from_rgb(6, 46, 111),
            surface:    SkiaColor::from_rgb(30, 30, 30),
            on_surface: SkiaColor::from_rgb(227, 227, 227),
            error:      SkiaColor::from_rgb(255, 180, 171),
            success:    SkiaColor::from_rgb(129, 201, 149),
            ..Default::default()
        }
    }
}