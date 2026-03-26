// ============================================================
// Rutter Framework — engine/mod.rs
// Núcleo stateful do framework. Guarda toda a infraestrutura
// de renderização (Skia surface, Taffy, FontSystem, cache de
// fontes) e implementa ensure_layout + redraw.
//
// Projetado para facilitar a futura migração para wgpu:
// o módulo `render` é a única dependência de Skia/softbuffer;
// o engine em si só conhece abstrações (sizes, nodes, points).
// ============================================================

pub mod cursor;
pub mod runner;
pub mod widget_state;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use arboard::Clipboard;
use skia_safe::{Color as SkiaColor, Font, Point};
use softbuffer::{Context, Surface};
use taffy::prelude::{NodeId, Style, TaffyTree};
use winit::{
    dpi::PhysicalSize,
    event::Modifiers,
    window::Window,
};
use cosmic_text::{FontSystem, SwashCache};

use crate::app::AppLogic;
use crate::layout::{build_taffy_tree, compute_layout, RutterContext};
use crate::render::draw_widgets;
use self::cursor::CursorBlink;

// ── Engine principal ─────────────────────────────────────────

pub struct RutterEngine<A: AppLogic> {
    // ── Janela e superfície ───────────────────────────────────
    pub window:  Option<Rc<Window>>,
    pub surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    context:     Option<Context<Rc<Window>>>, // mantido vivo enquanto surface existir

    // ── Sistema de fontes e renderização ─────────────────────
    pub font_system: Rc<RefCell<FontSystem>>,
    pub swash_cache: SwashCache,
    pub font_cache:  HashMap<(String, u32), Font>,

    // ── Layout (Taffy flexbox) ────────────────────────────────
    pub taffy:          TaffyTree<RutterContext>,
    pub last_root_node: NodeId,
    pub layout_dirty:   bool,

    // ── Estado da aplicação ───────────────────────────────────
    pub app_state: A::State,

    // ── Input e foco ─────────────────────────────────────────
    pub focused_input_id: Option<u64>,
    pub clipboard:        Clipboard,
    pub modifiers:        Modifiers,
    pub input_states:     HashMap<u64, crate::input_state::InputWidgetState>,
    pub widget_states:    HashMap<u64, crate::engine::widget_state::WidgetState>,
    pub drag_slider_id:   Option<u64>,
    pub active_scroll_id: Option<u64>,
    
    // ── Animação ──────────────────────────────────────────────
    pub cursor_blink: CursorBlink,
    pub scale_factor: f32,
    pub last_snapshot: std::time::Instant,
    pub snapshot_scheduled: bool,
}

impl<A: AppLogic> RutterEngine<A> {
    /// Cria o engine e inicializa o estado da aplicação.
    /// Ainda não cria janela — isso ocorre em `handle_resumed`.
    pub fn new() -> Self {
        let mut fs = FontSystem::new();
        let state  = A::new(&mut fs);

        // Nó raiz placeholder até o primeiro layout
        let mut taffy = TaffyTree::new();
        let root      = taffy.new_leaf(Style::default()).unwrap();

        Self {
            window:           None,
            surface:          None,
            context:          None,
            font_system:      Rc::new(RefCell::new(fs)),
            swash_cache:      SwashCache::new(),
            font_cache:       HashMap::new(),
            taffy,
            last_root_node:   root,
            layout_dirty:     true,
            app_state:        state,
            focused_input_id: None,
            input_states:     HashMap::new(),
            widget_states:    HashMap::new(),
            drag_slider_id:   None,
            active_scroll_id: None,
            clipboard:        Clipboard::new().expect("Falha ao inicializar clipboard"),
            modifiers:        Modifiers::default(),
            cursor_blink:     CursorBlink::new(),
            scale_factor:     1.0,
            last_snapshot:    std::time::Instant::now(),
            snapshot_scheduled: false,
        }
    }

    /// Chamado quando o event loop retoma (winit Resumed).
    /// Cria a janela, o contexto softbuffer e a surface.
    pub fn handle_resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attrs  = Window::default_attributes().with_title("Rutter");
        let window = Rc::new(event_loop.create_window(attrs).unwrap());

