// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

// ============================================================
// Rutter Framework — engine/mod.rs
// ============================================================

pub mod cursor;
pub mod gpu;
pub mod runner;
pub mod widget_state;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use arboard::Clipboard;
use cosmic_text::{FontSystem, SwashCache};
use skia_safe::{Color as SkiaColor, Font, Point};
use taffy::prelude::{NodeId, Style, TaffyTree};
use winit::{
    dpi::PhysicalSize,
    event::{Modifiers, WindowEvent},
    window::Window,
};

use self::cursor::CursorBlink;
use self::gpu::{BackendType, GraphicsBackend, create_best_backend};
use self::widget_state::{
    AnimState, ContextMenuState, ModalState, PopoverState, ScrollState, SelectState, SliderState,
    TabState, ToastState, VirtualGridState, VirtualListState, WidgetState,
};
use crate::accessibility::{
    AccessibilityInputs, IgnoredActionHandler, IgnoredDeactivationHandler, LazyActivationHandler,
    build_accessibility_update,
};
use crate::app::AppLogic;
use crate::layout::{
    RutterContext, SyncedLayoutTree, compute_layout, sync_taffy_tree_with_direction,
};
use crate::render::draw_widgets;
use crate::render::hit_test::{collect_input_ids, collect_stateful_ids};
use crate::render::text::TextBufferCache;
use crate::widget::{DialogAction, Widget};

#[derive(Debug, Clone, Copy)]
enum ToastRuntimeUpdate {
    EnsureVisible { id: u64, duration_ms: u32 },
    Dismiss { id: u64 },
}

fn collect_toast_runtime_updates<Msg>(widget: &Widget<Msg>, out: &mut Vec<ToastRuntimeUpdate>) {
    let mut path = Vec::new();
    collect_toast_runtime_updates_impl(widget, out, &mut path);
}

fn collect_toast_runtime_updates_impl<Msg>(
    widget: &Widget<Msg>,
    out: &mut Vec<ToastRuntimeUpdate>,
    path: &mut Vec<usize>,
) {
    match widget {
        Widget::Toast {
            visible,
            duration_ms,
            ..
        } => {
            let resolved_id = widget.resolved_id(path).unwrap();
            if *visible {
                out.push(ToastRuntimeUpdate::EnsureVisible {
                    id: resolved_id,
                    duration_ms: *duration_ms,
                });
            } else {
                out.push(ToastRuntimeUpdate::Dismiss { id: resolved_id });
            }
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_toast_runtime_updates_impl(child, out, path);
                path.pop();
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            collect_toast_runtime_updates_impl(child, out, path);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            collect_toast_runtime_updates_impl(anchor, out, path);
            path.pop();
            if *open {
                path.push(1);
                collect_toast_runtime_updates_impl(content, out, path);
                path.pop();
            }
        }
        _ => {}
    }
}

fn collect_focus_order<Msg>(widget: &Widget<Msg>, out: &mut Vec<u64>) {
    let mut path = Vec::new();
    if collect_overlay_focus_scope(widget, out, &mut path) {
        return;
    }
    collect_focus_order_impl(widget, out, &mut path);
}

fn collect_overlay_focus_scope<Msg>(
    widget: &Widget<Msg>,
    out: &mut Vec<u64>,
    path: &mut Vec<usize>,
) -> bool {
    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate().rev() {
                path.push(index);
                let found = collect_overlay_focus_scope(child, out, path);
                path.pop();
                if found {
                    return true;
                }
            }
            false
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. } => {
            path.push(0);
            let found = collect_overlay_focus_scope(child, out, path);
            path.pop();
            found
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            if *open {
                path.push(1);
                let found = collect_overlay_focus_scope(content, out, path);
                path.pop();
                if found {
                    return true;
                }
            }
            path.push(0);
            let found = collect_overlay_focus_scope(anchor, out, path);
            path.pop();
            found
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            if !expanded {
                return false;
            }
            path.push(0);
            let found = collect_overlay_focus_scope(child, out, path);
            path.pop();
            found
        }
        Widget::Modal { visible, child, .. } => {
            if !visible {
                return false;
            }
            path.push(0);
            let found = collect_overlay_focus_scope(child, out, path);
            if !found {
                collect_focus_order_impl(child, out, path);
            }
            path.pop();
            true
        }
        Widget::Dialog { visible, .. } => {
            if !visible {
                return false;
            }
            if let Some(cancel_id) = widget.dialog_action_focus_id(path, DialogAction::Cancel) {
                out.push(cancel_id);
            }
            if let Some(confirm_id) = widget.dialog_action_focus_id(path, DialogAction::Confirm) {
                out.push(confirm_id);
            }
            true
        }
        _ => false,
    }
}

