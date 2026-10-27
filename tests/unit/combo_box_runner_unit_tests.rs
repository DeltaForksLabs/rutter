//! Real controlled publication and runner key routing under the test clipboard display.
use super::*;
use crate::Theme;
use crate::combo_box::ComboBox;
use crate::engine::{
    RutterEngine,
    test_surface::{MemoryEventLoop, MemoryWindow},
};
use crate::widget::{InputState, Widget};
use cosmic_text::{Cursor, Edit, FontSystem};
use std::{collections::HashMap, rc::Rc};
use taffy::prelude::*;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, KeyLocation, ModifiersState, PhysicalKey},
    window::WindowId,
};

const ITEMS: &[&str] = &["Other", "São Paulo", "Santos", "Lisboa", "東京", "Tail"];
#[derive(Default)]
struct Model {
    query: String,
    picks: Vec<usize>,
    disabled: bool,
}
#[derive(Debug, Clone)]
enum Msg {
    Draft(String),
    Pick(usize),
    Button,
}
struct ComboApp;
impl AppLogic for ComboApp {
    type State = Model;
    type Message = Msg;
    fn new(_: &mut FontSystem) -> Model {
        Model::default()
    }
    fn view(model: &mut Model) -> Widget<'_, Msg> {
        Widget::Column {
            style: Style {
                gap: Size::length(12.0_f32),
                ..Style::default()
            },
            children: vec![
                Widget::combo_box(
                    62,
                    &model.query,
                    Msg::Draft,
                    ComboBox::new(ITEMS, Msg::Pick)
                        .with_visible_rows(2)
                        .unwrap()
                        .with_label("Destination")
                        .with_state(InputState::Error)
                        .with_error("Choose or keep draft"),
                    Style {
                        size: Size::from_lengths(240.0, 40.0),
                        ..Style::default()
                    },
                )
                .enabled(!model.disabled),
                Widget::Button {
                    text: "Next",
                    on_press: Msg::Button,
                    style: Style::default(),
                    color: None,
                    variant: Default::default(),
                }
                .with_id(63),
            ],
        }
    }
    fn update(model: &mut Model, message: Msg, _: &mut arboard::Clipboard) {
        match message {
            Msg::Draft(query) => model.query = query,
            Msg::Pick(index) => {
                model.picks.push(index);
                model.query = ITEMS[index].into();
            }
            Msg::Button => {}
        }
    }
}
fn publish(runner: &mut RutterRunner<ComboApp>) {
    runner.refresh_layout_before_keyboard_input().unwrap();
}
fn runner(query: &str) -> RutterRunner<ComboApp> {
    let mut engine = RutterEngine::<ComboApp>::new().unwrap();
    engine.app_state.query = query.into();
    engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(320, 240),
        scale: 1.0,
        popup: false,
        popup_state: None,
    }));
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut runner);
    runner.focus_widget(Some(62));
    publish(&mut runner);
    runner
}
fn press(runner: &mut RutterRunner<ComboApp>, named: NamedKey, modifiers: ModifiersState) {
    runner.engine.modifiers = modifiers.into();
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Tab),
                logical_key: Key::Named(named),
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
                text_with_all_modifiers: None,
                key_without_modifiers: Key::Named(named),
            },
        },
    );
    publish(runner);
}
fn key(runner: &mut RutterRunner<ComboApp>, named: NamedKey) {
    press(runner, named, ModifiersState::empty());
}

fn painted_toggle(runner: &mut RutterRunner<ComboApp>) -> Vec<skia_safe::Color> {
    use crate::render::{draw_widgets, text::TextBufferCache};
    let widget = ComboApp::view(&mut runner.engine.app_state);
    let mut surface = skia_safe::surfaces::raster_n32_premul((320, 240)).unwrap();
    draw_widgets(
        surface.canvas(),
        &runner.engine.taffy,
        runner.engine.last_root_node,
        &widget,
        &mut runner.engine.font_system.borrow_mut(),
        &mut cosmic_text::SwashCache::new(),
        skia_safe::Point::new(-10.0, -10.0),
        runner.engine.focused_widget_id,
        &runner.engine.input_states,
        &runner.engine.widget_states,
        &mut HashMap::new(),
        &mut TextBufferCache::default(),
        false,
        &Theme::light(),
        1.0,
    );
    let pixels = surface.peek_pixels().unwrap();
    (34..47)
        .flat_map(|y| (214..227).map(move |x| (x, y)))
        .map(|point| pixels.get_color(point))
        .collect()
}

