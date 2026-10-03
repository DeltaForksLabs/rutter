use super::*;
use crate::engine::dropdown_menu_runtime::{MenuActivation, MenuHoverOutcome};
use crate::engine::widget_state::ContextMenuState;
use crate::widget::ContextMenuEntry;

fn runtime() -> DropdownMenuRuntime<u8> {
    DropdownMenuRuntime::from_context_entries(&[
        ContextMenuEntry::disabled("Locked"),
        ContextMenuEntry::separator(),
        ContextMenuEntry::submenu(
            "More",
            vec![
                ContextMenuEntry::disabled("Unavailable"),
                ContextMenuEntry::submenu("Deeper", vec![ContextMenuEntry::item("Leaf", 42)]),
            ],
        ),
        ContextMenuEntry::disabled_submenu("Blocked", vec![ContextMenuEntry::item("Hidden", 9)]),
        ContextMenuEntry::item("Run", 7),
    ])
}

fn key_step(
    runtime: &DropdownMenuRuntime<u8>,
    state: &mut DropdownMenuState,
    key: NamedKey,
    direction: LayoutDirection,
) -> Option<u8> {
    let path = state.active_path().unwrap().to_vec();
    match navigate_menu_key(runtime, state, path, &Key::Named(key), direction, false) {
        MenuKeyOutcome::Focus(path) => state.activate_path(path),
        MenuKeyOutcome::Activate(path) => match runtime.activate_entry(state, path) {
            MenuActivation::Focus(path) => state.activate_path(path),
            MenuActivation::Action(message) => return Some(message),
            MenuActivation::Ignored => {}
        },
        MenuKeyOutcome::Ignored => {}
    }
    None
}

#[test]
fn context_menu_keyboard_opens_two_levels_and_emits_only_nested_leaf() {
    for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
        let runtime = runtime();
        let mut menu = ContextMenuState::default();
        menu.open_at(100.0, 50.0);
        menu.navigation.open_at_index(Some(0));
        let forward = if direction == LayoutDirection::Ltr {
            NamedKey::ArrowRight
        } else {
            NamedKey::ArrowLeft
        };
        let backward = if direction == LayoutDirection::Ltr {
            NamedKey::ArrowLeft
        } else {
            NamedKey::ArrowRight
        };
        assert_eq!(
            key_step(&runtime, &mut menu.navigation, NamedKey::Enter, direction),
            None
        );
        assert_eq!(
            key_step(
                &runtime,
                &mut menu.navigation,
                NamedKey::ArrowDown,
                direction
            ),
            None
        );
        assert_eq!(menu.navigation.active_path(), Some([2].as_slice()));
        assert_eq!(
            key_step(&runtime, &mut menu.navigation, forward, direction),
            None
        );
        assert_eq!(menu.navigation.active_path(), Some([2, 0].as_slice()));
        assert_eq!(
            key_step(
                &runtime,
                &mut menu.navigation,
                NamedKey::ArrowDown,
                direction
            ),
            None
        );
        assert_eq!(
            key_step(&runtime, &mut menu.navigation, NamedKey::Enter, direction),
            None
        );
        assert_eq!(menu.navigation.open_submenu_path(), [2, 1]);
        assert_eq!(
            key_step(&runtime, &mut menu.navigation, NamedKey::Enter, direction),
            Some(42)
        );
        assert_eq!(
            key_step(&runtime, &mut menu.navigation, backward, direction),
            None
        );
        assert_eq!(menu.navigation.active_path(), Some([2, 1].as_slice()));
        assert_eq!(menu.navigation.open_submenu_path(), [2]);
        menu.close();
        assert!(!menu.is_open);
        assert!(!menu.navigation.is_open());
        assert!(menu.navigation.open_submenu_path().is_empty());
    }
}

#[test]
fn context_menu_disabled_submenus_separators_and_closed_descendants_never_activate() {
    let runtime = runtime();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![0]),
        MenuActivation::Focus(_)
    ));
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![1]),
        MenuActivation::Ignored
    ));
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![3]),
        MenuActivation::Focus(_)
    ));
    assert!(state.open_submenu_path().is_empty());
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![3, 0]),
        MenuActivation::Ignored
    ));
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![2, 1, 0]),
        MenuActivation::Ignored
    ));
    state.close();
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![4]),
        MenuActivation::Ignored
    ));
}

#[test]
fn context_menu_hover_switches_branches_and_preserves_nested_leaf_action() {
    let runtime = runtime();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    assert!(runtime.hover_entry(&mut state, vec![2]).changed());
    assert_eq!(state.open_submenu_path(), [2]);
    assert!(runtime.hover_entry(&mut state, vec![2, 1]).changed());
    assert_eq!(state.open_submenu_path(), [2, 1]);
    assert!(runtime.hover_entry(&mut state, vec![2, 1, 0]).changed());
    assert!(matches!(
        runtime.activate_entry(&mut state, vec![2, 1, 0]),
        MenuActivation::Action(42)
    ));
    assert!(!runtime.hover_entry(&mut state, vec![1]).changed());
    assert!(runtime.hover_entry(&mut state, vec![3]).changed());
    assert!(state.open_submenu_path().is_empty());
    assert_eq!(state.active_path(), Some([3].as_slice()));
}

