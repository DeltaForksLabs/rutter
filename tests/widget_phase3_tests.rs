// ============================================================
// Testes de integração — Fase 3 (novos widgets)
// ============================================================

use std::collections::HashMap;
use taffy::{TraversePartialTree, prelude::Style};

use rutter::engine::runner::snap_to_step;
use rutter::engine::widget_state::{AnimState, ScrollState, SelectState, SliderState, WidgetState};
use rutter::layout::build_taffy_tree;
use rutter::render::hit_test::{
    collect_input_ids, collect_stateful_ids, find_select_callback, find_slider_callback,
};
use rutter::widget::Orientation;
use rutter::{InputState, Widget};

use cosmic_text::FontSystem;
use std::cell::RefCell;
use std::rc::Rc;

fn fs() -> Rc<RefCell<FontSystem>> {
    Rc::new(RefCell::new(FontSystem::new()))
}
fn empty_states() -> HashMap<u64, WidgetState> {
    HashMap::new()
}

// ── Mensagens de teste ───────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum M {
    Bool(bool),
    Float(f32),
    Usize(usize),
    Str(String),
    Unit,
}

// ── Construtores ─────────────────────────────────────────────

fn checkbox(_id_hint: u64, checked: bool) -> Widget<'static, M> {
    Widget::Checkbox {
        checked,
        on_change: M::Bool,
        label: "Test",
        style: Style::default(),
    }
}

fn switch_(checked: bool) -> Widget<'static, M> {
    Widget::Switch {
        checked,
        on_change: M::Bool,
        style: Style::default(),
    }
}

fn radio_(selected: bool) -> Widget<'static, M> {
    Widget::Radio {
        selected,
        on_select: || M::Unit,
        label: "opt",
        style: Style::default(),
    }
}

fn slider_(id: u64, val: f32) -> Widget<'static, M> {
    Widget::Slider {
        id,
        value: val,
        min: 0.0,
        max: 100.0,
        step: 1.0,
        on_change: M::Float,
        style: Style::default(),
        label: "",
    }
}

fn progress_(val: f32) -> Widget<'static, M> {
    Widget::ProgressBar {
        id: 0,
        value: val,
        indeterminate: false,
        style: Style::default(),
    }
}

fn spinner_(id: u64) -> Widget<'static, M> {
    Widget::Spinner {
        id,
        style: Style::default(),
    }
}

fn select_(id: u64) -> Widget<'static, M> {
    Widget::Select {
        id,
        options: &["A", "B", "C"],
        selected_index: 0,
        on_change: M::Usize,
        style: Style::default(),
        label: "",
        placeholder: "",
    }
}

fn divider_() -> Widget<'static, M> {
    Widget::Divider {
        style: Style::default(),
        orientation: Orientation::Horizontal,
    }
}

fn spacer_() -> Widget<'static, M> {
    Widget::Spacer {
        style: Style::default(),
    }
}

fn scroll_(id: u64) -> Widget<'static, M> {
    Widget::ScrollView {
        id,
        style: Style::default(),
        child: Box::new(spacer_()),
    }
}

fn tooltip_() -> Widget<'static, M> {
    Widget::Tooltip {
        text: "hint",
        style: Style::default(),
        child: Box::new(spacer_()),
    }
}

// ── Widget construction ───────────────────────────────────────

#[test]
fn checkbox_stores_checked_state() {
    let w = checkbox(1, true);
    if let Widget::Checkbox { checked, .. } = w {
        assert!(checked);
    }
}

#[test]
fn checkbox_on_change_inverts() {
    let w = checkbox(1, false);
    if let Widget::Checkbox {
        on_change, checked, ..
    } = w
    {
        assert_eq!(on_change(!checked), M::Bool(true));
    }
}

#[test]
fn switch_stores_state() {
    let w = switch_(true);
    if let Widget::Switch { checked, .. } = w {
        assert!(checked);
    }
}

