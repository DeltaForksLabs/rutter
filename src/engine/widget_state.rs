// ============================================================
// Rutter Framework — engine/widget_state.rs  (Fase 3)
//
// Estado interno owned pelo RutterEngine para widgets
// que precisam de dados entre frames:
//
//   SliderState    — posição do thumb + drag
//   ScrollState    — offset vertical de scroll
//   SelectState    — aberto/fechado
//   SpinnerState   — ângulo de animação (graus)
//   AnimProgress   — offset da barra indeterminada
// ============================================================

use std::time::Instant;

// ── Slider ───────────────────────────────────────────────────

/// Estado de drag de um Slider.
#[derive(Debug, Clone)]
pub struct SliderState {
    /// true enquanto o mouse está pressionado sobre o track
    pub dragging:          bool,
    /// posição X absoluta do cursor no início do drag
    pub drag_start_cursor: f32,
    /// valor no início do drag
    pub drag_start_value:  f32,
    /// posição X absoluta do início do track (atualizada no render)
    pub track_abs_x:       f32,
    /// largura do track em px lógicos (atualizada no render)
    pub track_width:       f32,
}

impl Default for SliderState {
    fn default() -> Self {
        Self {
            dragging:          false,
            drag_start_cursor: 0.0,
            drag_start_value:  0.0,
            track_abs_x:       0.0,
            track_width:       1.0,
        }
    }
}

impl SliderState {
    /// Calcula o valor normalizado (0.0–1.0) a partir da posição
    /// absoluta do cursor `abs_x`, respeitando os bounds do track.
    pub fn value_from_cursor(&self, abs_x: f32) -> f32 {
        if self.track_width <= 0.0 { return 0.0; }
        ((abs_x - self.track_abs_x) / self.track_width).clamp(0.0, 1.0)
    }
}

// ── ScrollView ───────────────────────────────────────────────

/// Estado de scroll de um ScrollView.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    /// Deslocamento vertical em px lógicos (positivo = scroll para baixo).
    pub offset_y:       f32,
    /// Altura total do conteúdo (atualizada no render).
    pub content_height: f32,
    /// Altura da viewport (atualizada no render).
    pub viewport_h:     f32,
}

impl ScrollState {
    /// Retorna o offset máximo permitido (não rola além do conteúdo).
    pub fn max_offset(&self) -> f32 {
        (self.content_height - self.viewport_h).max(0.0)
    }

    /// Aplica um delta de scroll com clamp nos bounds.
    pub fn scroll_by(&mut self, delta_y: f32) {
        self.offset_y = (self.offset_y + delta_y).clamp(0.0, self.max_offset());
    }

    /// Proporção visível (0.0–1.0) — usada para dimensionar o thumb.
    pub fn thumb_ratio(&self) -> f32 {
        if self.content_height <= 0.0 { return 1.0; }
        (self.viewport_h / self.content_height).clamp(0.0, 1.0)
    }

    /// Posição Y do thumb (0.0 = topo, `viewport_h` = fundo).
    pub fn thumb_y(&self) -> f32 {
        if self.max_offset() <= 0.0 { return 0.0; }
        (self.offset_y / self.max_offset())
            * (self.viewport_h * (1.0 - self.thumb_ratio()))
    }
}

// ── Select ───────────────────────────────────────────────────

/// Estado de abertura de um Select (dropdown).
#[derive(Debug, Clone, Default)]
pub struct SelectState {
    pub is_open:        bool,
    /// Índice da opção sob o cursor (para highlight de hover).
    pub hovered_option: Option<usize>,
}

// ── Spinner / ProgressBar indeterminate ──────────────────────

/// Estado de animação para Spinner e ProgressBar indeterminada.
#[derive(Debug, Clone)]
pub struct AnimState {
    /// Ângulo atual em graus (Spinner).
    pub angle:        f32,
    /// Offset normalizado (0.0–1.0) para barra indeterminada.
    pub anim_offset:  f32,
    /// Último tick de atualização.
    pub last_tick:    Instant,
}

