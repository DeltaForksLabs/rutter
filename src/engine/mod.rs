// ============================================================
// Rutter Framework — engine/mod.rs
// ============================================================

pub mod cursor;
pub mod runner;
pub mod widget_state;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use arboard::Clipboard;
use cosmic_text::{FontSystem, SwashCache};
use skia_safe::{Color as SkiaColor, Font, Point};
use softbuffer::{Context, Surface};
use taffy::prelude::{NodeId, Style, TaffyTree};
use winit::{dpi::PhysicalSize, event::Modifiers, window::Window};

use self::cursor::CursorBlink;
use self::widget_state::{
    AnimState, ModalState, ScrollState, SelectState, SliderState, TabState, ToastState,
    VirtualListState, WidgetState,
};
use crate::app::AppLogic;
use crate::layout::{RutterContext, SyncedLayoutTree, compute_layout, sync_taffy_tree};
use crate::render::draw_widgets;
use crate::render::hit_test::{collect_input_ids, collect_stateful_ids};
use crate::widget::Widget;

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
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => {
            path.push(0);
            collect_toast_runtime_updates_impl(child, out, path);
            path.pop();
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
}

#[derive(Debug, Clone)]
struct SliderRuntime<Msg> {
    on_change: fn(f32) -> Msg,
    min: f32,
    max: f32,
    step: f32,
}

#[derive(Debug, Clone)]
struct VListRuntime<Msg> {
    on_select: fn(usize) -> Msg,
    item_height: f32,
    item_count: usize,
}

#[derive(Debug)]
struct WidgetRuntimeCaches<Msg: Clone> {
    input_order: Vec<u64>,
    inputs: HashMap<u64, InputRuntime<Msg>>,
    sliders: HashMap<u64, SliderRuntime<Msg>>,
    selects: HashMap<u64, fn(usize) -> Msg>,
    tabs: HashMap<u64, fn(usize) -> Msg>,
    vlists: HashMap<u64, VListRuntime<Msg>>,
    toast_dismiss: HashMap<u64, Msg>,
}

impl<Msg: Clone> Default for WidgetRuntimeCaches<Msg> {
    fn default() -> Self {
        Self {
            input_order: Vec::new(),
            inputs: HashMap::new(),
            sliders: HashMap::new(),
            selects: HashMap::new(),
            tabs: HashMap::new(),
            vlists: HashMap::new(),
            toast_dismiss: HashMap::new(),
        }
    }
}

impl<Msg: Clone> WidgetRuntimeCaches<Msg> {
    fn clear(&mut self) {
        self.input_order.clear();
        self.inputs.clear();
        self.sliders.clear();
        self.selects.clear();
        self.tabs.clear();
        self.vlists.clear();
        self.toast_dismiss.clear();
    }
}

