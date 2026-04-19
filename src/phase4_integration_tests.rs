// ============================================================
// Testes de integração — Fase 4
// Modal, Toast, TabBar, VirtualList + correções de warnings
// ============================================================

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use taffy::prelude::Style;

use rutter::{ButtonVariant, InputState, Orientation, ToastKind, Widget};
use rutter::engine::widget_state::{
    AnimState, ModalState, ScrollState, SelectState,
    SliderState, TabState, ToastState, VirtualGridState, VirtualListState, WidgetState,
};
use rutter::engine::runner::snap_to_step;
use rutter::layout::{build_taffy_tree, OPTION_HEIGHT};
use rutter::render::hit_test::{
    collect_input_ids, collect_stateful_ids,
    find_input_callbacks, find_select_callback, find_slider_callback,
};

use std::cell::RefCell;
use std::rc::Rc;
use cosmic_text::FontSystem;

fn fs() -> Rc<RefCell<FontSystem>> { Rc::new(RefCell::new(FontSystem::new())) }
fn empty() -> HashMap<u64, WidgetState> { HashMap::new() }

#[derive(Debug, Clone, PartialEq)]
enum M { A, Str(String), Bool(bool), Usize(usize), Float(f32) }

// ── ToastState ───────────────────────────────────────────────

#[test]
fn toast_new_visible() {
    assert!(ToastState::new(1000).visible);
}

#[test]
fn toast_not_expired_at_creation() {
    assert!(!ToastState::new(5000).is_expired());
}

#[test]
fn toast_expires() {
    let t = ToastState::new(30);
    thread::sleep(Duration::from_millis(50));
    assert!(t.is_expired());
}

#[test]
fn toast_permanent_no_expiry() {
    let t = ToastState::new(0);
    thread::sleep(Duration::from_millis(50));
    assert!(!t.is_expired());
}

#[test]
fn toast_dismiss_before_expiry() {
    let mut t = ToastState::new(9000);
    t.dismiss();
    assert!(t.is_expired());
    assert!(!t.visible);
}

#[test]
fn toast_progress_near_one_at_start() {
    assert!(ToastState::new(2000).progress() > 0.9);
}

#[test]
fn toast_progress_decreases_over_time() {
    let t = ToastState::new(60);
    thread::sleep(Duration::from_millis(30));
    let p = t.progress();
    assert!(p < 1.0 && p > 0.0);
}

#[test]
fn toast_permanent_progress_always_one() {
    let t = ToastState::new(0);
    thread::sleep(Duration::from_millis(50));
    assert!((t.progress() - 1.0).abs() < f32::EPSILON);
}

// ── ModalState ───────────────────────────────────────────────

#[test]
fn modal_default_hidden() { assert!(!ModalState::default().visible); }

#[test]
fn modal_open_makes_visible() {
    let mut m = ModalState::default(); m.open();
    assert!(m.visible); assert!(m.backdrop_alpha > 0);
}

#[test]
fn modal_close_after_open() {
    let mut m = ModalState::default(); m.open(); m.close();
    assert!(!m.visible); assert_eq!(m.backdrop_alpha, 0);
}

#[test]
fn modal_reopen_works() {
    let mut m = ModalState::default();
    m.open(); m.close(); m.open();
    assert!(m.visible);
}

// ── TabState ─────────────────────────────────────────────────

#[test]
fn tab_default_zero() { assert_eq!(TabState::default().active, 0); }

#[test]
fn tab_set_active_updates() {
    let mut t = TabState::default(); t.set_active(2, 100.0);
    assert_eq!(t.active, 2);
}

#[test]
fn tab_underline_x_computed() {
    let mut t = TabState::default(); t.set_active(3, 80.0);
    assert!((t.underline_x - 240.0).abs() < f32::EPSILON);
}

#[test]
fn tab_first_tab_zero_offset() {
    let mut t = TabState::default(); t.set_active(0, 80.0);
    assert!((t.underline_x - 0.0).abs() < f32::EPSILON);
}

// ── VirtualListState ─────────────────────────────────────────

#[test]
fn vlist_visible_range_from_start() {
    let s = VirtualListState { scroll_y: 0.0, viewport_h: 300.0, ..Default::default() };
    let (f, l) = s.visible_range(30.0, 1000);
    assert_eq!(f, 0); assert!(l > 0 && l <= 12);
}

#[test]
fn vlist_visible_range_scrolled_down() {
    let s = VirtualListState { scroll_y: 300.0, viewport_h: 300.0, ..Default::default() };
    let (f, _l) = s.visible_range(30.0, 1000);
    assert_eq!(f, 10);
}

