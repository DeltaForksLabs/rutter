//! Native choice controls must dispatch the current view's owned context.
use super::*;

#[derive(Clone, Debug, PartialEq)]
struct ChoiceMessage {
    instance: String,
    epoch: u64,
    value: usize,
}

// Intentionally neither Clone nor Copy; cloning a callback must share ownership.
struct Capture {
    instance: String,
    epoch: u64,
    owner: Arc<()>,
}

impl Capture {
    fn message(&self, value: usize) -> ChoiceMessage {
        let _keep_alive = &self.owner;
        ChoiceMessage {
            instance: self.instance.clone(),
            epoch: self.epoch,
            value,
        }
    }
}

struct ChoiceModel {
    epoch: u64,
    checked: bool,
    legacy: bool,
    manual_tabs: bool,
    enabled: bool,
    visible: bool,
    active_tabs: [usize; 2],
    batch_effect: Option<BatchEffect>,
    duplicate_tabs: bool,
    owner: Arc<()>,
    messages: Vec<ChoiceMessage>,
}

#[derive(Clone, Copy)]
enum BatchEffect {
    Refresh,
    Disable,
    Remove,
    DuplicateTabs,
}

struct ChoiceApp;

fn expected(slot: usize, epoch: u64, value: usize) -> ChoiceMessage {
    ChoiceMessage {
        instance: format!("choice-{slot}"),
        epoch,
        value,
    }
}

fn choice<'a>(slot: usize, model: &ChoiceModel) -> Widget<'a, ChoiceMessage> {
    let style = control_style(240.0, 44.0);
    if model.legacy {
        return match slot {
            0 | 1 => Widget::Checkbox {
                checked: model.checked,
                on_change: if slot == 0 {
                    |v| expected(0, 0, v as usize)
                } else {
                    |v| expected(1, 0, v as usize)
                },
                label: "Checkbox",
                style,
            },
            2 | 3 => Widget::Switch {
                checked: model.checked,
                on_change: if slot == 2 {
                    |v| expected(2, 0, v as usize)
                } else {
                    |v| expected(3, 0, v as usize)
                },
                style,
            },
            4 | 5 => Widget::Radio {
                selected: false,
                on_select: if slot == 4 {
                    || expected(4, 0, 1)
                } else {
                    || expected(5, 0, 1)
                },
                label: "Radio",
                style,
            },
            _ => Widget::tab_bar(
                &["First", "Second", "Third"],
                model.active_tabs[slot - 6],
                if slot == 6 {
                    |v| expected(6, 0, v)
                } else {
                    |v| expected(7, 0, v)
                },
                style,
            ),
        };
    }
    let capture = Capture {
        instance: format!("choice-{slot}"),
        epoch: model.epoch,
        owner: model.owner.clone(),
    };
    match slot {
        0 | 1 => Widget::checkbox_with(
            model.checked,
            move |v| capture.message(v as usize),
            "Checkbox",
            style,
        ),
        2 | 3 => Widget::switch_with(model.checked, move |v| capture.message(v as usize), style),
        4 | 5 => Widget::radio_with(false, move || capture.message(1), "Radio", style),
        _ => Widget::tab_bar_with(
            &["First", "Second", "Third"],
            model.active_tabs[slot - 6],
            move |v| capture.message(v),
            style,
        ),
    }
}

impl AppLogic for ChoiceApp {
    type State = ChoiceModel;
    type Message = ChoiceMessage;

    fn new(_: &mut FontSystem) -> ChoiceModel {
        ChoiceModel {
            epoch: 0,
            checked: false,
            legacy: false,
            manual_tabs: false,
            enabled: true,
            visible: true,
            active_tabs: [0; 2],
            batch_effect: None,
            duplicate_tabs: false,
            owner: Arc::new(()),
            messages: Vec::new(),
        }
    }