#[test]
fn switch_on_change_inverts() {
    let w = switch_(true);
    if let Widget::Switch {
        on_change, checked, ..
    } = w
    {
        assert_eq!(on_change(!checked), M::Bool(false));
    }
}

#[test]
fn radio_selected_flag() {
    let w = radio_(true);
    if let Widget::Radio { selected, .. } = w {
        assert!(selected);
    }
}

#[test]
fn radio_unselected_flag() {
    let w = radio_(false);
    if let Widget::Radio { selected, .. } = w {
        assert!(!selected);
    }
}

#[test]
fn radio_on_select_produces_message() {
    let w = radio_(false);
    if let Widget::Radio { on_select, .. } = w {
        assert_eq!(on_select(), M::Unit);
    }
}

#[test]
fn slider_stores_value_and_range() {
    let w = slider_(1, 42.0);
    if let Widget::Slider {
        value, min, max, ..
    } = w
    {
        assert!((value - 42.0).abs() < f32::EPSILON);
        assert!((min - 0.0).abs() < f32::EPSILON);
        assert!((max - 100.0).abs() < f32::EPSILON);
    }
}

#[test]
fn slider_on_change_produces_float() {
    let w = slider_(1, 50.0);
    if let Widget::Slider { on_change, .. } = w {
        assert_eq!(on_change(75.5), M::Float(75.5));
    }
}

#[test]
fn progress_bar_value_stored() {
    let w = progress_(0.75);
    if let Widget::ProgressBar {
        value,
        indeterminate,
        ..
    } = w
    {
        assert!((value - 0.75).abs() < f32::EPSILON);
        assert!(!indeterminate);
    }
}

#[test]
fn progress_bar_indeterminate_flag() {
    let w: Widget<M> = Widget::ProgressBar {
        id: 7,
        value: 0.0,
        indeterminate: true,
        style: Style::default(),
    };
    if let Widget::ProgressBar { indeterminate, .. } = w {
        assert!(indeterminate);
    }
}

#[test]
fn spinner_stores_id() {
    let w = spinner_(7);
    if let Widget::Spinner { id, .. } = w {
        assert_eq!(id, 7);
    }
}

#[test]
fn select_stores_options_and_index() {
    let w = select_(5);
    if let Widget::Select {
        id,
        options,
        selected_index,
        ..
    } = w
    {
        assert_eq!(id, 5);
        assert_eq!(options.len(), 3);
        assert_eq!(selected_index, 0);
    }
}

#[test]
fn select_on_change_produces_usize() {
    let w = select_(1);
    if let Widget::Select { on_change, .. } = w {
        assert_eq!(on_change(2), M::Usize(2));
    }
}

#[test]
fn divider_orientation_horizontal() {
    let w = divider_();
    if let Widget::Divider { orientation, .. } = w {
        assert_eq!(orientation, Orientation::Horizontal);
    }
}

#[test]
fn divider_orientation_vertical() {
    let w: Widget<M> = Widget::Divider {
        style: Style::default(),
        orientation: Orientation::Vertical,
    };
    if let Widget::Divider { orientation, .. } = w {
        assert_eq!(orientation, Orientation::Vertical);
    }
}

#[test]
fn scroll_view_wraps_child() {
    let w = scroll_(3);
    if let Widget::ScrollView { id, .. } = w {
        assert_eq!(id, 3);
    }
}

#[test]
fn tooltip_stores_text() {
    let w = tooltip_();
    if let Widget::Tooltip { text, .. } = w {
        assert_eq!(text, "hint");
    }
}

// ── Layout / Taffy ────────────────────────────────────────────

#[test]
fn checkbox_creates_leaf_node() {
    let mut taffy = taffy::TaffyTree::new();
    let node = build_taffy_tree(&mut taffy, &checkbox(1, false), fs(), &empty_states());
    assert_eq!(taffy.child_count(node), 0);
}

