use super::*;

#[test]
fn demo_menu_contains_every_supported_entry_kind() {
    let state = DropdownMenuDemoState {
        theme: ExampleTheme::Dark,
        line_numbers: true,
        density: "Comfortable",
        last_action: String::new(),
    };
    let entries = menu_entries(&state);

    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == rutter::DropdownMenuEntryKind::Item)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == rutter::DropdownMenuEntryKind::Checkbox)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == rutter::DropdownMenuEntryKind::Submenu)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind() == rutter::DropdownMenuEntryKind::Separator)
    );
    assert!(entries.iter().any(DropdownMenuEntry::is_disabled));
}

#[test]
fn demo_updates_checkbox_and_radio_status() {
    let mut state = DropdownMenuDemoState {
        theme: ExampleTheme::Dark,
        line_numbers: true,
        density: "Comfortable",
        last_action: String::new(),
    };

    toggle_line_numbers(&mut state);
    assert!(!state.line_numbers);
    set_density(&mut state, "Compact");
    assert_eq!(state.density, "Compact");
}

#[test]
fn recent_files_exercise_scrollable_submenu() {
    let recent = recent_files_submenu();
    assert!(recent.submenu_entries().unwrap().len() > 10);
}
