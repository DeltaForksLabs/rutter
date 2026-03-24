// RUTTER FRAMEWORK v2.5 - CORREÇÕES CRÍTICAS
// 
// MUDANÇAS NESTA VERSÃO:
// 1. Cursor perfeitamente alinhado (vertical e horizontal)
// 2. Input single-line com scroll horizontal automático
// 3. Cursor piscando (500ms)
// 4. Atalhos: CTRL+C (copiar), CTRL+V (colar), CTRL+A (selecionar tudo)
//
// ============================================================================

use skia_safe::{
    canvas::Canvas,
    Color as SkiaColor, Font, FontMgr, FontStyle, Paint, Point, Rect as SkiaRect, RRect,
    Contains,
};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Instant, Duration};
use taffy::prelude::*;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent, Modifiers},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
    keyboard::{Key, NamedKey, ModifiersState},
};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, Edit, Editor, Action, Motion};
use arboard::Clipboard;

//================================================================================
// 1. ESTRUTURAS DE CONTEXTO E TRAITS
//================================================================================

#[derive(Debug, Clone)]
pub struct TextContext {
    pub content: String,
    pub font_size: f32,
}

#[derive(Debug, Clone, Default)]
pub enum RutterContext {
    #[default] None,
    Text(TextContext),
}

pub trait AppLogic {
    type State;
    type Message: Clone + std::fmt::Debug;

    fn new(font_system: &mut FontSystem) -> Self::State;
    fn view<'a>(state: &'a mut Self::State) -> Widget<'a, Self::Message>;
    fn update(state: &mut Self::State, message: Self::Message, clipboard: &mut Clipboard);
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum InputState {
    Idle,
    Focused,
    Error,
    Success,
}

