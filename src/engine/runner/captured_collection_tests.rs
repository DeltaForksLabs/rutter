//! Exercise captured collection policies through actual runner input/publication.
use super::*;
use crate::{CarouselConfig, KeyedVirtualItems, VirtualItemKey, VirtualSelectionWith};
use winit::keyboard::ModifiersState;

const COLLECTION: u64 = 91;
const WIDTH: f32 = 300.0;

#[derive(Clone, Debug, PartialEq)]
struct CollectionMessage {
    epoch: u64,
    indices: Vec<usize>,
}

// Deliberately non-Clone: callback clones must share the owned capture.
struct Capture {
    epoch: u64,
    owner: Arc<()>,
}
impl Capture {
    fn message(&self, indices: Vec<usize>) -> CollectionMessage {
        let _keep_alive = &self.owner;
        CollectionMessage {
            epoch: self.epoch,
            indices,
        }
    }
}

struct CollectionModel {
    family: usize,
    multiple: bool,
    epoch: u64,
    enabled: bool,
    visible: bool,
    selected: Vec<usize>,
    keys: Vec<VirtualItemKey>,
    owner: Arc<()>,
    messages: Vec<CollectionMessage>,
}
struct CollectionApp;

fn row(index: usize) -> Option<String> {
    Some(format!("Row {index} 東京 القاهرة"))
}
fn content<'a>(index: usize) -> Option<Widget<'a, CollectionMessage>> {
    Some(Widget::Text {
        content: row(index).unwrap(),
        style: Style::default(),
        color: None,
        size: 14.0,
    })
}
fn keyed_item<'a, const EPOCH: u64>(index: usize) -> Option<Widget<'a, CollectionMessage>> {
    let epoch = EPOCH;
    Some(Widget::checkbox_with(
        false,
        move |_| CollectionMessage {
            epoch,
            indices: vec![100 + index],
        },
        "Child",
        control_style(80.0, 24.0),
    ))
}

impl AppLogic for CollectionApp {
    type State = CollectionModel;
    type Message = CollectionMessage;

    fn new(_: &mut FontSystem) -> CollectionModel {
        CollectionModel {
            family: 0,
            multiple: false,
            epoch: 0,
            enabled: true,
            visible: true,
            selected: vec![],
            keys: (1..=30).map(|v| VirtualItemKey::new(v).unwrap()).collect(),
            owner: Arc::new(()),
            messages: vec![],
        }
    }

    fn view<'a>(model: &'a mut CollectionModel) -> Widget<'a, CollectionMessage> {
        if !model.visible {
            return Widget::Spacer {
                style: Style::default(),
            };
        }
        let capture = Capture {
            epoch: model.epoch,
            owner: model.owner.clone(),
        };
        let style = control_style(WIDTH, 120.0);
        let policy = if model.multiple {
            VirtualSelectionWith::multiple(&model.selected, move |v| capture.message(v))
        } else {
            VirtualSelectionWith::single(move |v| capture.message(vec![v]))
        };
        let widget = match model.family {
            0 => Widget::virtual_list_with_selection_with(40.0, 30, &row, policy, style),
            1 => {
                Widget::virtual_list_content_with_selection_with(40.0, 30, &content, policy, style)
            }
            2 => Widget::virtual_grid_with_selection_with(3, 40.0, 30, &row, policy, style),
            3 => Widget::virtual_grid_content_with_selection_with(
                3, 40.0, 30, &content, policy, style,
            ),
            family => {
                let capture = Capture {
                    epoch: model.epoch,
                    owner: model.owner.clone(),
                };
                let single = move |v| capture.message(vec![v]);
                let builder: &'a dyn Fn(usize) -> Option<Widget<'a, CollectionMessage>> =
                    if model.epoch == 0 {
                        &keyed_item::<0>
                    } else {
                        &keyed_item::<7>
                    };
                let items = KeyedVirtualItems::try_new(&model.keys, builder).unwrap();
                match family {
                    4 => Widget::interactive_virtual_list_content_with(40.0, items, single, style),
                    5 => {
                        Widget::interactive_virtual_grid_content_with(3, 40.0, items, single, style)
                    }
                    6 => Widget::carousel_view_with(
                        30,
                        content,
                        single,
                        CarouselConfig::uncontained(120.0).unwrap(),
                        style,
                    ),
                    _ => Widget::interactive_carousel_view_with(
                        items,
                        single,
                        CarouselConfig::uncontained(120.0).unwrap(),
                        style,
                    ),
                }
            }
        };
        widget.with_id(COLLECTION).enabled(model.enabled)
    }

    fn update(model: &mut CollectionModel, message: CollectionMessage, _: &mut arboard::Clipboard) {
        if model.multiple {
            model.selected = message.indices.clone();
        }
        model.messages.push(message);
    }
}