#[test]
fn combo_box_painted_arrow_tracks_keyboard_toggle_selection_and_blur() {
    let mut r = runner("");
    let closed = painted_toggle(&mut r);
    key(&mut r, NamedKey::ArrowDown);
    let open = painted_toggle(&mut r);
    assert_ne!(
        closed, open,
        "the actual painted chevron must change direction"
    );
    key(&mut r, NamedKey::Escape);
    assert_eq!(painted_toggle(&mut r), closed);
    r.focus_input_at(62, 220.0, 20.0, 240.0, 40.0, false);
    assert!(
        r.focused_input_rect.is_none(),
        "toggle cannot start text selection drag"
    );
    assert_eq!(painted_toggle(&mut r), open);
    r.focus_input_at(62, 220.0, 20.0, 240.0, 40.0, false);
    assert_eq!(painted_toggle(&mut r), closed);
    key(&mut r, NamedKey::ArrowDown);
    key(&mut r, NamedKey::Enter);
    assert_eq!(painted_toggle(&mut r), closed);
    key(&mut r, NamedKey::ArrowDown);
    r.focus_widget(None);
    assert_eq!(painted_toggle(&mut r), closed);
}

#[test]
fn combo_box_trailing_reservation_preserves_caret_origin_and_exact_toggle_hit() {
    let mut r = runner("Santos");
    let input = &r.engine.runtime_caches.inputs[&62];
    assert_eq!(input.leading_text_inset, 0.0);
    assert_eq!(
        input.visible_w,
        crate::combo_box::text_width((240.0, 40.0), Theme::light().spacing)
    );
    r.focus_input_at(62, 16.0, 20.0, 240.0, 40.0, false);
    assert_eq!(r.engine.input_states[&62].editor.cursor().index, 0);
    assert!(r.search_popup_is_open(62));
    // Same x as the square but outside its vertical extent: editor, not toggle.
    r.focus_input_at(62, 220.0, 1.0, 240.0, 40.0, false);
    assert!(r.search_popup_is_open(62));
    assert_eq!(r.engine.input_states[&62].editor.cursor().index, 6);
    r.focus_input_at(62, 220.0, 20.0, 240.0, 40.0, false);
    assert!(!r.search_popup_is_open(62));
    assert_eq!(r.engine.input_states[&62].editor.cursor().index, 6);
    let visible_w = r.engine.runtime_caches.inputs[&62].visible_w;
    r.engine.app_state.disabled = true;
    r.engine.layout_dirty = true;
    publish(&mut r);
    assert_eq!(
        r.engine.runtime_caches.disabled_input_presentations[&62].visible_w,
        visible_w
    );
}

#[test]
fn combo_box_empty_browse_reaches_tail_and_enter_commits_once() {
    let mut r = runner("");
    assert!(!r.search_popup_is_open(62));
    assert_eq!(
        r.engine.runtime_caches.searches[&62].results.len(),
        ITEMS.len()
    );
    key(&mut r, NamedKey::ArrowUp);
    assert!(r.search_popup_is_open(62));
    assert_eq!(r.search_hover(62), Some(ITEMS.len() - 1));
    key(&mut r, NamedKey::Enter);
    key(&mut r, NamedKey::Enter);
    assert_eq!(r.engine.app_state.picks, vec![5]);
    assert_eq!(r.engine.input_states[&62].text(), "Tail");
    assert_eq!(r.engine.focused_widget_id, Some(62));
    assert!(!r.search_popup_is_open(62));
}

#[test]
fn combo_box_filter_selection_uses_original_source_index() {
    let mut r = runner("sao");
    key(&mut r, NamedKey::ArrowDown);
    assert_eq!(r.engine.runtime_caches.searches[&62].results[0].index, 1);
    key(&mut r, NamedKey::Enter);
    assert_eq!(r.engine.app_state.picks, vec![1]);
    assert_eq!(r.engine.app_state.query, "São Paulo");
}

#[test]
fn combo_box_escape_and_tab_retain_unmatched_draft_without_commit() {
    let mut r = runner("unmatched draft");
    key(&mut r, NamedKey::ArrowDown);
    assert!(r.search_popup_is_open(62));
    key(&mut r, NamedKey::Escape);
    assert!(!r.search_popup_is_open(62));
    assert_eq!(r.engine.focused_widget_id, Some(62));
    key(&mut r, NamedKey::ArrowDown);
    let next_button = *r.engine.runtime_caches.buttons.keys().next().unwrap();
    key(&mut r, NamedKey::Tab);
    assert_eq!(r.engine.focused_widget_id, Some(next_button));
    press(&mut r, NamedKey::Tab, ModifiersState::SHIFT);
    assert_eq!(r.engine.focused_widget_id, Some(62));
    assert!(!r.search_popup_is_open(62));
    assert_eq!(r.engine.input_states[&62].text(), "unmatched draft");
    assert!(r.engine.app_state.picks.is_empty());
}

