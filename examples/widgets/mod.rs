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
pub mod color_picker_demo;
pub mod combo_box_demo;
pub mod context_menu_demo;
pub mod controls_demo;
pub mod counter_demo;
pub mod drag_drop_demo;
pub mod dropdown_menu_demo;
pub mod dynamic_island_demo;
pub mod form_demo;
pub mod modal_toast_demo;
pub mod multi_window_demo;
pub mod popover_demo;
pub mod progress_demo;
pub mod rich_text_demo;
pub mod scroll_demo;
pub mod slider_demo;
pub mod tab_demo;
pub mod table_demo;
pub mod table_of_contents_demo;
pub mod text_input_demo;
pub mod theme_selector;
pub mod time_demo;
pub mod vgrid_demo;
pub mod vlist_demo;

pub mod accordion_demo;
pub mod advanced_widgets_demo;
pub mod button_content_demo;
pub mod dialog_demo;
pub mod image_viewer_demo;
mod layout;
pub mod search_bar_demo;
pub mod text_area_demo;

#[cfg(test)]
mod tests {
    #[test]
    fn each_example_app_has_its_own_directory_and_entry_point() {
        let apps = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/apps");
        for entry in std::fs::read_dir(apps).expect("read example apps") {
            let entry = entry.expect("read example app entry");
            assert!(entry.file_type().expect("read example app type").is_dir());
            assert!(
                entry.path().join("main.rs").is_file(),
                "app entry point: {entry:?}"
            );
        }
    }

    #[test]
    fn example_directories_keep_apps_separate_and_widgets_self_contained() {
        let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let mut directories = Vec::new();
        for entry in std::fs::read_dir(&examples).expect("read examples directory") {
            let entry = entry.expect("read example entry");
            assert!(entry.file_type().expect("read example type").is_dir());
            directories.push(entry.file_name());
        }
        directories.sort();
        assert_eq!(directories, ["apps", "widgets"]);
        for entry in std::fs::read_dir(examples.join("widgets")).expect("read widget examples") {
            let entry = entry.expect("read widget example entry");
            assert!(
                entry
                    .file_type()
                    .expect("read widget example type")
                    .is_file()
            );
            let name = entry.file_name();
            let name = name.to_str().expect("UTF-8 widget example filename");
            assert!(
                !name.ends_with("_tests.rs"),
                "isolated widget tests: {name}"
            );
            if name.ends_with(".rs") {
                let source = std::fs::read_to_string(entry.path()).expect("read widget example");
                assert!(
                    !source
                        .lines()
                        .any(|line| line.trim_start().starts_with("#[path")),
                    "external widget module: {name}"
                );
            }
        }
    }

    #[test]
    fn all_example_sources_use_the_shared_spacing_vocabulary() {
        macro_rules! sources {
            ($($file:literal),+ $(,)?) => { [$(($file, include_str!(concat!("../", $file)))),+] };
        }
        let sources = sources!(
            "widgets/accordion_demo.rs",
            "widgets/advanced_widgets_demo.rs",
            "widgets/button_content_demo.rs",
            "widgets/calendar_demo.rs",
            "widgets/carousel_demo.rs",
            "widgets/color_picker_demo.rs",
            "widgets/combo_box_demo.rs",
            "widgets/context_menu_demo.rs",
            "widgets/controls_demo.rs",
            "widgets/counter_demo.rs",
            "widgets/dialog_demo.rs",
            "widgets/drag_drop_demo.rs",
            "widgets/dropdown_menu_demo.rs",
            "widgets/dynamic_island_demo.rs",
            "widgets/form_demo.rs",
            "widgets/image_viewer_demo.rs",
            "widgets/modal_toast_demo.rs",
            "widgets/multi_window_demo.rs",
            "widgets/popover_demo.rs",
            "widgets/progress_demo.rs",
            "widgets/rich_text_demo.rs",
            "widgets/scroll_demo.rs",
            "widgets/search_bar_demo.rs",
            "widgets/slider_demo.rs",
            "widgets/tab_demo.rs",
            "widgets/table_demo.rs",
            "widgets/table_of_contents_demo.rs",
            "widgets/text_area_demo.rs",
            "widgets/text_input_demo.rs",
            "widgets/time_demo.rs",
            "widgets/vgrid_demo.rs",
            "widgets/vlist_demo.rs",
            "apps/native_popup_clock/main.rs",
        );
        assert_eq!(sources.len(), 33);
        for (name, source) in sources {
            assert!(
                source.contains("rutter::spacing"),
                "{name}: import public spacing tokens"
            );
            // Only authored spacing declarations are inspected: calendar/time-picker
            // generated internals, typography, hit targets and borders are not spacing.
            for marker in ["padding:", "gap:", "margin:"] {
                for declaration in source.split(marker).skip(1) {
                    let end = declaration.find("},").unwrap_or(declaration.len());
                    let declaration = &declaration[..end];
                    let declaration = if declaration.trim_start().starts_with("Size {")
                        || declaration.trim_start().starts_with("Rect {")
                    {
                        declaration
                    } else {
                        declaration.split('\n').next().unwrap()
                    };
                    assert!(
                        !declaration.contains("::length(1")
                            && !declaration.contains("::length(2")
                            && !declaration.contains("::length(3")
                            && !declaration.contains("::length(4")
                            && !declaration.contains("::length(5")
                            && !declaration.contains("::length(6")
                            && !declaration.contains("::length(7")
                            && !declaration.contains("::length(8")
                            && !declaration.contains("::length(9"),
                        "{name}: off-scale authored {marker} {declaration}"
                    );
                    if declaration.contains("gap_size(") {
                        assert!(
                            declaration.contains("spacing::"),
                            "{name}: numeric gap alias"
                        );
                    }
                }
            }
        }
    }
}
