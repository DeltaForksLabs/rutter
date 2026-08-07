// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — main.rs  (v6.2)
//
// Ponto de entrada. Escolha qual demo rodar ajustando
// a constante DEMO abaixo, ou passe como argumento CLI:
//   cargo run -- form           # demo completa (padrão)
//   cargo run -- text_input
//   cargo run -- slider
//   cargo run -- progress
//   cargo run -- controls
//   cargo run -- tabs
//   cargo run -- scroll
//   cargo run -- modal_toast
//   cargo run -- vlist
//   cargo run -- vgrid
//   cargo run -- popover
//   cargo run -- calendar
//   cargo run -- carousel
//   cargo run -- multi_window
//   cargo run -- button_content
//   cargo run -- image_viewer
//   cargo run -- advanced
// ============================================================

#[path = "../examples/widgets/mod.rs"]
mod widget_examples;

fn main() {
    let demo = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "form".to_string());

    match demo.as_str() {
        "text_input" => widget_examples::text_input_demo::run(),
        "slider" => widget_examples::slider_demo::run(),
        "progress" => widget_examples::progress_demo::run(),
        "controls" => widget_examples::controls_demo::run(),
        "tabs" => widget_examples::tab_demo::run(),
        "scroll" => widget_examples::scroll_demo::run(),
        "modal_toast" => widget_examples::modal_toast_demo::run(),
        "popover" => widget_examples::popover_demo::run(),
        "calendar" => widget_examples::calendar_demo::run(),
        "carousel" => widget_examples::carousel_demo::run(),
        "multi_window" => widget_examples::multi_window_demo::run(),
        "button_content" => widget_examples::button_content_demo::run(),
        "image_viewer" => widget_examples::image_viewer_demo::run(),
        "vlist" => widget_examples::vlist_demo::run(),
        "vgrid" => widget_examples::vgrid_demo::run(),
        "accordion" => widget_examples::accordion_demo::run(),
        "dialog" => widget_examples::dialog_demo::run(),
        "search_bar" => widget_examples::search_bar_demo::run(),
        "text_area" => widget_examples::text_area_demo::run(),
        "advanced" => widget_examples::advanced_widgets_demo::run(),
        _ => widget_examples::form_demo::run(), // "form" ou qualquer outro
    }
}
