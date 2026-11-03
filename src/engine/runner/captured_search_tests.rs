//! Real event dispatch and publication for owned search and editable-picker callbacks.
use super::*;
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::render::search_overlay::{search_option_rect, search_popup_layout};
use crate::render::select_overlay::collector::collect_open_search_overlays;
use crate::widget::{Widget, resolve_search_suggestion_id};
use crate::{ChangeCallback, SearchMatcher, SearchSuggestions, SearchSuggestionsWith};
use accesskit::{Action, ActionHandler, ActionRequest, NodeId, TreeId};
use cosmic_text::{Cursor, Edit};
use std::sync::Arc;
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, PhysicalKey};

const VIEWPORT: PhysicalSize<u32> = PhysicalSize::new(320, 360);
const ITEMS: &[&str] = &["Other", "São Paulo", "Santos", "東京", "Tail"];

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Draft(usize, u64, String),
    Pick(usize, u64, usize),
}

// Neither context is Clone; dispatch and retained snapshots must share ownership.
struct DraftCapture(usize, u64, Arc<()>);
struct SelectionCapture(usize, u64, Arc<()>);
impl DraftCapture {
    fn message(&self, value: String) -> Message {
        let _owner = &self.2;
        Message::Draft(self.0, self.1, value)
    }
}
impl SelectionCapture {
    fn message(&self, value: usize) -> Message {
        let _owner = &self.2;
        Message::Pick(self.0, self.1, value)
    }
}

struct Model {
    epoch: u64,
    combo: bool,
    form: usize,
    informational: bool,
    plain: bool,
    pointer_wrapped: bool,
    enabled: bool,
    visible: bool,
    drafts: [String; 2],
    owner: Arc<()>,
    messages: Vec<Message>,
}
struct SearchApp;

fn legacy_draft(slot: usize) -> fn(String) -> Message {
    if slot == 0 {
        |v| Message::Draft(0, 0, v)
    } else {
        |v| Message::Draft(1, 0, v)
    }
}
fn legacy_pick(slot: usize) -> fn(usize) -> Message {
    if slot == 0 {
        |v| Message::Pick(0, 0, v)
    } else {
        |v| Message::Pick(1, 0, v)
    }
}

fn field(model: &Model, slot: usize) -> Widget<'_, Message> {
    let id = 41 + slot as u64;
    let style = Style {
        size: Size::from_lengths(260.0, 44.0),
        flex_shrink: 0.0,
        ..Style::default()
    };
    let draft = DraftCapture(slot, model.epoch, model.owner.clone());
    let pick = SelectionCapture(slot, model.epoch, model.owner.clone());
    let change = move |value| draft.message(value);
    if model.combo {
        let legacy = crate::combo_box::ComboBox::new(ITEMS, legacy_pick(slot))
            .with_label("Destination 東京")
            .with_error("Choose a destination")
            .with_visible_rows(2)
            .unwrap();
        if model.form == 3 {
            return Widget::combo_box(id, &model.drafts[slot], legacy_draft(slot), legacy, style);
        }
        if model.form == 1 {
            return Widget::combo_box_with(id, &model.drafts[slot], change, legacy, style);
        }
        let options = crate::combo_box::ComboBoxWith::new(ITEMS, move |value| pick.message(value))
            .with_label("Destination 東京")
            .with_error("Choose a destination")
            .with_visible_rows(2)
            .unwrap();
        return if model.form == 2 {
            Widget::combo_box_with(id, &model.drafts[slot], legacy_draft(slot), options, style)
        } else {
            Widget::combo_box_with(id, &model.drafts[slot], change, options, style)
        };
    }
    if model.plain {
        if model.form == 3 {
            return Widget::search_bar(legacy_draft(slot), None, None, None, "Search", style)
                .with_id(id);
        }
        return Widget::search_bar_with(change, None, None, None, "Search", style).with_id(id);
    }
    let legacy = SearchSuggestions::new(
        ITEMS,
        SearchMatcher::Fuzzy,
        3,
        (!model.informational).then_some(legacy_pick(slot)),
    )
    .unwrap();
    if model.form == 3 {
        return Widget::search_bar_with_suggestions(
            legacy_draft(slot),
            None,
            None,
            None,
            "Search",
            legacy,
            style,
        )
        .with_id(id);
    }
    if model.form == 1 {
        return Widget::search_bar_with_suggestions_with(
            change, None, None, None, "Search", legacy, style,
        )
        .with_id(id);
    }
    let suggestions = SearchSuggestionsWith::new(
        ITEMS,
        SearchMatcher::Fuzzy,
        3,
        (!model.informational).then(|| ChangeCallback::new(move |value| pick.message(value))),
    )
    .unwrap();
    if model.form == 2 {
        Widget::search_bar_with_suggestions_with(
            legacy_draft(slot),
            None,
            None,
            None,
            "Search",
            suggestions,
            style,
        )
        .with_id(id)
    } else {
        Widget::search_bar_with_suggestions_with(
            change,
            None,
            None,
            None,
            "Search",
            suggestions,
            style,
        )
        .with_id(id)
    }
}

