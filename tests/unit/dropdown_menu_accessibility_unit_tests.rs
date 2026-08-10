use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use accesskit::{Action, HasPopup, Role, TreeUpdate};
use cosmic_text::FontSystem;
use taffy::prelude::{Dimension, Size, Style, TaffyTree};
use winit::dpi::PhysicalSize;

use super::*;
use crate::accessibility::{AccessibilityInputs, build_accessibility_update};
use crate::engine::widget_state::{ScrollState, WidgetState};
use crate::layout::{build_taffy_tree, compute_layout};
use crate::{DropdownMenuEntry, Widget};

fn dropdown_update(open_submenu: bool) -> TreeUpdate {
    let entries = vec![
        DropdownMenuEntry::item("Open", ()),
        DropdownMenuEntry::separator(),
        DropdownMenuEntry::disabled_checkbox("Locked", true),
        DropdownMenuEntry::submenu("More", vec![DropdownMenuEntry::radio("Compact", true, ())]),
    ];
    let mut state = DropdownMenuState::default();
    if open_submenu {
        state.open_submenu(&entries, vec![3]);
    } else {
        state.open_at_first(&entries);
    }
    let widget = Widget::dropdown_menu("Actions", entries, trigger_style()).with_id(42);
    build_update(widget, WidgetState::DropdownMenu(state), None)
}

fn closed_dropdown_update() -> TreeUpdate {
    let widget = Widget::dropdown_menu(
        "Actions",
        vec![DropdownMenuEntry::item("Open", ())],
        trigger_style(),
    )
    .with_id(42);
    build_update(
        widget,
        WidgetState::DropdownMenu(DropdownMenuState::default()),
        None,
    )
}

fn build_update(widget: Widget<'_, ()>, state: WidgetState, focus: Option<u64>) -> TreeUpdate {
    let states = HashMap::from([(42, state)]);
    build_update_with_states(widget, states, focus)
}

fn build_update_with_states(
    widget: Widget<'_, ()>,
    states: HashMap<u64, WidgetState>,
    focus: Option<u64>,
) -> TreeUpdate {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, &widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(500, 400),
        fonts,
        &crate::render::RichTextRenderer::default(),
    );
    build_accessibility_update(
        &taffy,
        &widget,
        root,
        AccessibilityInputs {
            input_states: &HashMap::new(),
            widget_states: &states,
            focused_widget_id: focus,
            viewport: (500.0, 400.0),
            direction: crate::i18n::LayoutDirection::Ltr,
        },
    )
}

fn trigger_style() -> Style {
    Style {
        size: Size {
            width: Dimension::length(140.0),
            height: Dimension::length(36.0),
        },
        ..Style::default()
    }
}

fn nodes_with_role(update: &TreeUpdate, role: Role) -> Vec<&accesskit::Node> {
    update
        .nodes
        .iter()
        .filter_map(|(_, node)| (node.role() == role).then_some(node))
        .collect()
}

#[test]
fn closed_dropdown_exposes_collapsed_menu_button_only() {
    let update = closed_dropdown_update();
    let trigger = &nodes_with_role(&update, Role::Button)[0];

    assert_eq!(trigger.label(), Some("Actions"));
    assert_eq!(trigger.has_popup(), Some(HasPopup::Menu));
    assert_eq!(trigger.is_expanded(), Some(false));
    assert!(trigger.supports_action(Action::Expand));
    assert!(nodes_with_role(&update, Role::Menu).is_empty());
}

#[test]
fn open_dropdown_exposes_roles_disabled_state_and_omits_separator() {
    let update = dropdown_update(false);
    let trigger = &nodes_with_role(&update, Role::Button)[0];
    let menus = nodes_with_role(&update, Role::Menu);
    let items = nodes_with_role(&update, Role::MenuItem);
    let checks = nodes_with_role(&update, Role::MenuItemCheckBox);

    assert_eq!(trigger.is_expanded(), Some(true));
    assert_eq!(menus.len(), 1);
    assert!(!menus[0].clips_children());
    assert_eq!(items.len(), 2);
    assert_eq!(checks.len(), 1);
    assert!(checks[0].is_disabled());
    assert!(checks[0].supports_action(Action::Focus));
    assert!(!checks[0].supports_action(Action::Click));
}

