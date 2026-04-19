// ============================================================
// Rutter Framework — engine/widget_state.rs  (Fase 4)
//
// Novos estados:
//   ToastState      — timer de auto-dismiss
//   ModalState      — visível/oculto + backdrop fade
//   TabState        — aba ativa (redundante com widget, mas
//                     permite animação de underline)
//   VirtualListState— scroll offset + range visível
//   VirtualGridState— scroll vertical + seleção de célula
// ============================================================

use std::time::{Duration, Instant};

use crate::layout::{SCROLLBAR_W, VIRTUAL_GRID_GAP, VIRTUAL_GRID_PADDING};

// ── (mantidos da Fase 3) ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SliderState {
    pub dragging: bool,
    pub drag_start_cursor: f32,
    pub drag_start_value: f32,
    pub track_abs_x: f32,
    pub track_width: f32,
}

impl Default for SliderState {
    fn default() -> Self {
        Self {
            dragging: false,
            drag_start_cursor: 0.0,
            drag_start_value: 0.0,
            track_abs_x: 0.0,
            track_width: 1.0,
        }
    }
}

impl SliderState {
    pub fn value_from_cursor(&self, abs_x: f32) -> f32 {
        if self.track_width <= 0.0 {
            return 0.0;
        }
        ((abs_x - self.track_abs_x) / self.track_width).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset_y: f32,
    pub content_height: f32,
    pub viewport_h: f32,
}

impl ScrollState {
    pub fn max_offset(&self) -> f32 {
        (self.content_height - self.viewport_h).max(0.0)
    }
    pub fn scroll_by(&mut self, delta_y: f32) {
        self.offset_y = (self.offset_y + delta_y).clamp(0.0, self.max_offset());
    }
    pub fn thumb_ratio(&self) -> f32 {
        if self.content_height <= 0.0 {
            return 1.0;
        }
        (self.viewport_h / self.content_height).clamp(0.0, 1.0)
    }
    pub fn thumb_y(&self) -> f32 {
        if self.max_offset() <= 0.0 {
            return 0.0;
        }
        (self.offset_y / self.max_offset()) * (self.viewport_h * (1.0 - self.thumb_ratio()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct SelectState {
    pub is_open: bool,
    pub hovered_option: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct AnimState {
    pub angle: f32,
    pub anim_offset: f32,
    pub last_tick: Instant,
}

impl Default for AnimState {
    fn default() -> Self {
        Self {
            angle: 0.0,
            anim_offset: 0.0,
            last_tick: Instant::now(),
        }
    }
}

impl AnimState {
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;
        self.angle = (self.angle + dt * 360.0) % 360.0;
        self.anim_offset = (self.anim_offset + dt / 1.2) % 1.0;
        true
    }
}

// ── Fase 4 — novos estados ────────────────────────────────────

/// Estado de um Toast (notificação temporária).
#[derive(Debug, Clone)]
pub struct ToastState {
    pub visible: bool,
    pub created_at: Instant,
    pub duration_ms: u32,
    /// true quando o usuário clicou em dismiss antes do timer
    pub dismissed: bool,
}

impl ToastState {
    pub fn new(duration_ms: u32) -> Self {
        Self {
            visible: true,
            created_at: Instant::now(),
            duration_ms,
            dismissed: false,
        }
    }

    /// Verifica se o timer expirou. Retorna true se deve sumir.
    pub fn is_expired(&self) -> bool {
        if self.dismissed {
            return true;
        }
        if self.duration_ms == 0 {
            return false;
        } // permanente
        self.created_at.elapsed() >= Duration::from_millis(self.duration_ms as u64)
    }

    /// Progresso do timer: 1.0 = recém-criado, 0.0 = expirou.
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        let elapsed = self.created_at.elapsed().as_millis() as f32;
        let total = self.duration_ms as f32;
        (1.0 - elapsed / total).clamp(0.0, 1.0)
    }

    pub fn dismiss(&mut self) {
        self.dismissed = true;
        self.visible = false;
    }
}

/// Estado de um Modal (overlay com backdrop).
#[derive(Debug, Clone)]
pub struct ModalState {
    pub visible: bool,
    /// Alpha atual do backdrop (0–255) para fade-in/out.
    pub backdrop_alpha: u8,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            visible: false,
            backdrop_alpha: 0,
        }
    }
}

impl ModalState {
    pub fn open(&mut self) {
        self.visible = true;
        self.backdrop_alpha = 180;
    }
    pub fn close(&mut self) {
        self.visible = false;
        self.backdrop_alpha = 0;
    }
}

/// Estado de uma TabBar.
#[derive(Debug, Clone)]
pub struct TabState {
    pub active: usize,
    /// Posição X do underline (para animação suave — Fase 5).
    pub underline_x: f32,
}

impl Default for TabState {
    fn default() -> Self {
        Self {
            active: 0,
            underline_x: 0.0,
        }
    }
}

impl TabState {
    pub fn set_active(&mut self, idx: usize, tab_width: f32) {
        self.active = idx;
        self.underline_x = idx as f32 * tab_width;
    }
}

/// Estado de uma VirtualList.
#[derive(Debug, Clone, Default)]
pub struct VirtualListState {
    /// Offset vertical de scroll (px lógicos).
    pub scroll_y: f32,
    /// Altura da viewport (atualizada no render).
    pub viewport_h: f32,
    /// Índice da linha selecionada (None = nenhuma).
    pub selected_row: Option<usize>,
    /// Índice sob o cursor (para highlight de hover).
    pub hovered_row: Option<usize>,
}

impl VirtualListState {
    /// Faixa de índices visíveis dado `item_height` e `item_count`.
    pub fn visible_range(&self, item_height: f32, item_count: usize) -> (usize, usize) {
        if item_height <= 0.0 {
            return (0, 0);
        }
        let first = (self.scroll_y / item_height).floor() as usize;
        let count = (self.viewport_h / item_height).ceil() as usize + 1; // +1 buffer
        let last = (first + count).min(item_count);
        (first, last)
    }

