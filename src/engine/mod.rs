// ============================================================
// Rutter Framework — engine/mod.rs
// ============================================================

pub mod cursor;
pub mod runner;
pub mod widget_state;

use std::cell::RefCell;
use std::collections::HashMap;
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
use crate::render::hit_test::collect_stateful_ids;
use crate::widget::Widget;

#[derive(Debug, Clone, Copy)]
enum ToastRuntimeUpdate {
    EnsureVisible { id: u64, duration_ms: u32 },
    Dismiss { id: u64 },
}

fn collect_toast_runtime_updates<Msg>(widget: &Widget<Msg>, out: &mut Vec<ToastRuntimeUpdate>) {
    match widget {
        Widget::Toast {
            id,
            visible,
            duration_ms,
            ..
        } => {
            if *visible {
                out.push(ToastRuntimeUpdate::EnsureVisible {
                    id: *id,
                    duration_ms: *duration_ms,
                });
            } else {
                out.push(ToastRuntimeUpdate::Dismiss { id: *id });
            }
        }
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children {
                collect_toast_runtime_updates(child, out);
            }
        }
        Widget::Container { child, .. }
        | Widget::Tooltip { child, .. }
        | Widget::ScrollView { child, .. }
        | Widget::Accordion { child, .. }
        | Widget::Modal { child, .. }
        | Widget::Dialog { child, .. } => collect_toast_runtime_updates(child, out),
        _ => {}
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
            self.input_states
                .insert(id, crate::input_state::InputWidgetState::new(&mut fs));
        }
    }

    pub fn ensure_widget_states(&mut self) {
        let (stateful, toast_updates) = {
            let widget_tree = A::view(&mut self.app_state);
            let mut stateful = Vec::new();
            let mut toast_updates = Vec::new();
            collect_stateful_ids(&widget_tree, &mut stateful);
            collect_toast_runtime_updates(&widget_tree, &mut toast_updates);
            (stateful, toast_updates)
        };

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
        Self::update_viewports_impl(&mut self.widget_states, &self.taffy, &widget_tree, root);
        self.layout_dirty = false;
    }

    fn update_viewports_impl<Msg>(
        widget_states: &mut HashMap<u64, WidgetState>,
        taffy: &TaffyTree<RutterContext>,
        widget: &Widget<Msg>,
        node: NodeId,
    ) {
        if let Ok(layout) = taffy.layout(node) {
            match widget {
                Widget::ScrollView { id, child, .. } => {
                    if let Some(ws) = widget_states.get_mut(id) {
                        if let Some(s) = ws.as_scroll_mut() {
                            s.viewport_h = layout.size.height;
                            if let Ok(children) = taffy.children(node) {
                                if !children.is_empty() {
                                    if let Ok(child_layout) = taffy.layout(children[0]) {
                                        s.content_height = child_layout.size.height;
                                    }
                                }
                            }
                        }
                    }
                    if let Ok(children) = taffy.children(node) {
                        if !children.is_empty() {
                            Self::update_viewports_impl(
                                widget_states,
                                taffy,
                                child.as_ref(),
                                children[0],
                            );
                        }
                    }
                }
                Widget::VirtualList { id, .. } => {
                    if let Some(ws) = widget_states.get_mut(id) {
                        if let Some(s) = ws.as_vlist_mut() {
                            s.viewport_h = layout.size.height;
                        }
                    }
                }
                Widget::Column { children, .. } | Widget::Row { children, .. } => {
                    if let Ok(node_children) = taffy.children(node) {
                        for (i, child) in children.iter().enumerate() {
                            if i < node_children.len() {
                                Self::update_viewports_impl(
                                    widget_states,
                                    taffy,
                                    child,
                                    node_children[i],
                                );
                            }
                        }
                    }
                }
                Widget::Container { child, .. }
                | Widget::Tooltip { child, .. }
                | Widget::Accordion { child, .. }
                | Widget::Modal { child, .. }
                | Widget::Dialog { child, .. } => {
                    if let Ok(children) = taffy.children(node) {
                        if !children.is_empty() {
                            Self::update_viewports_impl(
                                widget_states,
                                taffy,
                                child.as_ref(),
                                children[0],
                            );
                        }
                    }
                }
                _ => {}
            }
        }
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
