// ============================================================
// Rutter Framework — engine/cursor.rs
// Gerencia o estado de visibilidade do cursor piscante.
// Separado do engine principal para facilitar reutilização
// em múltiplos inputs quando multi-window for implementado.
// ============================================================

use std::time::{Duration, Instant};

/// Controla o piscar do cursor em campos de texto focados.
///
/// O tick é acionado por `new_events` no runner (via
/// `ControlFlow::WaitUntil`) — não por redraw, evitando o
/// bug original onde `ControlFlow::Wait` impedia o piscar.
pub struct CursorBlink {
    visible: bool,
    last_toggle: Instant,
    blink_interval: Duration,
}

impl CursorBlink {
    pub fn new() -> Self {
        Self {
            visible: true,
            last_toggle: Instant::now(),
            blink_interval: Duration::from_millis(500),
        }
    }

    /// Avança o estado. Retorna `true` se a visibilidade mudou
    /// (indicando que um redraw é necessário).
    pub fn tick(&mut self) -> bool {
        if Instant::now().duration_since(self.last_toggle) >= self.blink_interval {
            self.visible = !self.visible;
            self.last_toggle = Instant::now();
            true
        } else {
            false
        }
    }

    /// Reinicia para visível. Chamar ao focar ou ao digitar para
    /// garantir que o cursor seja sempre visível após uma ação.
    pub fn reset(&mut self) {
        self.visible = true;
        self.last_toggle = Instant::now();
    }

    /// Retorna se o cursor deve ser desenhado neste frame.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Retorna o instante do próximo toggle (para WaitUntil).
    pub fn next_tick_at(&self) -> Instant {
        self.last_toggle + self.blink_interval
    }
}

impl Default for CursorBlink {
    fn default() -> Self {
        Self::new()
    }
}