    /// Offset máximo de scroll.
    pub fn max_scroll(&self, item_height: f32, item_count: usize) -> f32 {
        let total = item_height * item_count as f32;
        (total - self.viewport_h).max(0.0)
    }

    pub fn scroll_by(&mut self, delta_y: f32, item_height: f32, item_count: usize) {
        let max = self.max_scroll(item_height, item_count);
        self.scroll_y = (self.scroll_y + delta_y).clamp(0.0, max);
    }

    pub fn scroll_to_index(&mut self, idx: usize, item_height: f32, item_count: usize) {
        let target_y = idx as f32 * item_height;
        let max = self.max_scroll(item_height, item_count);
        self.scroll_y = target_y.clamp(0.0, max);
    }

    pub fn thumb_ratio(&self, item_height: f32, item_count: usize) -> f32 {
        let total = item_height * item_count as f32;
        if total <= 0.0 {
            return 1.0;
        }
        (self.viewport_h / total).clamp(0.0, 1.0)
    }

    pub fn thumb_y(&self, item_height: f32, item_count: usize) -> f32 {
        let max = self.max_scroll(item_height, item_count);
        if max <= 0.0 {
            return 0.0;
        }
        (self.scroll_y / max)
            * (self.viewport_h * (1.0 - self.thumb_ratio(item_height, item_count)))
    }
}

#[derive(Debug, Clone, Default)]
pub struct VirtualGridState {
    pub scroll_y: f32,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub selected_item: Option<usize>,
    pub hovered_item: Option<usize>,
}

pub(crate) fn normalize_virtual_grid_columns(columns: usize) -> usize {
    columns.max(1)
}

pub(crate) fn virtual_grid_row_count(item_count: usize, columns: usize) -> usize {
    let columns = normalize_virtual_grid_columns(columns);
    if item_count == 0 {
        0
    } else {
        item_count.div_ceil(columns)
    }
}

pub(crate) fn virtual_grid_cell_width(viewport_w: f32, columns: usize) -> f32 {
    let columns = normalize_virtual_grid_columns(columns);
    let content_w = (viewport_w
        - SCROLLBAR_W
        - 4.0
        - VIRTUAL_GRID_PADDING * 2.0
        - VIRTUAL_GRID_GAP * (columns.saturating_sub(1)) as f32)
        .max(1.0);
    content_w / columns as f32
}

pub(crate) fn virtual_grid_cell_left(column: usize, viewport_w: f32, columns: usize) -> f32 {
    let cell_w = virtual_grid_cell_width(viewport_w, columns);
    VIRTUAL_GRID_PADDING + column as f32 * (cell_w + VIRTUAL_GRID_GAP)
}

impl VirtualGridState {
    pub fn visible_row_range(
        &self,
        item_height: f32,
        item_count: usize,
        columns: usize,
    ) -> (usize, usize) {
        if item_height <= 0.0 {
            return (0, 0);
        }
        let row_count = virtual_grid_row_count(item_count, columns);
        let first = (self.scroll_y / item_height).floor() as usize;
        let count = (self.viewport_h / item_height).ceil() as usize + 1;
        let last = (first + count).min(row_count);
        (first, last)
    }

