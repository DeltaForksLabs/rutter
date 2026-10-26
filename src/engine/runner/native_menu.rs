// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Native surfaces for menus owned by a parent runner, including multirunner
//! surfaces. Widget state and layout stay in the parent engine; only the menu's
//! pixels and pointer events move.

use std::time::{Duration, Instant};

use raw_window_handle::HasWindowHandle;
use skia_safe::{Contains, Point, Rect};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ButtonSource, ElementState, MouseButton, PointerSource, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{PopupSurfaceGeometry, WindowId};

use super::dropdown_pointer::DropdownHoverUpdate;
use super::{RutterRunner, wheel_deltas};
use crate::app::AppLogic;
use crate::engine::dropdown_menu_runtime::MenuHoverOutcome;
use crate::engine::gpu::{GraphicsBackend, create_required_backend, validate_native_popup_size};
use crate::engine::run_error::RutterRunError;
use crate::engine::validate_runtime_reconstruction;
use crate::render::NativeMenuKind;
use crate::render::collect_open_context_menus;
use crate::render::select_overlay::collector::collect_open_dropdown_overlays;
use crate::widgets::dropdown_menu::{
    DropdownMenuSurface, build_native_menu_surfaces, resolve_native_menu_surfaces,
};

mod geometry;
mod presentation;
mod reconciliation;
use geometry::*;

enum NativeMenuRequest {
    Closed,
    Menu(MenuPlacement, Vec<DropdownMenuSurface>),
    Unavailable(NativeMenuKind, String),
}

fn menu_request(
    kind: NativeMenuKind,
    surfaces: Vec<DropdownMenuSurface>,
    scale: f32,
) -> NativeMenuRequest {
    let Some(rect) = menu_surface_union(&surfaces) else {
        return NativeMenuRequest::Closed;
    };
    match physical_menu_rect(kind, rect, scale) {
        Ok(placement) => NativeMenuRequest::Menu(placement, surfaces),
        Err(reason) => NativeMenuRequest::Unavailable(kind, reason),
    }
}

pub(super) struct NativeMenuSurface {
    pub(super) backend: Box<dyn GraphicsBackend>,
    pub(super) kind: NativeMenuKind,
    pub(super) ready: bool,
    parent_layout_revision: u64,
    configure_deadline: Instant,
    placement: MenuPlacement,
    requested_origin: Point,
    requested_surfaces: Vec<DropdownMenuSurface>,
    awaiting_configure: bool,
    configure_serial_before_request: Option<u32>,
    configured: Option<PopupSurfaceGeometry>,
    allocated_size: PhysicalSize<u32>,
    last_pointer: Option<PhysicalPosition<f64>>,
}

const POPUP_CONFIGURE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq, Eq)]
enum NativeHoverRedraw {
    None,
    Popup,
    Both,
}

fn native_hover_redraw(
    kind: NativeMenuKind,
    ready: bool,
    parent_dirty: bool,
    update: &DropdownHoverUpdate,
) -> NativeHoverRedraw {
    let NativeMenuKind::Context(id) = kind else {
        // Dropdown navigation may move focus to an embedded input, so its
        // parent frame also carries focus and accessibility changes.
        return NativeHoverRedraw::Both;
    };
    if !ready
        || parent_dirty
        // Focusing a partially visible row can reveal it even when reveal was
        // already enabled. Keep scrolled-panel changes on the conservative path.
        || (update.outcome.changed()
            && update.scroll_target.is_some_and(|target| target.max_scroll > 0.0))
        || matches!(
            update.outcome,
            MenuHoverOutcome::Ignored
                | MenuHoverOutcome::Changed {
                    geometry_changed: true
                }
        )
    {
        return NativeHoverRedraw::Both;
    }
    let is_local_entry = |hit: &Option<
        crate::render::dropdown_menu_overlay::DropdownMenuOverlayHit,
    >| {
        matches!(hit, Some(crate::render::dropdown_menu_overlay::DropdownMenuOverlayHit::Entry { id: menu_id, .. }) if *menu_id == id)
    };
    // Restrict popup-only updates to motion between rows of this ready context
    // menu. Entering/leaving panels and transparent gaps remains conservative
    // because lower-layer hover coverage can change there.
    if !is_local_entry(&update.hover) || !is_local_entry(&update.previous_hover) {
        return NativeHoverRedraw::Both;
    }
    if update.outcome.changed() || update.hover != update.previous_hover {
        NativeHoverRedraw::Popup
    } else {
        NativeHoverRedraw::None
    }
}

