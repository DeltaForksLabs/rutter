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
//   cargo run -- advanced
// ============================================================

mod demos;

fn main() {
    let demo = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "form".to_string());

    match demo.as_str() {
        "text_input" => demos::text_input_demo::run(),
        "slider" => demos::slider_demo::run(),
        "progress" => demos::progress_demo::run(),
        "controls" => demos::controls_demo::run(),
        "tabs" => demos::tab_demo::run(),
        "scroll" => demos::scroll_demo::run(),
        "modal_toast" => demos::modal_toast_demo::run(),
        "vlist" => demos::vlist_demo::run(),
        "vgrid" => demos::vgrid_demo::run(),
        "accordion" => demos::accordion_demo::run(),
        "dialog" => demos::dialog_demo::run(),
        "search_bar" => demos::search_bar_demo::run(),
        "text_area" => demos::text_area_demo::run(),
        "advanced" => demos::advanced_widgets_demo::run(),
        _ => demos::form_demo::run(), // "form" ou qualquer outro
    }
}