fn runner(family: usize, multiple: bool) -> RutterRunner<CollectionApp> {
    let mut engine = RutterEngine::<CollectionApp>::new().expect("isolated Xvfb clipboard");
    engine.app_state.family = family;
    engine.app_state.multiple = multiple;
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
fn publish(runner: &mut RutterRunner<CollectionApp>) {
    runner.engine.layout_dirty = true;
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}
fn event(runner: &mut RutterRunner<CollectionApp>, event: WindowEvent) {
    runner.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
}
fn key(runner: &mut RutterRunner<CollectionApp>, logical_key: Key) {
    event(
        runner,
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::ArrowDown),
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
fn pointer(runner: &mut RutterRunner<CollectionApp>, x: f64, y: f64, state: ElementState) {
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
fn click(runner: &mut RutterRunner<CollectionApp>, x: f64, y: f64) {
    pointer(runner, x, y, ElementState::Pressed);
    pointer(runner, x, y, ElementState::Released);
}
fn last(runner: &RutterRunner<CollectionApp>, epoch: u64, indices: &[usize]) {
    assert_eq!(
        runner.engine.app_state.messages.last(),
        Some(&CollectionMessage {
            epoch,
            indices: indices.to_vec()
        }),
        "family {}",
        runner.engine.app_state.family
    );
}

#[test]
fn captured_collections_pointer_keyboard_fallback_refresh_and_retirement() {
    for family in 0..8 {
        let mut runner = runner(family, false);
        let retained = match family {
            0 | 1 | 4 => runner.engine.runtime_caches.vlists[&COLLECTION]
                .on_select
                .clone(),
            2 | 3 | 5 => runner.engine.runtime_caches.vgrids[&COLLECTION]
                .on_select
                .clone(),
            _ => runner.engine.runtime_caches.carousels[&COLLECTION]
                .on_select
                .clone(),
        };
        let owner = Arc::downgrade(&runner.engine.app_state.owner);
        // Unoccupied row space and grid/carousel card edges bypass children.
        let (x, y, index) = match family {
            0 | 1 | 4 => (200.0, 60.0, 1),
            2 | 3 | 5 => (150.0, 34.0, 1),
            _ => (125.0, 2.0, 1),
        };
        click(&mut runner, x, y);
        last(&runner, 0, &[index]);
        runner.engine.app_state.epoch = 7;
        runner.engine.layout_dirty = true;
        key(
            &mut runner,
            Key::Named(if family >= 6 {
                NamedKey::ArrowRight
            } else {
                NamedKey::ArrowDown
            }),
        );
        let next = if matches!(family, 2 | 3 | 5) { 4 } else { 2 };
        last(&runner, 7, &[next]);
        assert_eq!(
            retained.call(9),
            CollectionMessage {
                epoch: 0,
                indices: vec![9]
            }
        );
        if family < 6 {
            runner.focus_widget(None);
            runner.engine.active_scroll_id = Some(COLLECTION);
            key(&mut runner, Key::Named(NamedKey::ArrowDown));
            last(
                &runner,
                7,
                &[if matches!(family, 2 | 3 | 5) { 7 } else { 3 }],
            );
        } else {
            key(&mut runner, Key::Named(NamedKey::End));
            last(&runner, 7, &[29]);
            let state = runner.engine.widget_states[&COLLECTION]
                .as_carousel()
                .unwrap();
            assert!(
                state.position.index > 0,
                "carousel must reveal the selected end card"
            );
        }
        let before = runner.engine.app_state.messages.len();
        runner.engine.app_state.enabled = false;
        runner.engine.layout_dirty = true;
        click(&mut runner, x, y);
        key(&mut runner, Key::Named(NamedKey::ArrowDown));
        assert_eq!(runner.engine.app_state.messages.len(), before);
        runner.engine.app_state.visible = false;
        publish(&mut runner);
        let caches = &runner.engine.runtime_caches;
        assert!(!caches.vlists.contains_key(&COLLECTION));
        assert!(!caches.vgrids.contains_key(&COLLECTION));
        assert!(!caches.carousels.contains_key(&COLLECTION));
        drop(runner);
        assert!(
            owner.upgrade().is_some(),
            "retained callback owns its old capture"
        );
        drop(retained);
        assert!(owner.upgrade().is_none());
    }
}

#[test]
fn captured_carousel_active_scroll_navigation_refreshes_without_focus() {
    for family in 6..8 {
        for remove in [false, true] {
            let mut runner = runner(family, false);
            runner.focus_widget(None);
            runner.engine.active_scroll_id = Some(COLLECTION);
            key(&mut runner, Key::Named(NamedKey::ArrowRight));
            last(&runner, 0, &[1]);
            runner.engine.app_state.epoch = 7;
            runner.engine.layout_dirty = true;
            key(&mut runner, Key::Named(NamedKey::End));
            last(&runner, 7, &[29]);
            key(&mut runner, Key::Named(NamedKey::ArrowLeft));
            last(&runner, 7, &[28]);
            key(&mut runner, Key::Named(NamedKey::Home));
            last(&runner, 7, &[0]);
            assert_eq!(runner.engine.focused_widget_id, None);

            let before = runner.engine.app_state.messages.len();
            if remove {
                runner.engine.app_state.visible = false;
            } else {
                runner.engine.app_state.enabled = false;
            }
            runner.engine.layout_dirty = true;
            key(&mut runner, Key::Named(NamedKey::ArrowRight));
            assert_eq!(runner.engine.app_state.messages.len(), before);
            assert!(
                !runner
                    .engine
                    .runtime_caches
                    .carousels
                    .contains_key(&COLLECTION)
            );
        }
    }
}

#[test]
fn captured_collections_multi_drag_modifiers_keyboard_and_current_capture() {
    for family in 0..4 {
        let mut runner = runner(family, true);
        let retained = runner.engine.runtime_caches.virtual_multi_selections[&COLLECTION]
            .on_change
            .clone();
        click(&mut runner, 20.0, 20.0);
        last(&runner, 0, &[0]);
        runner.engine.app_state.epoch = 7;
        runner.engine.layout_dirty = true;
        pointer(&mut runner, 20.0, 20.0, ElementState::Pressed);
        event(
            &mut runner,
            WindowEvent::PointerMoved {
                device_id: None,
                primary: true,
                position: PhysicalPosition::new(if family < 2 { 20.0 } else { 150.0 }, 100.0),
                source: PointerSource::Mouse,
            },
        );
        pointer(&mut runner, 20.0, 100.0, ElementState::Released);
        last(
            &runner,
            7,
            if family < 2 {
                &[0, 1, 2]
            } else {
                &[0, 1, 3, 4, 6, 7]
            },
        );
        runner.engine.modifiers = ModifiersState::SHIFT.into();
        key(&mut runner, Key::Named(NamedKey::ArrowDown));
        last(
            &runner,
            7,
            &(0..=if family < 2 { 3 } else { 10 }).collect::<Vec<_>>(),
        );
        runner.engine.modifiers = if cfg!(target_os = "macos") {
            ModifiersState::META
        } else {
            ModifiersState::CONTROL
        }
        .into();
        key(&mut runner, Key::Character("a".into()));
        last(&runner, 7, &(0..30).collect::<Vec<_>>());
        // Additive pointer input toggles one item without losing the selection.
        click(&mut runner, 20.0, 20.0);
        let toggled = runner
            .engine
            .app_state
            .messages
            .last()
            .unwrap()
            .indices
            .clone();
        assert_eq!(toggled.len(), 29);
        assert_eq!(runner.engine.app_state.messages.last().unwrap().epoch, 7);
        runner.focus_widget(None);
        runner.engine.active_scroll_id = Some(COLLECTION);
        runner.engine.modifiers = ModifiersState::empty().into();
        key(&mut runner, Key::Named(NamedKey::Home));
        last(&runner, 7, &[0]);
        assert_eq!(
            retained.call(vec![3]),
            CollectionMessage {
                epoch: 0,
                indices: vec![3]
            }
        );
        let before = runner.engine.app_state.messages.len();
        runner.engine.app_state.enabled = false;
        runner.engine.layout_dirty = true;
        key(&mut runner, Key::Named(NamedKey::ArrowDown));
        click(&mut runner, 20.0, 60.0);
        assert_eq!(runner.engine.app_state.messages.len(), before);
        runner.engine.app_state.visible = false;
        publish(&mut runner);
        assert!(
            !runner
                .engine
                .runtime_caches
                .virtual_multi_selections
                .contains_key(&COLLECTION)
        );
    }
}

#[test]
fn captured_collections_drag_refresh_disable_and_removal_use_live_publication() {
    for family in 0..4 {
        for remove in [false, true] {
            let mut runner = runner(family, true);
            let owner = Arc::downgrade(&runner.engine.app_state.owner);
            let retained = runner.engine.runtime_caches.virtual_multi_selections[&COLLECTION]
                .on_change
                .clone();
            pointer(&mut runner, 20.0, 20.0, ElementState::Pressed);
            runner.engine.app_state.epoch = 7;
            runner.engine.layout_dirty = true;
            event(
                &mut runner,
                WindowEvent::PointerMoved {
                    device_id: None,
                    primary: true,
                    position: PhysicalPosition::new(20.0, 60.0),
                    source: PointerSource::Mouse,
                },
            );
            last(&runner, 7, if family < 2 { &[0, 1] } else { &[0, 3] });
            let before = runner.engine.app_state.messages.len();
            if remove {
                runner.engine.app_state.visible = false;
            } else {
                runner.engine.app_state.enabled = false;
            }
            runner.engine.layout_dirty = true;
            event(
                &mut runner,
                WindowEvent::PointerMoved {
                    device_id: None,
                    primary: true,
                    position: PhysicalPosition::new(20.0, 100.0),
                    source: PointerSource::Mouse,
                },
            );
            pointer(&mut runner, 20.0, 100.0, ElementState::Released);
            assert_eq!(runner.engine.app_state.messages.len(), before);
            assert!(runner.virtual_multi_pointer_capture.is_none());
            drop(runner);
            assert!(owner.upgrade().is_some());
            drop(retained);
            assert!(owner.upgrade().is_none());
        }
    }
}

#[test]
fn captured_collections_keyed_descendants_refresh_in_dynamic_branches() {
    for family in [4, 5, 7] {
        let mut runner = runner(family, false);
        click(&mut runner, 20.0, 16.0);
        last(&runner, 0, &[100]);
        runner.engine.app_state.epoch = 7;
        runner.engine.layout_dirty = true;
        click(&mut runner, 20.0, 16.0);
        last(&runner, 7, &[100]);
        let child_focus = runner.engine.focused_widget_id.unwrap();
        assert_ne!(child_focus, COLLECTION);
        key(&mut runner, Key::Character(" ".into()));
        last(&runner, 7, &[100]);
        runner.engine.app_state.enabled = false;
        runner.engine.layout_dirty = true;
        let before = runner.engine.app_state.messages.len();
        key(&mut runner, Key::Character(" ".into()));
        assert_eq!(runner.engine.app_state.messages.len(), before);
    }
}
