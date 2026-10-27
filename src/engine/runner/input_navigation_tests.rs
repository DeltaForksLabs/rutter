//! Keyboard events pass through publication, application shortcuts and editing.
use super::*;
use crate::app::{ShortcutEvent, ShortcutKey, ShortcutNamedKey, ShortcutOutcome};
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::widget::{InputState, Widget};
use cosmic_text::Cursor;
use taffy::prelude::{Dimension, Size, Style};
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, ModifiersState, PhysicalKey};

struct NavigationApp;

#[derive(Default)]
struct Model {
    multiline: bool,
    disabled: bool,
    password: bool,
    intercept: bool,
    suggestion_kind: u8,
    changes: Vec<String>,
}

impl AppLogic for NavigationApp {
    type State = Model;
    type Message = String;
    fn new(_: &mut FontSystem) -> Model {
        Model::default()
    }
    fn view(model: &mut Model) -> Widget<'_, String> {
        let style = Style {
            size: Size {
                width: Dimension::length(240.0),
                height: Dimension::length(100.0),
            },
            flex_shrink: 0.0,
            ..Style::default()
        };
        let input = if model.suggestion_kind == 1 {
            Widget::search_bar_with_suggestions(
                String::from,
                None,
                None,
                None,
                "",
                crate::search::SearchSuggestions::new(
                    &["one two", "one two more"],
                    crate::SearchMatcher::Fuzzy,
                    6,
                    Some(|index| index.to_string()),
                )
                .unwrap(),
                style,
            )
        } else if model.suggestion_kind == 2 {
            Widget::combo_box(
                2,
                "one two",
                String::from,
                crate::combo_box::ComboBox::new(&["one two", "one two more"], |index| {
                    index.to_string()
                }),
                style,
            )
        } else if model.multiline {
            Widget::text_area(
                String::from,
                None,
                style,
                "Label",
                InputState::Idle,
                "",
                Some("Error".into()),
            )
        } else {
            Widget::text_input(
                String::from,
                None,
                style,
                "Label",
                "",
                InputState::Idle,
                Some("Error".into()),
                model.password,
            )
        }
        .with_id(2)
        .enabled(!model.disabled);
        Widget::scroll_view(
            Widget::Column {
                children: vec![
                    input,
                    Widget::Spacer {
                        style: Style {
                            size: Size {
                                width: Dimension::auto(),
                                height: Dimension::length(600.0),
                            },
                            flex_shrink: 0.0,
                            ..Style::default()
                        },
                    },
                ],
                style: Style {
                    flex_direction: taffy::FlexDirection::Column,
                    ..Style::default()
                },
            },
            Style {
                size: Size {
                    width: Dimension::length(320.0),
                    height: Dimension::length(180.0),
                },
                ..Style::default()
            },
        )
        .with_id(10)
    }
    fn update(model: &mut Model, message: String, _: &mut arboard::Clipboard) {
        model.changes.push(message);
    }
    fn shortcut(model: &Model, event: ShortcutEvent) -> ShortcutOutcome<String> {
        if model.intercept
            && event.control
            && event.key == ShortcutKey::Named(ShortcutNamedKey::ArrowRight)
        {
            ShortcutOutcome::Consumed
        } else {
            ShortcutOutcome::Ignored
        }
    }
}

fn runner(multiline: bool, password: bool) -> RutterRunner<NavigationApp> {
    runner_with_model(Model {
        multiline,
        password,
        ..Model::default()
    })
}

fn runner_with_model(model: Model) -> RutterRunner<NavigationApp> {
    let mut engine = RutterEngine::<NavigationApp>::new().expect("isolated clipboard service");
    engine.app_state = model;
    engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(320, 180),
        scale: 1.0,
        popup: false,
        popup_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    runner.refresh_layout_before_keyboard_input().unwrap();
    runner.focus_widget(Some(2));
    runner.engine.ensure_input_state(2);
    runner
}