impl NativeMenuSurface {
    pub(super) fn id(&self) -> WindowId {
        self.backend.window().id()
    }
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    // The caller has reconciled parent state/layout. Keep measurement separate
    // from the guard so it can also refresh an already-created popup's panels.
    fn requested_native_menu(&mut self) -> Result<NativeMenuRequest, RutterRunError> {
        let size = self.engine.window.as_ref().unwrap().surface_size();
        if size.width == 0 || size.height == 0 {
            return Ok(NativeMenuRequest::Closed);
        }
        let scale = self.engine.scale_factor;
        let viewport = (size.width as f32 / scale, size.height as f32 / scale);
        let theme = A::theme_for(&self.engine.app_state);
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        let mut contexts = Vec::new();
        collect_open_context_menus(
            &widget,
            &self.engine.widget_states,
            &mut Vec::new(),
            &mut contexts,
        );
        if let Some(context) = contexts.last() {
            let font = crate::render::text::get_cached_font(
                &mut self.engine.font_cache,
                "sans-serif",
                theme.font_body,
            );
            let surfaces = build_native_menu_surfaces(
                context.anchor,
                context.entries,
                &context.state,
                A::locale().direction(),
                &|entries| crate::widget::estimate_context_menu_width(entries, &font),
            );
            return Ok(menu_request(
                NativeMenuKind::Context(context.id),
                surfaces,
                scale,
            ));
        }
        let dropdowns = collect_open_dropdown_overlays(
            &widget,
            &self.engine.taffy,
            self.engine.last_root_node,
            &self.engine.widget_states,
            viewport,
        );
        let Some(dropdown) = dropdowns.last() else {
            return Ok(NativeMenuRequest::Closed);
        };
        let origin = Point::new(
            match A::locale().direction() {
                crate::i18n::LayoutDirection::Ltr => dropdown.anchor.left,
                crate::i18n::LayoutDirection::Rtl => dropdown.anchor.right,
            },
            dropdown.anchor.bottom + crate::widgets::dropdown_menu::ROOT_GAP,
        );
        let surfaces = build_native_menu_surfaces(
            origin,
            dropdown.entries,
            &dropdown.state,
            A::locale().direction(),
            &crate::widgets::dropdown_menu::estimate_level_width,
        );
        Ok(menu_request(
            NativeMenuKind::Dropdown(dropdown.id),
            surfaces,
            scale,
        ))
    }

    pub(crate) fn native_menu_window_id(&self) -> Option<WindowId> {
        self.native_menu.as_ref().map(NativeMenuSurface::id)
    }

    pub(crate) fn pending_native_menu_deadline(&self) -> Option<Instant> {
        self.native_menu
            .as_ref()
            .filter(|popup| !popup.ready)
            .map(|popup| popup.configure_deadline)
    }

