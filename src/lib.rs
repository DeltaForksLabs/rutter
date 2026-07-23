// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — lib.rs
// Ponto de entrada público. Declare módulos e re-exporte o
// que o usuário do framework precisa importar.
// ============================================================

pub(crate) mod accessibility;
pub mod app;
pub mod engine;
pub mod i18n;
pub mod input_limits;
pub mod input_state;
mod input_undo;
pub mod layout;
pub mod render;
pub mod theme;
pub mod widget;
mod widget_id;
mod widget_id_error;
mod widget_structure;

// ── Re-exports ergonômicos ───────────────────────────────────
pub use app::AppLogic;
pub use engine::run_error::RutterRunError;
pub use engine::runner::RutterRunner;
pub use i18n::{FluentCatalog, I18nError, LayoutDirection, Locale};
pub use input_limits::{InputKind, InputLimitError, InputLimits};
pub use render::text::TextShapeCacheLimits;
pub use theme::Theme;
pub use widget::{AUTO_ID, ButtonVariant, ContextMenuEntry, DialogPosition, InputState, Widget};
pub use widget_id::{WidgetId, WidgetIdError, WidgetIdSnapshot};

// ── Re-exports de dependências públicas ──────────────────────
pub use arboard;
pub use cosmic_text;
pub use skia_safe;
pub use taffy;
