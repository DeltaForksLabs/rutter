//! Captured table requests cross real input, rebuild, and AccessKit boundaries.
use super::*;
use crate::engine::table_runtime::TableInteractionModifiers;
use crate::table::*;
use winit::keyboard::ModifiersState;

const TABLE: u64 = 93;

#[derive(Clone, Debug, PartialEq)]
enum TableMessage {
    Selection(u64, Vec<TableRowKey>),
    Sort(u64, TableSort),
}

// Intentionally neither Clone nor Copy.
struct Capture {
    epoch: u64,
    owner: Arc<()>,
}
impl Capture {
    fn selection(&self, keys: Vec<TableRowKey>) -> TableMessage {
        let _keep_alive = &self.owner;
        TableMessage::Selection(self.epoch, keys)
    }
    fn sort(&self, sort: TableSort) -> TableMessage {
        let _keep_alive = &self.owner;
        TableMessage::Sort(self.epoch, sort)
    }
}

struct TableModelState {
    mode: usize,
    epoch: u64,
    enabled: bool,
    visible: bool,
    selected: Vec<TableRowKey>,
    sort: Option<TableSort>,
    owner: Arc<()>,
    messages: Vec<TableMessage>,
}
struct TableApp;

impl AppLogic for TableApp {
    type State = TableModelState;
    type Message = TableMessage;
    fn new(_: &mut FontSystem) -> Self::State {
        TableModelState {
            mode: 2,
            epoch: 0,
            enabled: true,
            visible: true,
            selected: vec![],
            sort: None,
            owner: Arc::new(()),
            messages: vec![],
        }
    }
    fn view(model: &mut Self::State) -> Widget<'_, TableMessage> {
        if !model.visible {
            return Widget::Spacer {
                style: Style::default(),
            };
        }
        let columns = vec![
            TableColumn::new(
                TableColumnKey::new(1),
                "Name 東京",
                TableColumnWidth::fixed(140.0).unwrap(),
            )
            .with_sortable(true),
            TableColumn::new(
                TableColumnKey::new(2),
                "State",
                TableColumnWidth::fixed(140.0).unwrap(),
            )
            .with_sortable(true),
        ];
        // Deliberately not numeric key order: selection must follow supplied display order.
        let keys = if model
            .sort
            .is_some_and(|s| s.direction() == TableSortDirection::Descending)
        {
            [20, 10, 30]
        } else {
            [30, 10, 20]
        };
        let rows = keys
            .into_iter()
            .map(|key| {
                TableRow::new(
                    TableRowKey::new(key),
                    [TableCell::new(key.to_string()), TableCell::new("Ready")],
                )
            })
            .collect::<Vec<_>>();
        let capture = Capture {
            epoch: model.epoch,
            owner: model.owner.clone(),
        };
        let selection = match model.mode {
            0 => TableSelectionWith::none(),
            1 => TableSelectionWith::single(model.selected.first().copied(), move |key| {
                capture.selection(key.into_iter().collect())
            }),
            _ => TableSelectionWith::multiple(&model.selected, move |keys| capture.selection(keys)),
        };
        let capture = Capture {
            epoch: model.epoch,
            owner: model.owner.clone(),
        };
        let options = TableOptions::default()
            .selection_with(selection)
            .sorting_with(TableSortingWith::new(model.sort, move |sort| {
                capture.sort(sort)
            }));
        Widget::table_with_options(
            TableModel::new("Inventory", columns, rows).unwrap(),
            options,
            control_style(280.0, 176.0),
        )
        .with_id(TABLE)
        .enabled(model.enabled)
    }
    fn update(model: &mut Self::State, message: TableMessage, _: &mut arboard::Clipboard) {
        match &message {
            TableMessage::Selection(_, keys) => model.selected = keys.clone(),
            TableMessage::Sort(_, sort) => model.sort = Some(*sort),
        }
        model.messages.push(message);
    }
}