#[test]
fn vlist_visible_range_clips_at_count() {
    let s = VirtualListState { scroll_y: 0.0, viewport_h: 600.0, ..Default::default() };
    let (_, l) = s.visible_range(30.0, 5);
    assert_eq!(l, 5);
}

#[test]
fn vlist_scroll_by_positive() {
    let mut s = VirtualListState { scroll_y: 0.0, viewport_h: 200.0, ..Default::default() };
    s.scroll_by(120.0, 30.0, 100);
    assert!((s.scroll_y - 120.0).abs() < f32::EPSILON);
}

#[test]
fn vlist_scroll_by_negative_clamp() {
    let mut s = VirtualListState { scroll_y: 10.0, viewport_h: 200.0, ..Default::default() };
    s.scroll_by(-50.0, 30.0, 100);
    assert!((s.scroll_y - 0.0).abs() < f32::EPSILON);
}

#[test]
fn vlist_scroll_to_index() {
    let mut s = VirtualListState { scroll_y: 0.0, viewport_h: 200.0, ..Default::default() };
    s.scroll_to_index(10, 30.0, 100);
    assert!((s.scroll_y - 300.0).abs() < f32::EPSILON);
}

#[test]
fn vlist_max_scroll() {
    let s = VirtualListState { viewport_h: 200.0, ..Default::default() };
    assert!((s.max_scroll(30.0, 100) - 2800.0).abs() < f32::EPSILON);
}

#[test]
fn vlist_thumb_ratio_quarter() {
    let s = VirtualListState { viewport_h: 200.0, ..Default::default() };
    // 100*30 = 3000, 200/3000 ≈ 0.067
    let r = s.thumb_ratio(30.0, 100);
    assert!((r - 200.0/3000.0).abs() < 0.001);
}

#[test]
fn vlist_selected_row_none_default() {
    assert!(VirtualListState::default().selected_row.is_none());
}

#[test]
fn vlist_hovered_row_none_default() {
    assert!(VirtualListState::default().hovered_row.is_none());
}

#[test]
fn vgrid_visible_rows_scrolled() {
    let s = VirtualGridState { scroll_y: 120.0, viewport_h: 180.0, ..Default::default() };
    let (first, last) = s.visible_row_range(60.0, 48, 4);
    assert_eq!(first, 2);
    assert!(last > first);
}

#[test]
fn vgrid_scroll_to_index_tracks_row() {
    let mut s = VirtualGridState { viewport_h: 180.0, ..Default::default() };
    s.scroll_to_index(10, 60.0, 48, 4);
    assert!((s.scroll_y - 120.0).abs() < f32::EPSILON);
}

// ── WidgetState accessors ─────────────────────────────────────

#[test]
fn toast_state_accessor() {
    let ws = WidgetState::Toast(ToastState::new(1000));
    assert!(ws.as_toast().is_some()); assert!(ws.as_modal().is_none());
}

#[test]
fn modal_state_accessor() {
    let ws = WidgetState::Modal(ModalState::default());
    assert!(ws.as_modal().is_some()); assert!(ws.as_tab().is_none());
}

#[test]
fn tab_state_accessor() {
    let ws = WidgetState::Tab(TabState::default());
    assert!(ws.as_tab().is_some()); assert!(ws.as_vlist().is_none());
}

#[test]
fn vlist_state_accessor() {
    let ws = WidgetState::VList(VirtualListState::default());
    assert!(ws.as_vlist().is_some()); assert!(ws.as_slider().is_none());
}

#[test]
fn toast_state_mut() {
    let mut ws = WidgetState::Toast(ToastState::new(5000));
    ws.as_toast_mut().unwrap().dismiss();
    assert!(!ws.as_toast().unwrap().visible);
}

#[test]
fn modal_state_mut() {
    let mut ws = WidgetState::Modal(ModalState::default());
    ws.as_modal_mut().unwrap().open();
    assert!(ws.as_modal().unwrap().visible);
}

#[test]
fn tab_state_mut() {
    let mut ws = WidgetState::Tab(TabState::default());
    ws.as_tab_mut().unwrap().set_active(3, 80.0);
    assert_eq!(ws.as_tab().unwrap().active, 3);
}

#[test]
fn vlist_state_mut() {
    let mut ws = WidgetState::VList(VirtualListState::default());
    ws.as_vlist_mut().unwrap().selected_row = Some(7);
    assert_eq!(ws.as_vlist().unwrap().selected_row, Some(7));
}