#[test]
fn switch_creates_leaf_node() {
    let mut taffy = taffy::TaffyTree::new();
    let node = build_taffy_tree(&mut taffy, &switch_(true), fs(), &empty_states());
    assert_eq!(taffy.child_count(node), 0);
}

#[test]
fn slider_creates_leaf_node() {
    let mut taffy = taffy::TaffyTree::new();
    let node = build_taffy_tree(&mut taffy, &slider_(1, 50.0), fs(), &empty_states());
    assert_eq!(taffy.child_count(node), 0);
}

#[test]
fn progress_bar_creates_leaf_node() {
    let mut taffy = taffy::TaffyTree::new();
    let node = build_taffy_tree(&mut taffy, &progress_(0.5), fs(), &empty_states());
    assert_eq!(taffy.child_count(node), 0);
}

#[test]
fn scroll_view_has_one_child() {
    let mut taffy = taffy::TaffyTree::new();
    let node = build_taffy_tree(&mut taffy, &scroll_(1), fs(), &empty_states());
    assert_eq!(taffy.child_count(node), 1);
}

#[test]
fn tooltip_has_one_child() {
    let mut taffy = taffy::TaffyTree::new();
    let node = build_taffy_tree(&mut taffy, &tooltip_(), fs(), &empty_states());
    assert_eq!(taffy.child_count(node), 1);
}

#[test]
fn select_closed_keeps_base_height() {
    let mut taffy = taffy::TaffyTree::new();
    let base = Style {
        size: taffy::geometry::Size {
            width: taffy::style::Dimension::length(200.0),
            height: taffy::style::Dimension::length(44.0),
        },
        ..Default::default()
    };
    let w: Widget<M> = Widget::Select {
        id: 99,
        options: &["X", "Y", "Z"],
        selected_index: 0,
        on_change: M::Usize,
        style: base,
        label: "",
        placeholder: "",
    };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
    let style = taffy.style(node).unwrap();
    assert_eq!(style.size.height, taffy::style::Dimension::length(44.0));
}

#[test]
fn select_open_keeps_base_height_for_overlay() {
    use rutter::engine::widget_state::SelectState;
    let mut states: HashMap<u64, WidgetState> = HashMap::new();
    states.insert(
        55,
        WidgetState::Select(SelectState {
            is_open: true,
            hovered_option: None,
        }),
    );

    let mut taffy = taffy::TaffyTree::new();
    let base = Style {
        size: taffy::geometry::Size {
            width: taffy::style::Dimension::length(200.0),
            height: taffy::style::Dimension::length(44.0),
        },
        ..Default::default()
    };
    let w: Widget<M> = Widget::Select {
        id: 55,
        options: &["A", "B", "C", "D"],
        selected_index: 0,
        on_change: M::Usize,
        style: base,
        label: "",
        placeholder: "",
    };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &states);
    let style = taffy.style(node).unwrap();
    assert_eq!(style.size.height, taffy::style::Dimension::length(44.0));
}

#[test]
fn column_with_all_new_widgets_correct_count() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::Column {
        style: Style::default(),
        children: vec![
            checkbox(1, false),
            switch_(true),
            radio_(false),
            slider_(1, 0.5),
            progress_(0.5),
            spinner_(9),
            divider_(),
            spacer_(),
        ],
    };
    let root = build_taffy_tree(&mut taffy, &w, fs(), &empty_states());
    assert_eq!(taffy.child_count(root), 8);
}

// ── collect_input_ids — não pega novos widgets ─────────────────