    pub(crate) fn sync_native_menu(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
    ) -> Result<(), RutterRunError> {
        if self.engine.window.is_none() {
            return Ok(());
        }
        self.reconcile_native_menu_model()?;
        if !self.engine.any_context_menu_open() && !self.any_dropdown_menu_open() {
            self.engine.native_menu_layout = None;
            if self.native_menu.take().is_some() {
                self.redraw();
            }
            return Ok(());
        }
        let (placement, surfaces) = match self.requested_native_menu()? {
            NativeMenuRequest::Menu(placement, surfaces) => (placement, surfaces),
            NativeMenuRequest::Unavailable(kind, reason) => {
                self.disable_native_menus(kind, reason)?;
                return Ok(());
            }
            NativeMenuRequest::Closed => {
                self.engine.native_menu_layout = None;
                if self.native_menu.take().is_some() {
                    self.redraw();
                }
                return Ok(());
            }
        };
        if self.native_menus_unavailable {
            if !self.overlay_menu_usable(placement.kind)? {
                self.disable_native_menus(
                    placement.kind,
                    "native menus are unavailable on this surface".to_owned(),
                )?;
            }
            return Ok(());
        }
        if self
            .native_menu
            .as_ref()
            .is_some_and(|popup| popup.kind != placement.kind)
        {
            self.native_menu = None;
        }
        if let Some(popup) = self.native_menu.as_mut() {
            let mut placement_changed = false;
            if popup.placement != placement {
                let window = popup.backend.window();
                let before = match window
                    .popup_surface_geometry(self.engine.window.as_ref().unwrap().scale_factor())
                {
                    Ok(geometry) => geometry,
                    Err(error) => {
                        let kind = popup.kind;
                        self.disable_native_menus(
                            kind,
                            format!("read popup configure before reposition: {error}"),
                        )?;
                        return Ok(());
                    }
                };
                if let Err(reason) = configured_rect(before) {
                    let kind = popup.kind;
                    self.disable_native_menus(kind, reason)?;
                    return Ok(());
                }
                popup.configure_serial_before_request = before.configure_serial;
                let actual_size = window.request_popup_layout(
                    menu_positioner(
                        placement.position,
                        self.engine.window.as_ref().unwrap().surface_size(),
                        self.engine.scale_factor,
                    ),
                    placement
                        .size
                        .to_logical::<f64>(f64::from(self.engine.scale_factor))
                        .into(),
                );
                popup.awaiting_configure = actual_size.is_none();
                if let Some(actual_size) = actual_size
                    && let Err(reason) = validate_native_popup_size(actual_size)
                {
                    let kind = popup.kind;
                    self.disable_native_menus(kind, reason)?;
                    return Ok(());
                }
                popup.placement = placement;
                popup.ready = false;
                self.engine.native_menu_layout = None;
                popup.configure_deadline = Instant::now() + POPUP_CONFIGURE_TIMEOUT;
                popup.backend.window().request_redraw();
                placement_changed = true;
            }
            popup.requested_surfaces = surfaces;
            popup.requested_origin = Point::new(
                placement.position.x as f32 / self.engine.scale_factor,
                placement.position.y as f32 / self.engine.scale_factor,
            );
            self.resolve_native_menu()?;
            if placement_changed {
                self.redraw();
            }
            return Ok(());
        }
        self.create_native_menu(event_loop, placement, surfaces)
    }

