use rutter::{DropdownMenuEntry, DropdownMenuEntryKind, Widget, WidgetIdSnapshot};
use taffy::prelude::{Dimension, Size, Style};

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Run,
    Toggle,
}

#[test]
fn dropdown_menu_constructor_owns_recursive_entries_and_trigger_style() {
    let style = Style {
        size: Size {
            width: Dimension::length(180.0),
            height: Dimension::length(40.0),
        },
        ..Style::default()
    };
    let widget = Widget::dropdown_menu(
        "Actions",
        vec![DropdownMenuEntry::submenu(
            "More",
            vec![DropdownMenuEntry::item("Run", Msg::Run)],
        )],
        style,
    );

    let Widget::DropdownMenu {
        label,
        entries,
        style,
        ..
    } = widget
    else {
        panic!("expected Widget::DropdownMenu, got a different widget variant");
    };
    assert_eq!(label, "Actions");
    assert_eq!(style.size.width, Dimension::length(180.0));
    assert_eq!(entries[0].kind(), DropdownMenuEntryKind::Submenu);
    assert_eq!(
        entries[0].submenu_entries().unwrap()[0].label(),
        Some("Run")
    );
}

#[test]
fn dropdown_menu_identity_is_stable_across_state_value_changes() {
    let first = Widget::dropdown_menu(
        "View",
        vec![DropdownMenuEntry::checkbox("Grid", false, Msg::Toggle)],
        Style::default(),
    )
    .with_id(77);
    let next = Widget::dropdown_menu(
        "View",
        vec![DropdownMenuEntry::checkbox("Grid", true, Msg::Toggle)],
        Style::default(),
    )
    .with_id(77);

    let first_snapshot = WidgetIdSnapshot::capture(&first).unwrap();
    let next_snapshot = WidgetIdSnapshot::capture(&next).unwrap();

    first_snapshot
        .validate_reconstruction(&next_snapshot)
        .unwrap();
}