    fn view(model: &mut ChoiceModel) -> Widget<'_, ChoiceMessage> {
        Widget::Column {
            children: if model.visible {
                (0..8)
                    .map(|slot| {
                        let widget = choice(slot, model);
                        if model.duplicate_tabs && slot >= 6 {
                            widget.with_id(76)
                        } else if model.manual_tabs && slot >= 6 {
                            widget.with_id(slot as u64 + 70)
                        } else {
                            widget
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            },
            style: Style::default(),
        }
        .enabled(model.enabled)
    }

    fn update(model: &mut ChoiceModel, message: ChoiceMessage, _: &mut arboard::Clipboard) {
        if message.instance == "choice-6" {
            model.active_tabs[0] = message.value;
        } else if message.instance == "choice-7" {
            model.active_tabs[1] = message.value;
        }
        if let Some(effect) = model.batch_effect {
            model.checked = message.value != 0;
            model.epoch += 1;
            match effect {
                BatchEffect::Refresh => {}
                BatchEffect::Disable => model.enabled = false,
                BatchEffect::Remove => model.visible = false,
                BatchEffect::DuplicateTabs => model.duplicate_tabs = true,
            }
        }
        model.messages.push(message);
    }
}

fn choice_runner() -> RutterRunner<ChoiceApp> {
    let mut engine = RutterEngine::<ChoiceApp>::new().expect("owned Xvfb clipboard");
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish_choices(&mut runner);
    runner
}

fn publish_choices(runner: &mut RutterRunner<ChoiceApp>) {
    runner.engine.layout_dirty = true;
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}

fn choice_event(runner: &mut RutterRunner<ChoiceApp>, event: WindowEvent) {
    runner.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
}

fn choice_key(runner: &mut RutterRunner<ChoiceApp>, key: NamedKey) {
    choice_event(
        runner,
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Space),
                logical_key: Key::Named(key),
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
                text_with_all_modifiers: None,
                key_without_modifiers: Key::Named(key),
            },
        },
    );
}

fn choice_action(runner: &mut RutterRunner<ChoiceApp>, id: u64, action: Action) {
    runner
        .engine
        .accessibility_actions
        .handler()
        .do_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: NodeId(id),
            data: None,
        });
    runner.process_accessibility_actions();
    assert!(runner.fatal_error.is_none());
}

fn choice_pointer(runner: &mut RutterRunner<ChoiceApp>, slot: usize) {
    let position = PhysicalPosition::new(120.0, (slot as f64 + 0.5) * 44.0);
    for state in [ElementState::Pressed, ElementState::Released] {
        choice_event(
            runner,
            WindowEvent::PointerButton {
                device_id: None,
                state,
                primary: true,
                position,
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
    }
}

fn ids(runner: &RutterRunner<ChoiceApp>) -> Vec<u64> {
    runner.engine.runtime_caches.focus_order.clone()
}

fn choice_accessibility_tree(runner: &mut RutterRunner<ChoiceApp>) -> accesskit::TreeUpdate {
    use crate::accessibility::{AccessibilityInputs, build_accessibility_update};

    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
    let view = ChoiceApp::view(&mut runner.engine.app_state);
    build_accessibility_update(
        &runner.engine.taffy,
        &view,
        runner.engine.last_root_node,
        AccessibilityInputs {
            input_states: &runner.engine.input_states,
            widget_states: &runner.engine.widget_states,
            focused_widget_id: runner.engine.focused_widget_id,
            viewport: (VIEWPORT.width as f32, VIEWPORT.height as f32),
            direction: crate::LayoutDirection::Ltr,
        },
    )
}

fn queue_choice_clicks(runner: &mut RutterRunner<ChoiceApp>, targets: &[u64]) {
    let mut inbox = runner.engine.accessibility_actions.handler();
    for &target in targets {
        inbox.do_action(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target),
            data: None,
        });
    }
}

