// ============================================================
// Rutter Framework — lib.rs
// Ponto de entrada público. Declare módulos e re-exporte o
// que o usuário do framework precisa importar.
// ============================================================

pub mod app;
pub mod widget;
pub mod theme;
pub mod layout;
pub mod render;
pub mod engine;
pub mod input_state;

// ── Re-exports ergonômicos ───────────────────────────────────
pub use app::AppLogic;
pub use engine::runner::RutterRunner;
pub use theme::Theme;
pub use widget::{ButtonVariant, InputState, Widget};