pub struct RutterEngine<A: AppLogic> {
    pub window: Option<Rc<Window>>,
    pub surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    context: Option<Context<Rc<Window>>>,
    pub font_system: Rc<RefCell<FontSystem>>,
    pub swash_cache: SwashCache,
    pub font_cache: HashMap<(String, u32), Font>,
    pub taffy: TaffyTree<RutterContext>,
    layout_tree: SyncedLayoutTree,
    runtime_caches: WidgetRuntimeCaches<A::Message>,
    pub last_root_node: NodeId,
    pub layout_dirty: bool,
    pub app_state: A::State,
    pub input_states: HashMap<u64, crate::input_state::InputWidgetState>,
    pub widget_states: HashMap<u64, WidgetState>,
    pub focused_input_id: Option<u64>,
    pub active_scroll_id: Option<u64>,
    pub drag_slider_id: Option<u64>,
    pub clipboard: Clipboard,
    pub modifiers: Modifiers,
    pub cursor_blink: CursorBlink,
    pub last_snapshot: std::time::Instant,
    pub snapshot_scheduled: bool,
    pub scale_factor: f32,
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
            surface: None,
            context: None,
            font_system: Rc::new(RefCell::new(fs)),
            swash_cache: SwashCache::new(),
            font_cache: HashMap::new(),
            layout_tree: SyncedLayoutTree::placeholder(root),
            runtime_caches: WidgetRuntimeCaches::default(),
            taffy,
            last_root_node: root,
            layout_dirty: true,
            app_state: state,
            input_states: HashMap::new(),
            widget_states: HashMap::new(),
            focused_input_id: None,
            active_scroll_id: None,
            drag_slider_id: None,
            clipboard: Clipboard::new().expect("clipboard init falhou"),
            modifiers: Modifiers::default(),
            cursor_blink: CursorBlink::new(),
            last_snapshot: std::time::Instant::now(),
            snapshot_scheduled: false,
            scale_factor: 1.0,
            has_animated: false,
        }
    }

    pub fn handle_resumed(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        let attrs = Window::default_attributes().with_title("Rutter");
        let window = Rc::new(el.create_window(attrs).unwrap());
        self.scale_factor = window.scale_factor() as f32;
        let ctx = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&ctx, window.clone()).unwrap();
        self.window = Some(window);
        self.context = Some(ctx);
        self.surface = Some(surface);
        self.layout_dirty = true;
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
            if let Some(id) = self.focused_input_id {
                if let Some(s) = self.input_states.get_mut(&id) {
                    s.snapshot();
                }
            }
            self.snapshot_scheduled = false;
        }
    }

    pub fn ensure_input_state(&mut self, id: u64) {
        if !self.input_states.contains_key(&id) {
            let mut fs = self.font_system.borrow_mut();
            let mut state = crate::input_state::InputWidgetState::new(&mut fs);
            state.set_metrics(&mut fs, A::theme().font_body);
            self.input_states.insert(id, state);
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
            .focused_input_id
            .is_some_and(|id| !live_input_ids.contains(&id))
        {
            self.focused_input_id = None;
        }
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
                    "vlist" => WidgetState::VList(VirtualListState::default()),
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
        let root = sync_taffy_tree(
            &mut self.taffy,
            &mut self.layout_tree,
            &widget_tree,
            &self.widget_states,
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
        spacing: f32,
        path: &mut Vec<usize>,
    ) {
        let layout = node.and_then(|node| taffy.layout(node).ok());
        match widget {
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
                    },
                );
            }
            Widget::Slider {
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
                        min: *min,
                        max: *max,
                        step: *step,
                    },
                );
            }
            Widget::Select { on_change, .. } => {
                runtime_caches
                    .selects
                    .insert(widget.resolved_id(path).unwrap(), *on_change);
            }
            Widget::TabBar { on_change, .. } => {
                runtime_caches
                    .tabs
                    .insert(widget.resolved_id(path).unwrap(), *on_change);
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
                        spacing,
                        path,
                    );
                    path.pop();
                }
            }
            Widget::Container { child, .. }
            | Widget::Tooltip { child, .. }
            | Widget::Accordion { child, .. }
            | Widget::Modal { child, .. }
            | Widget::Dialog { child, .. } => {
                path.push(0);
                Self::sync_runtime_metadata_impl(
                    runtime_caches,
                    widget_states,
                    taffy,
                    child.as_ref(),
                    Self::first_child(node, taffy),
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
        let mut sk =
            skia_safe::surfaces::raster_n32_premul((phys.width as i32, phys.height as i32))
                .expect("Surface Skia falhou");
        sk.canvas().clear(SkiaColor::WHITE);
        sk.canvas().scale((self.scale_factor, self.scale_factor));

        let lc = Point::new(
            cursor_pos.x / self.scale_factor,
            cursor_pos.y / self.scale_factor,
        );

        draw_widgets(
            sk.canvas(),
            &self.taffy,
            self.last_root_node,
            &mut widget_tree,
            &mut self.font_system.borrow_mut(),
            &mut self.swash_cache,
            lc,
            self.focused_input_id,
            &self.input_states,
            &self.widget_states,
            &mut self.font_cache,
            self.cursor_blink.is_visible(),
            &A::theme(),
            self.scale_factor,
        );

        let sf = self.surface.as_mut().unwrap();
        let mut buf = sf.buffer_mut().unwrap();
        let px = sk.peek_pixels().unwrap();
        let raw = px.bytes().unwrap();
        for (i, pixel) in buf.iter_mut().enumerate() {
            let o = i * 4;
            if o + 2 < raw.len() {
                *pixel = ((raw[o] as u32) << 16) | ((raw[o + 1] as u32) << 8) | raw[o + 2] as u32;
            }
        }
        buf.present().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use cosmic_text::FontSystem;

    use crate::layout::build_taffy_tree;
    use crate::widget::InputState;

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

        let mut widget_states =
            HashMap::from([(5, WidgetState::VList(VirtualListState::default()))]);
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
        assert_eq!((input.on_change)("abc".into()), Msg::Str("abc".into()));
        assert_eq!(input.on_submit, Some(Msg::Submit));
        assert!(input.is_password);
        assert!((input.visible_w - 168.0).abs() < f32::EPSILON);

        let slider = runtime_caches.sliders.get(&2).unwrap();
        assert_eq!(slider.min, 0.0);
        assert_eq!(slider.max, 100.0);
        assert_eq!(slider.step, 5.0);
        assert_eq!((slider.on_change)(55.0), Msg::Float(55.0));

        assert_eq!(
            runtime_caches.selects.get(&3).copied().unwrap()(1),
            Msg::Usize(1)
        );
        assert_eq!(
            runtime_caches.tabs.get(&4).copied().unwrap()(1),
            Msg::Usize(1)
        );

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
    }
}