#[test]
fn open_submenu_exposes_nested_menu_and_radio_item() {
    let update = dropdown_update(true);
    let menus = nodes_with_role(&update, Role::Menu);
    let radios = nodes_with_role(&update, Role::MenuItemRadio);

    assert_eq!(menus.len(), 2);
    assert_eq!(radios.len(), 1);
    assert_eq!(radios[0].label(), Some("Compact"));
    assert_eq!(radios[0].toggled(), Some(accesskit::Toggled::True));
}

#[test]
fn closed_dropdown_falls_back_from_hidden_item_focus_to_tree_root() {
    let widget = Widget::dropdown_menu(
        "Actions",
        vec![DropdownMenuEntry::item("Open", ())],
        trigger_style(),
    )
    .with_id(42);
    let hidden_item = widget.dropdown_menu_item_focus_id(&[], &[0]).unwrap();
    let update = build_update(
        widget,
        WidgetState::DropdownMenu(DropdownMenuState::default()),
        Some(hidden_item),
    );

    assert_eq!(update.focus, accesskit::NodeId(0));
}

#[test]
fn clipped_closed_trigger_uses_visible_scroll_view_bounds() {
    let menu = Widget::dropdown_menu(
        "Actions",
        vec![DropdownMenuEntry::item("Open", ())],
        Style {
            flex_shrink: 0.0,
            ..trigger_style()
        },
    )
    .with_id(42);
    let widget = Widget::scroll_view(
        menu,
        Style {
            size: Size {
                width: Dimension::length(140.0),
                height: Dimension::length(20.0),
            },
            ..Style::default()
        },
    )
    .with_id(41);
    let states = HashMap::from([
        (
            41,
            WidgetState::Scroll(ScrollState {
                offset_y: 10.0,
                content_height: 36.0,
                viewport_h: 20.0,
            }),
        ),
        (42, WidgetState::DropdownMenu(DropdownMenuState::default())),
    ]);

    let update = build_update_with_states(widget, states, None);
    let bounds = nodes_with_role(&update, Role::Button)[0].bounds().unwrap();

    assert_eq!(bounds.y0, 0.0);
    assert_eq!(bounds.y1, 20.0);
}

#[test]
fn stale_disabled_submenu_omits_unreachable_descendants() {
    let enabled = vec![DropdownMenuEntry::submenu(
        "More",
        vec![DropdownMenuEntry::item("Child", ())],
    )];
    let mut state = DropdownMenuState::default();
    assert!(state.open_submenu(&enabled, vec![0]));
    let widget = Widget::dropdown_menu(
        "Actions",
        vec![DropdownMenuEntry::disabled_submenu(
            "More",
            vec![DropdownMenuEntry::item("Child", ())],
        )],
        trigger_style(),
    )
    .with_id(42);

    let update = build_update(widget, WidgetState::DropdownMenu(state), None);
    let submenu = nodes_with_role(&update, Role::MenuItem)[0];

    assert_eq!(nodes_with_role(&update, Role::Menu).len(), 1);
    assert_eq!(submenu.is_expanded(), Some(false));
    assert!(submenu.is_disabled());
}

#[test]
fn long_menu_item_bounds_stay_inside_unclipped_popup_node() {
    let entries = (0..30)
        .map(|_| DropdownMenuEntry::item("Entry", ()))
        .enumerate()
        .map(|(index, entry)| entry.with_key(index as u64))
        .collect::<Vec<_>>();
    let mut state = DropdownMenuState::default();
    state.open_at_first(&entries);
    let widget = Widget::dropdown_menu("Actions", entries, trigger_style()).with_id(42);

    let update = build_update(widget, WidgetState::DropdownMenu(state), None);
    let menu_bounds = nodes_with_role(&update, Role::Menu)[0].bounds().unwrap();
    let items = nodes_with_role(&update, Role::MenuItem);

    assert_eq!(items.len(), 30);
    assert!(items.iter().all(|item| {
        let bounds = item.bounds().unwrap();
        bounds.x0 >= menu_bounds.x0
            && bounds.y0 >= menu_bounds.y0
            && bounds.x1 <= menu_bounds.x1
            && bounds.y1 <= menu_bounds.y1
    }));
}