    fn create_native_menu(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        placement: MenuPlacement,
        surfaces: Vec<DropdownMenuSurface>,
    ) -> Result<(), RutterRunError> {
        let parent = self.engine.window.as_ref().unwrap();
        let required_backend = self.engine.backend_type().unwrap();
        let result = parent
            .window_handle()
            .map_err(|error| error.to_string())
            .and_then(|handle| {
                // The parent remains alive until the popup backend is dropped. Winit requires
                // this invariant for its unsafe parent raw handle on native platforms.
                let attributes = unsafe {
                    native_menu_attributes(
                        placement,
                        parent.surface_size(),
                        self.engine.scale_factor,
                    )
                    .with_parent_window(Some(handle.as_raw()))
                };
                create_required_backend(event_loop, attributes, required_backend)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(backend) => {
                let configured_size = backend.window().surface_size();
                if let Err(reason) = validate_native_popup_size(configured_size) {
                    self.disable_native_menus(placement.kind, reason)?;
                    return Ok(());
                }
                self.native_menu = Some(NativeMenuSurface {
                    backend,
                    kind: placement.kind,
                    ready: false,
                    parent_layout_revision: self.engine.layout_revision,
                    configure_deadline: Instant::now() + POPUP_CONFIGURE_TIMEOUT,
                    placement,
                    requested_origin: Point::new(
                        placement.position.x as f32 / self.engine.scale_factor,
                        placement.position.y as f32 / self.engine.scale_factor,
                    ),
                    requested_surfaces: surfaces,
                    awaiting_configure: false,
                    configure_serial_before_request: None,
                    configured: None,
                    allocated_size: configured_size,
                    last_pointer: None,
                });
                self.resolve_native_menu()?;
                self.redraw();
            }
            Err(error) => self.disable_native_menus(placement.kind, error)?,
        }
        Ok(())
    }

    fn disable_native_menus(
        &mut self,
        kind: NativeMenuKind,
        reason: String,
    ) -> Result<(), RutterRunError> {
        let id = match kind {
            NativeMenuKind::Context(id) | NativeMenuKind::Dropdown(id) => id,
        };
        let overlay_usable = self.overlay_menu_usable(kind)?;
        eprintln!("rutter: native menu unavailable; overlay_usable={overlay_usable}: {reason}");
        self.native_menu = None;
        self.engine.native_menu_layout = None;
        self.native_menus_unavailable = true;
        if !overlay_usable {
            self.close_dropdown_menu(id, true);
        }
        if let Some(message) = A::native_menu_unavailable(
            &self.engine.app_state,
            crate::app::NativeMenuFailure {
                menu_id: id,
                reason,
                overlay_usable,
            },
        ) {
            A::update(
                &mut self.engine.app_state,
                message,
                &mut self.engine.clipboard,
            );
            self.invalidate_published_model();
        }
        self.redraw();
        Ok(())
    }

    fn native_pointer_position(
        &mut self,
        position: PhysicalPosition<f64>,
    ) -> Result<Option<Point>, RutterRunError> {
        if !self.reconcile_native_menu_before_input()? {
            return Ok(None);
        }
        let popup = self.native_menu.as_mut().unwrap();
        let snapshot = popup.configured.unwrap();
        popup.last_pointer = Some(position);
        // Parent hover coverage must see the same pointer as popup hit testing,
        // especially when motion enters a transparent gap and repaints both.
        self.engine.last_mouse_pos = popup_logical_pointer(position, snapshot);
        self.cursor_physical = PhysicalPosition::new(
            f64::from(self.engine.last_mouse_pos.x * self.engine.scale_factor),
            f64::from(self.engine.last_mouse_pos.y * self.engine.scale_factor),
        );
        self.cursor_pos = Point::new(self.cursor_physical.x as f32, self.cursor_physical.y as f32);
        Ok(Some(self.engine.last_mouse_pos))
    }

    pub(super) fn native_menu_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => self.engine.modifiers = modifiers,
            WindowEvent::CloseRequested => {
                match self.native_menu.as_ref().map(|popup| popup.kind) {
                    Some(NativeMenuKind::Dropdown(id)) => self.close_dropdown_menu(id, true),
                    Some(NativeMenuKind::Context(_)) => {
                        self.engine.close_all_context_menus();
                    }
                    None => return,
                }
                self.native_menu = None;
                self.engine.native_menu_layout = None;
                self.redraw();
            }
            WindowEvent::PointerMoved {
                position,
                source: PointerSource::Mouse,
                ..
            } => {
                let previous_point = self.engine.last_mouse_pos;
                let was_ready = self.native_menu.as_ref().is_some_and(|popup| popup.ready);
                let parent_dirty = self.engine.layout_dirty;
                match self.native_pointer_position(position) {
                    Ok(Some(_)) => {}
                    Ok(None) => return,
                    Err(error) => {
                        self.terminate_for_error(event_loop, error);
                        return;
                    }
                }
                match self.refresh_dropdown_hover_from(Some(previous_point)) {
                    Err(error) => self.terminate_for_error(event_loop, error),
                    Ok(update) => {
                        let Some(popup) = self.native_menu.as_ref() else {
                            return;
                        };
                        match native_hover_redraw(
                            popup.kind,
                            was_ready && popup.ready,
                            parent_dirty || self.engine.layout_dirty,
                            &update,
                        ) {
                            NativeHoverRedraw::None => {}
                            NativeHoverRedraw::Popup => popup.backend.window().request_redraw(),
                            NativeHoverRedraw::Both => self.redraw(),
                        }
                    }
                }
            }
            WindowEvent::PointerButton {
                button: ButtonSource::Mouse(button),
                state: ElementState::Pressed,
                position,
                ..
            } if matches!(button, MouseButton::Left | MouseButton::Right) => {
                let cursor = match self.native_pointer_position(position) {
                    Ok(Some(cursor)) => cursor,
                    Ok(None) => return,
                    Err(error) => {
                        self.terminate_for_error(event_loop, error);
                        return;
                    }
                };
                if let Err(error) = self.press_native_menu(cursor, button) {
                    self.terminate_for_error(event_loop, error);
                } else {
                    self.redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.native_menu.is_some() => {
                match self.reconcile_native_menu_before_input() {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(error) => {
                        self.terminate_for_error(event_loop, error);
                        return;
                    }
                }
                if let Some(position) = self
                    .native_menu
                    .as_ref()
                    .and_then(|popup| popup.last_pointer)
                {
                    match self.native_pointer_position(position) {
                        Ok(Some(_)) => {}
                        Ok(None) => return,
                        Err(error) => {
                            self.terminate_for_error(event_loop, error);
                            return;
                        }
                    }
                }
                match self.refresh_dropdown_scroll_target() {
                    Ok(Some(target)) => {
                        let scale = self
                            .native_menu
                            .as_ref()
                            .and_then(|popup| popup.configured.as_ref())
                            .map_or(self.engine.scale_factor, |geometry| {
                                geometry.scale_factor as f32
                            });
                        self.scroll_open_dropdown(target, wheel_deltas(delta, scale).1);
                        self.redraw();
                    }
                    Ok(None) => {}
                    Err(error) => self.terminate_for_error(event_loop, error),
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match self.reconcile_native_menu_before_input() {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(error) => {
                        self.terminate_for_error(event_loop, error);
                        return;
                    }
                }
                if let Err(error) = self.refresh_layout_before_keyboard_input() {
                    self.terminate_for_error(event_loop, error);
                    return;
                }
                if self.handle_application_shortcut(&event) || self.handle_text_commit(&event) {
                    return;
                }
                self.handle_key(&event.logical_key, event.repeat);
            }
            WindowEvent::SurfaceResized(size) => {
                if let Some(popup) = self.native_menu.as_mut() {
                    if let Err(reason) = validate_native_popup_size(size) {
                        let kind = popup.kind;
                        if let Err(error) = self.disable_native_menus(kind, reason) {
                            self.terminate_for_error(event_loop, error);
                        }
                    } else {
                        if popup.configure_serial_before_request.is_none() {
                            popup.awaiting_configure = false;
                        }
                        if let Err(error) = self.resolve_native_menu() {
                            self.terminate_for_error(event_loop, error);
                            return;
                        }
                        self.redraw();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Moved(_) => {
                if let Err(error) = self.resolve_native_menu() {
                    self.terminate_for_error(event_loop, error);
                }
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                match self.resolve_native_menu() {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(error) => {
                        self.terminate_for_error(event_loop, error);
                        return;
                    }
                }
                if let Err(error) = self.present_native_menu_frame() {
                    self.terminate_for_error(event_loop, error);
                }
            }
            _ => {}
        }
    }

    fn press_native_menu(
        &mut self,
        cursor: Point,
        button: MouseButton,
    ) -> Result<(), RutterRunError> {
        let Some(kind) = self.native_menu.as_ref().map(|popup| popup.kind) else {
            return Ok(());
        };
        let id = match kind {
            NativeMenuKind::Context(id) | NativeMenuKind::Dropdown(id) => id,
        };
        let (hit, scroll) = self.native_cursor_targets(cursor)?;
        if self
            .native_menu
            .as_ref()
            .is_none_or(|popup| popup.kind != kind)
        {
            return Ok(());
        }
        if let Some(hit) = hit {
            self.handle_dropdown_pointer_hit(hit, button);
        } else if button == MouseButton::Right || scroll.is_none() {
            self.close_dropdown_menu(id, true);
        }
        Ok(())
    }

    pub(super) fn native_cursor_targets(
        &mut self,
        point: Point,
    ) -> Result<
        (
            Option<crate::render::dropdown_menu_overlay::DropdownMenuOverlayHit>,
            Option<crate::render::dropdown_menu_overlay::DropdownMenuScrollTarget>,
        ),
        RutterRunError,
    > {
        use crate::render::dropdown_menu_overlay::DropdownMenuScrollTarget;
        if !self.reconcile_native_menu_before_input()? {
            return Ok((None, None));
        }
        let Some((kind, surfaces)) = self.engine.native_menu_layout.as_ref() else {
            return Ok((None, None));
        };
        let id = match kind {
            NativeMenuKind::Context(id) | NativeMenuKind::Dropdown(id) => *id,
        };
        let Some(surface) = surfaces
            .iter()
            .rev()
            .find(|surface| surface.rect.contains(point))
        else {
            return Ok((None, None));
        };
        let scroll = Some(DropdownMenuScrollTarget {
            id,
            level: surface.level_path.len(),
            current_scroll: surface.scroll_y,
            max_scroll: surface.max_scroll,
        });
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        let hit = match kind {
            NativeMenuKind::Dropdown(_) => {
                let size = self
                    .engine
                    .window
                    .as_ref()
                    .unwrap()
                    .surface_size()
                    .to_logical::<f32>(f64::from(self.engine.scale_factor));
                let overlays = collect_open_dropdown_overlays(
                    &widget,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    &self.engine.widget_states,
                    (size.width, size.height),
                );
                overlays.iter().find(|menu| menu.id == id).and_then(|menu| {
                    crate::render::dropdown_menu_overlay::entry_hit(menu, surface, point)
                })
            }
            NativeMenuKind::Context(_) => {
                let mut menus = Vec::new();
                collect_open_context_menus(
                    &widget,
                    &self.engine.widget_states,
                    &mut Vec::new(),
                    &mut menus,
                );
                menus.iter().find(|menu| menu.id == id).and_then(|menu| {
                    crate::render::context_menu_overlay::context_entry_hit(menu, surface, point)
                })
            }
        };
        Ok((hit, scroll))
    }

    fn overlay_menu_usable(&mut self, kind: NativeMenuKind) -> Result<bool, RutterRunError> {
        self.reconcile_native_menu_model()?;
        let size = self
            .engine
            .window
            .as_ref()
            .unwrap()
            .surface_size()
            .to_logical::<f32>(f64::from(self.engine.scale_factor));
        let viewport = (size.width, size.height);
        let font = crate::render::text::get_cached_font(
            &mut self.engine.font_cache,
            "sans-serif",
            A::theme_for(&self.engine.app_state).font_body,
        );
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        let surfaces = match kind {
            NativeMenuKind::Context(id) => {
                let mut menus = Vec::new();
                collect_open_context_menus(
                    &widget,
                    &self.engine.widget_states,
                    &mut Vec::new(),
                    &mut menus,
                );
                menus.iter().find(|menu| menu.id == id).map(|menu| {
                    crate::render::context_menu_overlay::context_surfaces(
                        menu,
                        viewport,
                        A::locale().direction(),
                        &font,
                    )
                })
            }
            NativeMenuKind::Dropdown(id) => {
                let menus = collect_open_dropdown_overlays(
                    &widget,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    &self.engine.widget_states,
                    viewport,
                );
                menus.iter().find(|menu| menu.id == id).map(|menu| {
                    crate::widgets::dropdown_menu::build_open_menu_surfaces(
                        menu.anchor,
                        menu.entries,
                        &menu.state,
                        Rect::from_xywh(0.0, 0.0, viewport.0, viewport.1),
                        A::locale().direction(),
                    )
                })
            }
        };
        Ok(surfaces.is_some_and(|surfaces| {
            !surfaces.is_empty()
                && surfaces.iter().all(|surface| {
                    usable_menu_area(
                        PhysicalSize::new(
                            surface.rect.width().floor() as u32,
                            surface.rect.height().floor() as u32,
                        ),
                        1.0,
                        kind,
                    )
                })
        }))
    }
}

#[cfg(test)]
#[path = "native_menu/tests.rs"]
mod tests;

#[cfg(test)]
mod reconciliation_tests;