pub enum Widget<'a, Msg> {
    Column { children: Vec<Widget<'a, Msg>>, style: Style },
    Row { children: Vec<Widget<'a, Msg>>, style: Style },
    Container { child: Box<Widget<'a, Msg>>, style: Style, color: Option<SkiaColor>, radius: f32 },
    Button { text: &'a str, on_press: Msg, style: Style, color: SkiaColor },
    Text { content: String, style: Style, color: SkiaColor, size: f32 },
    TextInput {
        editor: &'a mut Editor<'static>,
        on_change: fn(String) -> Msg,
        on_submit: Option<Msg>,
        style: Style,
        id: u64,
        label: &'a str,
        state: InputState,
        error_msg: Option<String>,
        is_password: bool,
    },
}

//================================================================================
// 2. CURSOR PISCANTE
//================================================================================

struct CursorBlink {
    visible: bool,
    last_toggle: Instant,
    blink_interval: Duration,
}

impl CursorBlink {
    fn new() -> Self {
        Self {
            visible: true,
            last_toggle: Instant::now(),
            blink_interval: Duration::from_millis(500),
        }
    }
    
    fn update(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_toggle) >= self.blink_interval {
            self.visible = !self.visible;
            self.last_toggle = now;
            true // Mudou de estado
        } else {
            false
        }
    }
    
    fn reset(&mut self) {
        self.visible = true;
        self.last_toggle = Instant::now();
    }
    
    fn is_visible(&self) -> bool {
        self.visible
    }
}

//================================================================================
// 3. ENGINE
//================================================================================

pub struct RutterEngine<A: AppLogic> {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    context: Option<Context<Rc<Window>>>,
    
    taffy: TaffyTree<RutterContext>,
    font_system: Rc<RefCell<FontSystem>>,
    swash_cache: SwashCache,
    
    app_state: A::State,
    
    layout_dirty: bool,
    last_root_node: NodeId,
    
    focused_input_id: Option<u64>,
    clipboard: Clipboard,
    modifiers: Modifiers,
    
    font_cache: HashMap<(String, u32), Font>,
    
    // NOVO: Cursor piscante
    cursor_blink: CursorBlink,
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
            taffy,
            font_system: Rc::new(RefCell::new(fs)),
            swash_cache: SwashCache::new(),
            app_state: state,
            layout_dirty: true,
            last_root_node: root,
            focused_input_id: None,
            clipboard: Clipboard::new().expect("Clipboard init failed"),
            modifiers: Modifiers::default(),
            font_cache: HashMap::new(),
            cursor_blink: CursorBlink::new(),
        }
    }

    pub fn handle_resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title("Rutter Framework Demo");
        let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(window.clone());
        let context = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();
        self.context = Some(context);
        self.surface = Some(surface);
        self.layout_dirty = true;
    }

    fn ensure_layout(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if !self.layout_dirty { 
            return; 
        }

        let available_space = Size {
            width: AvailableSpace::Definite(size.width as f32),
            height: AvailableSpace::Definite(size.height as f32),
        };

        let widget_tree = A::view(&mut self.app_state);
        self.taffy.clear();
        let root = build_taffy_tree(&mut self.taffy, &widget_tree, self.font_system.clone());
        self.last_root_node = root;

        let fs_rc = self.font_system.clone();
        self.taffy.compute_layout_with_measure(root, available_space, |known, available, _, ctx, _| {
             match ctx {
                Some(RutterContext::Text(t_ctx)) => {
                    let mut fs = fs_rc.borrow_mut();
                    let mut buffer = Buffer::new(&mut fs, Metrics::new(t_ctx.font_size, t_ctx.font_size * 1.2));
                    match available.width {
                        AvailableSpace::Definite(px) => buffer.set_size(&mut fs, Some(px), None),
                        AvailableSpace::MaxContent => buffer.set_size(&mut fs, None, None),
                        AvailableSpace::MinContent => buffer.set_size(&mut fs, Some(0.0), None),
                    }
                    buffer.set_text(&mut fs, &t_ctx.content, &Attrs::new(), Shaping::Advanced, None);
                    buffer.shape_until_scroll(&mut fs, true);
                    let (w, h) = buffer.size();
                    Size { 
                        width: known.width.unwrap_or(w.unwrap_or(0.0)), 
                        height: known.height.unwrap_or(h.unwrap_or(0.0)) 
                    }
                },
                _ => Size::ZERO,
            }
        }).unwrap();

        self.layout_dirty = false;
    }

    fn redraw(&mut self, cursor_pos: Point) {
        let window = match self.window.as_ref() { Some(w) => w, None => return };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 { return; }

        // Atualizar cursor piscante e redesenhar se mudou
        if self.cursor_blink.update() && self.focused_input_id.is_some() {
            window.request_redraw();
        }

        self.ensure_layout(size);
        
        let mut widget_tree = A::view(&mut self.app_state);

        let mut skia_surface = skia_safe::surfaces::raster_n32_premul((size.width as i32, size.height as i32)).unwrap();
        let canvas = skia_surface.canvas();
        
        canvas.clear(SkiaColor::WHITE);

        draw_widgets(
            canvas, &self.taffy, self.last_root_node, &mut widget_tree, 
            &mut self.font_system.borrow_mut(), &mut self.swash_cache, 
            cursor_pos, self.focused_input_id, &mut self.font_cache,
            self.cursor_blink.is_visible() // NOVO: passa visibilidade do cursor
        );

        let surface = self.surface.as_mut().unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        let pixmap = skia_surface.peek_pixels().unwrap();
        let bytes = pixmap.bytes().unwrap();
        
        for (i, pixel) in buffer.iter_mut().enumerate() {
            let offset = i * 4;
            if offset + 3 < bytes.len() {
                let r = bytes[offset + 0] as u32;
                let g = bytes[offset + 1] as u32;
                let b = bytes[offset + 2] as u32;
                *pixel = (r << 16) | (g << 8) | b;
            }
        }
        buffer.present().unwrap();
    }
}

//================================================================================
// 4. RUNNER
//================================================================================

