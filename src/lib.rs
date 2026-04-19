// ============================================================
// Rutter Framework — lib.rs
// Ponto de entrada público. Declare módulos e re-exporte o
// que o usuário do framework precisa importar.
// ============================================================

pub mod app;
pub mod engine;
pub mod input_state;
pub mod layout;
pub mod render;
pub mod theme;
pub mod widget;

// ── Re-exports ergonômicos ───────────────────────────────────
pub use app::AppLogic;
pub use engine::runner::RutterRunner;
pub use theme::Theme;
pub use widget::{AUTO_ID, ButtonVariant, InputState, Widget};

// ── Re-exports de dependências públicas ──────────────────────
pub use arboard;
pub use cosmic_text;
pub use skia_safe;
pub use taffy;
