use super::*;
use crate::DropdownMenuEntry;

#[test]
fn dropdown_item_focus_is_live_only_while_parent_overlay_is_visible() {
    let mut caches = WidgetRuntimeCaches::<()>::default();
    caches.dropdown_menu_items.insert(
        9,
        DropdownMenuItemRuntime {
            parent_id: 7,
            path: vec![0],
        },
    );

    assert!(!runtime_focus_is_live(&caches, 9));
    caches.visible_dropdown_menus.insert(7);
    assert!(runtime_focus_is_live(&caches, 9));
}

#[test]
fn dropdown_trigger_focus_requires_accessible_overlay_ownership() {
    let mut caches = WidgetRuntimeCaches::<()>::default();
    caches.dropdown_menus.insert(7, runtime_with_label("Run"));
    caches.focus_order.push(7);

    assert!(!runtime_focus_is_live(&caches, 7));
    caches.visible_dropdown_triggers.insert(7);
    assert!(runtime_focus_is_live(&caches, 7));
}

#[test]
fn suppressed_dropdown_state_closes_before_render_and_accessibility() {
    let entries = [DropdownMenuEntry::item("Run", ())];
    let mut menu = crate::dropdown_menu::DropdownMenuState::default();
    menu.open_at_first(&entries);
    let mut states = HashMap::from([(7, WidgetState::DropdownMenu(menu))]);

    let closed = close_suppressed_dropdowns(&mut states, &HashSet::new());

    assert_eq!(closed, HashSet::from([7]));
    assert!(!states[&7].as_dropdown_menu().unwrap().is_open());
}

#[test]
fn suppressed_dropdown_item_focus_is_detected_for_overlay_transfer() {
    let mut caches = WidgetRuntimeCaches::<()>::default();
    caches.dropdown_menu_items.insert(
        9,
        DropdownMenuItemRuntime {
            parent_id: 7,
            path: vec![0],
        },
    );

    assert!(focus_belongs_to_dropdowns(
        &caches,
        Some(9),
        &HashSet::from([7])
    ));
}

#[test]
fn empty_modal_focus_scope_falls_back_to_dialog_node() {
    let modal = Widget::modal(
        true,
        Widget::Spacer {
            style: Style::default(),
        },
        None::<()>,
        Style::default(),
    )
    .with_id(50);
    let mut focus_order = Vec::new();

    collect_focus_order(&modal, &mut focus_order);

    assert_eq!(focus_order, vec![50]);
}

#[test]
fn overlay_focus_transfer_skips_fully_clipped_dropdown_trigger() {
    let mut caches = WidgetRuntimeCaches::<()>::default();
    caches.dropdown_menus.insert(7, runtime_with_label("Run"));
    caches.focus_order.extend([7, 8]);

    assert_eq!(first_live_focus_id(&caches), Some(8));
}

#[test]
fn changed_dropdown_topology_closes_retained_menu_state() {
    let mut current = WidgetRuntimeCaches::<()>::default();
    let mut next = WidgetRuntimeCaches::<()>::default();
    current.dropdown_menus.insert(7, runtime_with_label("Run"));
    next.dropdown_menus.insert(7, runtime_with_label("Save"));
    let entries = [DropdownMenuEntry::item("Run", ())];
    let mut menu = crate::dropdown_menu::DropdownMenuState::default();
    menu.open_at_first(&entries);
    let mut states = HashMap::from([(7, WidgetState::DropdownMenu(menu))]);

    let changed = close_changed_dropdown_topologies(&current, &next, &mut states);

    assert_eq!(changed, HashSet::from([7]));
    assert!(!states[&7].as_dropdown_menu().unwrap().is_open());
}

fn runtime_with_label(label: &'static str) -> DropdownMenuRuntime<()> {
    let entries = vec![DropdownMenuEntry::item(label, ())];
    let widget = Widget::dropdown_menu("Actions", entries.clone(), Style::default()).with_id(7);
    DropdownMenuRuntime::from_widget(&widget, &[], &entries)
}