fn text(runner: &mut RutterRunner<NavigationApp>, text: &str, cursor: Cursor) {
    let mut fonts = runner.engine.font_system.borrow_mut();
    let state = runner.engine.input_states.get_mut(&2).unwrap();
    state.set_text(&mut fonts, text);
    state.snapshot();
    state.editor.set_cursor(cursor);
}

fn key(runner: &mut RutterRunner<NavigationApp>, key: NamedKey, modifiers: ModifiersState) {
    runner.engine.modifiers = modifiers.into();
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::ArrowRight),
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
    assert!(runner.fatal_error.is_none());
}

#[test]
fn input_navigation_ctrl_shift_reverses_shrinks_and_crosses_anchor_without_edits() {
    let mut runner = runner(true, false);
    text(&mut runner, "one two\nthree four", Cursor::new(0, 4));
    let shift = ModifiersState::CONTROL | ModifiersState::SHIFT;
    for (arrow, end) in [
        (NamedKey::ArrowRight, 8),
        (NamedKey::ArrowRight, 14),
        (NamedKey::ArrowLeft, 8),
        (NamedKey::ArrowLeft, 4),
        (NamedKey::ArrowLeft, 0),
    ] {
        key(&mut runner, arrow, shift);
        let state = &runner.engine.input_states[&2];
        assert_eq!(state.selection_anchor, Some(4));
        assert_eq!(
            state.selection,
            Some(crate::input_state::TextSelection { start: 4, end })
        );
    }
    key(&mut runner, NamedKey::ArrowRight, ModifiersState::CONTROL);
    assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), 4);
    assert_eq!(runner.engine.input_states[&2].selection, None);
    key(&mut runner, NamedKey::ArrowRight, ModifiersState::empty());
    assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), 5);
    assert!(runner.engine.app_state.changes.is_empty());
    assert!(!runner.engine.snapshot_scheduled);
    assert_eq!(
        runner.engine.input_states[&2].undo.current(),
        Some("one two\nthree four")
    );
    let mut fonts = runner.engine.font_system.borrow_mut();
    let state = runner.engine.input_states.get_mut(&2).unwrap();
    assert!(state.try_undo(&mut fonts).unwrap());
    assert_eq!(state.text(), "");
}

#[test]
fn input_navigation_page_uses_body_viewport_and_shift_anchor_with_visible_caret() {
    let mut runner = runner(true, false);
    text(&mut runner, &"abcdef\n".repeat(30), Cursor::new(0, 3));
    let viewport = runner.engine.runtime_caches.inputs[&2].visible_h;
    let font = NavigationApp::theme_for(&runner.engine.app_state).font_body;
    assert_eq!(
        viewport,
        100.0 - NavigationApp::theme_for(&runner.engine.app_state).spacing * 2.0
    );
    let rows = (viewport / (font * 1.3)).floor().max(1.0) as usize;
    key(&mut runner, NamedKey::PageDown, ModifiersState::SHIFT);
    let state = &runner.engine.input_states[&2];
    assert_eq!(state.editor.cursor().line, rows);
    assert_eq!(state.editor.cursor().index, 3);
    assert_eq!(state.selection_anchor, Some(3));
    assert_eq!(state.selection.unwrap().end, rows * 7 + 3);
    let top = rows as f32 * font * 1.3;
    assert!(top >= state.scroll_y && top + font * 1.3 <= state.scroll_y + viewport + 1.0);
    key(&mut runner, NamedKey::PageUp, ModifiersState::SHIFT);
    assert_eq!(runner.engine.input_states[&2].selection_anchor, Some(3));
    assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), 3);
    assert!(runner.engine.app_state.changes.is_empty());
    assert!(!runner.engine.snapshot_scheduled);
}

