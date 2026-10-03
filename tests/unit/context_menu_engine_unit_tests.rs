use super::*;
use crate::ContextMenuEntry;
use crate::engine::dropdown_menu_runtime::MenuActivation;
use crate::layout::build_taffy_tree_with_direction;

struct ContextOwnerApp;

impl AppLogic for ContextOwnerApp {
    type State = ();
    type Message = u8;
    fn new(_: &mut FontSystem) {}
    fn view<'a>(_: &'a mut ()) -> Widget<'a, u8> {
        Widget::Spacer {
            style: Style::default(),
        }
    }
    fn update(_: &mut (), _: u8, _: &mut Clipboard) {}
}

fn context<'a>(entries: &'a [ContextMenuEntry<'a, u8>]) -> Widget<'a, u8> {
    Widget::context_menu(
        Widget::Spacer {
            style: Style {
                size: taffy::prelude::Size {
                    width: taffy::prelude::Dimension::length(200.0),
                    height: taffy::prelude::Dimension::length(60.0),
                },
                ..Style::default()
            },
        },
        entries,
        Style::default(),
    )
    .with_id(7)
}

// Exercise the production metadata and ownership collectors without constructing
// a native window or a display-backed clipboard.
fn collect_runtime(
    widget: &Widget<'_, u8>,
    states: &mut HashMap<u64, WidgetState>,
) -> WidgetRuntimeCaches<u8> {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree_with_direction(
        &mut taffy,
        widget,
        fonts.clone(),
        states,
        crate::i18n::LayoutDirection::Ltr,
    );
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(800, 500),
        fonts,
        &crate::render::RichTextRenderer::default(),
    );
    let mut caches = WidgetRuntimeCaches::default();
    RutterEngine::<ContextOwnerApp>::sync_runtime_metadata(
        &mut caches,
        states,
        &mut HashMap::new(),
        RuntimeMetadataSources {
            input_states: &HashMap::new(),
            taffy: &taffy,
        },
        widget,
        Some(root),
        8.0,
    )
    .unwrap();
    caches.visible_context_menu_owners =
        crate::render::select_overlay::collector::collect_context_menu_owners(
            widget,
            &taffy,
            root,
            states,
            (800.0, 500.0),
        )
        .into_keys()
        .collect();
    caches
}

#[test]
fn context_menu_decoration_reconstruction_preserves_navigation_typeahead_and_actions() {
    let plain = [ContextMenuEntry::submenu(
        "More",
        vec![ContextMenuEntry::item("Copy", 42)],
    )];
    let decorated = [ContextMenuEntry::submenu(
        "More",
        vec![
            ContextMenuEntry::item("Copy", 99)
                .with_svg_icon(b"invalid")
                .with_shortcut_label("CTRL+C"),
        ],
    )
    .with_shortcut_label("ALT+M")
    .with_svg_icon(b"different")];
    let mut menu = ContextMenuState::default();
    menu.open_at(20.0, 20.0);
    menu.navigation.expand_submenu(vec![0], Some(0));
    menu.navigation
        .update_typeahead("co", std::time::Instant::now());
    let mut states = HashMap::from([(7, WidgetState::ContextMenu(menu))]);
    let before = collect_runtime(&context(&plain), &mut states);
    let after = collect_runtime(&context(&decorated), &mut states);
    assert!(before.dropdown_menus[&7].has_same_topology(&after.dropdown_menus[&7]));
    reconcile_context_menu_owners(&after, &mut states);
    assert!(close_changed_dropdown_topologies(&before, &after, &mut states).is_empty());
    let menu = states.get_mut(&7).unwrap().as_context_menu_mut().unwrap();
    assert!(menu.is_open);
    assert_eq!(menu.navigation.open_submenu_path(), [0]);
    assert_eq!(menu.navigation.active_path(), Some([0, 0].as_slice()));
    assert_eq!(menu.navigation.typeahead_buffer(), "co");
    assert_eq!(
        after.dropdown_menus[&7].typeahead_path(&[0, 0], "co"),
        Some(vec![0, 0])
    );
    assert_eq!(
        after.dropdown_menus[&7].typeahead_path(&[0, 0], "ctrl"),
        None
    );
    assert!(matches!(
        after.dropdown_menus[&7].activate_entry(&mut menu.navigation, vec![0, 0]),
        MenuActivation::Action(99)
    ));
}

#[test]
fn context_menu_post_opening_runtime_keeps_updated_submenu_actions_reachable() {
    let old_entries = [ContextMenuEntry::disabled_submenu(
        "More",
        vec![ContextMenuEntry::disabled("Old")],
    )];
    let new_entries = [ContextMenuEntry::submenu(
        "More",
        vec![ContextMenuEntry::submenu(
            "Deeper",
            vec![ContextMenuEntry::item("Leaf", 42)],
        )],
    )];
    let mut states = HashMap::from([(7, WidgetState::ContextMenu(ContextMenuState::default()))]);
    let before = collect_runtime(&context(&old_entries), &mut states);
    let after = collect_runtime(&context(&new_entries), &mut states);
    assert!(!before.dropdown_menus[&7].has_same_topology(&after.dropdown_menus[&7]));
    reconcile_context_menu_owners(&after, &mut states);
    close_changed_dropdown_topologies(&before, &after, &mut states);
    states
        .get_mut(&7)
        .unwrap()
        .as_context_menu_mut()
        .unwrap()
        .open_at(20.0, 20.0);
    let next = collect_runtime(&context(&new_entries), &mut states);
    reconcile_context_menu_owners(&next, &mut states);
    assert!(close_changed_dropdown_topologies(&after, &next, &mut states).is_empty());
    let menu = states.get_mut(&7).unwrap().as_context_menu_mut().unwrap();
    assert!(menu.is_open);
    for path in [vec![0], vec![0, 0]] {
        let MenuActivation::Focus(path) =
            next.dropdown_menus[&7].activate_entry(&mut menu.navigation, path)
        else {
            panic!("updated submenu must open");
        };
        menu.navigation.activate_path(path);
    }
    assert!(matches!(
        next.dropdown_menus[&7].activate_entry(&mut menu.navigation, vec![0, 0, 0]),
        MenuActivation::Action(42)
    ));
}