impl Default for AnimState {
    fn default() -> Self {
        Self { angle: 0.0, anim_offset: 0.0, last_tick: Instant::now() }
    }
}

impl AnimState {
    /// Avança a animação com base no tempo decorrido desde o último tick.
    /// Retorna `true` se mudou (precisa redraw).
    pub fn tick(&mut self) -> bool {
        let now     = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;

        // Spinner: 360°/s → uma rotação por segundo
        self.angle = (self.angle + elapsed * 360.0) % 360.0;
        // Barra indeterminada: percorre 0→1 em ~1.2s
        self.anim_offset = (self.anim_offset + elapsed / 1.2) % 1.0;
        true
    }
}

// ── Enum unificado ────────────────────────────────────────────

/// Estado interno de qualquer widget com estado em Fase 3.
/// Armazenado no `RutterEngine` por ID do widget.
#[derive(Debug)]
pub enum WidgetState {
    Slider(SliderState),
    Scroll(ScrollState),
    Select(SelectState),
    Anim(AnimState),
}

impl WidgetState {
    pub fn as_slider(&self)     -> Option<&SliderState>  { if let Self::Slider(s) = self { Some(s) } else { None } }
    pub fn as_slider_mut(&mut self) -> Option<&mut SliderState> { if let Self::Slider(s) = self { Some(s) } else { None } }
    pub fn as_scroll(&self)     -> Option<&ScrollState>  { if let Self::Scroll(s) = self { Some(s) } else { None } }
    pub fn as_scroll_mut(&mut self) -> Option<&mut ScrollState> { if let Self::Scroll(s) = self { Some(s) } else { None } }
    pub fn as_select(&self)     -> Option<&SelectState>  { if let Self::Select(s) = self { Some(s) } else { None } }
    pub fn as_select_mut(&mut self) -> Option<&mut SelectState> { if let Self::Select(s) = self { Some(s) } else { None } }
    pub fn as_anim(&self)       -> Option<&AnimState>    { if let Self::Anim(s) = self { Some(s) } else { None } }
    pub fn as_anim_mut(&mut self)   -> Option<&mut AnimState>   { if let Self::Anim(s) = self { Some(s) } else { None } }
}