    pub fn max_scroll(&self, item_height: f32, item_count: usize, columns: usize) -> f32 {
        let total = item_height * virtual_grid_row_count(item_count, columns) as f32;
        (total - self.viewport_h).max(0.0)
    }

    pub fn scroll_by(&mut self, delta_y: f32, item_height: f32, item_count: usize, columns: usize) {
        let max = self.max_scroll(item_height, item_count, columns);
        self.scroll_y = (self.scroll_y + delta_y).clamp(0.0, max);
    }

    pub fn scroll_to_index(
        &mut self,
        idx: usize,
        item_height: f32,
        item_count: usize,
        columns: usize,
    ) {
        let row = idx / normalize_virtual_grid_columns(columns);
        let target_y = row as f32 * item_height;
        let max = self.max_scroll(item_height, item_count, columns);
        self.scroll_y = target_y.clamp(0.0, max);
    }

    pub fn thumb_ratio(&self, item_height: f32, item_count: usize, columns: usize) -> f32 {
        let total = item_height * virtual_grid_row_count(item_count, columns) as f32;
        if total <= 0.0 {
            return 1.0;
        }
        (self.viewport_h / total).clamp(0.0, 1.0)
    }

    pub fn thumb_y(&self, item_height: f32, item_count: usize, columns: usize) -> f32 {
        let max = self.max_scroll(item_height, item_count, columns);
        if max <= 0.0 {
            return 0.0;
        }
        (self.scroll_y / max)
            * (self.viewport_h * (1.0 - self.thumb_ratio(item_height, item_count, columns)))
    }

