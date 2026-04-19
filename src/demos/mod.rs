// ============================================================
// Rutter Framework — src/demos/mod.rs
//
// Módulo de demonstrações individuais por widget.
// Cada sub-módulo é um binário independente (AppLogic completo).
//
// Para rodar uma demo específica via Cargo:
//   cargo run --example text_input
//   cargo run --example slider
//   ... (configure em Cargo.toml com [[example]])
//
// Ou, para rodar a demo completa integrada (original):
//   cargo run
// ============================================================

pub mod controls_demo;
pub mod form_demo;
pub mod modal_toast_demo;
pub mod progress_demo;
pub mod scroll_demo;
pub mod slider_demo;
pub mod tab_demo;
pub mod text_input_demo;
pub mod vgrid_demo;
pub mod vlist_demo;

pub mod accordion_demo;
pub mod advanced_widgets_demo;
pub mod dialog_demo;
pub mod search_bar_demo;
pub mod text_area_demo;