pub struct RutterRunner<A: AppLogic> {
    engine: RutterEngine<A>,
    cursor_pos: Point, 
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub fn run() {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app: RutterRunner<A> = RutterRunner { 
            engine: RutterEngine::new(), 
            cursor_pos: Point::new(0.0, 0.0) 
        };
        event_loop.run_app(&mut app).unwrap();
    }
}

impl<A: AppLogic + 'static> ApplicationHandler for RutterRunner<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) { 
        self.engine.handle_resumed(el); 
    }
    
    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                    if let Some(s) = self.engine.surface.as_mut() { s.resize(w, h).unwrap(); }
                    self.engine.layout_dirty = true;
                    if let Some(w) = self.engine.window.as_ref() { w.request_redraw(); }
                }
            }

            WindowEvent::ModifiersChanged(mods) => self.engine.modifiers = mods,
            
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = Point::new(position.x as f32, position.y as f32);
            }
            
            WindowEvent::MouseInput { state: ElementState::Pressed, button: winit::event::MouseButton::Left, .. } => {
                let size = self.engine.window.as_ref().unwrap().inner_size();
                self.engine.ensure_layout(size);
                let widget_tree = A::view(&mut self.engine.app_state);
                self.engine.focused_input_id = None; 
                
                if let Some(hit) = hit_test(&widget_tree, &self.engine.taffy, self.engine.last_root_node, self.cursor_pos, Point::new(0.0, 0.0)) {
                    match hit {
                        HitResult::Message(msg) => {
                            A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                            self.engine.layout_dirty = true;
                        },
                        HitResult::InputFocus(id) => {
                            self.engine.focused_input_id = Some(id);
                            self.engine.cursor_blink.reset(); // Reset cursor quando foca
                            self.engine.layout_dirty = true;
                        }
                    }
                    if let Some(w) = self.engine.window.as_ref() { w.request_redraw(); }
                }
            }
            
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    // NAVEGAÇÃO COM TAB
                    if let Key::Named(NamedKey::Tab) = event.logical_key {
                        let widget_tree = A::view(&mut self.engine.app_state);
                        let mut input_ids = Vec::new();
                        collect_input_ids(&widget_tree, &mut input_ids);
                        
                        if !input_ids.is_empty() {
                            if let Some(current_id) = self.engine.focused_input_id {
                                if let Some(current_idx) = input_ids.iter().position(|&id| id == current_id) {
                                    let next_idx = if self.engine.modifiers.state().shift_key() {
                                        if current_idx == 0 { input_ids.len() - 1 } else { current_idx - 1 }
                                    } else {
                                        (current_idx + 1) % input_ids.len()
                                    };
                                    self.engine.focused_input_id = Some(input_ids[next_idx]);
                                    self.engine.cursor_blink.reset();
                                }
                            } else {
                                self.engine.focused_input_id = Some(input_ids[0]);
                                self.engine.cursor_blink.reset();
                            }
                            self.engine.layout_dirty = true;
                            if let Some(w) = self.engine.window.as_ref() { w.request_redraw(); }
                        }
                        return;
                    }
                    
                    // ATALHOS CTRL+C, CTRL+V, CTRL+A
                    if self.engine.modifiers.state().control_key() {
                        if let Key::Character(ref text) = event.logical_key {
                            match text.to_lowercase().as_str() {
                                "c" => {
                                    // CTRL+C: Copiar texto selecionado
                                    if let Some(focused_id) = self.engine.focused_input_id {
                                        let mut widget_tree = A::view(&mut self.engine.app_state);
                                        if let Some((editor, _, _)) = find_input_mut(&mut widget_tree, focused_id) {
                                            let full_text = editor.with_buffer(|buffer| {
                                                let mut text = String::new();
                                                for line in buffer.lines.iter() {
                                                    text.push_str(line.text());
                                                }
                                                text
                                            });
                                            let _ = self.engine.clipboard.set_text(full_text);
                                        }
                                    }
                                    return;
                                },
                                "v" => {
                                    // CTRL+V: Colar do clipboard
                                    if let Some(focused_id) = self.engine.focused_input_id {
                                        if let Ok(clipboard_text) = self.engine.clipboard.get_text() {
                                            let mut widget_tree = A::view(&mut self.engine.app_state);
                                            if let Some((editor, on_change, _)) = find_input_mut(&mut widget_tree, focused_id) {
                                                editor.insert_string(&clipboard_text, None);
                                                
                                                let full_text = editor.with_buffer(|buffer| {
                                                    let mut text = String::new();
                                                    for line in buffer.lines.iter() {
                                                        text.push_str(line.text());
                                                    }
                                                    text
                                                });
                                                
                                                let msg = on_change(full_text);
                                                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                                                self.engine.layout_dirty = true;
                                                self.engine.cursor_blink.reset();
                                                if let Some(w) = self.engine.window.as_ref() { w.request_redraw(); }
                                            }
                                        }
                                    }
                                    return;
                                },
                                "a" => {
                                    // CTRL+A: Selecionar tudo (por enquanto, só copia tudo)
                                    if let Some(focused_id) = self.engine.focused_input_id {
                                        let mut widget_tree = A::view(&mut self.engine.app_state);
                                        if let Some((editor, _, _)) = find_input_mut(&mut widget_tree, focused_id) {
                                            let full_text = editor.with_buffer(|buffer| {
                                                let mut text = String::new();
                                                for line in buffer.lines.iter() {
                                                    text.push_str(line.text());
                                                }
                                                text
                                            });
                                            let _ = self.engine.clipboard.set_text(full_text);
                                        }
                                    }
                                    return;
                                },
                                _ => {}
                            }
                        }
                    }
                    
                    // PROCESSAR INPUT
                    if let Some(focused_id) = self.engine.focused_input_id {
                        let mut widget_tree = A::view(&mut self.engine.app_state);
                        if let Some((editor, on_change, _)) = find_input_mut(&mut widget_tree, focused_id) {
                            let mut needs_update = false;
                            
                            if let Some(action) = map_winit_to_action(&event.logical_key, self.engine.modifiers) {
                                let mut fs = self.engine.font_system.borrow_mut();
                                editor.action(&mut fs, action);
                                needs_update = true;
                            } 
                            else if let Key::Named(NamedKey::Space) = event.logical_key {
                                editor.insert_string(" ", None);
                                needs_update = true;
                            }
                            else if let Key::Character(ref text) = event.logical_key {
                                if !text.is_empty() {
                                    editor.insert_string(text.as_str(), None);
                                    needs_update = true;
                                }
                            }

                            if needs_update {
                                self.engine.cursor_blink.reset(); // Reset cursor ao digitar
                                
                                let full_text = editor.with_buffer(|buffer| {
                                    let mut text = String::new();
                                    for line in buffer.lines.iter() {
                                        text.push_str(line.text());
                                    }
                                    text
                                });

                                let msg = on_change(full_text);
                                A::update(&mut self.engine.app_state, msg, &mut self.engine.clipboard);
                                self.engine.layout_dirty = true;
                                if let Some(w) = self.engine.window.as_ref() { w.request_redraw(); }
                            }
                        }
                    }
                }
            }
            
            WindowEvent::RedrawRequested => self.engine.redraw(self.cursor_pos),
            _ => (),
        }
    }
}