#[test]
fn collect_ignores_checkbox_switch_radio() {
    let w: Widget<M> = Widget::Column {
        style: Style::default(),
        children: vec![checkbox(1, false), switch_(false), radio_(true)],
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert!(ids.is_empty());
}

#[test]
fn collect_ignores_slider_spinner_progress() {
    let w: Widget<M> = Widget::Column {
        style: Style::default(),
        children: vec![slider_(5, 0.5), spinner_(6), progress_(0.3)],
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert!(ids.is_empty());
}

#[test]
fn collect_descends_into_scroll_view() {
    let inner: Widget<M> = Widget::TextInput {
        id: 77,
        on_change: M::Str,
        on_submit: None,
        style: Style::default(),
        label: "",
        placeholder: "",
        state: InputState::Idle,
        error_msg: None,
        is_password: false,
    };
    let w = Widget::ScrollView {
        id: 1,
        style: Style::default(),
        child: Box::new(inner),
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![77]);
}

#[test]
fn collect_descends_into_tooltip() {
    let inner: Widget<M> = Widget::TextInput {
        id: 88,
        on_change: M::Str,
        on_submit: None,
        style: Style::default(),
        label: "",
        placeholder: "",
        state: InputState::Idle,
        error_msg: None,
        is_password: false,
    };
    let w = Widget::Tooltip {
        text: "x",
        style: Style::default(),
        child: Box::new(inner),
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![88]);
}

// ── collect_stateful_ids ──────────────────────────────────────

#[test]
fn stateful_finds_slider() {
    let w = slider_(42, 0.0);
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 42 && *k == "slider"));
}

#[test]
fn stateful_finds_spinner() {
    let w = spinner_(3);
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 3 && *k == "anim"));
}

#[test]
fn stateful_finds_select() {
    let w = select_(7);
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 7 && *k == "select"));
}

#[test]
fn stateful_finds_scroll_view() {
    let w = scroll_(9);
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 9 && *k == "scroll"));
}

// ── Callbacks ─────────────────────────────────────────────────

#[test]
fn find_slider_callback_found_in_column() {
    let w: Widget<M> = Widget::Column {
        style: Style::default(),
        children: vec![slider_(1, 0.5), slider_(2, 0.2)],
    };
    assert!(find_slider_callback(&w, 1).is_some());
    assert!(find_slider_callback(&w, 2).is_some());
    assert!(find_slider_callback(&w, 99).is_none());
}

#[test]
fn find_select_callback_found() {
    let w = select_(5);
    let cb = find_select_callback(&w, 5);
    assert!(cb.is_some());
    assert_eq!(cb.unwrap()(1), M::Usize(1));
}

#[test]
fn find_select_callback_nested() {
    let w: Widget<M> = Widget::Column {
        style: Style::default(),
        children: vec![select_(10), select_(20)],
    };
    assert!(find_select_callback(&w, 10).is_some());
    assert!(find_select_callback(&w, 20).is_some());
    assert!(find_select_callback(&w, 99).is_none());
}

// ── snap_to_step ──────────────────────────────────────────────

#[test]
fn snap_zero_step_clamps() {
    assert!((snap_to_step(-1.0, 0.0, 10.0, 0.0) - 0.0).abs() < f32::EPSILON);
    assert!((snap_to_step(15.0, 0.0, 10.0, 0.0) - 10.0).abs() < f32::EPSILON);
}

#[test]
fn snap_step_1_rounds_correctly() {
    assert!((snap_to_step(2.3, 0.0, 10.0, 1.0) - 2.0).abs() < 0.001);
    assert!((snap_to_step(2.7, 0.0, 10.0, 1.0) - 3.0).abs() < 0.001);
}

#[test]
fn snap_step_5_rounds_correctly() {
    assert!((snap_to_step(12.0, 0.0, 100.0, 5.0) - 10.0).abs() < 0.001);
    assert!((snap_to_step(13.0, 0.0, 100.0, 5.0) - 15.0).abs() < 0.001);
}

#[test]
fn snap_step_0_5() {
    assert!((snap_to_step(2.3, 0.0, 10.0, 0.5) - 2.5).abs() < 0.001);
    assert!((snap_to_step(2.1, 0.0, 10.0, 0.5) - 2.0).abs() < 0.001);
}