#[test]
fn input_navigation_shortcut_precedence_single_line_pages_password_and_disabled() {
    let mut runner = runner(false, true);
    text(&mut runner, "one two", Cursor::new(0, 0));
    runner.engine.app_state.intercept = true;
    key(&mut runner, NamedKey::ArrowRight, ModifiersState::CONTROL);
    assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), 0);
    runner.engine.app_state.intercept = false;
    key(
        &mut runner,
        NamedKey::ArrowRight,
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    );
    assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), 4);
    assert_eq!(runner.engine.input_states[&2].display_text(true), "•••••••");
    let selection = runner.engine.input_states[&2].selection;
    let parent_scroll = runner.engine.widget_states[&10]
        .as_scroll()
        .unwrap()
        .offset_y;
    key(&mut runner, NamedKey::PageDown, ModifiersState::empty());
    key(&mut runner, NamedKey::PageUp, ModifiersState::SHIFT);
    assert_eq!(runner.engine.input_states[&2].selection, selection);
    assert_eq!(
        runner.engine.widget_states[&10]
            .as_scroll()
            .unwrap()
            .offset_y,
        parent_scroll
    );
    assert!(!runner.engine.input_states[&2].undo.can_undo());
    runner.engine.app_state.disabled = true;
    runner.engine.layout_dirty = true;
    key(&mut runner, NamedKey::ArrowRight, ModifiersState::CONTROL);
    assert!(runner.engine.focused_input_id().is_none());
    assert!(runner.engine.app_state.changes.is_empty());
}

#[test]
fn input_navigation_search_and_combo_republication_preserve_word_selection_and_popup() {
    for kind in [1, 2] {
        let mut runner = runner_with_model(Model {
            suggestion_kind: kind,
            ..Model::default()
        });
        text(&mut runner, "one two", Cursor::new(0, 0));
        assert!(runner.expand_search_accessibility_target(2));
        runner.engine.layout_dirty = true;
        runner.refresh_layout_before_keyboard_input().unwrap();
        assert!(
            !runner.engine.widget_states[&2]
                .as_search()
                .unwrap()
                .dismissed
        );
        key(
            &mut runner,
            NamedKey::ArrowRight,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        );
        runner.engine.layout_dirty = true;
        runner.refresh_layout_before_keyboard_input().unwrap();
        let state = &runner.engine.input_states[&2];
        assert_eq!(state.cursor_byte_index(), 4);
        assert_eq!(state.selection_anchor, Some(0));
        assert_eq!(state.selection.unwrap().normalized(), (0, 4));
        assert!(
            !runner.engine.widget_states[&2]
                .as_search()
                .unwrap()
                .dismissed
        );
        let selection = state.selection;
        key(&mut runner, NamedKey::PageDown, ModifiersState::empty());
        assert_eq!(runner.engine.input_states[&2].selection, selection);
        assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), 4);
        assert!(runner.engine.app_state.changes.is_empty());
    }
}

#[test]
fn input_navigation_existing_home_end_paragraph_and_ime_commit_paths_remain_usable() {
    let mut runner = runner(true, false);
    text(&mut runner, "one two\nthree four", Cursor::new(0, 0));
    for (key_name, modifiers, expected) in [
        (NamedKey::End, ModifiersState::empty(), 7),
        (NamedKey::Home, ModifiersState::empty(), 0),
        (NamedKey::ArrowDown, ModifiersState::CONTROL, 7),
        (NamedKey::ArrowUp, ModifiersState::CONTROL, 0),
        (NamedKey::End, ModifiersState::CONTROL, 18),
        (NamedKey::Home, ModifiersState::CONTROL, 0),
    ] {
        key(&mut runner, key_name, modifiers);
        assert_eq!(runner.engine.input_states[&2].cursor_byte_index(), expected);
    }
    key(
        &mut runner,
        NamedKey::ArrowRight,
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    );
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::Ime(Ime::Commit("é".into())),
    );
    assert_eq!(runner.engine.input_states[&2].text(), "étwo\nthree four");
    assert_eq!(runner.engine.app_state.changes, ["étwo\nthree four"]);
}