#[test]
fn choice_tabs_published_children_support_focus_click_and_selected_state_updates() {
    use accesskit::Role;

    for legacy in [false, true] {
        for manual in [false, true] {
            let mut runner = choice_runner();
            runner.engine.app_state.legacy = legacy;
            runner.engine.app_state.manual_tabs = manual;
            publish_choices(&mut runner);
            let tree = choice_accessibility_tree(&mut runner);
            let lists: Vec<_> = tree
                .nodes
                .iter()
                .filter(|(_, n)| n.role() == Role::TabList)
                .collect();
            assert_eq!(lists.len(), 2);
            let mut original_ids = Vec::new();
            for (slot, (parent, list)) in lists.iter().enumerate() {
                assert!(!list.supports_action(Action::Focus));
                assert_eq!(list.children().len(), 3);
                let runtime = &runner.engine.runtime_caches.tabs[&parent.0];
                for (index, &id) in list.children().iter().enumerate() {
                    assert_eq!(id.0, runtime.focus_ids[index]);
                    let (_, tab) = tree
                        .nodes
                        .iter()
                        .find(|(node_id, _)| *node_id == id)
                        .unwrap();
                    assert_eq!(tab.role(), Role::Tab);
                    assert_eq!(tab.label(), Some(["First", "Second", "Third"][index]));
                    assert_eq!(tab.is_selected(), Some(index == 0));
                    assert!(tab.supports_action(Action::Focus));
                    assert!(tab.supports_action(Action::Click));
                    original_ids.push(id);
                }
                // Use only IDs discovered from the published tree, including for
                // a focused tab that is not yet the application-selected tab.
                let target = list.children()[2];
                let message_count = runner.engine.app_state.messages.len();
                choice_action(&mut runner, target.0, Action::Focus);
                let focused = choice_accessibility_tree(&mut runner);
                assert_eq!(focused.focus, target);
                assert_eq!(runner.engine.app_state.messages.len(), message_count);
                let (_, tab) = focused.nodes.iter().find(|(id, _)| *id == target).unwrap();
                assert_eq!(tab.is_selected(), Some(false));
                choice_action(&mut runner, target.0, Action::Click);
                assert_eq!(
                    runner.engine.app_state.messages.last(),
                    Some(&expected(slot + 6, 0, 2))
                );
            }
            let selected = choice_accessibility_tree(&mut runner);
            for (index, id) in original_ids.iter().enumerate() {
                let (_, tab) = selected
                    .nodes
                    .iter()
                    .find(|(node_id, _)| node_id == id)
                    .unwrap();
                assert_eq!(tab.is_selected(), Some(index % 3 == 2));
            }
        }
    }
}

#[test]
fn choice_accessibility_batch_refreshes_checked_state_and_owned_callback_context() {
    for legacy in [false, true] {
        for slot in [0, 2, 4, 6] {
            let mut runner = choice_runner();
            runner.engine.app_state.legacy = legacy;
            runner.engine.app_state.batch_effect = Some(BatchEffect::Refresh);
            publish_choices(&mut runner);
            let tree = choice_accessibility_tree(&mut runner);
            let role = match slot {
                0 => accesskit::Role::CheckBox,
                2 => accesskit::Role::Switch,
                4 => accesskit::Role::RadioButton,
                _ => accesskit::Role::Tab,
            };
            let target = tree
                .nodes
                .iter()
                .find(|(_, node)| node.role() == role)
                .unwrap()
                .0
                .0;
            queue_choice_clicks(&mut runner, &[target, target]);
            runner.process_accessibility_actions();
            assert!(runner.fatal_error.is_none());
            let first = usize::from(slot != 6);
            let second = usize::from(slot == 4);
            assert_eq!(
                runner.engine.app_state.messages,
                vec![
                    expected(slot, 0, first),
                    expected(slot, u64::from(!legacy), second),
                ]
            );
        }
    }
}

#[test]
fn choice_accessibility_batch_suppresses_targets_disabled_or_removed_by_first_action() {
    for legacy in [false, true] {
        for effect in [BatchEffect::Disable, BatchEffect::Remove] {
            let mut runner = choice_runner();
            runner.engine.app_state.legacy = legacy;
            runner.engine.app_state.batch_effect = Some(effect);
            publish_choices(&mut runner);
            let tree = choice_accessibility_tree(&mut runner);
            let targets: Vec<_> = tree
                .nodes
                .iter()
                .filter_map(|(id, node)| node.supports_action(Action::Click).then_some(id.0))
                .collect();
            assert!(!targets.is_empty());
            queue_choice_clicks(&mut runner, &targets);
            runner.process_accessibility_actions();
            assert!(runner.fatal_error.is_none());
            assert_eq!(runner.engine.app_state.messages.len(), 1);
            assert!(runner.engine.runtime_caches.focus_order.is_empty());
        }
    }
}

