// ============================================================
// Rutter Framework — examples/widgets/mod.rs
//
// Módulo de demonstrações individuais por widget.
// Cada sub-módulo expõe um AppLogic completo usado pelo binário principal.
//
// Para rodar uma demo específica via Cargo:
//   cargo run -- text_input
//   cargo run -- slider
// ============================================================

pub mod calendar_demo;
pub mod carousel_demo;
pub mod controls_demo;
pub mod dropdown_menu_demo;
pub mod form_demo;
pub mod modal_toast_demo;
pub mod multi_window_demo;
pub mod popover_demo;
pub mod progress_demo;
pub mod rich_text_demo;
pub mod scroll_demo;
pub mod slider_demo;
pub mod tab_demo;
pub mod text_input_demo;
pub mod theme_selector;
pub mod vgrid_demo;
pub mod vlist_demo;

pub mod accordion_demo;
pub mod advanced_widgets_demo;
pub mod button_content_demo;
pub mod dialog_demo;
pub mod image_viewer_demo;
pub mod search_bar_demo;
pub mod text_area_demo;

#[cfg(test)]
#[path = "../../tests/unit/all_examples_theme_unit_tests.rs"]
mod tests;
