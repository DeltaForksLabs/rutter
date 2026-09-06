// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — main.rs  (v6.2)
//
// Demo launcher. Pass one of the alphabetically listed names below as the
// first CLI argument; omitting it starts the complete `form` demo by default:
//   cargo run -- accordion
//   cargo run -- advanced
//   cargo run -- button_content
//   cargo run -- calendar
//   cargo run -- carousel
//   cargo run -- controls
//   cargo run -- counter
//   cargo run -- dialog
//   cargo run -- dropdown_menu
//   cargo run -- form           # complete demo (default)
//   cargo run -- image_viewer
//   cargo run -- modal_toast
//   cargo run -- multi_window
//   cargo run -- popover
//   cargo run -- progress
//   cargo run -- rich_text
//   cargo run -- scroll
//   cargo run -- search_bar
//   cargo run -- slider
//   cargo run -- tabs
//   cargo run -- table_of_contents
//   cargo run -- text_area
//   cargo run -- text_input
//   cargo run -- time
//   cargo run -- vgrid
//   cargo run -- vlist
// ============================================================

#[path = "../examples/widgets/mod.rs"]
mod widget_examples;

fn main() {
    let demo = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "form".to_string());

    match demo.as_str() {
        "accordion" => widget_examples::accordion_demo::run(),
        "advanced" => widget_examples::advanced_widgets_demo::run(),
        "button_content" => widget_examples::button_content_demo::run(),
        "calendar" => widget_examples::calendar_demo::run(),
        "carousel" => widget_examples::carousel_demo::run(),
        "controls" => widget_examples::controls_demo::run(),
        "counter" => widget_examples::counter_demo::run(),
        "dialog" => widget_examples::dialog_demo::run(),
        "dropdown_menu" => widget_examples::dropdown_menu_demo::run(),
        "form" => widget_examples::form_demo::run(),
        "image_viewer" => widget_examples::image_viewer_demo::run(),
        "modal_toast" => widget_examples::modal_toast_demo::run(),
        "multi_window" => widget_examples::multi_window_demo::run(),
        "popover" => widget_examples::popover_demo::run(),
        "progress" => widget_examples::progress_demo::run(),
        "rich_text" => widget_examples::rich_text_demo::run(),
        "scroll" => widget_examples::scroll_demo::run(),
        "search_bar" => widget_examples::search_bar_demo::run(),
        "slider" => widget_examples::slider_demo::run(),
        "tabs" => widget_examples::tab_demo::run(),
        "table_of_contents" => widget_examples::table_of_contents_demo::run(),
        "text_area" => widget_examples::text_area_demo::run(),
        "text_input" => widget_examples::text_input_demo::run(),
        "time" => widget_examples::time_demo::run(),
        "vgrid" => widget_examples::vgrid_demo::run(),
        "vlist" => widget_examples::vlist_demo::run(),
        _ => widget_examples::form_demo::run(),
    }
}