#[test]
fn snap_negative_range() {
    assert!((snap_to_step(-3.0, -10.0, 10.0, 5.0) - (-5.0)).abs() < 0.001);
    assert!((snap_to_step(-7.0, -10.0, 10.0, 5.0) - (-5.0)).abs() < 0.001);
}

// ── WidgetState acessores ─────────────────────────────────────

#[test]
fn widget_state_slider_mut_accessible() {
    let mut ws = WidgetState::Slider(SliderState::default());
    ws.as_slider_mut().unwrap().dragging = true;
    assert!(ws.as_slider().unwrap().dragging);
}

#[test]
fn widget_state_scroll_mut_accessible() {
    let mut ws = WidgetState::Scroll(ScrollState::default());
    ws.as_scroll_mut().unwrap().scroll_by(50.0);
    assert!(
        (ws.as_scroll().unwrap().offset_y - 0.0).abs() < f32::EPSILON,
        "clampado em 0 pois content_height=0"
    );
}

#[test]
fn widget_state_scroll_scrolls_with_content() {
    let mut ws = WidgetState::Scroll(ScrollState {
        offset_y: 0.0,
        content_height: 500.0,
        viewport_h: 200.0,
    });
    ws.as_scroll_mut().unwrap().scroll_by(80.0);
    assert!((ws.as_scroll().unwrap().offset_y - 80.0).abs() < f32::EPSILON);
}

#[test]
fn widget_state_select_toggle() {
    let mut ws = WidgetState::Select(SelectState::default());
    assert!(!ws.as_select().unwrap().is_open);
    ws.as_select_mut().unwrap().is_open = true;
    assert!(ws.as_select().unwrap().is_open);
}

#[test]
fn widget_state_anim_ticks() {
    use std::{thread, time::Duration};
    let mut ws = WidgetState::Anim(AnimState::default());
    thread::sleep(Duration::from_millis(30));
    ws.as_anim_mut().unwrap().tick();
    assert!(ws.as_anim().unwrap().angle > 0.0);
}

// ── ScrollState avançado ──────────────────────────────────────

#[test]
fn scroll_thumb_ratio_quarter() {
    let s = ScrollState {
        offset_y: 0.0,
        content_height: 800.0,
        viewport_h: 200.0,
    };
    assert!((s.thumb_ratio() - 0.25).abs() < 0.001);
}

#[test]
fn scroll_multiple_scrolls_clamp() {
    let mut s = ScrollState {
        offset_y: 0.0,
        content_height: 300.0,
        viewport_h: 200.0,
    };
    for _ in 0..10 {
        s.scroll_by(40.0);
    }
    assert!(
        (s.offset_y - 100.0).abs() < f32::EPSILON,
        "max_offset = 100"
    );
}

#[test]
fn scroll_reverse_scrolls() {
    let mut s = ScrollState {
        offset_y: 50.0,
        content_height: 300.0,
        viewport_h: 200.0,
    };
    s.scroll_by(-30.0);
    assert!((s.offset_y - 20.0).abs() < f32::EPSILON);
}

// ── SliderState avançado ──────────────────────────────────────

#[test]
fn slider_drag_state_transitions() {
    let mut s = SliderState::default();
    assert!(!s.dragging);
    s.dragging = true;
    assert!(s.dragging);
    s.dragging = false;
    assert!(!s.dragging);
}

#[test]
fn slider_quarter_position() {
    let s = SliderState {
        track_abs_x: 0.0,
        track_width: 200.0,
        ..Default::default()
    };
    let v = s.value_from_cursor(50.0);
    assert!((v - 0.25).abs() < 0.001);
}

#[test]
fn slider_three_quarter_position() {
    let s = SliderState {
        track_abs_x: 0.0,
        track_width: 200.0,
        ..Default::default()
    };
    let v = s.value_from_cursor(150.0);
    assert!((v - 0.75).abs() < 0.001);
}