//================================================================================
// 5. HELPERS E RENDERIZAÇÃO
//================================================================================

fn map_winit_to_action(key: &Key, _mods: Modifiers) -> Option<Action> {
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(Action::Motion(Motion::Left)),
        Key::Named(NamedKey::ArrowRight) => Some(Action::Motion(Motion::Right)),
        Key::Named(NamedKey::ArrowUp) => Some(Action::Motion(Motion::Up)),
        Key::Named(NamedKey::ArrowDown) => Some(Action::Motion(Motion::Down)),
        Key::Named(NamedKey::Backspace) => Some(Action::Backspace),
        Key::Named(NamedKey::Delete) => Some(Action::Delete),
        Key::Named(NamedKey::Enter) => Some(Action::Enter),
        Key::Named(NamedKey::Home) => Some(Action::Motion(Motion::Home)),
        Key::Named(NamedKey::End) => Some(Action::Motion(Motion::End)),
        _ => None,
    }
}

fn build_taffy_tree<'a, Msg>(taffy: &mut TaffyTree<RutterContext>, widget: &Widget<'a, Msg>, fs: Rc<RefCell<FontSystem>>) -> NodeId {
    match widget {
        Widget::Column { children, style } => {
            let s = Style { flex_direction: FlexDirection::Column, ..style.clone() };
            let ids: Vec<_> = children.iter().map(|c| build_taffy_tree(taffy, c, fs.clone())).collect();
            taffy.new_with_children(s, &ids).unwrap()
        }
        Widget::Row { children, style } => {
            let s = Style { flex_direction: FlexDirection::Row, ..style.clone() };
            let ids: Vec<_> = children.iter().map(|c| build_taffy_tree(taffy, c, fs.clone())).collect();
            taffy.new_with_children(s, &ids).unwrap()
        }
        Widget::Container { child, style, .. } => {
            let id = build_taffy_tree(taffy, child, fs.clone());
            taffy.new_with_children(style.clone(), &[id]).unwrap()
        }
        Widget::Button { style, .. } | Widget::TextInput { style, .. } => taffy.new_leaf(style.clone()).unwrap(),
        Widget::Text { content, style, size, .. } => {
             let context = RutterContext::Text(TextContext { content: content.clone(), font_size: *size });
             taffy.new_leaf_with_context(style.clone(), context).unwrap()
        }
    }
}