fn runner(mode: usize) -> RutterRunner<TableApp> {
    let mut engine = RutterEngine::<TableApp>::new().expect("isolated Xvfb clipboard");
    engine.app_state.mode = mode;
    engine.window = Some(Rc::new(MemoryWindow {
        size: VIEWPORT,
        scale: 1.0,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut runner);
    runner
}
fn publish(runner: &mut RutterRunner<TableApp>) {
    runner.engine.layout_dirty = true;
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}
fn event(runner: &mut RutterRunner<TableApp>, event: WindowEvent) {
    runner.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
}
fn click(runner: &mut RutterRunner<TableApp>, x: f64, y: f64) {
    for state in [ElementState::Pressed, ElementState::Released] {
        event(
            runner,
            WindowEvent::PointerButton {
                device_id: None,
                state,
                primary: true,
                position: PhysicalPosition::new(x, y),
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
    }
}
fn key(runner: &mut RutterRunner<TableApp>, logical_key: Key) {
    event(
        runner,
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Space),
                logical_key: logical_key.clone(),
                key_without_modifiers: logical_key,
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
                text_with_all_modifiers: None,
            },
        },
    );
}
fn selection(runner: &RutterRunner<TableApp>, epoch: u64, keys: &[u64]) {
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&TableMessage::Selection(
            epoch,
            keys.iter().copied().map(TableRowKey::new).collect()
        ))
    );
}
fn published_target(runner: &mut RutterRunner<TableApp>, header: bool) -> NodeId {
    publish(runner);
    let widget = TableApp::view(&mut runner.engine.app_state);
    let update = crate::accessibility::build_accessibility_update(
        &runner.engine.taffy,
        &widget,
        runner.engine.last_root_node,
        crate::accessibility::AccessibilityInputs {
            input_states: &runner.engine.input_states,
            widget_states: &runner.engine.widget_states,
            focused_widget_id: Some(TABLE),
            viewport: (320.0, 640.0),
            direction: LayoutDirection::Ltr,
        },
    );
    update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role()
                == if header {
                    accesskit::Role::ColumnHeader
                } else {
                    accesskit::Role::GridCell
                }
        })
        .unwrap()
        .0
}
fn accessibility(runner: &mut RutterRunner<TableApp>, target: NodeId) {
    runner
        .engine
        .accessibility_actions
        .handler()
        .do_action(ActionRequest {
            action: Action::Click,
            target_node: target,
            target_tree: TreeId::ROOT,
            data: None,
        });
    runner.process_accessibility_actions();
}