#[test]
fn vgrid_state_accessor() {
    let ws = WidgetState::VGrid(VirtualGridState::default());
    assert!(ws.as_vgrid().is_some()); assert!(ws.as_slider().is_none());
}

#[test]
fn vgrid_state_mut() {
    let mut ws = WidgetState::VGrid(VirtualGridState::default());
    ws.as_vgrid_mut().unwrap().selected_item = Some(8);
    assert_eq!(ws.as_vgrid().unwrap().selected_item, Some(8));
}

// ── Widget construction ───────────────────────────────────────

#[test]
fn tabbar_stores_tabs_and_active() {
    let w: Widget<M> = Widget::TabBar { id: 1, tabs: &["A","B","C"], active: 1,
        on_change: M::Usize, style: Style::default() };
    if let Widget::TabBar { tabs, active, id, .. } = w {
        assert_eq!(tabs.len(), 3); assert_eq!(active, 1); assert_eq!(id, 1);
    }
}

#[test]
fn tabbar_on_change_produces_message() {
    let w: Widget<M> = Widget::TabBar { id: 1, tabs: &["A"], active: 0,
        on_change: M::Usize, style: Style::default() };
    if let Widget::TabBar { on_change, .. } = w {
        assert_eq!(on_change(2), M::Usize(2));
    }
}

#[test]
fn modal_stores_visible_flag() {
    let w: Widget<M> = Widget::Modal { id: 1, visible: true, on_dismiss: Some(M::A),
        style: Style::default(), child: Box::new(Widget::Spacer { style: Style::default() }) };
    if let Widget::Modal { visible, .. } = w { assert!(visible); }
}

#[test]
fn modal_stores_on_dismiss() {
    let w: Widget<M> = Widget::Modal { id: 1, visible: false, on_dismiss: Some(M::A),
        style: Style::default(), child: Box::new(Widget::Spacer { style: Style::default() }) };
    if let Widget::Modal { on_dismiss, .. } = w { assert_eq!(on_dismiss, Some(M::A)); }
}

#[test]
fn toast_stores_kind() {
    let w: Widget<M> = Widget::Toast { id: 1, visible: true, message: "hi", kind: ToastKind::Error,
        duration_ms: 3000, on_dismiss: None };
    if let Widget::Toast { kind, .. } = w { assert_eq!(kind, ToastKind::Error); }
}

#[test]
fn toast_kinds_distinct() {
    assert_ne!(ToastKind::Info, ToastKind::Error);
    assert_ne!(ToastKind::Success, ToastKind::Warning);
}

#[test]
fn vlist_stores_item_count_and_height() {
    let w: Widget<M> = Widget::VirtualList { id: 1, item_height: 32.0, item_count: 500,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    if let Widget::VirtualList { item_count, item_height, .. } = w {
        assert_eq!(item_count, 500);
        assert!((item_height - 32.0).abs() < f32::EPSILON);
    }
}

#[test]
fn vlist_items_fn_called() {
    let w: Widget<M> = Widget::VirtualList { id: 1, item_height: 30.0, item_count: 3,
        items: &|i| Some(format!("item-{i}")), on_select: M::Usize, style: Style::default() };
    if let Widget::VirtualList { items, .. } = w {
        assert_eq!(items(0), Some("item-0".to_string()));
        assert_eq!(items(2), Some("item-2".to_string()));
    }
}

#[test]
fn vgrid_stores_columns_and_items() {
    let w: Widget<M> = Widget::VirtualGrid { id: 1, columns: 4, item_height: 64.0, item_count: 12,
        items: &|i| Some(format!("cell-{i}")), on_select: M::Usize, style: Style::default() };
    if let Widget::VirtualGrid { columns, item_height, items, .. } = w {
        assert_eq!(columns, 4);
        assert!((item_height - 64.0).abs() < f32::EPSILON);
        assert_eq!(items(5), Some("cell-5".to_string()));
    }
}

// ── Layout (Taffy) ────────────────────────────────────────────

#[test]
fn tabbar_taffy_leaf() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::TabBar { id: 1, tabs: &["A","B"], active: 0,
        on_change: M::Usize, style: Style::default() };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
    assert_eq!(taffy.child_count(node), 0);
}

#[test]
fn modal_invisible_zero_size_taffy() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::Modal { id: 1, visible: false, on_dismiss: None,
        style: Style::default(), child: Box::new(Widget::Spacer { style: Style::default() }) };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
    let s = taffy.style(node).unwrap();
    assert_eq!(s.size.width, taffy::style::Dimension::length(0.0));
}