fn get_cached_font(cache: &mut HashMap<(String, u32), Font>, family: &str, size: f32) -> Font {
    let key = (family.to_string(), size as u32);
    cache.entry(key).or_insert_with(|| {
        let tf = FontMgr::new()
            .match_family_style(family, FontStyle::normal())
            .expect("Font not found");
        Font::new(tf, size)
    }).clone()
}

fn draw_widgets<Msg>(
    canvas: &Canvas, 
    taffy: &TaffyTree<RutterContext>, 
    node: NodeId, 
    widget: &mut Widget<Msg>,
    fs: &mut FontSystem, 
    sc: &mut SwashCache, 
    mouse_pos: Point, 
    focused_id: Option<u64>,
    font_cache: &mut HashMap<(String, u32), Font>,
    cursor_visible: bool // NOVO: visibilidade do cursor
) {
    let layout = taffy.layout(node).unwrap();
    let pos = Point::new(layout.location.x, layout.location.y);
    let size = (layout.size.width, layout.size.height);
    canvas.save();
    canvas.translate((pos.x, pos.y));
    let local_mouse = Point::new(mouse_pos.x - pos.x, mouse_pos.y - pos.y);

    match widget {
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node).unwrap();
            for (i, child) in children.iter_mut().enumerate() {
                draw_widgets(canvas, taffy, ids[i], child, fs, sc, local_mouse, focused_id, font_cache, cursor_visible);
            }
        }
        Widget::Container { child, color, radius, .. } => {
            if let Some(c) = color {
                let mut p = Paint::default(); p.set_color(*c); p.set_anti_alias(true);
                canvas.draw_rrect(RRect::new_rect_xy(SkiaRect::from_xywh(0.0, 0.0, size.0, size.1), *radius, *radius), &p);
            }
            let ids = taffy.children(node).unwrap();
            draw_widgets(canvas, taffy, ids[0], child, fs, sc, local_mouse, focused_id, font_cache, cursor_visible);
        }
        Widget::Button { text, color, .. } => {
            let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
            let mut p = Paint::default(); 
            p.set_color(if rect.contains(local_mouse) { *color } else { *color });
            p.set_anti_alias(true);
            canvas.draw_rrect(RRect::new_rect_xy(rect, 8.0, 8.0), &p);
            draw_text(canvas, text, (0.0,0.0).into(), size, SkiaColor::WHITE, 16.0, font_cache, true);
        }
        Widget::TextInput { editor, style: _, id, state, error_msg, label, is_password, .. } => {
            let is_focused = focused_id == Some(*id);
            let rect = SkiaRect::from_xywh(0.0, 0.0, size.0, size.1);
            
            // Fundo Branco
            let mut bg = Paint::default(); bg.set_color(SkiaColor::WHITE); bg.set_anti_alias(true);
            canvas.draw_rrect(RRect::new_rect_xy(rect, 6.0, 6.0), &bg);

            // Borda
            let mut border = Paint::default(); border.set_style(skia_safe::paint::Style::Stroke); border.set_anti_alias(true);
            let b_col = match state {
                InputState::Error => SkiaColor::from_rgb(255, 80, 80),
                InputState::Success => SkiaColor::from_rgb(80, 200, 80),
                _ if is_focused => SkiaColor::from_rgb(103, 80, 164),
                _ => SkiaColor::from_rgb(200, 200, 200),
            };
            border.set_color(b_col); border.set_stroke_width(if is_focused { 2.0 } else { 1.0 });
            canvas.draw_rrect(RRect::new_rect_xy(rect, 6.0, 6.0), &border);

            // Label
            let font_label = get_cached_font(font_cache, "sans-serif", 12.0);
            let mut p_lbl = Paint::default(); p_lbl.set_color(b_col);
            canvas.draw_str(label, (5.0, -5.0), &font_label, &p_lbl);

            // CORREÇÃO: Single-line com scroll horizontal
            editor.with_buffer_mut(|buf| {
                buf.set_size(fs, Some(10000.0), Some(size.1)); // Largura "infinita" para single-line
                buf.shape_until_scroll(fs, true);
            });
            
            canvas.save();
            canvas.translate((10.0, 10.0));
            
            // CLIPAGEM: Limitar desenho ao tamanho do input
            canvas.clip_rect(SkiaRect::from_xywh(0.0, 0.0, size.0 - 20.0, size.1), None, true);
            
            // Calcular scroll horizontal
            let cursor = editor.cursor();
            let cursor_line = cursor.line;
            let cursor_index = cursor.index;
            
            let mut cursor_x = 0.0;
            editor.with_buffer(|buf| {
                for run in buf.layout_runs() {
                    if run.line_i == cursor_line {
                        for glyph in run.glyphs.iter() {
                            if glyph.start >= cursor_index {
                                cursor_x = glyph.x;
                                break;
                            }
                            cursor_x = glyph.x + glyph.w;
                        }
                        break;
                    }
                }
            });
            
            // Scroll para manter cursor visível
            let visible_width = size.0 - 20.0;
            let scroll_offset = if cursor_x > visible_width {
                -(cursor_x - visible_width + 20.0)
            } else {
                0.0
            };
            
            canvas.translate((scroll_offset, 0.0));
            
            // Desenhar Cursor (se focado e visível)
            if is_focused && cursor_visible {
                let mut p = Paint::default();
                p.set_color(SkiaColor::from_rgb(103, 80, 164));
                // Cursor com altura da fonte (16px)
                canvas.draw_rect(SkiaRect::from_xywh(cursor_x, 2.0, 2.0, 16.0), &p);
            }

            // Desenhar texto
            editor.with_buffer(|buf| {
                for run in buf.layout_runs() {
                    let font = get_cached_font(font_cache, "sans-serif", 16.0);
                    let mut p = Paint::default(); 
                    p.set_color(SkiaColor::BLACK);
                    p.set_anti_alias(true);
                    
                    if *is_password {
                        let char_count = run.text.chars().count();
                        let masked = "•".repeat(char_count);
                        canvas.draw_str(&masked, (0.0, run.line_y), &font, &p);
                    } else {
                        canvas.draw_str(run.text, (0.0, run.line_y), &font, &p);
                    }
                }
            });
            
            canvas.restore();
            canvas.restore();
            
            // Mensagem de erro
            if let Some(msg) = error_msg {
                let font_error = get_cached_font(font_cache, "sans-serif", 11.0);
                let mut p = Paint::default(); p.set_color(SkiaColor::from_rgb(255, 80, 80));
                canvas.draw_str(msg, (5.0, size.1 + 12.0), &font_error, &p);
            }
        }
        Widget::Text { content, color, size: font_size, .. } => {
             draw_text(canvas, content, (0.0,0.0).into(), size, *color, *font_size, font_cache, false);
        }
    }
    canvas.restore();
}