#[test]
fn choice_accessibility_batch_refresh_failure_is_fatal_and_stops_remaining_actions() {
    let mut runner = choice_runner();
    runner.engine.app_state.batch_effect = Some(BatchEffect::DuplicateTabs);
    let tree = choice_accessibility_tree(&mut runner);
    let target = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == accesskit::Role::CheckBox)
        .unwrap()
        .0
        .0;
    queue_choice_clicks(&mut runner, &[target, target, target]);
    runner.process_accessibility_actions();
    assert!(matches!(
        runner.fatal_error,
        Some(crate::RutterRunError::WidgetId(_))
    ));
    assert_eq!(runner.engine.app_state.messages, vec![expected(0, 0, 1)]);
}

#[test]
fn captured_choices_pointer_keyboard_accessibility_use_distinct_current_owned_contexts() {
    for input in ["pointer", "keyboard", "accessibility"] {
        let mut runner = choice_runner();
        let order = ids(&runner);
        assert_eq!(order.len(), 12); // six controls plus two three-tab bars
        for slot in 0..8 {
            let focus = order[if slot < 6 { slot } else { 6 + (slot - 6) * 3 }];
            runner.focus_widget(Some(focus));
            runner.engine.app_state.epoch = slot as u64 + 10;
            runner.engine.layout_dirty = true;
            match input {
                "pointer" => choice_pointer(&mut runner, slot),
                "keyboard" => choice_key(
                    &mut runner,
                    if slot < 6 {
                        NamedKey::Enter
                    } else {
                        NamedKey::ArrowRight
                    },
                ),
                _ => choice_action(
                    &mut runner,
                    if slot < 6 {
                        focus
                    } else {
                        order[7 + (slot - 6) * 3]
                    },
                    Action::Click,
                ),
            }
            assert_eq!(
                runner.engine.app_state.messages.last(),
                Some(&expected(slot, slot as u64 + 10, 1)),
                "{input}, slot={slot}"
            );
            assert_eq!(runner.engine.app_state.messages.len(), slot + 1);
        }
        runner.engine.app_state.checked = true;
        for (slot, &focus) in order.iter().take(4).enumerate() {
            runner.focus_widget(Some(focus));
            runner.engine.app_state.epoch = 40;
            runner.engine.layout_dirty = true;
            match input {
                "pointer" => choice_pointer(&mut runner, slot),
                "keyboard" => choice_key(&mut runner, NamedKey::Enter),
                _ => choice_action(&mut runner, focus, Action::Click),
            }
            assert_eq!(
                runner.engine.app_state.messages.last(),
                Some(&expected(slot, 40, 0))
            );
        }
    }
}

#[test]
fn captured_choices_transitions_preserve_auto_ids_structure_tab_focus_and_snapshot_lifetimes() {
    let mut runner = choice_runner();
    let original_ids = ids(&runner);
    let old_checkbox = runner.engine.runtime_caches.checkboxes[&original_ids[0]]
        .on_change
        .clone();
    let old_switch = runner.engine.runtime_caches.switches[&original_ids[2]]
        .on_change
        .clone();
    let old_radio = runner.engine.runtime_caches.radios[&original_ids[4]].clone();
    let tab_parent = runner.engine.runtime_caches.tab_items[&original_ids[6]].parent_id;
    let old_tab = runner.engine.runtime_caches.tabs[&tab_parent]
        .on_change
        .clone();
    let weak = Arc::downgrade(&runner.engine.app_state.owner);
    let original_structure = {
        let view = ChoiceApp::view(&mut runner.engine.app_state);
        crate::widget::id::validate_widget_id_snapshot(&view).unwrap()
    };
    runner.engine.app_state.owner = Arc::new(());
    for legacy in [true, false] {
        runner.engine.app_state.legacy = legacy;
        runner.engine.app_state.epoch = 20;
        publish_choices(&mut runner);
        assert_eq!(ids(&runner), original_ids);
        let view = ChoiceApp::view(&mut runner.engine.app_state);
        assert_eq!(
            crate::widget::id::validate_widget_id_snapshot(&view).unwrap(),
            original_structure
        );
        drop(view);
        for slot in 0..8 {
            choice_pointer(&mut runner, slot);
            assert_eq!(
                runner.engine.app_state.messages.last(),
                Some(&expected(slot, if legacy { 0 } else { 20 }, 1))
            );
        }
    }
    assert_eq!(old_checkbox.call(true), expected(0, 0, 1));
    assert_eq!(old_switch.call(true), expected(2, 0, 1));
    assert_eq!(old_radio.call(()), expected(4, 0, 1));
    assert_eq!(old_tab.call(2), expected(6, 0, 2));
    assert!(weak.upgrade().is_some());
    drop((old_checkbox, old_switch, old_radio, old_tab));
    assert!(
        weak.upgrade().is_none(),
        "scratch/runtime caches must release superseded captures"
    );
}

