use super::*;
use crate::DropdownMenuEntry;
use taffy::prelude::Style;

#[test]
fn dropdown_menu_owners_follow_recursive_non_separator_paths() {
    let children = vec![
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::item("Export", 1),
    ];
    let entries = vec![
        DropdownMenuEntry::disabled_item("Unavailable"),
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::submenu("More", children),
    ];
    let widget = Widget::dropdown_menu("File", entries, Style::default()).with_id(41);
    let snapshot = WidgetIdSnapshot::capture(&widget).unwrap();
    let root_popup = widget.dropdown_menu_popup_id(&[]).unwrap();
    let submenu_popup = widget.dropdown_menu_submenu_popup_id(&[], &[2]).unwrap();
    let disabled = widget.dropdown_menu_item_focus_id(&[], &[0]).unwrap();
    let nested = widget.dropdown_menu_item_focus_id(&[], &[2, 1]).unwrap();

    assert_eq!(snapshot.owners.len(), 6);
    assert_eq!(snapshot.owners[&41].widget_type, "DropdownMenu");
    assert_eq!(
        snapshot.owners[&root_popup].family,
        WidgetIdTag::DropdownMenuPopup
    );
    assert_eq!(
        snapshot.owners[&submenu_popup].family,
        WidgetIdTag::DropdownMenuPopup
    );
    assert_eq!(snapshot.owners[&disabled].widget_type, "DropdownMenuItem");
    assert_eq!(snapshot.owners[&nested].path, vec![2, 1]);
    let separator = widget.dropdown_menu_item_focus_id(&[], &[1]).unwrap();
    assert!(!snapshot.owners.contains_key(&separator));
    assert_ne!(
        nested,
        widget.dropdown_menu_item_focus_id(&[], &[1, 2]).unwrap()
    );
    let tags = [
        WidgetIdTag::DropdownMenu,
        WidgetIdTag::DropdownMenuPopup,
        WidgetIdTag::DropdownMenuItem,
    ];
    assert_eq!(tags.map(|tag| tag as u64), [27, 28, 29]);
}
