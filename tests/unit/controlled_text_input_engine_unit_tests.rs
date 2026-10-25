use super::*;
use crate::input_state::{InputWidgetState, TextSelection};
use crate::widget::InputState;
use cosmic_text::{Cursor, Edit, Selection};

struct ControlledInputApp;

impl AppLogic for ControlledInputApp {
    type State = ();
    type Message = ();

    fn new(_: &mut FontSystem) {}

    fn view(_: &mut ()) -> Widget<'_, ()> {
        Widget::Spacer {
            style: Style::default(),
        }
    }

    fn update(_: &mut (), _: (), _: &mut Clipboard) {
        panic!("model synchronization must not dispatch application messages");
    }

    fn input_limits(id: u64, kind: InputKind) -> InputLimits {
        InputLimits {
            max_bytes: if id == 3 { 4 } else { kind.limits().max_bytes },
            ..kind.limits()
        }
    }
}

fn unexpected_change(_: String) {
    panic!("model synchronization must not invoke on_change");
}

fn controlled_input(value: &str, id: u64) -> Widget<'_, ()> {
    Widget::controlled_text_input(
        value,
        unexpected_change,
        None,
        Style::default(),
        "Channel",
        "",
        InputState::Idle,
        None,
        false,
    )
    .with_id(id)
}

fn runtime_for(widget: &Widget<'_, ()>) -> WidgetRuntimeCaches<()> {
    let mut caches = WidgetRuntimeCaches::default();
    let taffy = TaffyTree::new();
    RutterEngine::<ControlledInputApp>::sync_runtime_metadata(
        &mut caches,
        &mut HashMap::new(),
        &mut HashMap::new(),
        RuntimeMetadataSources {
            input_states: &HashMap::new(),
            taffy: &taffy,
        },
        widget,
        None,
        8.0,
    )
    .unwrap();
    caches
}

fn select_middle(state: &mut InputWidgetState) {
    state.editor.set_cursor(Cursor::new(0, 2));
    state
        .editor
        .set_selection(Selection::Normal(Cursor::new(0, 1)));
    state.selection = Some(TextSelection { start: 1, end: 2 });
    state.selection_anchor = Some(1);
}

#[test]
fn controlled_text_input_initial_value_is_owned_and_applied_without_callbacks() {
    let caches = {
        let value = String::from("#123abc");
        runtime_for(&controlled_input(&value, 1))
    };
    let mut fonts = FontSystem::new();
    let mut state = InputWidgetState::new(&mut fonts);

    assert!(
        caches.inputs[&1]
            .sync_model_value(&mut state, &mut fonts)
            .unwrap()
    );

    assert_eq!(state.text(), "#123abc");
    assert_eq!(state.undo.current(), Some("#123abc"));
    assert!(!state.undo.can_undo());
    assert_eq!(
        state.limits(),
        ControlledInputApp::input_limits(1, InputKind::TextInput)
    );
}

#[test]
fn controlled_text_input_callback_echo_preserves_caret_selection_and_history() {
    let mut fonts = FontSystem::new();
    let mut state = InputWidgetState::new(&mut fonts);
    let initial = runtime_for(&controlled_input("abc", 1));
    initial.inputs[&1]
        .sync_model_value(&mut state, &mut fonts)
        .unwrap();
    state.editor.set_cursor(Cursor::new(0, 3));
    assert!(state.try_insert_text(&mut fonts, "d").unwrap());
    state.snapshot();
    select_middle(&mut state);
    let cursor = state.editor.cursor();
    let selection = state.editor.selection();
    let echo = runtime_for(&controlled_input(&state.text(), 1));

    assert!(
        !echo.inputs[&1]
            .sync_model_value(&mut state, &mut fonts)
            .unwrap()
    );

    assert_eq!(state.editor.cursor(), cursor);
    assert_eq!(state.editor.selection(), selection);
    assert_eq!(state.selection, Some(TextSelection { start: 1, end: 2 }));
    assert_eq!(state.selection_anchor, Some(1));
    assert_eq!(state.undo.current(), Some("abcd"));
    assert!(state.try_undo(&mut fonts).unwrap());
    assert_eq!(state.text(), "abc");
}