#[test]
fn captured_choices_dirty_disable_or_removal_blocks_pointer_keyboard_and_accessibility() {
    for removed in [false, true] {
        for input in ["pointer", "keyboard", "accessibility"] {
            for slot in 0..8 {
                let mut runner = choice_runner();
                let order = ids(&runner);
                let focus = order[if slot < 6 { slot } else { 6 + (slot - 6) * 3 }];
                runner.focus_widget(Some(focus));
                let weak = Arc::downgrade(&runner.engine.app_state.owner);
                runner.engine.app_state.owner = Arc::new(());
                runner.engine.app_state.visible = !removed;
                runner.engine.app_state.enabled = removed;
                runner.engine.layout_dirty = true;
                match input {
                    "pointer" => choice_pointer(&mut runner, slot),
                    "keyboard" => choice_key(&mut runner, NamedKey::Enter),
                    _ => choice_action(&mut runner, focus, Action::Click),
                }
                assert!(
                    runner.engine.app_state.messages.is_empty(),
                    "{input}, slot={slot}"
                );
                assert!(ids(&runner).is_empty());
                assert!(runner.engine.runtime_caches.checkboxes.is_empty());
                assert!(runner.engine.runtime_caches.switches.is_empty());
                assert!(runner.engine.runtime_caches.radios.is_empty());
                assert!(runner.engine.runtime_caches.tabs.is_empty());
                assert!(runner.engine.runtime_caches.tab_items.is_empty());
                assert!(weak.upgrade().is_none());
            }
        }
    }
}

#[test]
fn captured_choices_tab_traversal_and_manual_tab_navigation_match_legacy_forms() {
    let mut runner = choice_runner();
    runner.engine.app_state.manual_tabs = true;
    publish_choices(&mut runner);
    let order = ids(&runner);
    runner.focus_widget(None);
    for &focus in &order {
        choice_key(&mut runner, NamedKey::Tab);
        assert_eq!(runner.engine.focused_widget_id, Some(focus));
    }
    assert!(runner.engine.app_state.messages.is_empty());
    for legacy in [true, false] {
        runner.engine.app_state.legacy = legacy;
        runner.engine.app_state.epoch = 30;
        publish_choices(&mut runner);
        assert_eq!(ids(&runner), order);
        for slot in 6..8 {
            let parent = slot as u64 + 70;
            let focus_ids = runner.engine.runtime_caches.tabs[&parent].focus_ids.clone();
            assert_eq!(
                runner.engine.runtime_caches.tab_items[&focus_ids[0]].parent_id,
                parent
            );
            runner.focus_widget(Some(focus_ids[1]));
            for (key, index) in [
                (NamedKey::End, 2),
                (NamedKey::Home, 0),
                (NamedKey::ArrowDown, 1),
                (NamedKey::ArrowUp, 0),
                (NamedKey::Enter, 0),
            ] {
                choice_key(&mut runner, key);
                assert_eq!(runner.engine.focused_widget_id, Some(focus_ids[index]));
                assert_eq!(
                    runner.engine.app_state.messages.last(),
                    Some(&expected(slot, if legacy { 0 } else { 30 }, index))
                );
            }
        }
    }
}