#[test]
fn captured_tables_pointer_keyboard_accesskit_refresh_disable_and_release() {
    for mode in [0, 1, 2] {
        let mut runner = runner(mode);
        let old = runner.engine.runtime_caches.tables[&TABLE].clone();
        let owner = Arc::downgrade(&runner.engine.app_state.owner);
        click(&mut runner, 20.0, 60.0);
        if mode == 0 {
            assert!(runner.engine.app_state.messages.is_empty());
        } else {
            selection(&runner, 0, &[30]);
        }
        runner.engine.app_state.epoch = 7;
        runner.engine.app_state.owner = Arc::new(());
        runner.engine.layout_dirty = true;
        click(&mut runner, 20.0, 105.0);
        if mode != 0 {
            selection(&runner, 7, &[10]);
            key(&mut runner, Key::Named(NamedKey::ArrowDown));
            key(&mut runner, Key::Character(" ".into()));
            selection(&runner, 7, &[20]);
            let target = published_target(&mut runner, false);
            runner.engine.app_state.epoch = 8;
            runner.engine.layout_dirty = true;
            accessibility(&mut runner, target);
            selection(&runner, 8, &[30]);
            runner.engine.modifiers = if cfg!(target_os = "macos") {
                ModifiersState::META
            } else {
                ModifiersState::CONTROL
            }
            .into();
            click(&mut runner, 20.0, 60.0);
            selection(&runner, 8, &[]);
            runner.engine.modifiers = ModifiersState::SHIFT.into();
            click(&mut runner, 20.0, 150.0);
            selection(&runner, 8, if mode == 1 { &[20] } else { &[30, 10, 20] });
            runner.engine.modifiers = ModifiersState::empty().into();
            runner.engine.app_state.enabled = false;
            runner.engine.layout_dirty = true;
            let before = runner.engine.app_state.messages.len();
            accessibility(&mut runner, target);
            click(&mut runner, 20.0, 105.0);
            key(&mut runner, Key::Character(" ".into()));
            assert_eq!(runner.engine.app_state.messages.len(), before);
            runner.engine.app_state.visible = false;
            runner.engine.layout_dirty = true;
            accessibility(&mut runner, target);
            assert_eq!(runner.engine.app_state.messages.len(), before);
        }
        runner.engine.app_state.visible = false;
        publish(&mut runner);
        assert!(!runner.engine.runtime_caches.tables.contains_key(&TABLE));
        let before = runner.engine.app_state.messages.len();
        click(&mut runner, 20.0, 60.0);
        key(&mut runner, Key::Character(" ".into()));
        assert_eq!(runner.engine.app_state.messages.len(), before);
        assert!(owner.upgrade().is_some());
        let old_message = old
            .message_for_target(
                crate::widgets::table::TableCellTarget::header(TableColumnKey::new(1)),
                None,
                TableInteractionModifiers {
                    toggle: false,
                    range: false,
                },
            )
            .unwrap();
        assert!(matches!(old_message, TableMessage::Sort(0, _)));
        if mode != 0 {
            assert_eq!(
                old.message_for_target(
                    crate::widgets::table::TableCellTarget::body(
                        TableRowKey::new(30),
                        TableColumnKey::new(1),
                    ),
                    None,
                    TableInteractionModifiers {
                        toggle: false,
                        range: false,
                    },
                ),
                Some(TableMessage::Selection(0, vec![TableRowKey::new(30)]))
            );
        }
        drop(old);
        assert!(
            owner.upgrade().is_none(),
            "retired caches release the old captured owner"
        );
    }
}

#[test]
fn captured_tables_sort_cycle_and_sorted_range_toggle() {
    let mut runner = runner(2);
    click(&mut runner, 20.0, 20.0);
    assert_eq!(
        runner.engine.app_state.sort.unwrap().direction(),
        TableSortDirection::Ascending
    );
    runner.engine.app_state.epoch = 7;
    runner.engine.layout_dirty = true;
    key(&mut runner, Key::Named(NamedKey::Enter));
    assert!(
        matches!(runner.engine.app_state.messages.last(), Some(TableMessage::Sort(7, sort)) if sort.direction() == TableSortDirection::Descending)
    );
    click(&mut runner, 20.0, 60.0);
    selection(&runner, 7, &[20]);
    runner.engine.modifiers = ModifiersState::SHIFT.into();
    click(&mut runner, 20.0, 150.0);
    selection(&runner, 7, &[20, 10, 30]);
    runner.engine.modifiers = if cfg!(target_os = "macos") {
        ModifiersState::META
    } else {
        ModifiersState::CONTROL
    }
    .into();
    click(&mut runner, 20.0, 105.0);
    selection(&runner, 7, &[20, 30]);
    runner.engine.modifiers = ModifiersState::empty().into();
    let target = published_target(&mut runner, true);
    accessibility(&mut runner, target);
    assert_eq!(
        runner.engine.app_state.sort.unwrap().direction(),
        TableSortDirection::Ascending
    );
    click(&mut runner, 160.0, 20.0);
    assert_eq!(
        runner.engine.app_state.sort.unwrap().column(),
        TableColumnKey::new(2)
    );
    assert_eq!(
        runner.engine.app_state.sort.unwrap().direction(),
        TableSortDirection::Ascending
    );
}