#[test]
fn controlled_text_input_external_replacement_clears_selection_and_undo_redo() {
    let mut fonts = FontSystem::new();
    let mut state = InputWidgetState::new(&mut fonts);
    state.try_set_text(&mut fonts, "old").unwrap();
    state.snapshot();
    state.try_set_text(&mut fonts, "edited").unwrap();
    state.snapshot();
    assert!(state.try_undo(&mut fonts).unwrap());
    assert!(state.undo.can_redo());
    select_middle(&mut state);
    let caches = runtime_for(&controlled_input("replacement", 1));

    assert!(
        caches.inputs[&1]
            .sync_model_value(&mut state, &mut fonts)
            .unwrap()
    );

    assert_eq!(state.text(), "replacement");
    assert_eq!(state.selection, None);
    assert_eq!(state.selection_anchor, None);
    assert_eq!(state.editor.selection(), Selection::None);
    assert_eq!(state.undo.current(), Some("replacement"));
    assert!(!state.try_undo(&mut fonts).unwrap());
    assert!(!state.try_redo(&mut fonts).unwrap());
    assert!(state.try_insert_text(&mut fonts, "!").unwrap());
    state.snapshot();
    assert!(state.try_undo(&mut fonts).unwrap());
    assert_eq!(state.text(), "replacement");
}

#[test]
fn controlled_text_input_rejects_application_limit_violations_atomically() {
    let caches = runtime_for(&controlled_input("12345", 3));
    let mut fonts = FontSystem::new();
    let mut state = InputWidgetState::new(&mut fonts);
    state.try_set_text(&mut fonts, "old").unwrap();
    state.snapshot();
    select_middle(&mut state);
    let cursor = state.editor.cursor();
    let selection = state.editor.selection();

    assert_eq!(
        caches.inputs[&3].sync_model_value(&mut state, &mut fonts),
        Err(InputLimitError::BytesExceeded { actual: 5, max: 4 })
    );

    assert_eq!(state.text(), "old");
    assert_eq!(state.editor.cursor(), cursor);
    assert_eq!(state.editor.selection(), selection);
    assert_eq!(state.selection, Some(TextSelection { start: 1, end: 2 }));
    assert_eq!(state.selection_anchor, Some(1));
    assert_eq!(state.undo.current(), Some("old"));
    assert!(state.undo.can_undo());
    let valid = runtime_for(&controlled_input("1234", 3));
    assert!(
        valid.inputs[&3]
            .sync_model_value(&mut state, &mut fonts)
            .unwrap()
    );
    assert_eq!(state.text(), "1234");
}

#[test]
fn controlled_text_input_rejects_multiline_model_values() {
    let caches = runtime_for(&controlled_input("a\nb", 1));
    let mut fonts = FontSystem::new();
    let mut state = InputWidgetState::new(&mut fonts);

    assert_eq!(
        caches.inputs[&1].sync_model_value(&mut state, &mut fonts),
        Err(InputLimitError::LinesExceeded { actual: 2, max: 1 })
    );
    assert!(state.text().is_empty());
}

#[test]
fn controlled_text_input_instances_synchronize_independently() {
    let widget = Widget::Row {
        children: vec![controlled_input("red", 1), controlled_input("blue", 2)],
        style: Style::default(),
    };
    let caches = runtime_for(&widget);
    let mut fonts = FontSystem::new();
    let mut first = InputWidgetState::new(&mut fonts);
    let mut second = InputWidgetState::new(&mut fonts);
    caches.inputs[&1]
        .sync_model_value(&mut first, &mut fonts)
        .unwrap();
    caches.inputs[&2]
        .sync_model_value(&mut second, &mut fonts)
        .unwrap();
    select_middle(&mut second);
    let second_cursor = second.editor.cursor();
    let changed = runtime_for(&controlled_input("green", 1));

    changed.inputs[&1]
        .sync_model_value(&mut first, &mut fonts)
        .unwrap();

    assert_eq!(first.text(), "green");
    assert_eq!(second.text(), "blue");
    assert_eq!(second.editor.cursor(), second_cursor);
    assert_eq!(second.selection, Some(TextSelection { start: 1, end: 2 }));
}

#[test]
fn uncontrolled_text_input_keeps_retained_edits_and_history() {
    let widget = Widget::text_input(
        unexpected_change,
        None,
        Style::default(),
        "Uncontrolled",
        "",
        InputState::Idle,
        None,
        false,
    )
    .with_id(1);
    let caches = runtime_for(&widget);
    let mut fonts = FontSystem::new();
    let mut state = InputWidgetState::new(&mut fonts);
    state.try_set_text(&mut fonts, "retained").unwrap();
    state.snapshot();
    select_middle(&mut state);

    assert!(
        !caches.inputs[&1]
            .sync_model_value(&mut state, &mut fonts)
            .unwrap()
    );

    assert!(caches.inputs[&1].model_value.is_none());
    assert_eq!(state.text(), "retained");
    assert_eq!(state.editor.cursor(), Cursor::new(0, 2));
    assert_eq!(state.selection, Some(TextSelection { start: 1, end: 2 }));
    assert_eq!(state.undo.current(), Some("retained"));
    assert!(state.undo.can_undo());
}