    pub fn index_at(
        &self,
        local_x: f32,
        local_y: f32,
        item_height: f32,
        item_count: usize,
        columns: usize,
    ) -> Option<usize> {
        if item_height <= 0.0 || item_count == 0 {
            return None;
        }

        let columns = normalize_virtual_grid_columns(columns);
        let row = ((local_y + self.scroll_y) / item_height).floor().max(0.0) as usize;
        let cell_w = virtual_grid_cell_width(self.viewport_w, columns);
        let usable_w = VIRTUAL_GRID_PADDING * 2.0
            + cell_w * columns as f32
            + VIRTUAL_GRID_GAP * columns.saturating_sub(1) as f32;
        if local_x < VIRTUAL_GRID_PADDING || local_x > usable_w {
            return None;
        }

        for col in 0..columns {
            let left = virtual_grid_cell_left(col, self.viewport_w, columns);
            let right = left + cell_w;
            if local_x >= left && local_x <= right {
                let index = row * columns + col;
                return (index < item_count).then_some(index);
            }
        }
        None
    }
}

// ── Enum unificado ────────────────────────────────────────────

#[derive(Debug)]
pub enum WidgetState {
    Slider(SliderState),
    Scroll(ScrollState),
    Select(SelectState),
    Anim(AnimState),
    Toast(ToastState),
    Modal(ModalState),
    Tab(TabState),
    VList(VirtualListState),
    VGrid(VirtualGridState),
}

impl WidgetState {
    pub fn as_slider(&self) -> Option<&SliderState> {
        if let Self::Slider(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_slider_mut(&mut self) -> Option<&mut SliderState> {
        if let Self::Slider(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_scroll(&self) -> Option<&ScrollState> {
        if let Self::Scroll(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_scroll_mut(&mut self) -> Option<&mut ScrollState> {
        if let Self::Scroll(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_select(&self) -> Option<&SelectState> {
        if let Self::Select(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_select_mut(&mut self) -> Option<&mut SelectState> {
        if let Self::Select(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_anim(&self) -> Option<&AnimState> {
        if let Self::Anim(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_anim_mut(&mut self) -> Option<&mut AnimState> {
        if let Self::Anim(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_toast(&self) -> Option<&ToastState> {
        if let Self::Toast(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_toast_mut(&mut self) -> Option<&mut ToastState> {
        if let Self::Toast(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_modal(&self) -> Option<&ModalState> {
        if let Self::Modal(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_modal_mut(&mut self) -> Option<&mut ModalState> {
        if let Self::Modal(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_tab(&self) -> Option<&TabState> {
        if let Self::Tab(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_tab_mut(&mut self) -> Option<&mut TabState> {
        if let Self::Tab(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_vlist(&self) -> Option<&VirtualListState> {
        if let Self::VList(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_vlist_mut(&mut self) -> Option<&mut VirtualListState> {
        if let Self::VList(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_vgrid(&self) -> Option<&VirtualGridState> {
        if let Self::VGrid(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_vgrid_mut(&mut self) -> Option<&mut VirtualGridState> {
        if let Self::VGrid(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

// ── Testes unitários ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    // ── ToastState ───────────────────────────────────────────

    #[test]
    fn toast_starts_visible() {
        let t = ToastState::new(3000);
        assert!(t.visible);
        assert!(!t.dismissed);
    }

    #[test]
    fn toast_not_expired_immediately() {
        let t = ToastState::new(3000);
        assert!(!t.is_expired());
    }

    #[test]
    fn toast_expires_after_duration() {
        let t = ToastState::new(30); // 30ms
        thread::sleep(Duration::from_millis(50));
        assert!(t.is_expired());
    }

    #[test]
    fn toast_permanent_never_expires() {
        let t = ToastState::new(0);
        thread::sleep(Duration::from_millis(50));
        assert!(!t.is_expired());
    }

    #[test]
    fn toast_dismiss_marks_expired() {
        let mut t = ToastState::new(5000);
        assert!(!t.is_expired());
        t.dismiss();
        assert!(t.is_expired());
        assert!(!t.visible);
    }

    #[test]
    fn toast_progress_starts_near_one() {
        let t = ToastState::new(1000);
        assert!(t.progress() > 0.95);
    }

    #[test]
    fn toast_progress_after_duration_near_zero() {
        let t = ToastState::new(30);
        thread::sleep(Duration::from_millis(50));
        let p = t.progress();
        assert!(p <= 0.0, "progress={p}");
    }

    #[test]
    fn toast_permanent_progress_is_one() {
        let t = ToastState::new(0);
        assert!((t.progress() - 1.0).abs() < f32::EPSILON);
    }

    // ── ModalState ───────────────────────────────────────────

    #[test]
    fn modal_starts_hidden() {
        assert!(!ModalState::default().visible);
    }

    #[test]
    fn modal_open_sets_visible() {
        let mut m = ModalState::default();
        m.open();
        assert!(m.visible);
        assert!(m.backdrop_alpha > 0);
    }

    #[test]
    fn modal_close_clears() {
        let mut m = ModalState::default();
        m.open();
        m.close();
        assert!(!m.visible);
        assert_eq!(m.backdrop_alpha, 0);
    }

    // ── TabState ─────────────────────────────────────────────

    #[test]
    fn tab_starts_at_zero() {
        assert_eq!(TabState::default().active, 0);
    }

    #[test]
    fn tab_set_active_updates_index() {
        let mut t = TabState::default();
        t.set_active(2, 100.0);
        assert_eq!(t.active, 2);
    }

    #[test]
    fn tab_underline_x_matches_index() {
        let mut t = TabState::default();
        t.set_active(3, 80.0);
        assert!((t.underline_x - 240.0).abs() < f32::EPSILON);
    }

    // ── VirtualListState ─────────────────────────────────────

    #[test]
    fn vlist_visible_range_full_when_fewer_items() {
        let s = VirtualListState {
            scroll_y: 0.0,
            viewport_h: 300.0,
            ..Default::default()
        };
        let (first, last) = s.visible_range(30.0, 5);
        assert_eq!(first, 0);
        assert_eq!(last, 5);
    }

    #[test]
    fn vlist_visible_range_clips_at_count() {
        let s = VirtualListState {
            scroll_y: 0.0,
            viewport_h: 300.0,
            ..Default::default()
        };
        let (first, last) = s.visible_range(30.0, 1000);
        assert_eq!(first, 0);
        assert!(last <= 12); // ~10 items + 1 buffer
    }

    #[test]
    fn vlist_visible_range_scrolled() {
        let s = VirtualListState {
            scroll_y: 300.0,
            viewport_h: 300.0,
            ..Default::default()
        };
        let (first, last) = s.visible_range(30.0, 1000);
        assert_eq!(first, 10); // 300 / 30 = 10
        assert!(last > 10);
    }

    #[test]
    fn vlist_scroll_by_clamps() {
        let mut s = VirtualListState {
            scroll_y: 0.0,
            viewport_h: 200.0,
            ..Default::default()
        };
        s.scroll_by(-50.0, 30.0, 100);
        assert!((s.scroll_y - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_scroll_by_forward() {
        let mut s = VirtualListState {
            scroll_y: 0.0,
            viewport_h: 200.0,
            ..Default::default()
        };
        s.scroll_by(90.0, 30.0, 100);
        assert!((s.scroll_y - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_scroll_by_clamps_at_max() {
        let mut s = VirtualListState {
            scroll_y: 0.0,
            viewport_h: 200.0,
            ..Default::default()
        };
        // max = 100*30 - 200 = 2800
        s.scroll_by(9999.0, 30.0, 100);
        assert!((s.scroll_y - 2800.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_scroll_to_index_correct() {
        let mut s = VirtualListState {
            scroll_y: 0.0,
            viewport_h: 200.0,
            ..Default::default()
        };
        s.scroll_to_index(20, 30.0, 100);
        assert!((s.scroll_y - 600.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_max_scroll_calculation() {
        let s = VirtualListState {
            viewport_h: 200.0,
            ..Default::default()
        };
        assert!((s.max_scroll(30.0, 100) - 2800.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_thumb_ratio_small_list() {
        let s = VirtualListState {
            viewport_h: 300.0,
            ..Default::default()
        };
        // 10 items * 30 = 300 total = viewport → ratio 1.0
        assert!((s.thumb_ratio(30.0, 10) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_thumb_ratio_large_list() {
        let s = VirtualListState {
            viewport_h: 200.0,
            ..Default::default()
        };
        // 100*30=3000, 200/3000 ≈ 0.067
        let r = s.thumb_ratio(30.0, 100);
        assert!((r - 200.0 / 3000.0).abs() < 0.001);
    }

    #[test]
    fn vlist_no_items_zero_max_scroll() {
        let s = VirtualListState::default();
        assert!((s.max_scroll(30.0, 0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vlist_selected_row_none_by_default() {
        assert!(VirtualListState::default().selected_row.is_none());
    }

    // ── VirtualGridState ─────────────────────────────────────

    #[test]
    fn vgrid_visible_row_range_scrolled() {
        let s = VirtualGridState {
            scroll_y: 180.0,
            viewport_h: 180.0,
            ..Default::default()
        };
        let (first, last) = s.visible_row_range(60.0, 100, 4);
        assert_eq!(first, 3);
        assert!(last > first);
    }

    #[test]
    fn vgrid_max_scroll_calculation() {
        let s = VirtualGridState {
            viewport_h: 180.0,
            ..Default::default()
        };
        assert!((s.max_scroll(60.0, 40, 4) - 420.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vgrid_scroll_to_index_uses_row() {
        let mut s = VirtualGridState {
            viewport_h: 180.0,
            ..Default::default()
        };
        s.scroll_to_index(9, 60.0, 40, 4);
        assert!((s.scroll_y - 120.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vgrid_index_at_maps_point_to_cell() {
        let s = VirtualGridState {
            viewport_w: 420.0,
            viewport_h: 240.0,
            ..Default::default()
        };
        assert_eq!(s.index_at(32.0, 24.0, 60.0, 40, 4), Some(0));
        assert_eq!(s.index_at(220.0, 24.0, 60.0, 40, 4), Some(2));
    }

    #[test]
    fn vgrid_selected_item_none_by_default() {
        assert!(VirtualGridState::default().selected_item.is_none());
    }

    // ── WidgetState accessors ─────────────────────────────────

    #[test]
    fn widget_state_toast_accessor() {
        let ws = WidgetState::Toast(ToastState::new(1000));
        assert!(ws.as_toast().is_some());
        assert!(ws.as_modal().is_none());
    }

    #[test]
    fn widget_state_modal_accessor() {
        let ws = WidgetState::Modal(ModalState::default());
        assert!(ws.as_modal().is_some());
        assert!(ws.as_tab().is_none());
    }

    #[test]
    fn widget_state_tab_accessor() {
        let ws = WidgetState::Tab(TabState::default());
        assert!(ws.as_tab().is_some());
        assert!(ws.as_vlist().is_none());
    }

    #[test]
    fn widget_state_vlist_accessor() {
        let ws = WidgetState::VList(VirtualListState::default());
        assert!(ws.as_vlist().is_some());
        assert!(ws.as_slider().is_none());
    }

    #[test]
    fn widget_state_vlist_mut() {
        let mut ws = WidgetState::VList(VirtualListState::default());
        ws.as_vlist_mut().unwrap().selected_row = Some(5);
        assert_eq!(ws.as_vlist().unwrap().selected_row, Some(5));
    }

    #[test]
    fn widget_state_vgrid_accessor() {
        let ws = WidgetState::VGrid(VirtualGridState::default());
        assert!(ws.as_vgrid().is_some());
        assert!(ws.as_vlist().is_none());
    }

    #[test]
    fn widget_state_vgrid_mut() {
        let mut ws = WidgetState::VGrid(VirtualGridState::default());
        ws.as_vgrid_mut().unwrap().selected_item = Some(9);
        assert_eq!(ws.as_vgrid().unwrap().selected_item, Some(9));
    }

    #[test]
    fn widget_state_modal_mut_open_close() {
        let mut ws = WidgetState::Modal(ModalState::default());
        ws.as_modal_mut().unwrap().open();
        assert!(ws.as_modal().unwrap().visible);
        ws.as_modal_mut().unwrap().close();
        assert!(!ws.as_modal().unwrap().visible);
    }
}
