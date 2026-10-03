use super::*;

#[test]
fn demo_wraps_a_visible_right_click_area_in_context_menu() {
    let mut state = ContextMenuDemoState::default();
    let Widget::Column { children, .. } = ContextMenuDemo::view(&mut state) else {
        panic!("expected widget demo column");
    };
    let menu = children
        .iter()
        .find(|child| matches!(child, Widget::ContextMenu { id, .. } if *id == CONTEXT_MENU_ID))
        .expect("expected ContextMenu with a stable ID");
    let Widget::ContextMenu { child, entries, .. } = menu else {
        unreachable!();
    };
    assert!(matches!(
        child.as_ref(),
        Widget::Container { color: Some(_), .. }
    ));
    assert_eq!(entries.len(), 8);
    assert!(children.iter().any(|child| matches!(child,
        Widget::Text { content, .. } if content.contains("Right-click the highlighted area"))));
}

#[test]
fn demo_nested_leaf_updates_the_displayed_last_action() {
    let mut state = ContextMenuDemoState::default();
    let organize = find_entry(&state.entries, "Organize");
    assert!(!organize.is_disabled());
    let move_to = find_entry(organize.submenu_entries().unwrap(), "Move to");
    assert!(!move_to.is_disabled());
    let projects = find_entry(move_to.submenu_entries().unwrap(), "Projects");
    let message = projects.action_message().unwrap().clone();
    apply_demo_message(&mut state, message);
    let Widget::Column { children, .. } = ContextMenuDemo::view(&mut state) else {
        panic!("expected widget demo column");
    };
    assert!(children.iter().any(|child| matches!(child,
        Widget::Text { content, .. } if content == "Last action: Selected: Move to Projects")));
}

#[test]
fn demo_includes_separator_disabled_item_and_disabled_submenu() {
    let state = ContextMenuDemoState::default();
    assert!(
        state
            .entries
            .iter()
            .any(|entry| matches!(entry, ContextMenuEntry::Separator))
    );
    let paste = find_entry(&state.entries, "Paste (unavailable)");
    assert!(paste.is_disabled());
    assert!(paste.action_message().is_none());
    let organize = find_entry(&state.entries, "Organize");
    let sync = find_entry(organize.submenu_entries().unwrap(), "Sync (offline)");
    assert!(sync.is_disabled());
    assert!(sync.submenu_entries().is_some());
}

#[test]
fn theme_switch_preserves_menu_entries_and_selected_action() {
    let mut state = ContextMenuDemoState::default();
    let entries = state.entries.clone();
    apply_demo_message(&mut state, Msg::Action("Rename"));
    apply_demo_message(&mut state, Msg::ThemeChanged(ExampleTheme::Light));
    assert_eq!(
        ContextMenuDemo::theme_for(&state).surface,
        Theme::light().surface
    );
    assert_eq!(state.last_action, "Selected: Rename");
    assert_eq!(state.entries, entries);
}

fn find_entry<'a>(
    entries: &'a [ContextMenuEntry<'static, Msg>],
    label: &str,
) -> &'a ContextMenuEntry<'static, Msg> {
    entries
        .iter()
        .find(|entry| entry.label() == Some(label))
        .expect("expected demo entry")
}

fn shortcut_event(key: ShortcutKey, control: bool) -> ShortcutEvent {
    ShortcutEvent {
        key,
        control,
        alt: false,
        shift: false,
        super_key: false,
        repeat: false,
    }
}

#[test]
fn demo_copy_icon_and_shortcut_dispatch_the_same_action() {
    let mut state = ContextMenuDemoState::default();
    let copy = find_entry(&state.entries, "Copy");
    assert_eq!(copy.svg_icon(), Some(COPY_ICON_SVG));
    assert_eq!(copy.shortcut_label(), Some("CTRL+C"));
    let click_message = copy.action_message().unwrap().clone();
    let event = shortcut_event(ShortcutKey::Character("c".into()), true);
    let ShortcutOutcome::Message(message) = ContextMenuDemo::shortcut(&state, event) else {
        panic!("expected CTRL+C to dispatch Copy");
    };
    assert_eq!(message, click_message);
    apply_demo_message(&mut state, message);
    assert_eq!(state.last_action, "Selected: Copy");
}

#[test]
fn demo_shortcuts_respect_modifiers_disabled_paste_and_repeat() {
    let state = ContextMenuDemoState::default();
    let copy = shortcut_event(ShortcutKey::Character("C".into()), true);
    assert_eq!(
        ContextMenuDemo::shortcut(&state, copy.clone()),
        ShortcutOutcome::Message(Msg::Action("Copy"))
    );
    let mut ignored = vec![
        shortcut_event(ShortcutKey::Character("c".into()), false),
        shortcut_event(ShortcutKey::Character("v".into()), true),
    ];
    for modifier in 0..3 {
        let mut event = copy.clone();
        match modifier {
            0 => event.shift = true,
            1 => event.alt = true,
            _ => event.super_key = true,
        }
        ignored.push(event);
    }
    for event in ignored {
        assert_eq!(
            ContextMenuDemo::shortcut(&state, event),
            ShortcutOutcome::Ignored
        );
    }
    let mut repeat = copy;
    repeat.repeat = true;
    assert_eq!(
        ContextMenuDemo::shortcut(&state, repeat),
        ShortcutOutcome::Consumed
    );
}

#[test]
fn demo_enabled_shortcut_labels_have_corresponding_bindings() {
    let state = ContextMenuDemoState::default();
    for (label, key, control) in [
        ("Open", ShortcutKey::Character("o".into()), true),
        (
            "Rename",
            ShortcutKey::Named(ShortcutNamedKey::Function(2)),
            false,
        ),
        (
            "Delete",
            ShortcutKey::Named(ShortcutNamedKey::Delete),
            false,
        ),
    ] {
        let entry = find_entry(&state.entries, label);
        assert!(entry.svg_icon().is_some());
        assert!(entry.shortcut_label().is_some());
        assert_eq!(
            ContextMenuDemo::shortcut(&state, shortcut_event(key, control)),
            ShortcutOutcome::Message(entry.action_message().unwrap().clone())
        );
    }
    let paste = find_entry(&state.entries, "Paste (unavailable)");
    assert_eq!(paste.shortcut_label(), Some("CTRL+V"));
    assert!(paste.is_disabled());
}