        let ctx     = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&ctx, window.clone()).unwrap();

        self.window  = Some(window);
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
                if let Some(state) = self.input_states.get_mut(&id) {
                    state.snapshot();
                }
            }
            self.snapshot_scheduled = false;
        }
    }

    pub fn ensure_input_state(&mut self, id: u64) {
        if !self.input_states.contains_key(&id) {
            let mut fs = self.font_system.borrow_mut();
            self.input_states.insert(id, crate::input_state::InputWidgetState::new(&mut fs));
        }
    }
    
    pub fn has_animated(&self) -> bool {
        self.widget_states.values().any(|ws| ws.as_anim().is_some())
    }
    
    pub fn tick_animations(&mut self) -> bool {
        let mut dirty = false;
        for ws in self.widget_states.values_mut() {
            if let Some(anim) = ws.as_anim_mut() {
                if anim.tick() { dirty = true; }
            }
        }
        if dirty { self.layout_dirty = true; }
        dirty
    }

    /// Reconstrói o Taffy tree e recalcula o layout se `layout_dirty`.
    /// Operação idempotente — não faz nada se o layout estiver fresco.
    pub fn ensure_layout(&mut self, size: PhysicalSize<u32>) {
        if !self.layout_dirty { return; }

        let widget_tree = A::view(&mut self.app_state);
        self.taffy.clear();

        let root = build_taffy_tree(&mut self.taffy, &widget_tree, self.font_system.clone(), &self.widget_states);
        self.last_root_node = root;

        compute_layout(&mut self.taffy, root, size, self.font_system.clone());
        self.layout_dirty = false;
    }

    /// Renderiza o frame atual na surface softbuffer.
    ///
    /// Pipeline:
    ///   1. ensure_layout (se dirty)
    ///   2. Constrói widget_tree
    ///   3. Renderiza em surface Skia off-screen
    ///   4. Copia pixels Skia → buffer softbuffer (BGRA → BGR packed)
    ///   5. Present
    ///
    /// TODO (wgpu): substituir os passos 3–5 por upload de textura
    ///              e draw call wgpu, mantendo os passos 1–2 inalterados.
    pub fn redraw(&mut self, cursor_pos: Point) {
        let window = match self.window.as_ref() { Some(w) => w.clone(), None => return };
        let size   = window.inner_size();
        if size.width == 0 || size.height == 0 { return; }

        self.ensure_layout(size);

        let widget_tree = A::view(&mut self.app_state);

        // ── Renderizar em surface Skia off-screen ─────────────
        let mut sk_surface = skia_safe::surfaces::raster_n32_premul(
            (size.width as i32, size.height as i32),
        )
        .expect("Falha ao criar surface Skia");

        sk_surface.canvas().clear(SkiaColor::WHITE);

        draw_widgets(
            sk_surface.canvas(),
            &self.taffy,
            self.last_root_node,
            &widget_tree,
            &mut self.font_system.borrow_mut(),
            &mut self.swash_cache,
            cursor_pos,
            self.focused_input_id,
            &self.input_states,
            &self.widget_states,
            &mut self.font_cache,
            self.cursor_blink.is_visible(),
            &A::theme(),
            self.scale_factor,
        );

        // ── Copiar pixels Skia (RGBA) → softbuffer (0xRRGGBB) ─
        let surface = self.surface.as_mut().unwrap();
        let mut buf = surface.buffer_mut().unwrap();
        let pixmap  = sk_surface.peek_pixels().unwrap();
        let bytes   = pixmap.bytes().unwrap();

        for (i, pixel) in buf.iter_mut().enumerate() {
            let o = i * 4;
            if o + 2 < bytes.len() {
                let r = bytes[o    ] as u32;
                let g = bytes[o + 1] as u32;
                let b = bytes[o + 2] as u32;
                *pixel = (r << 16) | (g << 8) | b;
            }
        }
        buf.present().unwrap();
    }
}
