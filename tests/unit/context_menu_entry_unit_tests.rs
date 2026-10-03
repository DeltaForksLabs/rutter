use super::ContextMenuEntry;
use crate::widgets::dropdown_menu::{
    DropdownMenuEntryAccess, DropdownMenuEntryKind, flatten_entry_paths,
};

#[test]
fn context_menu_decoration_builders_commute_merge_and_keep_borrowed_metadata() {
    let label = String::from("Copy");
    let shortcut = String::from("CTRL+C");
    let icon = Vec::from(b"<svg/>".as_slice());
    let first = ContextMenuEntry::item(&label, 42)
        .with_svg_icon(&icon)
        .with_shortcut_label(&shortcut);
    let second = ContextMenuEntry::item(&label, 42)
        .with_shortcut_label(&shortcut)
        .with_svg_icon(&icon);
    assert_eq!(first, second);
    assert_eq!(first.label().unwrap().as_ptr(), label.as_ptr());
    assert_eq!(first.svg_icon().unwrap().as_ptr(), icon.as_ptr());
    assert_eq!(first.shortcut_label().unwrap().as_ptr(), shortcut.as_ptr());
    assert_eq!(first.action_message(), Some(&42));
    let ContextMenuEntry::Decorated { entry, .. } = &first else {
        panic!("expected one wrapper")
    };
    assert!(matches!(**entry, ContextMenuEntry::Item { .. }));
    let replaced = first
        .with_shortcut_label("New")
        .with_svg_icon(b"replacement");
    assert_eq!(replaced.shortcut_label(), Some("New"));
    assert_eq!(replaced.svg_icon(), Some(b"replacement".as_slice()));
    assert_eq!(replaced.action_message(), Some(&42));
}

#[test]
fn context_menu_decorations_preserve_semantics_paths_and_separator_no_op() {
    let plain = [
        ContextMenuEntry::submenu("More", vec![ContextMenuEntry::item("Copy", 42)]),
        ContextMenuEntry::disabled("Locked"),
        ContextMenuEntry::disabled_submenu("Blocked", vec![ContextMenuEntry::item("Hidden", 9)]),
        ContextMenuEntry::separator(),
    ];
    let decorated: Vec<_> = plain
        .iter()
        .cloned()
        .map(|entry| entry.with_svg_icon(b"svg").with_shortcut_label("CTRL+C"))
        .collect();
    assert_eq!(flatten_entry_paths(&plain), flatten_entry_paths(&decorated));
    assert_eq!(
        flatten_entry_paths(&decorated),
        [vec![0], vec![0, 0], vec![1], vec![2], vec![2, 0], vec![3]]
    );
    assert_eq!(decorated[0].entry_kind(), DropdownMenuEntryKind::Submenu);
    assert_eq!(
        decorated[0].submenu_entries().unwrap()[0].action_message(),
        Some(&42)
    );
    assert!(decorated[0].action_message().is_none());
    assert!(!decorated[0].is_disabled());
    for entry in &decorated[1..3] {
        assert!(entry.is_disabled());
        assert!(entry.action_message().is_none());
        assert!(!entry.entry_is_activatable());
    }
    assert_eq!(decorated[3], ContextMenuEntry::Separator);
    assert_eq!(
        ContextMenuEntry::<()>::separator()
            .with_shortcut_label("X")
            .with_svg_icon(b"X"),
        ContextMenuEntry::Separator
    );
    assert!(!decorated[3].entry_is_focusable());
}

#[test]
fn context_menu_semantic_accessors_unwrap_manually_nested_decorations_without_clone() {
    struct NonClone(u8);
    let entry = ContextMenuEntry::Decorated {
        entry: Box::new(ContextMenuEntry::item("Copy", NonClone(42)).with_svg_icon(b"svg")),
        svg_icon: None,
        shortcut_label: Some("CTRL+C"),
    };
    assert_eq!(entry.label(), Some("Copy"));
    assert_eq!(entry.action_message().unwrap().0, 42);
    assert_eq!(entry.svg_icon(), Some(b"svg".as_slice()));
    assert_eq!(entry.shortcut_label(), Some("CTRL+C"));
    assert_eq!(entry.entry_kind(), DropdownMenuEntryKind::Item);
    assert!(!entry.is_disabled());
}