impl AppLogic for SearchApp {
    type State = Model;
    type Message = Message;
    fn new(_: &mut FontSystem) -> Model {
        Model {
            epoch: 0,
            combo: false,
            form: 0,
            informational: false,
            plain: false,
            pointer_wrapped: false,
            enabled: true,
            visible: true,
            drafts: Default::default(),
            owner: Arc::new(()),
            messages: Vec::new(),
        }
    }
    fn view(model: &mut Model) -> Widget<'_, Message> {
        Widget::Column {
            children: if model.visible {
                (0..2)
                    .map(|slot| {
                        let input = field(model, slot);
                        if model.pointer_wrapped {
                            Widget::pointer_region(
                                crate::WidgetId::manual(70 + slot as u64).unwrap(),
                                input,
                                crate::PointerRegionConfig::new(|_| {
                                    Message::Draft(0, 0, "pointer".into())
                                }),
                                Style::default(),
                            )
                        } else {
                            input
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
    fn update(model: &mut Model, message: Message, _: &mut arboard::Clipboard) {
        match &message {
            Message::Draft(slot, _, text) => model.drafts[*slot] = text.clone(),
            Message::Pick(slot, _, index) => model.drafts[*slot] = ITEMS[*index].into(),
        }
        model.messages.push(message);
    }
}

fn publish(runner: &mut RutterRunner<SearchApp>) {
    runner.engine.layout_dirty = true;
    runner.engine.try_ensure_layout_for_input(VIEWPORT).unwrap();
}
fn runner(combo: bool) -> RutterRunner<SearchApp> {
    let mut engine = RutterEngine::<SearchApp>::new().expect("owned Xvfb clipboard");
    engine.app_state.combo = combo;
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
fn event(runner: &mut RutterRunner<SearchApp>, event: WindowEvent) {
    runner.window_event(&MemoryEventLoop::default(), WindowId::from_raw(1), event);
    assert!(runner.fatal_error.is_none(), "{:?}", runner.fatal_error);
}
fn key(runner: &mut RutterRunner<SearchApp>, key: NamedKey) {
    event(
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
fn action(runner: &mut RutterRunner<SearchApp>, id: u64, action: Action) {
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
fn pointer(runner: &mut RutterRunner<SearchApp>, point: Point) {
    for state in [ElementState::Pressed, ElementState::Released] {
        event(
            runner,
            WindowEvent::PointerButton {
                device_id: None,
                state,
                primary: true,
                position: PhysicalPosition::new(point.x as f64, point.y as f64),
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
    }
}
fn first_option_center(runner: &mut RutterRunner<SearchApp>, id: u64) -> Point {
    publish(runner);
    let view = SearchApp::view(&mut runner.engine.app_state);
    let overlays = collect_open_search_overlays(
        &view,
        &runner.engine.taffy,
        runner.engine.last_root_node,
        &runner.engine.widget_states,
        &runner.engine.input_states,
        Some(id),
        (320.0, 360.0),
    );
    let overlay = overlays.iter().find(|overlay| overlay.id == id).unwrap();
    assert_eq!(overlay.matches[0].index, 1);
    let popup = search_popup_layout(overlay, (320.0, 360.0));
    assert!(popup.rect.left >= 0.0 && popup.rect.right <= 320.0);
    assert!(popup.rect.top >= 0.0 && popup.rect.bottom <= 360.0);
    search_option_rect(popup, 0).center()
}
fn query(runner: &mut RutterRunner<SearchApp>, id: u64, text: &str) {
    runner.focus_widget(Some(id));
    runner.engine.app_state.epoch += 1;
    runner.engine.layout_dirty = true;
    event(runner, WindowEvent::Ime(Ime::Commit(text.into())));
    publish(runner);
    action(runner, id, Action::Expand);
    publish(runner);
}

#[test]
fn captured_search_ime_before_redraw_uses_distinct_nonclone_contexts_and_echo_preserves_cursor() {
    for combo in [false, true] {
        let mut r = runner(combo);
        for slot in 0..2 {
            let id = 41 + slot as u64;
            r.focus_widget(Some(id));
            r.engine.app_state.epoch = 10 + slot as u64;
            r.engine.layout_dirty = true;
            event(&mut r, WindowEvent::Ime(Ime::Commit("sao".into())));
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message::Draft(slot, 10 + slot as u64, "sao".into()))
            );
            let cursor = r.engine.input_states[&id].editor.cursor();
            publish(&mut r);
            assert_eq!(r.engine.input_states[&id].editor.cursor(), cursor);
            assert_eq!(r.engine.runtime_caches.searches[&id].results[0].index, 1);
            assert!(!r.engine.runtime_caches.inputs[&id].is_password);
            assert!(!r.engine.runtime_caches.inputs[&id].is_multiline);
            assert_eq!(
                r.engine.runtime_caches.inputs[&id].leading_text_inset,
                if combo {
                    0.0
                } else {
                    crate::search::SEARCH_BAR_LEADING_TEXT_INSET
                }
            );
        }
        if combo {
            r.engine.app_state.drafts[1] = "東京".into();
            publish(&mut r);
            assert_eq!(r.engine.input_states[&42].text(), "東京");
            assert_eq!(r.engine.runtime_caches.searches[&42].results[0].index, 3);
        }
    }
}

#[test]
fn captured_search_pointer_keyboard_accessibility_select_original_indices_after_dirty_refresh() {
    for combo in [false, true] {
        for input in ["pointer", "keyboard", "accessibility"] {
            let mut r = runner(combo);
            for slot in 0..2 {
                let id = 41 + slot as u64;
                query(&mut r, id, "sao");
                let point = first_option_center(&mut r, id);
                r.engine.app_state.epoch = 50 + slot as u64;
                r.engine.layout_dirty = true;
                match input {
                    "pointer" => pointer(&mut r, point),
                    "keyboard" => {
                        key(&mut r, NamedKey::ArrowDown);
                        key(&mut r, NamedKey::Enter);
                    }
                    _ => action(&mut r, resolve_search_suggestion_id(id, 1), Action::Click),
                }
                assert_eq!(
                    r.engine.app_state.messages.last(),
                    Some(&Message::Pick(slot, 50 + slot as u64, 1)),
                    "{combo}, {input}"
                );
                assert_eq!(r.engine.focused_widget_id, Some(id));
                publish(&mut r);
                if combo {
                    assert_eq!(r.engine.input_states[&id].text(), "São Paulo");
                }
            }
            assert_eq!(
                r.engine
                    .app_state
                    .messages
                    .iter()
                    .filter(|m| matches!(m, Message::Pick(..)))
                    .count(),
                2
            );
        }
    }
}

#[test]
fn captured_search_disable_removal_suppress_ime_and_all_selection_dispatch() {
    for combo in [false, true] {
        for removed in [false, true] {
            for input in ["ime", "pointer", "keyboard", "accessibility"] {
                let mut r = runner(combo);
                query(&mut r, 41, "sao");
                let point = first_option_center(&mut r, 41);
                let count = r.engine.app_state.messages.len();
                r.engine.app_state.visible = !removed;
                r.engine.app_state.enabled = removed;
                r.engine.layout_dirty = true;
                match input {
                    "ime" => event(&mut r, WindowEvent::Ime(Ime::Commit("blocked".into()))),
                    "pointer" => pointer(&mut r, point),
                    "keyboard" => {
                        key(&mut r, NamedKey::ArrowDown);
                        key(&mut r, NamedKey::Enter);
                    }
                    _ => action(&mut r, resolve_search_suggestion_id(41, 1), Action::Click),
                }
                assert_eq!(r.engine.app_state.messages.len(), count);
                assert!(r.engine.runtime_caches.searches.is_empty());
                assert!(r.engine.runtime_caches.inputs.is_empty());
            }
        }
    }
}

#[test]
fn captured_search_optional_informational_and_plain_fields_never_select() {
    for plain in [false, true] {
        let mut r = runner(false);
        r.engine.app_state.informational = true;
        r.engine.app_state.plain = plain;
        publish(&mut r);
        query(&mut r, 41, "sao");
        if plain {
            assert!(r.engine.runtime_caches.searches.is_empty());
        } else {
            assert!(r.engine.runtime_caches.searches[&41].on_select.is_none());
            let point = first_option_center(&mut r, 41);
            pointer(&mut r, point);
        }
        key(&mut r, NamedKey::ArrowDown);
        key(&mut r, NamedKey::Enter);
        action(&mut r, resolve_search_suggestion_id(41, 1), Action::Click);
        assert!(
            r.engine
                .app_state
                .messages
                .iter()
                .all(|m| matches!(m, Message::Draft(..)))
        );
    }
}

#[test]
fn captured_search_mixed_legacy_forms_and_retained_callbacks_preserve_identity_and_ownership() {
    for combo in [false, true] {
        let mut r = runner(combo);
        let old_draft = r.engine.runtime_caches.inputs[&41].on_change.clone();
        let old_pick = r.engine.runtime_caches.searches[&41]
            .on_select
            .clone()
            .unwrap();
        let weak = Arc::downgrade(&r.engine.app_state.owner);
        let original = crate::widget::id::validate_widget_id_snapshot(&SearchApp::view(
            &mut r.engine.app_state,
        ))
        .unwrap();
        r.engine.app_state.owner = Arc::new(());
        for form in [3, 1, 2, 0] {
            r.engine.app_state.form = form;
            r.engine.app_state.epoch = 20;
            r.engine.app_state.drafts = Default::default();
            publish(&mut r);
            assert_eq!(
                crate::widget::id::validate_widget_id_snapshot(&SearchApp::view(
                    &mut r.engine.app_state
                ))
                .unwrap(),
                original
            );
            r.focus_widget(Some(41));
            r.engine
                .input_states
                .get_mut(&41)
                .unwrap()
                .set_text(&mut r.engine.font_system.borrow_mut(), "");
            event(&mut r, WindowEvent::Ime(Ime::Commit("sao".into())));
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message::Draft(
                    0,
                    if form == 2 || form == 3 { 0 } else { 20 },
                    "sao".into()
                ))
            );
            publish(&mut r);
            action(&mut r, 41, Action::Expand);
            publish(&mut r);
            action(&mut r, resolve_search_suggestion_id(41, 1), Action::Click);
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message::Pick(
                    0,
                    if form == 1 || form == 3 { 0 } else { 20 },
                    1
                ))
            );
        }
        assert_eq!(
            old_draft.call("retained".into()),
            Message::Draft(0, 0, "retained".into())
        );
        assert_eq!(old_pick.call(3), Message::Pick(0, 0, 3));
        assert!(weak.upgrade().is_some());
        drop((old_draft, old_pick));
        assert!(
            weak.upgrade().is_none(),
            "superseded runtime/scratch captures must be released"
        );
    }
}

#[test]
fn captured_combo_browse_keeps_all_rows_and_does_not_commit_on_escape_tab_or_blur() {
    let mut r = runner(true);
    r.focus_widget(Some(41));
    key(&mut r, NamedKey::ArrowUp);
    assert_eq!(
        r.engine.runtime_caches.searches[&41].results.len(),
        ITEMS.len()
    );
    key(&mut r, NamedKey::Enter);
    assert_eq!(
        r.engine.app_state.messages.last(),
        Some(&Message::Pick(0, 0, 4))
    );
    for named in [NamedKey::Escape, NamedKey::Tab] {
        key(&mut r, NamedKey::ArrowDown);
        let count = r.engine.app_state.messages.len();
        key(&mut r, named);
        assert_eq!(r.engine.app_state.messages.len(), count);
        r.focus_widget(Some(41));
    }
    key(&mut r, NamedKey::ArrowDown);
    r.focus_widget(None);
    assert_eq!(r.engine.app_state.messages.len(), 1);
    assert_eq!(r.engine.app_state.drafts[0], "Tail");
    // Editor selection stays a native single-line selection, independent of hover.
    r.engine
        .input_states
        .get_mut(&41)
        .unwrap()
        .editor
        .set_cursor(Cursor::new(0, 2));
    publish(&mut r);
    assert_eq!(r.engine.input_states[&41].editor.cursor().index, 2);
}

#[test]
fn pointer_region_search_retains_consecutive_uncontrolled_edits_across_publications() {
    for form in [0, 3] {
        let mut r = runner(false);
        r.engine.app_state.form = form;
        r.engine.app_state.plain = true;
        r.engine.app_state.pointer_wrapped = true;
        publish(&mut r);
        r.focus_widget(Some(41));
        for (commit, expected) in [("a", "a"), ("b", "ab"), ("東京", "ab東京")] {
            r.engine.app_state.epoch += 1;
            r.engine.layout_dirty = true;
            event(&mut r, WindowEvent::Ime(Ime::Commit(commit.into())));
            assert_eq!(r.engine.input_states[&41].text(), expected);
            assert_eq!(
                r.engine.app_state.messages.last(),
                Some(&Message::Draft(
                    0,
                    if form == 3 {
                        0
                    } else {
                        r.engine.app_state.epoch
                    },
                    expected.into(),
                ))
            );
            publish(&mut r);
            assert_eq!(r.engine.input_states[&41].text(), expected);
            assert_eq!(
                r.engine.input_states[&41].cursor_byte_index(),
                expected.len()
            );
        }
        assert_eq!(r.engine.app_state.messages.len(), 3);
        assert!(r.engine.runtime_caches.inputs[&41].model_value.is_none());
    }
}

#[test]
fn pointer_region_controlled_combo_retains_caret_selection_and_undo_redo_after_echo() {
    for form in [0, 3] {
        let mut r = runner(true);
        r.engine.app_state.form = form;
        r.engine.app_state.pointer_wrapped = true;
        publish(&mut r);
        r.focus_widget(Some(41));
        for commit in ["one", " two"] {
            event(&mut r, WindowEvent::Ime(Ime::Commit(commit.into())));
            // IME commits debounce history for 500 ms. Close each edit group
            // before publication so this retention test needs no wall-clock wait
            // and cannot repair history that publication has discarded.
            r.engine.input_states.get_mut(&41).unwrap().snapshot();
            publish(&mut r);
            let state = &r.engine.input_states[&41];
            assert_eq!(
                state.undo.current(),
                Some(r.engine.app_state.drafts[0].as_str())
            );
            assert!(state.undo.can_undo());
        }
        assert_eq!(r.engine.app_state.drafts[0], "one two");
        key(&mut r, NamedKey::Home);
        r.engine.modifiers = winit::keyboard::ModifiersState::SHIFT.into();
        key(&mut r, NamedKey::ArrowRight);
        key(&mut r, NamedKey::ArrowRight);
        r.engine.modifiers = winit::keyboard::ModifiersState::empty().into();
        let state = &r.engine.input_states[&41];
        assert_eq!(state.cursor_byte_index(), 2);
        assert_eq!(state.selection_anchor, Some(0));
        assert_eq!(state.selection.unwrap().normalized(), (0, 2));
        let selection = state.selection;
        let history = state.undo.current().unwrap().to_owned();
        assert!(state.undo.can_undo());
        publish(&mut r);
        let state = &r.engine.input_states[&41];
        assert_eq!(state.cursor_byte_index(), 2);
        assert_eq!(state.selection_anchor, Some(0));
        assert_eq!(state.selection, selection);
        assert_eq!(state.undo.current(), Some(history.as_str()));
        assert!(state.undo.can_undo());

        // Replacing the retained selection must use its original range, and
        // history must remain usable after the controlled value is echoed.
        event(&mut r, WindowEvent::Ime(Ime::Commit("X".into())));
        r.engine.input_states.get_mut(&41).unwrap().snapshot();
        publish(&mut r);
        assert_eq!(r.engine.app_state.drafts[0], "Xe two");
        assert_eq!(r.engine.input_states[&41].cursor_byte_index(), 1);
        assert_eq!(r.engine.input_states[&41].undo.current(), Some("Xe two"));
        r.do_undo();
        publish(&mut r);
        assert_eq!(r.engine.app_state.drafts[0], "one two");
        assert_eq!(r.engine.input_states[&41].text(), "one two");
        assert!(r.engine.input_states[&41].undo.can_redo());
        r.do_redo();
        publish(&mut r);
        assert_eq!(r.engine.app_state.drafts[0], "Xe two");
        assert_eq!(r.engine.input_states[&41].text(), "Xe two");
    }
}