struct LifecycleApp;

struct LifecycleModel {
    value: String,
    disabled: bool,
    present: bool,
    controlled: bool,
    password: bool,
    echoes: usize,
}

#[derive(Clone, Debug)]
enum LifecycleMessage {
    Changed(String),
    Submit,
}

impl LifecycleModel {
    // Echo the actual cached on_change result without constructing a clipboard.
    fn reduce(&mut self, message: LifecycleMessage) {
        let LifecycleMessage::Changed(value) = message else {
            panic!("layout synchronization must not submit the field");
        };
        self.value = value;
        self.echoes += 1;
    }
}

fn lifecycle_style() -> Style {
    use taffy::prelude::{Dimension, Size};

    Style {
        size: Size {
            width: Dimension::length(160.0),
            height: Dimension::length(40.0),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

impl AppLogic for LifecycleApp {
    type State = LifecycleModel;
    type Message = LifecycleMessage;

    fn new(_: &mut FontSystem) -> LifecycleModel {
        LifecycleModel {
            value: "abc".to_owned(),
            disabled: false,
            present: true,
            controlled: true,
            password: false,
            echoes: 0,
        }
    }

    fn view(state: &mut LifecycleModel) -> Widget<'_, LifecycleMessage> {
        let mut children = Vec::new();
        if state.present {
            children.push(Widget::Container {
                child: Box::new(Widget::TextInput {
                    id: 1,
                    model_value: state.controlled.then_some(state.value.as_str()),
                    on_change: LifecycleMessage::Changed,
                    on_submit: Some(LifecycleMessage::Submit),
                    style: lifecycle_style(),
                    label: "Controlled",
                    placeholder: "",
                    state: InputState::Idle,
                    error_msg: None,
                    is_password: state.password,
                }),
                style: Style::default(),
                color: None,
                radius: 0.0,
            });
        }
        children.push(
            Widget::text_input(
                LifecycleMessage::Changed,
                None,
                lifecycle_style(),
                "Uncontrolled",
                "",
                InputState::Idle,
                None,
                false,
            )
            .with_id(2),
        );
        children.push(Widget::Button {
            text: "Submit",
            on_press: LifecycleMessage::Submit,
            style: lifecycle_style(),
            color: None,
            variant: crate::widget::ButtonVariant::Primary,
        });
        children.push(
            Widget::slider(
                0.5,
                0.0,
                1.0,
                0.1,
                |_| LifecycleMessage::Submit,
                lifecycle_style(),
                "Level",
            )
            .with_id(4),
        );
        Widget::Column {
            children,
            style: Style::default(),
        }
        .enabled(!state.disabled)
    }

    fn update(_: &mut LifecycleModel, _: LifecycleMessage, _: &mut Clipboard) {
        panic!("model synchronization must not dispatch application messages");
    }
}

fn lifecycle_engine() -> RutterEngine<LifecycleApp> {
    // The engine's clipboard requires a display; run lifecycle tests through
    // the isolated Xvfb harness rather than altering its production field type.
    RutterEngine::new().expect("engine lifecycle tests require an isolated clipboard/display")
}

fn refresh_lifecycle(engine: &mut RutterEngine<LifecycleApp>) {
    engine.layout_dirty = true;
    engine
        .try_ensure_layout_for_input(PhysicalSize::new(400, 300))
        .unwrap();
}

fn echo_lifecycle_edit(engine: &mut RutterEngine<LifecycleApp>) {
    let callback = engine.runtime_caches.inputs[&1].on_change;
    engine
        .app_state
        .reduce(callback(engine.input_states[&1].text()));
    refresh_lifecycle(engine);
}

#[test]
fn layout_materializes_initial_controlled_text_before_any_focus() {
    let mut engine = lifecycle_engine();
    assert!(engine.input_states.is_empty());

    engine
        .try_ensure_layout_for_input(PhysicalSize::new(400, 300))
        .unwrap();

    assert_eq!(engine.input_states[&1].text(), "abc");
    assert_eq!(engine.input_states[&1].undo.current(), Some("abc"));
    assert!(!engine.input_states[&1].undo.can_undo());
    assert!(!engine.input_states.contains_key(&2));
    assert_eq!(engine.focused_widget_id, None);
    assert_eq!(engine.focused_input_id(), None);
    assert_eq!(engine.app_state.echoes, 0);
    assert_eq!(engine.layout_revision, 1);
}

#[test]
fn layout_callback_echo_preserves_focus_selection_scroll_and_undo_redo() {
    let mut engine = lifecycle_engine();
    refresh_lifecycle(&mut engine);
    engine.focused_widget_id = Some(1);
    let (cursor, selection) = {
        let mut fonts = engine.font_system.borrow_mut();
        let state = engine.input_states.get_mut(&1).unwrap();
        state.editor.set_cursor(Cursor::new(0, 3));
        assert!(state.try_insert_text(&mut fonts, "d").unwrap());
        state.snapshot();
        select_middle(state);
        state.scroll_x = 12.0;
        state.scroll_y = 3.0;
        (state.editor.cursor(), state.editor.selection())
    };

    echo_lifecycle_edit(&mut engine);

    let state = &engine.input_states[&1];
    assert_eq!(engine.focused_input_id(), Some(1));
    assert_eq!(state.editor.cursor(), cursor);
    assert_eq!(state.editor.selection(), selection);
    assert_eq!(state.selection, Some(TextSelection { start: 1, end: 2 }));
    assert_eq!(state.selection_anchor, Some(1));
    assert_eq!((state.scroll_x, state.scroll_y), (12.0, 3.0));
    assert_eq!(state.undo.current(), Some("abcd"));
    assert_eq!(engine.app_state.echoes, 1);

    assert!(
        engine
            .input_states
            .get_mut(&1)
            .unwrap()
            .try_undo(&mut engine.font_system.borrow_mut())
            .unwrap()
    );
    echo_lifecycle_edit(&mut engine);
    assert_eq!(engine.app_state.value, "abc");
    assert_eq!(engine.focused_input_id(), Some(1));
    assert!(engine.input_states[&1].undo.can_redo());

    assert!(
        engine
            .input_states
            .get_mut(&1)
            .unwrap()
            .try_redo(&mut engine.font_system.borrow_mut())
            .unwrap()
    );
    echo_lifecycle_edit(&mut engine);
    assert_eq!(engine.app_state.value, "abcd");
    assert_eq!(engine.focused_input_id(), Some(1));
    assert!(engine.input_states[&1].undo.can_undo());
    assert_eq!(engine.app_state.echoes, 3);
}

#[test]
fn layout_external_replacement_starts_new_history_without_callbacks() {
    let mut engine = lifecycle_engine();
    refresh_lifecycle(&mut engine);
    engine.focused_widget_id = Some(1);
    {
        let mut fonts = engine.font_system.borrow_mut();
        let state = engine.input_states.get_mut(&1).unwrap();
        state.editor.set_cursor(Cursor::new(0, 3));
        assert!(state.try_insert_text(&mut fonts, "d").unwrap());
        state.snapshot();
        assert!(state.try_undo(&mut fonts).unwrap());
        select_middle(state);
        assert!(state.undo.can_redo());
    }
    engine.app_state.value = "replacement".to_owned();

    refresh_lifecycle(&mut engine);

    let state = engine.input_states.get_mut(&1).unwrap();
    assert_eq!(state.text(), "replacement");
    assert_eq!(state.editor.selection(), Selection::None);
    assert_eq!(state.selection, None);
    assert_eq!(state.selection_anchor, None);
    assert_eq!(state.undo.current(), Some("replacement"));
    assert!(
        !state
            .try_undo(&mut engine.font_system.borrow_mut())
            .unwrap()
    );
    assert!(
        !state
            .try_redo(&mut engine.font_system.borrow_mut())
            .unwrap()
    );
    assert_eq!(engine.focused_input_id(), Some(1));
    assert_eq!(engine.app_state.echoes, 0);
}

fn assert_disabled_lifecycle(engine: &mut RutterEngine<LifecycleApp>) {
    assert!(engine.runtime_caches.inputs.is_empty());
    assert!(engine.runtime_caches.buttons.is_empty());
    assert!(engine.runtime_caches.sliders.is_empty());
    assert!(engine.runtime_caches.input_order.is_empty());
    assert!(engine.runtime_caches.focus_order.is_empty());
    assert!(!runtime_focus_is_live(&engine.runtime_caches, 1));
    assert_eq!(engine.focused_widget_id, None);
    assert_eq!(engine.focused_input_id(), None);
    engine.ensure_input_state(2);
    assert!(!engine.input_states.contains_key(&2));
    assert_eq!(engine.app_state.echoes, 0);
}

#[test]
fn disabled_layout_presents_initial_and_external_values_without_interaction() {
    let mut engine = lifecycle_engine();
    engine.app_state.disabled = true;
    refresh_lifecycle(&mut engine);
    assert_eq!(engine.input_states[&1].text(), "abc");
    assert_disabled_lifecycle(&mut engine);

    engine.app_state.value = "disabled replacement".to_owned();
    refresh_lifecycle(&mut engine);
    assert_eq!(engine.input_states[&1].text(), "disabled replacement");
    assert_disabled_lifecycle(&mut engine);

    engine.app_state.disabled = false;
    refresh_lifecycle(&mut engine);
    assert!(
        engine
            .runtime_caches
            .disabled_input_presentations
            .is_empty()
    );
    assert!(engine.runtime_caches.inputs.contains_key(&1));
    assert_eq!(engine.focused_widget_id, None);
    engine.focused_widget_id = Some(1);
    engine.app_state.disabled = true;
    engine.app_state.value = "disabled again".to_owned();
    refresh_lifecycle(&mut engine);
    assert_eq!(engine.input_states[&1].text(), "disabled again");
    assert_disabled_lifecycle(&mut engine);
}

#[test]
fn disabled_presentation_is_pruned_when_uncontrolled_or_removed() {
    let mut engine = lifecycle_engine();
    engine.app_state.disabled = true;
    refresh_lifecycle(&mut engine);
    engine.app_state.controlled = false;
    engine.app_state.value = "must not replace retained text".to_owned();
    refresh_lifecycle(&mut engine);
    assert!(
        engine
            .runtime_caches
            .disabled_input_presentations
            .is_empty()
    );
    assert!(
        engine
            .runtime_cache_scratch
            .disabled_input_presentations
            .is_empty()
    );
    assert_eq!(engine.input_states[&1].text(), "abc");

    engine.app_state.controlled = true;
    refresh_lifecycle(&mut engine);
    assert_eq!(engine.runtime_caches.disabled_input_presentations.len(), 1);
    engine.app_state.present = false;
    refresh_lifecycle(&mut engine);
    refresh_lifecycle(&mut engine);
    assert!(
        engine
            .runtime_caches
            .disabled_input_presentations
            .is_empty()
    );
    assert!(
        engine
            .runtime_cache_scratch
            .disabled_input_presentations
            .is_empty()
    );
    assert!(!engine.input_states.contains_key(&1));
}

#[test]
fn unfocused_external_replacement_resets_old_scroll_but_echo_does_not() {
    let mut engine = lifecycle_engine();
    engine.app_state.value = "long input ".repeat(80);
    refresh_lifecycle(&mut engine);
    {
        let mut fonts = engine.font_system.borrow_mut();
        let runtime = &engine.runtime_caches.inputs[&1];
        let state = engine.input_states.get_mut(&1).unwrap();
        state
            .editor
            .set_cursor(Cursor::new(0, engine.app_state.value.len()));
        state.update_scroll(
            &mut fonts,
            runtime.visible_w,
            runtime.visible_h,
            14.0,
            false,
            false,
        );
        assert!(state.scroll_x > 0.0);
        state.scroll_y = 25.0;
    }
    let old_scroll = (
        engine.input_states[&1].scroll_x,
        engine.input_states[&1].scroll_y,
    );
    refresh_lifecycle(&mut engine);
    assert_eq!(
        (
            engine.input_states[&1].scroll_x,
            engine.input_states[&1].scroll_y
        ),
        old_scroll,
    );

    engine.app_state.value = "short".to_owned();
    refresh_lifecycle(&mut engine);
    assert_eq!(engine.input_states[&1].text(), "short");
    assert_eq!(
        (
            engine.input_states[&1].scroll_x,
            engine.input_states[&1].scroll_y
        ),
        (0.0, 0.0),
    );
    assert_eq!(engine.focused_widget_id, None);
}

#[test]
fn password_runtime_clones_and_disabled_presentation_debug_redact_models() {
    let mut engine = lifecycle_engine();
    let secret = "private-password-model";
    engine.app_state.password = true;
    engine.app_state.value = secret.to_owned();
    refresh_lifecycle(&mut engine);
    let cloned = engine.runtime_caches.inputs[&1].clone();
    assert!(cloned.model_value.as_deref() == Some(secret));
    assert!(!format!("{cloned:?}").contains(secret));
    assert!(!format!("{:?}", engine.runtime_caches).contains(secret));
    assert!(engine.input_states[&1].is_sensitive());
    assert!(!engine.input_states[&1].undo.can_undo());

    engine.app_state.disabled = true;
    refresh_lifecycle(&mut engine);
    assert!(!format!("{:?}", engine.runtime_caches).contains(secret));
    assert!(engine.runtime_cache_scratch.inputs.is_empty());
    assert!(
        engine
            .runtime_cache_scratch
            .disabled_input_presentations
            .is_empty()
    );
    assert_disabled_lifecycle(&mut engine);
}