#[test]
fn combo_box_caret_keys_and_modifiers_do_not_navigate_options() {
    let mut r = runner("Santos");
    key(&mut r, NamedKey::ArrowDown);
    let hover = r.search_hover(62);
    key(&mut r, NamedKey::Home);
    assert_eq!(r.engine.input_states[&62].editor.cursor().index, 0);
    key(&mut r, NamedKey::End);
    assert_eq!(r.engine.input_states[&62].editor.cursor().index, 6);
    key(&mut r, NamedKey::ArrowLeft);
    assert_eq!(r.engine.input_states[&62].editor.cursor().index, 5);
    for modifiers in [
        ModifiersState::SHIFT,
        ModifiersState::CONTROL,
        ModifiersState::ALT,
        ModifiersState::META,
    ] {
        assert!({
            r.engine.modifiers = modifiers.into();
            !r.handle_search_input_key(62, &Key::Named(NamedKey::ArrowDown))
        });
        assert_eq!(r.search_hover(62), hover);
    }
}

#[test]
fn combo_box_controlled_echo_preserves_editor_and_external_reset_replaces_results() {
    let mut r = runner("San");
    let mut fonts = r.engine.font_system.borrow_mut();
    let input = r.engine.input_states.get_mut(&62).unwrap();
    input.editor.set_cursor(Cursor::new(0, 3));
    input.try_insert_text(&mut fonts, "t").unwrap();
    let cursor = input.editor.cursor();
    drop(fonts);
    r.engine.app_state.query = "Sant".into();
    r.engine.layout_dirty = true;
    publish(&mut r);
    assert_eq!(r.engine.input_states[&62].editor.cursor(), cursor);
    key(&mut r, NamedKey::ArrowDown);
    r.engine.app_state.query = "東京".into();
    r.engine.layout_dirty = true;
    publish(&mut r);
    assert_eq!(r.search_hover(62), None);
    assert_eq!(r.engine.input_states[&62].text(), "東京");
    assert_eq!(r.engine.runtime_caches.searches[&62].results[0].index, 4);
    assert!(
        r.focus_search_accessibility_target(crate::widget::resolve_search_suggestion_id(62, 4))
    );
    assert_eq!(r.search_hover(62), Some(0));
}

#[test]
fn combo_box_toggle_pointer_selection_outside_and_disabled() {
    let mut r = runner("");
    r.focus_input_at(62, 220.0, 20.0, 240.0, 40.0, false);
    assert!(r.search_popup_is_open(62));
    r.focus_input_at(62, 220.0, 20.0, 240.0, 40.0, false);
    assert!(!r.search_popup_is_open(62));
    r.focus_input_at(62, 80.0, 10.0, 240.0, 40.0, false);
    assert!(r.search_popup_is_open(62));
    assert!(r.activate_search_original_index(62, 3));
    assert_eq!(r.engine.app_state.picks, vec![3]);
    publish(&mut r);
    key(&mut r, NamedKey::ArrowDown);
    r.focus_widget(None);
    assert!(!r.search_popup_is_open(62));
    r.engine.app_state.disabled = true;
    r.engine.layout_dirty = true;
    publish(&mut r);
    assert!(!r.engine.runtime_caches.inputs.contains_key(&62));
    assert!(!r.engine.runtime_caches.searches.contains_key(&62));
    assert!(!r.expand_search_accessibility_target(62));
    assert!(!r.activate_search_original_index(62, 0));
    assert_eq!(r.engine.input_states[&62].text(), "Lisboa");
}

#[test]
fn combo_box_ime_commit_filters_and_wheel_consumes_at_window_boundaries() {
    use skia_safe::Point;
    use winit::event::Ime;
    let mut r = runner("");
    key(&mut r, NamedKey::ArrowDown);
    r.engine.last_mouse_pos = Point::new(100.0, 70.0);
    r.engine.active_scroll_id = Some(999);
    assert!(matches!(
        r.refresh_scroll_target_at_cursor().unwrap(),
        Some(super::super::WheelPopupTarget::Search(62))
    ));
    assert_eq!(r.engine.active_scroll_id, None);
    for _ in 0..ITEMS.len() {
        r.scroll_open_search(62, 1.0);
    }
    assert_eq!(r.search_hover(62), Some(5));
    assert!(!r.scroll_open_search(62, 1.0));
    assert_eq!(r.engine.active_scroll_id, None);
    r.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::Ime(Ime::Commit("東京".into())),
    );
    publish(&mut r);
    assert_eq!(r.engine.app_state.query, "東京");
    assert_eq!(r.engine.input_states[&62].text(), "東京");
    assert_eq!(r.search_hover(62), None);
    assert!(r.search_popup_is_open(62));
    assert_eq!(r.engine.runtime_caches.searches[&62].results[0].index, 4);
    assert!(r.engine.app_state.picks.is_empty());
}