#[test]
fn modal_visible_has_child_in_taffy() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::Modal { id: 1, visible: true, on_dismiss: None,
        style: Style::default(), child: Box::new(Widget::Spacer { style: Style::default() }) };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
    assert_eq!(taffy.child_count(node), 1);
}

#[test]
fn toast_zero_layout_size() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::Toast { id: 1, visible: true, message: "x", kind: ToastKind::Info,
        duration_ms: 1000, on_dismiss: None };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
    let s = taffy.style(node).unwrap();
    assert_eq!(s.size.width, taffy::style::Dimension::length(0.0));
}

#[test]
fn vlist_taffy_leaf() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::VirtualList { id: 1, item_height: 30.0, item_count: 100,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
    assert_eq!(taffy.child_count(node), 0);
}

#[test]
fn vgrid_taffy_leaf() {
    let mut taffy = taffy::TaffyTree::new();
    let w: Widget<M> = Widget::VirtualGrid { id: 1, columns: 3, item_height: 64.0, item_count: 100,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    let node = build_taffy_tree(&mut taffy, &w, fs(), &empty());
    assert_eq!(taffy.child_count(node), 0);
}

// ── collect_stateful_ids ─────────────────────────────────────

#[test]
fn stateful_tabbar() {
    let w: Widget<M> = Widget::TabBar { id: 5, tabs: &["A"], active: 0,
        on_change: M::Usize, style: Style::default() };
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 5 && *k == "tab"));
}

#[test]
fn stateful_modal() {
    let w: Widget<M> = Widget::Modal { id: 7, visible: false, on_dismiss: None,
        style: Style::default(), child: Box::new(Widget::Spacer { style: Style::default() }) };
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 7 && *k == "modal"));
}

#[test]
fn stateful_toast() {
    let w: Widget<M> = Widget::Toast { id: 9, visible: true, message: "x", kind: ToastKind::Success,
        duration_ms: 3000, on_dismiss: None };
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 9 && *k == "toast"));
}

#[test]
fn stateful_vlist() {
    let w: Widget<M> = Widget::VirtualList { id: 11, item_height: 30.0, item_count: 50,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 11 && *k == "vlist"));
}

#[test]
fn stateful_vgrid() {
    let w: Widget<M> = Widget::VirtualGrid { id: 12, columns: 4, item_height: 64.0, item_count: 50,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    assert!(out.iter().any(|(id, k)| *id == 12 && *k == "vgrid"));
}

#[test]
fn no_duplicate_scroll_id_in_collect() {
    // FIX WARNING: padrão duplicado do v5 causaria double-push
    let w: Widget<M> = Widget::ScrollView { id: 5, style: Style::default(),
        child: Box::new(Widget::Spacer { style: Style::default() }) };
    let mut out = vec![];
    collect_stateful_ids(&w, &mut out);
    let count = out.iter().filter(|(id, k)| *id == 5 && *k == "scroll").count();
    assert_eq!(count, 1, "scroll ID deve aparecer apenas uma vez");
}

// ── inputs_inside_modal ───────────────────────────────────────

#[test]
fn inputs_collected_inside_modal() {
    let inner: Widget<M> = Widget::TextInput { id: 55, on_change: M::Str, on_submit: None,
        style: Style::default(), label: "", placeholder: "",
        state: InputState::Idle, error_msg: None, is_password: false };
    let w = Widget::Modal { id: 1, visible: true, on_dismiss: None,
        style: Style::default(), child: Box::new(inner) };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![55]);
}

#[test]
fn vlist_not_in_input_ids() {
    let w: Widget<M> = Widget::VirtualList { id: 1, item_height: 30.0, item_count: 10,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert!(ids.is_empty());
}

#[test]
fn vgrid_not_in_input_ids() {
    let w: Widget<M> = Widget::VirtualGrid { id: 1, columns: 4, item_height: 64.0, item_count: 10,
        items: &|_| None, on_select: M::Usize, style: Style::default() };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert!(ids.is_empty());
}

// ── snap_to_step ─────────────────────────────────────────────

#[test]
fn snap_various() {
    assert!((snap_to_step(2.3, 0.0, 10.0, 1.0) - 2.0).abs() < 0.001);
    assert!((snap_to_step(2.7, 0.0, 10.0, 1.0) - 3.0).abs() < 0.001);
    assert!((snap_to_step(2.3, 0.0, 10.0, 0.5) - 2.5).abs() < 0.001);
    assert!((snap_to_step(-5.0, 0.0, 10.0, 1.0) - 0.0).abs() < 0.001);
    assert!((snap_to_step(15.0, 0.0, 10.0, 1.0) - 10.0).abs() < 0.001);
}