fn collect_focus_order_impl<Msg>(widget: &Widget<Msg>, out: &mut Vec<u64>, path: &mut Vec<usize>) {
    match widget {
        Widget::Button { .. }
        | Widget::ButtonContent { .. }
        | Widget::Checkbox { .. }
        | Widget::Switch { .. }
        | Widget::Radio { .. }
        | Widget::TextInput { .. }
        | Widget::TextArea { .. }
        | Widget::SearchBar { .. }
        | Widget::Slider { .. }
        | Widget::Select { .. }
        | Widget::VirtualList { .. }
        | Widget::VirtualGrid { .. } => out.push(widget.keyboard_focus_id(path).unwrap()),
        Widget::TabBar { tabs, .. } => {
            for index in 0..tabs.len() {
                if let Some(focus_id) = widget.tab_focus_id(path, index) {
                    out.push(focus_id);
                }
            }
        }
        Widget::Accordion {
            expanded, child, ..
        } => {
            out.push(widget.keyboard_focus_id(path).unwrap());
            if *expanded {
                path.push(0);
                collect_focus_order_impl(child, out, path);
                path.pop();
            }
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for (index, child) in children.iter().enumerate() {
                path.push(index);
                collect_focus_order_impl(child, out, path);
                path.pop();
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ContextMenu { child, .. }
        | Widget::ScrollView { child, .. } => {
            path.push(0);
            collect_focus_order_impl(child, out, path);
            path.pop();
        }
        Widget::Popover {
            anchor,
            content,
            open,
            ..
        } => {
            path.push(0);
            collect_focus_order_impl(anchor, out, path);
            path.pop();
            if *open {
                path.push(1);
                collect_focus_order_impl(content, out, path);
                path.pop();
            }
        }
        Widget::Modal { visible, child, .. } => {
            if *visible {
                path.push(0);
                collect_focus_order_impl(child, out, path);
                path.pop();
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
struct InputRuntime<Msg: Clone> {
    on_change: fn(String) -> Msg,
    on_submit: Option<Msg>,
    is_password: bool,
    is_multiline: bool,
    visible_w: f32,
    visible_h: f32,
}

#[derive(Debug, Clone)]
struct SliderRuntime<Msg> {
    on_change: fn(f32) -> Msg,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
}

#[derive(Debug, Clone)]
struct ToggleRuntime<Msg> {
    on_change: fn(bool) -> Msg,
    checked: bool,
}

#[derive(Debug, Clone)]
struct SelectRuntime<Msg> {
    on_change: fn(usize) -> Msg,
    selected_index: usize,
    option_count: usize,
}

#[derive(Debug, Clone)]
struct TabRuntime<Msg> {
    on_change: fn(usize) -> Msg,
    tab_count: usize,
    focus_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
struct TabFocusRuntime {
    parent_id: u64,
    index: usize,
}

#[derive(Debug, Clone)]
struct VListRuntime<Msg> {
    on_select: fn(usize) -> Msg,
    item_height: f32,
    item_count: usize,
}

#[derive(Debug, Clone)]
struct VGridRuntime<Msg> {
    on_select: fn(usize) -> Msg,
    columns: usize,
    item_height: f32,
    item_count: usize,
}

#[derive(Debug)]
struct WidgetRuntimeCaches<Msg: Clone> {
    input_order: Vec<u64>,
    focus_order: Vec<u64>,
    inputs: HashMap<u64, InputRuntime<Msg>>,
    buttons: HashMap<u64, Msg>,
    checkboxes: HashMap<u64, ToggleRuntime<Msg>>,
    switches: HashMap<u64, ToggleRuntime<Msg>>,
    radios: HashMap<u64, fn() -> Msg>,
    accordions: HashMap<u64, Msg>,
    sliders: HashMap<u64, SliderRuntime<Msg>>,
    selects: HashMap<u64, SelectRuntime<Msg>>,
    tabs: HashMap<u64, TabRuntime<Msg>>,
    tab_items: HashMap<u64, TabFocusRuntime>,
    vlists: HashMap<u64, VListRuntime<Msg>>,
    vgrids: HashMap<u64, VGridRuntime<Msg>>,
    toast_dismiss: HashMap<u64, Msg>,
}

impl<Msg: Clone> Default for WidgetRuntimeCaches<Msg> {
    fn default() -> Self {
        Self {
            input_order: Vec::new(),
            focus_order: Vec::new(),
            inputs: HashMap::new(),
            buttons: HashMap::new(),
            checkboxes: HashMap::new(),
            switches: HashMap::new(),
            radios: HashMap::new(),
            accordions: HashMap::new(),
            sliders: HashMap::new(),
            selects: HashMap::new(),
            tabs: HashMap::new(),
            tab_items: HashMap::new(),
            vlists: HashMap::new(),
            vgrids: HashMap::new(),
            toast_dismiss: HashMap::new(),
        }
    }
}

impl<Msg: Clone> WidgetRuntimeCaches<Msg> {
    fn clear(&mut self) {
        self.input_order.clear();
        self.focus_order.clear();
        self.inputs.clear();
        self.buttons.clear();
        self.checkboxes.clear();
        self.switches.clear();
        self.radios.clear();
        self.accordions.clear();
        self.sliders.clear();
        self.selects.clear();
        self.tabs.clear();
        self.tab_items.clear();
        self.vlists.clear();
        self.vgrids.clear();
        self.toast_dismiss.clear();
    }
}

pub struct RutterEngine<A: AppLogic> {
    pub window: Option<Rc<Window>>,
    accessibility_adapter: Option<accesskit_winit::Adapter>,
    graphics_backend: Option<Box<dyn GraphicsBackend>>,
    pub font_system: Rc<RefCell<FontSystem>>,
    pub swash_cache: SwashCache,
    pub font_cache: HashMap<(String, u32), Font>,
    pub text_cache: TextBufferCache,
    pub taffy: TaffyTree<RutterContext>,
    layout_tree: SyncedLayoutTree,
    runtime_caches: WidgetRuntimeCaches<A::Message>,
    pub last_root_node: NodeId,
    pub layout_dirty: bool,
    pub app_state: A::State,
    pub input_states: HashMap<u64, crate::input_state::InputWidgetState>,
    pub widget_states: HashMap<u64, WidgetState>,
    pub focused_widget_id: Option<u64>,
    pub active_scroll_id: Option<u64>,
    pub drag_slider_id: Option<u64>,
    pub clipboard: Clipboard,
    pub modifiers: Modifiers,
    pub cursor_blink: CursorBlink,
    pub last_snapshot: std::time::Instant,
    pub snapshot_scheduled: bool,
    pub scale_factor: f32,
    pub last_mouse_pos: Point,
    pub has_animated: bool,
}

impl<A: AppLogic> RutterEngine<A> {
    pub fn new() -> Self {
        let mut fs = FontSystem::new();
        let state = A::new(&mut fs);
        let mut taffy = TaffyTree::new();
        let root = taffy.new_leaf(Style::default()).unwrap();
        Self {
            window: None,
            accessibility_adapter: None,
            graphics_backend: None,
            font_system: Rc::new(RefCell::new(fs)),
            swash_cache: SwashCache::new(),
            font_cache: HashMap::new(),
            text_cache: TextBufferCache::default(),
            layout_tree: SyncedLayoutTree::placeholder(root),
            runtime_caches: WidgetRuntimeCaches::default(),
            taffy,
            last_root_node: root,
            layout_dirty: true,
            app_state: state,
            input_states: HashMap::new(),
            widget_states: HashMap::new(),
            focused_widget_id: None,
            active_scroll_id: None,
            drag_slider_id: None,
            clipboard: Clipboard::new().expect("clipboard init falhou"),
            modifiers: Modifiers::default(),
            cursor_blink: CursorBlink::new(),
            last_snapshot: std::time::Instant::now(),
            snapshot_scheduled: false,
            scale_factor: 1.0,
            last_mouse_pos: Point::new(0.0, 0.0),
            has_animated: false,
        }
    }

    pub fn handle_resumed(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Rutter")
            .with_visible(false);
        let backend = create_best_backend(el, attrs)
            .unwrap_or_else(|err| panic!("graphics backend init failed: {err}"));
        if cfg!(debug_assertions) {
            eprintln!("rutter: initialized {} backend", backend.backend_type());
        }
        let window = backend.window().clone();
        self.accessibility_adapter = Some(accesskit_winit::Adapter::with_direct_handlers(
            el,
            &window,
            LazyActivationHandler,
            IgnoredActionHandler,
            IgnoredDeactivationHandler,
        ));
        self.scale_factor = window.scale_factor() as f32;
        window.set_visible(true);
        self.window = Some(window);
        self.graphics_backend = Some(backend);
        self.handle_resize(self.window.as_ref().unwrap().inner_size());
        self.layout_dirty = true;
    }

    pub fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        if let Some(backend) = self.graphics_backend.as_mut() {
            backend
                .resize(size)
                .unwrap_or_else(|err| panic!("graphics backend resize failed: {err}"));
        }
        self.layout_dirty = true;
    }

    pub fn process_accessibility_event(&mut self, event: &WindowEvent) {
        let Some(adapter) = self.accessibility_adapter.as_mut() else {
            return;
        };
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        adapter.process_event(&window, event);
    }

    pub fn backend_type(&self) -> Option<BackendType> {
        self.graphics_backend
            .as_ref()
            .map(|backend| backend.backend_type())
    }

    pub fn update_scale(&mut self, scale: f64) {
        self.scale_factor = scale as f32;
        self.layout_dirty = true;
    }
    pub fn schedule_snapshot(&mut self) {
        self.snapshot_scheduled = true;
        self.last_snapshot = std::time::Instant::now();
    }
    pub fn maybe_snapshot(&mut self) {
        if self.snapshot_scheduled && self.last_snapshot.elapsed().as_millis() > 500 {
            if let Some(id) = self.focused_input_id() {
                if let Some(s) = self.input_states.get_mut(&id) {
                    s.snapshot();
                }
            }
            self.snapshot_scheduled = false;
        }
    }

    pub fn focused_input_id(&self) -> Option<u64> {
        self.focused_widget_id
            .filter(|id| self.runtime_caches.inputs.contains_key(id))
    }

    pub fn ensure_input_state(&mut self, id: u64) {
        let sensitive = self
            .runtime_caches
            .inputs
            .get(&id)
            .map(|input| input.is_password)
            .unwrap_or(false);
        if !self.input_states.contains_key(&id) {
            let mut fs = self.font_system.borrow_mut();
            let mut state = crate::input_state::InputWidgetState::new(&mut fs);
            state.set_metrics(&mut fs, A::theme().font_body);
            self.input_states.insert(id, state);
        }
        if let Some(state) = self.input_states.get_mut(&id) {
            state.set_sensitive(sensitive);
        }
    }

    pub fn ensure_widget_states(&mut self) {
        let (stateful, input_ids, toast_updates) = {
            let widget_tree = A::view(&mut self.app_state);
            let mut stateful = Vec::new();
            let mut input_ids = Vec::new();
            let mut toast_updates = Vec::new();
            collect_stateful_ids(&widget_tree, &mut stateful);
            collect_input_ids(&widget_tree, &mut input_ids);
            collect_toast_runtime_updates(&widget_tree, &mut toast_updates);
            (stateful, input_ids, toast_updates)
        };

        let live_widget_ids = stateful.iter().map(|(id, _)| *id).collect::<HashSet<_>>();
        let live_input_ids = input_ids.into_iter().collect::<HashSet<_>>();

        self.widget_states
            .retain(|id, _| live_widget_ids.contains(id));
        self.input_states
            .retain(|id, _| live_input_ids.contains(id));
        if self
            .active_scroll_id
            .is_some_and(|id| !live_widget_ids.contains(&id))
        {
            self.active_scroll_id = None;
        }
        if self
            .drag_slider_id
            .is_some_and(|id| !live_widget_ids.contains(&id))
        {
            self.drag_slider_id = None;
        }

        self.apply_widget_runtime_state(&toast_updates);

        let mut has_anim = false;
        for (id, kind) in &stateful {
            self.widget_states
                .entry(*id)
                .or_insert_with(|| match *kind {
                    "slider" => WidgetState::Slider(SliderState::default()),
                    "scroll" => WidgetState::Scroll(ScrollState::default()),
                    "select" => WidgetState::Select(SelectState::default()),
                    "anim" => {
                        has_anim = true;
                        WidgetState::Anim(AnimState::default())
                    }
                    "tab" => WidgetState::Tab(TabState::default()),
                    "modal" => WidgetState::Modal(ModalState::default()),
                    "toast" => WidgetState::Toast(ToastState::new(3000)),
                    "context_menu" => WidgetState::ContextMenu(ContextMenuState::default()),
                    "popover" => WidgetState::Popover(PopoverState::default()),
                    "vlist" => WidgetState::VList(VirtualListState::default()),
                    "vgrid" => WidgetState::VGrid(VirtualGridState::default()),
                    _ => WidgetState::Anim(AnimState::default()),
                });
            if *kind == "anim" {
                has_anim = true;
            }
        }
        self.has_animated = has_anim;
    }

    fn apply_widget_runtime_state(&mut self, updates: &[ToastRuntimeUpdate]) {
        for update in updates {
            match *update {
                ToastRuntimeUpdate::EnsureVisible { id, duration_ms } => {
                    let restart = self
                        .widget_states
                        .get(&id)
                        .and_then(|s| s.as_toast())
                        .map(|t| {
                            !t.visible
                                || t.dismissed
                                || t.is_expired()
                                || t.duration_ms != duration_ms
                        })
                        .unwrap_or(true);
                    if restart {
                        self.widget_states
                            .insert(id, WidgetState::Toast(ToastState::new(duration_ms)));
                    }
                }
                ToastRuntimeUpdate::Dismiss { id } => {
                    if let Some(ws) = self.widget_states.get_mut(&id) {
                        if let Some(t) = ws.as_toast_mut() {
                            t.dismiss();
                        }
                    }
                }
            }
        }
    }

    pub fn init_toast(&mut self, id: u64, duration_ms: u32) {
        self.widget_states
            .insert(id, WidgetState::Toast(ToastState::new(duration_ms)));
    }

    pub fn any_context_menu_open(&self) -> bool {
        self.widget_states
            .values()
            .filter_map(WidgetState::as_context_menu)
            .any(|state| state.is_open)
    }

    pub fn close_all_context_menus(&mut self) -> bool {
        let mut changed = false;
        for state in self.widget_states.values_mut() {
            if let Some(menu) = state.as_context_menu_mut() {
                if menu.is_open {
                    menu.close();
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn open_context_menu(&mut self, id: u64, anchor: Point) {
        for (&widget_id, state) in self.widget_states.iter_mut() {
            if let Some(menu) = state.as_context_menu_mut() {
                if widget_id == id {
                    menu.open_at(anchor.x, anchor.y);
                } else {
                    menu.close();
                }
            }
        }
    }

    pub fn any_popover_open(&self) -> bool {
        self.widget_states
            .values()
            .filter_map(WidgetState::as_popover)
            .any(|state| state.is_open)
    }

    pub fn close_popover(&mut self, id: u64) -> bool {
        let Some(popover) = self
            .widget_states
            .get_mut(&id)
            .and_then(|state| state.as_popover_mut())
        else {
            return false;
        };
        if popover.is_open {
            popover.close();
            return true;
        }
        false
    }

    pub fn close_all_popovers(&mut self) -> bool {
        let mut changed = false;
        for state in self.widget_states.values_mut() {
            if let Some(popover) = state.as_popover_mut() {
                if popover.is_open {
                    popover.close();
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn open_modal(&mut self, id: u64) {
        if let Some(ws) = self.widget_states.get_mut(&id) {
            if let Some(m) = ws.as_modal_mut() {
                m.open();
            }
        }
        self.layout_dirty = true;
    }
    pub fn close_modal(&mut self, id: u64) {
        if let Some(ws) = self.widget_states.get_mut(&id) {
            if let Some(m) = ws.as_modal_mut() {
                m.close();
            }
        }
        self.layout_dirty = true;
    }

    pub fn tick_animations(&mut self) -> bool {
        let mut changed = false;
        for state in self.widget_states.values_mut() {
            if let Some(a) = state.as_anim_mut() {
                if a.tick() {
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn ensure_layout(&mut self, size: PhysicalSize<u32>) {
        if !self.layout_dirty {
            return;
        }
        let logical = PhysicalSize::new(
            (size.width as f32 / self.scale_factor) as u32,
            (size.height as f32 / self.scale_factor) as u32,
        );
        let widget_tree = A::view(&mut self.app_state);
        let root = sync_taffy_tree_with_direction(
            &mut self.taffy,
            &mut self.layout_tree,
            &widget_tree,
            &self.widget_states,
            A::locale().direction(),
        );
        self.last_root_node = root;
        compute_layout(&mut self.taffy, root, logical, self.font_system.clone());
        self.runtime_caches.clear();
        Self::sync_runtime_metadata(
            &mut self.runtime_caches,
            &mut self.widget_states,
            &self.taffy,
            &widget_tree,
            Some(root),
            A::theme().spacing,
        );
        collect_focus_order(&widget_tree, &mut self.runtime_caches.focus_order);
        drop(widget_tree);
        self.sync_input_buffers();
        if self
            .focused_widget_id
            .is_some_and(|id| !self.runtime_caches.focus_order.contains(&id))
        {
            self.focused_widget_id = None;
        }
        self.layout_dirty = false;
    }

    fn sync_runtime_metadata<Msg: Clone>(
        runtime_caches: &mut WidgetRuntimeCaches<Msg>,
        widget_states: &mut HashMap<u64, WidgetState>,
        taffy: &TaffyTree<RutterContext>,
        widget: &Widget<Msg>,
        node: Option<NodeId>,
        spacing: f32,
    ) {
        let mut path = Vec::new();
        Self::sync_runtime_metadata_impl(
            runtime_caches,
            widget_states,
            taffy,
            widget,
            node,
            Point::new(0.0, 0.0),
            spacing,
            &mut path,
        );
    }

    fn sync_runtime_metadata_impl<Msg: Clone>(
        runtime_caches: &mut WidgetRuntimeCaches<Msg>,
        widget_states: &mut HashMap<u64, WidgetState>,
        taffy: &TaffyTree<RutterContext>,
        widget: &Widget<Msg>,
        node: Option<NodeId>,
        abs: Point,
        spacing: f32,
        path: &mut Vec<usize>,
    ) {
        let layout = node.and_then(|node| taffy.layout(node).ok());
        let abs_pos = layout
            .map(|layout| Point::new(abs.x + layout.location.x, abs.y + layout.location.y))
            .unwrap_or(abs);
        match widget {
            Widget::Button { on_press, .. } | Widget::ButtonContent { on_press, .. } => {
                runtime_caches
                    .buttons
                    .insert(widget.keyboard_focus_id(path).unwrap(), on_press.clone());
            }
            Widget::TextInput {
                on_change,
                on_submit,
                is_password,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                runtime_caches.input_order.push(resolved_id);
                runtime_caches.inputs.insert(
                    resolved_id,
                    InputRuntime {
                        on_change: *on_change,
                        on_submit: on_submit.clone(),
                        is_password: *is_password,
                        is_multiline: false,
                        visible_w: Self::visible_input_width(layout, spacing),
                        visible_h: Self::visible_input_height(layout, spacing),
                    },
                );
            }
            Widget::TextArea {
                on_change,
                on_submit,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                runtime_caches.input_order.push(resolved_id);
                runtime_caches.inputs.insert(
                    resolved_id,
                    InputRuntime {
                        on_change: *on_change,
                        on_submit: on_submit.clone(),
                        is_password: false,
                        is_multiline: true,
                        visible_w: Self::visible_input_width(layout, spacing),
                        visible_h: Self::visible_input_height(layout, spacing),
                    },
                );
            }
            Widget::SearchBar {
                on_change,
                on_submit,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                runtime_caches.input_order.push(resolved_id);
                runtime_caches.inputs.insert(
                    resolved_id,
                    InputRuntime {
                        on_change: *on_change,
                        on_submit: on_submit.clone(),
                        is_password: false,
                        is_multiline: false,
                        visible_w: Self::visible_input_width(layout, spacing),
                        visible_h: Self::visible_input_height(layout, spacing),
                    },
                );
            }
            Widget::Checkbox {
                checked, on_change, ..
            } => {
                runtime_caches.checkboxes.insert(
                    widget.keyboard_focus_id(path).unwrap(),
                    ToggleRuntime {
                        on_change: *on_change,
                        checked: *checked,
                    },
                );
            }
            Widget::Switch {
                checked, on_change, ..
            } => {
                runtime_caches.switches.insert(
                    widget.keyboard_focus_id(path).unwrap(),
                    ToggleRuntime {
                        on_change: *on_change,
                        checked: *checked,
                    },
                );
            }
            Widget::Radio { on_select, .. } => {
                runtime_caches
                    .radios
                    .insert(widget.keyboard_focus_id(path).unwrap(), *on_select);
            }
            Widget::Slider {
                value,
                on_change,
                min,
                max,
                step,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                runtime_caches.sliders.insert(
                    resolved_id,
                    SliderRuntime {
                        on_change: *on_change,
                        value: *value,
                        min: *min,
                        max: *max,
                        step: *step,
                    },
                );
            }
            Widget::Select {
                options,
                selected_index,
                on_change,
                ..
            } => {
                runtime_caches.selects.insert(
                    widget.resolved_id(path).unwrap(),
                    SelectRuntime {
                        on_change: *on_change,
                        selected_index: *selected_index,
                        option_count: options.len(),
                    },
                );
            }
            Widget::Accordion {
                on_toggle, child, ..
            } => {
                runtime_caches
                    .accordions
                    .insert(widget.keyboard_focus_id(path).unwrap(), on_toggle.clone());
                path.push(0);
                Self::sync_runtime_metadata_impl(
                    runtime_caches,
                    widget_states,
                    taffy,
                    child.as_ref(),
                    Self::first_child(node, taffy),
                    abs_pos,
                    spacing,
                    path,
                );
                path.pop();
            }
            Widget::TabBar {
                tabs, on_change, ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                let focus_ids = (0..tabs.len())
                    .filter_map(|index| widget.tab_focus_id(path, index))
                    .collect::<Vec<_>>();
                for (index, focus_id) in focus_ids.iter().copied().enumerate() {
                    runtime_caches.tab_items.insert(
                        focus_id,
                        TabFocusRuntime {
                            parent_id: resolved_id,
                            index,
                        },
                    );
                }
                runtime_caches.tabs.insert(
                    resolved_id,
                    TabRuntime {
                        on_change: *on_change,
                        tab_count: tabs.len(),
                        focus_ids,
                    },
                );
            }
            Widget::Dialog {
                on_confirm,
                on_cancel,
                child,
                ..
            } => {
                if let Some(cancel_id) = widget.dialog_action_focus_id(path, DialogAction::Cancel) {
                    runtime_caches.buttons.insert(cancel_id, on_cancel.clone());
                }
                if let Some(confirm_id) = widget.dialog_action_focus_id(path, DialogAction::Confirm)
                {
                    runtime_caches
                        .buttons
                        .insert(confirm_id, on_confirm.clone());
                }
                path.push(0);
                Self::sync_runtime_metadata_impl(
                    runtime_caches,
                    widget_states,
                    taffy,
                    child.as_ref(),
                    Self::first_child(node, taffy),
                    abs_pos,
                    spacing,
                    path,
                );
                path.pop();
            }
            Widget::Toast {
                on_dismiss: Some(msg),
                ..
            } => {
                runtime_caches
                    .toast_dismiss
                    .insert(widget.resolved_id(path).unwrap(), msg.clone());
            }
            Widget::ScrollView { child, .. } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                if let Some(layout) = layout {
                    if let Some(ws) = widget_states.get_mut(&resolved_id) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.viewport_h = layout.size.height;
                            if let Some(child_node) = Self::first_child(node, taffy) {
                                if let Ok(child_layout) = taffy.layout(child_node) {
                                    s.content_height = child_layout.size.height;
                                }
                            }
                        }
                    }
                }
                path.push(0);
                Self::sync_runtime_metadata_impl(
                    runtime_caches,
                    widget_states,
                    taffy,
                    child.as_ref(),
                    Self::first_child(node, taffy),
                    abs_pos,
                    spacing,
                    path,
                );
                path.pop();
            }
            Widget::VirtualList {
                item_height,
                item_count,
                on_select,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                runtime_caches.vlists.insert(
                    resolved_id,
                    VListRuntime {
                        on_select: *on_select,
                        item_height: *item_height,
                        item_count: *item_count,
                    },
                );
                if let Some(layout) = layout {
                    if let Some(ws) = widget_states.get_mut(&resolved_id) {
                        if let Some(s) = ws.as_vlist_mut() {
                            s.viewport_h = layout.size.height;
                        }
                    }
                }
            }
            Widget::VirtualGrid {
                columns,
                item_height,
                item_count,
                on_select,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                runtime_caches.vgrids.insert(
                    resolved_id,
                    VGridRuntime {
                        on_select: *on_select,
                        columns: *columns,
                        item_height: *item_height,
                        item_count: *item_count,
                    },
                );
                if let Some(layout) = layout {
                    if let Some(ws) = widget_states.get_mut(&resolved_id) {
                        if let Some(s) = ws.as_vgrid_mut() {
                            s.viewport_w = layout.size.width;
                            s.viewport_h = layout.size.height;
                        }
                    }
                }
            }
            Widget::Popover {
                anchor,
                content,
                open,
                ..
            } => {
                let resolved_id = widget.resolved_id(path).unwrap();
                let node_children = Self::children_for(node, taffy);
                if let Some(ws) = widget_states.get_mut(&resolved_id) {
                    if let Some(popover) = ws.as_popover_mut() {
                        popover.set_open(*open);
                        if let Some(anchor_node) = node_children.first().copied() {
                            if let Ok(anchor_layout) = taffy.layout(anchor_node) {
                                popover.set_anchor_rect(
                                    abs_pos.x + anchor_layout.location.x,
                                    abs_pos.y + anchor_layout.location.y,
                                    anchor_layout.size.width,
                                    anchor_layout.size.height,
                                );
                            }
                        }
                    }
                }

                if let Some(anchor_node) = node_children.first().copied() {
                    path.push(0);
                    Self::sync_runtime_metadata_impl(
                        runtime_caches,
                        widget_states,
                        taffy,
                        anchor.as_ref(),
                        Some(anchor_node),
                        abs_pos,
                        spacing,
                        path,
                    );
                    path.pop();
                }

                if *open {
                    if let Some(popup_node) = node_children.get(1).copied() {
                        if let Some(content_node) = Self::first_child(Some(popup_node), taffy) {
                            path.push(1);
                            Self::sync_runtime_metadata_impl(
                                runtime_caches,
                                widget_states,
                                taffy,
                                content.as_ref(),
                                Some(content_node),
                                Point::new(0.0, 0.0),
                                spacing,
                                path,
                            );
                            path.pop();
                        }
                    }
                }
            }
            Widget::Column { children, .. } | Widget::Row { children, .. } => {
                let node_children = Self::children_for(node, taffy);
                for (i, child) in children.iter().enumerate() {
                    path.push(i);
                    Self::sync_runtime_metadata_impl(
                        runtime_caches,
                        widget_states,
                        taffy,
                        child,
                        node_children.get(i).copied(),
                        abs_pos,
                        spacing,
                        path,
                    );
                    path.pop();
                }
            }
            Widget::Container { child, .. }
            | Widget::Tooltip { child, .. }
            | Widget::ContextMenu { child, .. }
            | Widget::Modal { child, .. } => {
                path.push(0);
                Self::sync_runtime_metadata_impl(
                    runtime_caches,
                    widget_states,
                    taffy,
                    child.as_ref(),
                    Self::first_child(node, taffy),
                    abs_pos,
                    spacing,
                    path,
                );
                path.pop();
            }
            _ => {}
        }
    }

    fn visible_input_width(layout: Option<&taffy::tree::Layout>, spacing: f32) -> f32 {
        layout
            .map(|layout| (layout.size.width - spacing * 4.0).max(24.0))
            .unwrap_or(260.0)
    }

    fn visible_input_height(layout: Option<&taffy::tree::Layout>, spacing: f32) -> f32 {
        layout
            .map(|layout| (layout.size.height - spacing * 2.0).max(18.0))
            .unwrap_or(18.0)
    }

    fn sync_input_buffers(&mut self) {
        let theme = A::theme();
        let focused_input = self.focused_input_id();
        let mut fs = self.font_system.borrow_mut();

        for (&id, runtime) in &self.runtime_caches.inputs {
            let Some(state) = self.input_states.get_mut(&id) else {
                continue;
            };

            state.set_sensitive(runtime.is_password);
            state.sync_layout(
                &mut fs,
                runtime.visible_w,
                theme.font_body,
                runtime.is_multiline,
            );

            if focused_input == Some(id) {
                state.update_scroll(
                    &mut fs,
                    runtime.visible_w,
                    runtime.visible_h,
                    theme.font_body,
                    runtime.is_password,
                    runtime.is_multiline,
                );
            }
        }
    }

    fn children_for(node: Option<NodeId>, taffy: &TaffyTree<RutterContext>) -> Vec<NodeId> {
        node.and_then(|node| taffy.children(node).ok())
            .unwrap_or_default()
    }

    fn first_child(node: Option<NodeId>, taffy: &TaffyTree<RutterContext>) -> Option<NodeId> {
        Self::children_for(node, taffy).into_iter().next()
    }

    #[cfg(test)]
    fn sync_runtime_metadata_for_test<Msg: Clone>(
        runtime_caches: &mut WidgetRuntimeCaches<Msg>,
        widget_states: &mut HashMap<u64, WidgetState>,
        taffy: &TaffyTree<RutterContext>,
        widget: &Widget<Msg>,
        root: NodeId,
        spacing: f32,
    ) {
        runtime_caches.clear();
        Self::sync_runtime_metadata(
            runtime_caches,
            widget_states,
            taffy,
            widget,
            Some(root),
            spacing,
        );
    }

    pub fn redraw(&mut self, cursor_pos: Point) {
        let window = match self.window.as_ref() {
            Some(w) => w.clone(),
            None => return,
        };
        let phys = window.inner_size();
        if phys.width == 0 || phys.height == 0 {
            return;
        }

        self.maybe_snapshot();
        self.ensure_widget_states();
        self.ensure_layout(phys);

        let mut widget_tree = A::view(&mut self.app_state);
        let lc = Point::new(
            cursor_pos.x / self.scale_factor,
            cursor_pos.y / self.scale_factor,
        );
        let accessibility_update = self.accessibility_adapter.is_some().then(|| {
            build_accessibility_update(
                &self.taffy,
                &widget_tree,
                self.last_root_node,
                AccessibilityInputs {
                    input_states: &self.input_states,
                    focused_widget_id: self.focused_widget_id,
                },
            )
        });
        if let (Some(adapter), Some(update)) =
            (self.accessibility_adapter.as_mut(), accessibility_update)
        {
            adapter.update_if_active(|| update);
        }

        {
            let backend = self
                .graphics_backend
                .as_mut()
                .expect("graphics backend not initialized");
            let canvas = backend
                .begin_frame()
                .unwrap_or_else(|err| panic!("graphics begin_frame failed: {err}"));
            canvas.restore_to_count(1);
            canvas.clear(SkiaColor::WHITE);
            canvas.reset_matrix();
            canvas.scale((self.scale_factor, self.scale_factor));

            draw_widgets(
                canvas,
                &self.taffy,
                self.last_root_node,
                &mut widget_tree,
                &mut self.font_system.borrow_mut(),
                &mut self.swash_cache,
                lc,
                self.focused_widget_id,
                &self.input_states,
                &self.widget_states,
                &mut self.font_cache,
                &mut self.text_cache,
                self.cursor_blink.is_visible(),
                &A::theme(),
                self.scale_factor,
            );
        }

        self.graphics_backend
            .as_mut()
            .expect("graphics backend not initialized")
            .end_frame()
            .unwrap_or_else(|err| panic!("graphics end_frame failed: {err}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use cosmic_text::FontSystem;

    use crate::layout::build_taffy_tree;
    use crate::widget::{DialogAction, DialogPosition, InputState};

    fn fs() -> Rc<RefCell<FontSystem>> {
        Rc::new(RefCell::new(FontSystem::new()))
    }

    fn base_style(width: f32, height: f32) -> Style {
        Style {
            size: taffy::geometry::Size {
                width: taffy::style::Dimension::length(width),
                height: taffy::style::Dimension::length(height),
            },
            ..Style::default()
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Str(String),
        Float(f32),
        Usize(usize),
        Submit,
        Dismiss,
    }

    struct DummyApp;

    impl AppLogic for DummyApp {
        type State = ();
        type Message = Msg;

        fn new(_: &mut FontSystem) -> Self::State {}

        fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Style::default(),
            }
        }

        fn update(_: &mut Self::State, _: Self::Message, _: &mut Clipboard) {}
    }

    #[test]
    fn runtime_metadata_collects_callbacks_and_layout_props() {
        let widget = Widget::Column {
            children: vec![
                Widget::TextInput {
                    id: 1,
                    on_change: Msg::Str,
                    on_submit: Some(Msg::Submit),
                    style: base_style(200.0, 40.0),
                    label: "",
                    placeholder: "",
                    state: InputState::Idle,
                    error_msg: None,
                    is_password: true,
                },
                Widget::Slider {
                    id: 2,
                    value: 25.0,
                    min: 0.0,
                    max: 100.0,
                    step: 5.0,
                    on_change: Msg::Float,
                    style: base_style(240.0, 20.0),
                    label: "",
                },
                Widget::Select {
                    id: 3,
                    options: &["A", "B"],
                    selected_index: 0,
                    on_change: Msg::Usize,
                    style: base_style(240.0, 40.0),
                    label: "",
                    placeholder: "",
                },
                Widget::TabBar {
                    id: 4,
                    tabs: &["One", "Two"],
                    active: 0,
                    on_change: Msg::Usize,
                    style: base_style(240.0, 40.0),
                },
                Widget::VirtualList {
                    id: 5,
                    item_height: 30.0,
                    item_count: 50,
                    items: &|_| None,
                    on_select: Msg::Usize,
                    style: base_style(240.0, 180.0),
                },
                Widget::VirtualGrid {
                    id: 7,
                    columns: 4,
                    item_height: 64.0,
                    item_count: 60,
                    items: &|_| None,
                    on_select: Msg::Usize,
                    style: base_style(240.0, 192.0),
                },
                Widget::Toast {
                    id: 6,
                    visible: true,
                    message: "done",
                    kind: crate::widget::ToastKind::Info,
                    position: crate::widget::ToastPosition::BottomRight,
                    duration_ms: 1000,
                    on_dismiss: Some(Msg::Dismiss),
                },
            ],
            style: Style::default(),
        };

        let mut widget_states = HashMap::from([
            (5, WidgetState::VList(VirtualListState::default())),
            (7, WidgetState::VGrid(VirtualGridState::default())),
        ]);
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &widget, fs(), &widget_states);
        compute_layout(&mut taffy, root, PhysicalSize::new(400, 500), fs());

        let mut runtime_caches = WidgetRuntimeCaches::<Msg>::default();
        RutterEngine::<DummyApp>::sync_runtime_metadata_for_test(
            &mut runtime_caches,
            &mut widget_states,
            &taffy,
            &widget,
            root,
            DummyApp::theme().spacing,
        );

        assert_eq!(runtime_caches.input_order, vec![1]);
        let input = runtime_caches.inputs.get(&1).unwrap();
        let input_node = taffy.children(root).unwrap()[0];
        let expected_h = RutterEngine::<DummyApp>::visible_input_height(
            taffy.layout(input_node).ok(),
            DummyApp::theme().spacing,
        );
        assert_eq!((input.on_change)("abc".into()), Msg::Str("abc".into()));
        assert_eq!(input.on_submit, Some(Msg::Submit));
        assert!(input.is_password);
        assert!((input.visible_w - 168.0).abs() < f32::EPSILON);
        assert!((input.visible_h - expected_h).abs() < f32::EPSILON);

        let slider = runtime_caches.sliders.get(&2).unwrap();
        assert_eq!(slider.min, 0.0);
        assert_eq!(slider.max, 100.0);
        assert_eq!(slider.step, 5.0);
        assert_eq!((slider.on_change)(55.0), Msg::Float(55.0));

        let select = runtime_caches.selects.get(&3).unwrap();
        assert_eq!(select.selected_index, 0);
        assert_eq!(select.option_count, 2);
        assert_eq!((select.on_change)(1), Msg::Usize(1));

        let tab = runtime_caches.tabs.get(&4).unwrap();
        assert_eq!(tab.tab_count, 2);
        assert_eq!(tab.focus_ids.len(), 2);
        assert_eq!((tab.on_change)(1), Msg::Usize(1));

        let vlist = runtime_caches.vlists.get(&5).unwrap();
        assert_eq!(vlist.item_height, 30.0);
        assert_eq!(vlist.item_count, 50);
        assert_eq!((vlist.on_select)(7), Msg::Usize(7));
        assert_eq!(
            widget_states
                .get(&5)
                .and_then(|s| s.as_vlist())
                .map(|s| s.viewport_h),
            Some(180.0)
        );

        let vgrid = runtime_caches.vgrids.get(&7).unwrap();
        assert_eq!(vgrid.columns, 4);
        assert_eq!(vgrid.item_height, 64.0);
        assert_eq!(vgrid.item_count, 60);
        assert_eq!((vgrid.on_select)(9), Msg::Usize(9));
        assert_eq!(
            widget_states
                .get(&7)
                .and_then(|s| s.as_vgrid())
                .map(|s| (s.viewport_w, s.viewport_h)),
            Some((240.0, 192.0))
        );

        assert_eq!(runtime_caches.toast_dismiss.get(&6), Some(&Msg::Dismiss));
    }

    #[test]
    fn runtime_metadata_keeps_hidden_children_registered() {
        let widget = Widget::Modal {
            id: 9,
            visible: false,
            child: Box::new(Widget::TextInput {
                id: 10,
                on_change: Msg::Str,
                on_submit: Some(Msg::Submit),
                style: base_style(220.0, 40.0),
                label: "",
                placeholder: "",
                state: InputState::Idle,
                error_msg: None,
                is_password: false,
            }),
            on_dismiss: None,
            style: Style::default(),
        };

        let widget_states = HashMap::new();
        let mut taffy = TaffyTree::new();
        let root = build_taffy_tree(&mut taffy, &widget, fs(), &widget_states);
        compute_layout(&mut taffy, root, PhysicalSize::new(400, 300), fs());

        let mut runtime_caches = WidgetRuntimeCaches::<Msg>::default();
        let mut widget_states = HashMap::new();
        RutterEngine::<DummyApp>::sync_runtime_metadata_for_test(
            &mut runtime_caches,
            &mut widget_states,
            &taffy,
            &widget,
            root,
            DummyApp::theme().spacing,
        );

        assert_eq!(runtime_caches.input_order, vec![10]);
        let input = runtime_caches.inputs.get(&10).unwrap();
        assert_eq!(
            (input.on_change)("hidden".into()),
            Msg::Str("hidden".into())
        );
        assert_eq!(input.on_submit, Some(Msg::Submit));
        assert!((input.visible_w - 260.0).abs() < f32::EPSILON);
        assert!(input.visible_h >= 18.0);
    }

    #[test]
    fn focus_order_expands_tab_bar_into_individual_tabs() {
        let tabbar = Widget::TabBar {
            id: 22,
            tabs: &["Home", "Explore", "Library"],
            active: 1,
            on_change: Msg::Usize,
            style: base_style(320.0, 40.0),
        };

        let mut focus_order = Vec::new();
        collect_focus_order(&tabbar, &mut focus_order);

        let expected = (0..3)
            .map(|index| tabbar.tab_focus_id(&[], index).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(focus_order, expected);
        assert_ne!(focus_order[0], tabbar.resolved_id(&[]).unwrap());
    }

    #[test]
    fn visible_dialog_traps_focus_on_dialog_actions() {
        let dialog = Widget::Dialog {
            id: 30,
            title: "Confirm",
            message: "Proceed?",
            confirm_label: "Confirm",
            cancel_label: "Cancel",
            visible: true,
            on_confirm: Msg::Submit,
            on_cancel: Msg::Dismiss,
            on_dismiss: None,
            position: DialogPosition::Center,
            style: base_style(400.0, 240.0),
            child: Box::new(Widget::Spacer {
                style: Style::default(),
            }),
        };
        let cancel_id = dialog
            .dialog_action_focus_id(&[1], DialogAction::Cancel)
            .unwrap();
        let confirm_id = dialog
            .dialog_action_focus_id(&[1], DialogAction::Confirm)
            .unwrap();

        let widget = Widget::Column {
            style: Style::default(),
            children: vec![
                Widget::Button {
                    text: "Open",
                    on_press: Msg::Submit,
                    style: base_style(140.0, 40.0),
                    color: None,
                    variant: crate::widget::ButtonVariant::Primary,
                },
                dialog,
            ],
        };

        let mut focus_order = Vec::new();
        collect_focus_order(&widget, &mut focus_order);

        assert_eq!(focus_order, vec![cancel_id, confirm_id]);
    }
}