#[test]
fn context_menu_hidden_collapsed_disabled_and_removed_owners_close_retained_navigation() {
    let entries = [ContextMenuEntry::submenu(
        "More",
        vec![ContextMenuEntry::item("Leaf", 42)],
    )];
    let owner = context(&entries);
    let hidden_modal = Widget::modal(false, context(&entries), None, Style::default()).with_id(50);
    let hidden_dialog = Widget::dialog(
        "Title",
        "Message",
        "Confirm",
        "Cancel",
        false,
        1,
        2,
        None,
        Style::default(),
        context(&entries),
    )
    .with_id(51);
    let collapsed = Widget::Accordion {
        id: 52,
        title: "Group",
        expanded: false,
        on_toggle: 0,
        child: Box::new(context(&entries)),
        style: Style::default(),
    };
    let disabled = context(&entries).enabled(false);
    let removed = Widget::Spacer {
        style: Style::default(),
    };
    for widget in [hidden_modal, hidden_dialog, collapsed, disabled, removed] {
        let mut menu = ContextMenuState::default();
        menu.open_at(20.0, 20.0);
        menu.navigation.expand_submenu(vec![0], Some(0));
        let mut states = HashMap::from([(7, WidgetState::ContextMenu(menu))]);
        let before = collect_runtime(&owner, &mut states);
        let next = collect_runtime(&widget, &mut states);
        reconcile_context_menu_owners(&next, &mut states);
        let menu = states.get_mut(&7).unwrap().as_context_menu_mut().unwrap();
        assert!(!menu.is_open);
        assert!(menu.navigation.open_submenu_path().is_empty());
        assert!(matches!(
            before.dropdown_menus[&7].activate_entry(&mut menu.navigation, vec![0, 0]),
            MenuActivation::Ignored
        ));
    }
}

#[test]
fn context_menu_owner_covered_by_a_modal_cannot_retain_keyboard_capture() {
    let entries = [ContextMenuEntry::item("Run", 42)];
    let modal = Widget::modal(
        true,
        Widget::Spacer {
            style: Style::default(),
        },
        None,
        Style::default(),
    )
    .with_id(50);
    let widget = Widget::Column {
        children: vec![context(&entries), modal],
        style: Style::default(),
    };
    let mut menu = ContextMenuState::default();
    menu.open_at(20.0, 20.0);
    let mut states = HashMap::from([(7, WidgetState::ContextMenu(menu))]);
    let next = collect_runtime(&widget, &mut states);
    assert!(next.dropdown_menus.contains_key(&7));
    assert!(!next.visible_context_menu_owners.contains(&7));
    reconcile_context_menu_owners(&next, &mut states);
    assert!(!states[&7].as_context_menu().unwrap().is_open);
}

#[test]
fn context_menu_visible_modal_and_accordion_owners_keep_retained_navigation() {
    let entries = [ContextMenuEntry::submenu(
        "More",
        vec![ContextMenuEntry::item("Leaf", 42)],
    )];
    let modal = Widget::modal(true, context(&entries), None, Style::default()).with_id(50);
    let accordion = Widget::Accordion {
        id: 52,
        title: "Group",
        expanded: true,
        on_toggle: 0,
        child: Box::new(context(&entries)),
        style: Style::default(),
    };
    for widget in [modal, accordion] {
        let mut menu = ContextMenuState::default();
        menu.open_at(20.0, 20.0);
        menu.navigation.expand_submenu(vec![0], Some(0));
        let mut states = HashMap::from([(7, WidgetState::ContextMenu(menu))]);
        let next = collect_runtime(&widget, &mut states);
        assert!(next.visible_context_menu_owners.contains(&7));
        reconcile_context_menu_owners(&next, &mut states);
        let menu = states[&7].as_context_menu().unwrap();
        assert!(menu.is_open);
        assert_eq!(menu.navigation.open_submenu_path(), [0]);
    }
}

#[test]
fn context_menu_missing_runtime_closes_even_if_owner_visibility_is_stale() {
    let mut menu = ContextMenuState::default();
    menu.open_at(20.0, 20.0);
    let mut states = HashMap::from([(7, WidgetState::ContextMenu(menu))]);
    let mut next = WidgetRuntimeCaches::<u8>::default();
    next.visible_context_menu_owners.insert(7);
    reconcile_context_menu_owners(&next, &mut states);
    assert!(!states[&7].as_context_menu().unwrap().is_open);
}

#[test]
fn context_menu_direct_open_flag_initializes_visible_flat_navigation() {
    let entries = [ContextMenuEntry::item("Run", 42)];
    let menu = ContextMenuState {
        is_open: true,
        ..ContextMenuState::default()
    };
    let mut states = HashMap::from([(7, WidgetState::ContextMenu(menu))]);
    let next = collect_runtime(&context(&entries), &mut states);
    reconcile_context_menu_owners(&next, &mut states);
    let menu = states.get_mut(&7).unwrap().as_context_menu_mut().unwrap();
    assert!(menu.is_open && menu.navigation.is_open());
    assert!(matches!(
        next.dropdown_menus[&7].activate_entry(&mut menu.navigation, vec![0]),
        MenuActivation::Action(42)
    ));
}