fn draw_text(
    canvas: &Canvas, 
    text: &str, 
    _pos: Point, 
    size: (f32, f32), 
    color: SkiaColor, 
    font_size: f32, 
    font_cache: &mut HashMap<(String, u32), Font>,
    center: bool
) {
    let font = get_cached_font(font_cache, "sans-serif", font_size);
    let mut p = Paint::default(); 
    p.set_color(color); 
    p.set_anti_alias(true);
    
    let y = (size.1 / 2.0) + (font_size / 3.0);
    
    if center {
        let text_width = font.measure_str(text, Some(&p)).0;
        let x = (size.0 - text_width) / 2.0;
        canvas.draw_str(text, (x, y), &font, &p);
    } else {
        canvas.draw_str(text, (0.0, y), &font, &p);
    }
}

enum HitResult<Msg> { Message(Msg), InputFocus(u64) }

fn hit_test<Msg: Clone>(widget: &Widget<Msg>, taffy: &TaffyTree<RutterContext>, node_id: NodeId, mouse: Point, abs: Point) -> Option<HitResult<Msg>> {
    let layout = taffy.layout(node_id).unwrap();
    let abs_pos = Point::new(abs.x + layout.location.x, abs.y + layout.location.y);
    let size = layout.size;
    let rect = SkiaRect::from_xywh(abs_pos.x, abs_pos.y, size.width, size.height);
    if !rect.contains(mouse) { return None; }

    match widget {
        Widget::Button { on_press, .. } => Some(HitResult::Message(on_press.clone())),
        Widget::TextInput { id, .. } => Some(HitResult::InputFocus(*id)),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            let ids = taffy.children(node_id).unwrap();
            for (i, child) in children.iter().enumerate().rev() {
                if let Some(res) = hit_test(child, taffy, ids[i], mouse, abs_pos) { return Some(res); }
            }
            None
        }
        Widget::Container { child, .. } => {
            let ids = taffy.children(node_id).unwrap();
            hit_test(child.as_ref(), taffy, ids[0], mouse, abs_pos)
        }
        _ => None
    }
}

fn collect_input_ids<Msg>(widget: &Widget<Msg>, ids: &mut Vec<u64>) {
    match widget {
        Widget::TextInput { id, .. } => ids.push(*id),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children { collect_input_ids(child, ids); }
        }
        Widget::Container { child, .. } => collect_input_ids(child, ids),
        _ => {}
    }
}

fn find_input_mut<'a, Msg>(widget: &'a mut Widget<'a, Msg>, target_id: u64) -> Option<(&'a mut Editor<'static>, fn(String) -> Msg, Option<Msg>)> where Msg: Clone {
    match widget {
        Widget::TextInput { editor, id, on_change, on_submit, .. } if *id == target_id => Some((editor, *on_change, on_submit.clone())),
        Widget::Column { children, .. } | Widget::Row { children, .. } => {
            for child in children { if let Some(r) = find_input_mut(child, target_id) { return Some(r); } }
            None
        }
        Widget::Container { child, .. } => find_input_mut(child, target_id),
        _ => None,
    }
}