#[test]
fn context_menu_same_row_hover_settles_but_restores_reveal_after_scroll() {
    let runtime = runtime();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(4));
    state.scroll_level(0, 40.0, 100.0);
    assert_eq!(
        runtime.hover_entry(&mut state, vec![4]),
        MenuHoverOutcome::Changed {
            geometry_changed: true
        }
    );
    assert!(state.should_reveal_active());
    assert_eq!(state.scroll_offset(0), 40.0);
    let settled = state.clone();
    for _ in 0..3 {
        assert_eq!(
            runtime.hover_entry(&mut state, vec![4]),
            MenuHoverOutcome::Unchanged
        );
        assert_eq!(state, settled);
    }
}

#[test]
fn context_menu_same_submenu_hover_preserves_child_scroll_then_collapses_keyboard_descendants() {
    let runtime = runtime();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(2));
    assert!(runtime.hover_entry(&mut state, vec![2]).changed());
    state.scroll_level(1, 75.0, 100.0);
    assert!(runtime.hover_entry(&mut state, vec![2]).changed());
    let settled = state.clone();
    assert_eq!(
        runtime.hover_entry(&mut state, vec![2]),
        MenuHoverOutcome::Unchanged
    );
    assert_eq!(state, settled);
    assert_eq!(state.scroll_offset(1), 75.0);

    assert!(matches!(
        runtime.activate_entry(&mut state, vec![2, 1]),
        MenuActivation::Focus(_)
    ));
    assert_eq!(state.open_submenu_path(), [2, 1]);
    assert_eq!(
        runtime.hover_entry(&mut state, vec![2]),
        MenuHoverOutcome::Changed {
            geometry_changed: true
        }
    );
    assert_eq!(state.open_submenu_path(), [2]);
    assert_eq!(state.active_path(), Some([2].as_slice()));
    assert_eq!(state.scroll_offset(1), 75.0);
    assert_eq!(
        runtime.hover_entry(&mut state, vec![2]),
        MenuHoverOutcome::Unchanged
    );
}

#[test]
fn context_menu_hover_branch_switch_resets_only_descendant_scroll_and_ignores_unreachable_rows() {
    let runtime = DropdownMenuRuntime::from_context_entries(&[
        ContextMenuEntry::submenu("First", vec![ContextMenuEntry::item("Leaf", 1)]),
        ContextMenuEntry::submenu("Second", vec![ContextMenuEntry::item("Other", 2)]),
        ContextMenuEntry::separator(),
        ContextMenuEntry::disabled_submenu("Locked", vec![ContextMenuEntry::item("Hidden", 3)]),
    ]);
    let mut state = DropdownMenuState::default();
    state.open_at_index(None);
    state.scroll_level(0, 10.0, 100.0);
    assert!(runtime.hover_entry(&mut state, vec![0]).changed());
    state.scroll_level(1, 50.0, 100.0);
    assert!(runtime.hover_entry(&mut state, vec![1]).changed());
    assert_eq!(state.open_submenu_path(), [1]);
    assert_eq!(state.scroll_offset(0), 10.0);
    assert_eq!(state.scroll_offset(1), 0.0);
    let settled = state.clone();
    for path in [vec![2], vec![0, 0], vec![3, 0], vec![99]] {
        assert_eq!(
            runtime.hover_entry(&mut state, path),
            MenuHoverOutcome::Ignored
        );
        assert_eq!(state, settled);
    }
    assert!(runtime.hover_entry(&mut state, vec![3]).changed());
    assert!(state.open_submenu_path().is_empty());
    assert_eq!(state.active_path(), Some([3].as_slice()));
    assert_eq!(
        runtime.hover_entry(&mut state, vec![3]),
        MenuHoverOutcome::Unchanged
    );
}

#[test]
fn context_menu_keyboard_wraps_and_typeahead_searches_current_level() {
    let runtime = runtime();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    key_step(
        &runtime,
        &mut state,
        NamedKey::ArrowUp,
        LayoutDirection::Ltr,
    );
    assert_eq!(state.active_path(), Some([4].as_slice()));
    key_step(&runtime, &mut state, NamedKey::Home, LayoutDirection::Ltr);
    assert_eq!(state.active_path(), Some([0].as_slice()));
    let outcome = navigate_menu_key(
        &runtime,
        &mut state,
        vec![0],
        &Key::Character("mor".into()),
        LayoutDirection::Ltr,
        false,
    );
    assert!(matches!(outcome, MenuKeyOutcome::Focus(path) if path == [2]));
    assert_eq!(state.typeahead_buffer(), "mor");
}

#[test]
fn context_menu_control_typeahead_is_suppressed_until_modifier_release() {
    let runtime = runtime();
    let mut state = DropdownMenuState::default();
    state.open_at_index(Some(0));
    let key = Key::Character("mor".into());
    assert!(matches!(
        navigate_menu_key(
            &runtime,
            &mut state,
            vec![0],
            &key,
            LayoutDirection::Ltr,
            true
        ),
        MenuKeyOutcome::Ignored
    ));
    assert!(state.typeahead_buffer().is_empty());
    assert!(
        matches!(navigate_menu_key(&runtime, &mut state, vec![0], &key, LayoutDirection::Ltr, false), MenuKeyOutcome::Focus(path) if path == [2])
    );
    assert_eq!(state.typeahead_buffer(), "mor");
}