// ── Testes unitários ─────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // ── SliderState ──────────────────────────────────────────

    #[test]
    fn slider_value_from_cursor_at_start() {
        let s = SliderState { track_abs_x: 10.0, track_width: 100.0, ..Default::default() };
        assert!((s.value_from_cursor(10.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_value_from_cursor_at_end() {
        let s = SliderState { track_abs_x: 10.0, track_width: 100.0, ..Default::default() };
        assert!((s.value_from_cursor(110.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_value_from_cursor_mid() {
        let s = SliderState { track_abs_x: 0.0, track_width: 200.0, ..Default::default() };
        assert!((s.value_from_cursor(100.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn slider_value_clamped_below_zero() {
        let s = SliderState { track_abs_x: 50.0, track_width: 100.0, ..Default::default() };
        assert!((s.value_from_cursor(0.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_value_clamped_above_one() {
        let s = SliderState { track_abs_x: 0.0, track_width: 100.0, ..Default::default() };
        assert!((s.value_from_cursor(500.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_zero_track_width_returns_zero() {
        let s = SliderState { track_abs_x: 0.0, track_width: 0.0, ..Default::default() };
        assert!((s.value_from_cursor(50.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_default_not_dragging() {
        assert!(!SliderState::default().dragging);
    }

    // ── ScrollState ──────────────────────────────────────────

    #[test]
    fn scroll_max_offset_no_overflow() {
        let s = ScrollState { offset_y: 0.0, content_height: 100.0, viewport_h: 200.0 };
        assert!((s.max_offset() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_max_offset_with_overflow() {
        let s = ScrollState { offset_y: 0.0, content_height: 500.0, viewport_h: 200.0 };
        assert!((s.max_offset() - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_by_clamps_at_zero() {
        let mut s = ScrollState { offset_y: 0.0, content_height: 500.0, viewport_h: 200.0 };
        s.scroll_by(-100.0);
        assert!((s.offset_y - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_by_clamps_at_max() {
        let mut s = ScrollState { offset_y: 0.0, content_height: 500.0, viewport_h: 200.0 };
        s.scroll_by(1000.0);
        assert!((s.offset_y - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_by_increments_correctly() {
        let mut s = ScrollState { offset_y: 0.0, content_height: 500.0, viewport_h: 200.0 };
        s.scroll_by(50.0);
        assert!((s.offset_y - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_thumb_ratio_full_visibility() {
        let s = ScrollState { offset_y: 0.0, content_height: 100.0, viewport_h: 100.0 };
        assert!((s.thumb_ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_thumb_ratio_half_visibility() {
        let s = ScrollState { offset_y: 0.0, content_height: 400.0, viewport_h: 200.0 };
        assert!((s.thumb_ratio() - 0.5).abs() < 0.001);
    }

    #[test]
    fn scroll_thumb_y_at_top() {
        let s = ScrollState { offset_y: 0.0, content_height: 400.0, viewport_h: 200.0 };
        assert!((s.thumb_y() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_thumb_y_at_bottom() {
        let s = ScrollState { offset_y: 300.0, content_height: 500.0, viewport_h: 200.0 };
        // max_offset=300, thumb_ratio=0.4, thumb travel = 200*(1-0.4)=120, at 100% → 120
        let expected_thumb_y = 300.0 / 300.0 * (200.0 * (1.0 - 0.4));
        assert!((s.thumb_y() - expected_thumb_y).abs() < 1.0);
    }

    #[test]
    fn scroll_default_offset_is_zero() {
        let s = ScrollState::default();
        assert!((s.offset_y - 0.0).abs() < f32::EPSILON);
    }

    // ── SelectState ──────────────────────────────────────────

    #[test]
    fn select_starts_closed() {
        assert!(!SelectState::default().is_open);
    }

    #[test]
    fn select_toggle_open() {
        let mut s = SelectState::default();
        s.is_open = true;
        assert!(s.is_open);
    }

    #[test]
    fn select_hovered_option_default_none() {
        assert!(SelectState::default().hovered_option.is_none());
    }

    // ── AnimState ────────────────────────────────────────────

    #[test]
    fn anim_angle_starts_zero() {
        assert!((AnimState::default().angle - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn anim_offset_starts_zero() {
        assert!((AnimState::default().anim_offset - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn anim_tick_advances_angle() {
        let mut a = AnimState::default();
        thread::sleep(Duration::from_millis(50));
        a.tick();
        assert!(a.angle > 0.0, "ângulo deve ter avançado");
    }

    #[test]
    fn anim_angle_wraps_at_360() {
        let mut a = AnimState { angle: 359.0, anim_offset: 0.0, last_tick: Instant::now() };
        thread::sleep(Duration::from_millis(10));
        a.tick();
        // Após wrap, ângulo deve ser < 360
        assert!(a.angle < 360.0);
    }

    #[test]
    fn anim_offset_wraps_at_one() {
        let mut a = AnimState { angle: 0.0, anim_offset: 0.99, last_tick: Instant::now() };
        thread::sleep(Duration::from_millis(30));
        a.tick();
        assert!(a.anim_offset < 1.0);
    }

    // ── WidgetState enum ─────────────────────────────────────

    #[test]
    fn widget_state_slider_accessor() {
        let w = WidgetState::Slider(SliderState::default());
        assert!(w.as_slider().is_some());
        assert!(w.as_scroll().is_none());
    }

    #[test]
    fn widget_state_scroll_accessor() {
        let w = WidgetState::Scroll(ScrollState::default());
        assert!(w.as_scroll().is_some());
        assert!(w.as_slider().is_none());
    }

    #[test]
    fn widget_state_select_accessor() {
        let w = WidgetState::Select(SelectState::default());
        assert!(w.as_select().is_some());
        assert!(w.as_anim().is_none());
    }

    #[test]
    fn widget_state_anim_accessor() {
        let w = WidgetState::Anim(AnimState::default());
        assert!(w.as_anim().is_some());
        assert!(w.as_select().is_none());
    }
}
