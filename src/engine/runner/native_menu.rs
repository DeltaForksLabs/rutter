// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Native surfaces for menus owned by a parent runner, including multirunner
//! surfaces. Widget state and layout stay in the parent engine; only the menu's
//! pixels and pointer events move.

use std::time::{Duration, Instant};

use raw_window_handle::HasWindowHandle;
use skia_safe::{Point, Rect};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ButtonSource, ElementState, MouseButton, PointerSource, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{
    WindowAnchor, WindowAttributes, WindowGravity, WindowId, WindowPositioner, WindowType,
};

use super::dropdown_pointer::DropdownHoverUpdate;
use super::{RutterRunner, wheel_deltas};
use crate::app::AppLogic;
use crate::engine::dropdown_menu_runtime::MenuHoverOutcome;
use crate::engine::gpu::{GraphicsBackend, create_required_backend};
use crate::engine::run_error::RutterRunError;
use crate::engine::validate_runtime_reconstruction;
use crate::render::NativeMenuKind;
use crate::render::collect_open_context_menus;
use crate::render::dropdown_menu_overlay::hit_test_dropdown_menu_overlay;
use crate::render::hit_test::hit_test_context_menu_overlay_with_direction;
use crate::render::select_overlay::collector::collect_open_dropdown_overlays;
use crate::widgets::dropdown_menu::build_open_menu_surfaces;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MenuPlacement {
    kind: NativeMenuKind,
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

pub(super) struct NativeMenuSurface {
    pub(super) backend: Box<dyn GraphicsBackend>,
    pub(super) kind: NativeMenuKind,
    pub(super) ready: bool,
    configure_deadline: Instant,
    placement: MenuPlacement,
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

fn menu_surface_union(
    surfaces: &[crate::widgets::dropdown_menu::DropdownMenuSurface],
) -> Option<Rect> {
    let first = surfaces.first()?;
    Some(surfaces.iter().skip(1).fold(first.rect, |union, surface| {
        Rect::from_ltrb(
            union.left.min(surface.rect.left),
            union.top.min(surface.rect.top),
            union.right.max(surface.rect.right),
            union.bottom.max(surface.rect.bottom),
        )
    }))
}

impl NativeMenuSurface {
    pub(super) fn id(&self) -> WindowId {
        self.backend.window().id()
    }
}

fn physical_menu_rect(kind: NativeMenuKind, rect: Rect, scale: f32) -> Option<MenuPlacement> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let edges = [rect.left, rect.top, rect.right, rect.bottom];
    if edges.iter().any(|edge| !edge.is_finite()) {
        return None;
    }
    let left = (rect.left * scale).floor();
    let top = (rect.top * scale).floor();
    let right = (rect.right * scale).ceil();
    let bottom = (rect.bottom * scale).ceil();
    if left < i32::MIN as f32
        || top < i32::MIN as f32
        || right > i32::MAX as f32
        || bottom > i32::MAX as f32
        || right <= left
        || bottom <= top
    {
        return None;
    }
    Some(MenuPlacement {
        kind,
        position: PhysicalPosition::new(left as i32, top as i32),
        size: PhysicalSize::new((right - left) as u32, (bottom - top) as u32),
    })
}

fn popup_pointer_in_parent(
    local: PhysicalPosition<f64>,
    origin: PhysicalPosition<i32>,
) -> PhysicalPosition<f64> {
    PhysicalPosition::new(local.x + f64::from(origin.x), local.y + f64::from(origin.y))
}

fn menu_positioner(position: PhysicalPosition<i32>) -> WindowPositioner {
    WindowPositioner::new(
        WindowAnchor::TopLeft,
        (position.into(), PhysicalSize::new(1, 1).into()),
        PhysicalPosition::new(0, 0).into(),
        WindowGravity::BottomRight,
        // Geometry has already been clamped to the parent viewport. Additional
        // compositor flips/slides would detach it from its hit-test coordinates.
        Default::default(),
    )
}

fn native_menu_attributes(placement: MenuPlacement) -> WindowAttributes {
    WindowAttributes::default()
        .with_window_type(WindowType::Popup)
        .with_decorations(false)
        // A submenu chain uses its rectangular union. Pixels between its panels
        // must expose the parent, not cover it with an opaque popup background.
        .with_transparent(true)
        .with_active(true)
        .with_positioner(menu_positioner(placement.position))
        .with_surface_size(placement.size)
        .with_visible(true)
}

fn popup_fits_parent(
    actual_position: PhysicalPosition<i32>,
    actual_size: PhysicalSize<u32>,
    requested: MenuPlacement,
    parent_size: PhysicalSize<u32>,
    parent_scale: f32,
) -> bool {
    let allowed_adjustment = 16.0 * f64::from(parent_scale);
    let dx = (i64::from(actual_position.x) - i64::from(requested.position.x)).abs();
    let dy = (i64::from(actual_position.y) - i64::from(requested.position.y)).abs();
    actual_position.x >= 0
        && actual_position.y >= 0
        && i64::from(actual_position.x) + i64::from(actual_size.width)
            <= i64::from(parent_size.width)
        && i64::from(actual_position.y) + i64::from(actual_size.height)
            <= i64::from(parent_size.height)
        && actual_size.width >= requested.size.width
        && actual_size.height >= requested.size.height
        && (dx as f64) <= allowed_adjustment
        && (dy as f64) <= allowed_adjustment
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    fn requested_native_menu(&mut self) -> Result<Option<MenuPlacement>, RutterRunError> {
        let size = self.engine.window.as_ref().unwrap().surface_size();
        if size.width == 0 || size.height == 0 {
            return Ok(None);
        }
        self.engine.try_ensure_widget_states()?;
        self.engine.try_ensure_layout(size)?;
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
            let surfaces = crate::render::context_menu_overlay::context_surfaces(
                context,
                viewport,
                A::locale().direction(),
                &font,
            );
            let Some(rect) = menu_surface_union(&surfaces) else {
                return Ok(None);
            };
            return Ok(physical_menu_rect(
                NativeMenuKind::Context(context.id),
                rect,
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
            return Ok(None);
        };
        let bounds = Rect::from_xywh(0.0, 0.0, viewport.0, viewport.1);
        let surfaces = build_open_menu_surfaces(
            dropdown.anchor,
            dropdown.entries,
            &dropdown.state,
            bounds,
            A::locale().direction(),
        );
        let Some(rect) = menu_surface_union(&surfaces) else {
            return Ok(None);
        };
        Ok(physical_menu_rect(
            NativeMenuKind::Dropdown(dropdown.id),
            rect,
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
        if self.engine.window.is_none() || self.native_menus_unavailable {
            return Ok(());
        }
        if !self.engine.any_context_menu_open() && !self.any_dropdown_menu_open() {
            if self.native_menu.take().is_some() {
                self.redraw();
            }
            return Ok(());
        }
        let Some(placement) = self.requested_native_menu()? else {
            if self.native_menu.take().is_some() {
                self.redraw();
            }
            return Ok(());
        };
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
                window.set_positioner(menu_positioner(placement.position));
                if popup.placement.size != placement.size {
                    let actual_size = window
                        .request_surface_size(placement.size.into())
                        .unwrap_or_else(|| window.surface_size());
                    if actual_size.width > 0 && actual_size.height > 0 {
                        popup.backend.resize(actual_size)?;
                    }
                }
                popup.placement = placement;
                popup.ready = false;
                popup.configure_deadline = Instant::now() + POPUP_CONFIGURE_TIMEOUT;
                popup.backend.window().request_redraw();
                placement_changed = true;
            }
            self.native_menu_fits_parent();
            if placement_changed {
                self.redraw();
            }
            return Ok(());
        }
        self.create_native_menu(event_loop, placement);
        Ok(())
    }

    fn create_native_menu(&mut self, event_loop: &dyn ActiveEventLoop, placement: MenuPlacement) {
        let parent = self.engine.window.as_ref().unwrap();
        let required_backend = self.engine.backend_type().unwrap();
        let result = parent
            .window_handle()
            .map_err(|error| error.to_string())
            .and_then(|handle| {
                // The parent remains alive until the popup backend is dropped. Winit requires
                // this invariant for its unsafe parent raw handle on native platforms.
                let attributes = unsafe {
                    native_menu_attributes(placement).with_parent_window(Some(handle.as_raw()))
                };
                create_required_backend(event_loop, attributes, required_backend)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(mut backend) => {
                let configured_size = backend.window().surface_size();
                // Surface-size requests may configure asynchronously. Keep the
                // parent's overlay visible until the popup can fit the menu.
                let size = if configured_size == placement.size {
                    configured_size
                } else {
                    backend
                        .window()
                        .request_surface_size(placement.size.into())
                        .unwrap_or_else(|| backend.window().surface_size())
                };
                if let Err(error) = backend.resize(size) {
                    self.disable_native_menus(error.to_string());
                    return;
                }
                self.native_menu = Some(NativeMenuSurface {
                    backend,
                    kind: placement.kind,
                    ready: false,
                    configure_deadline: Instant::now() + POPUP_CONFIGURE_TIMEOUT,
                    placement,
                });
                self.native_menu_fits_parent();
                self.redraw();
            }
            Err(error) => self.disable_native_menus(error),
        }
    }

    fn disable_native_menus(&mut self, reason: String) {
        eprintln!("rutter: native menu unavailable; using in-window overlay: {reason}");
        self.native_menu = None;
        self.native_menus_unavailable = true;
        self.redraw();
    }

    fn native_menu_fits_parent(&mut self) -> bool {
        let Some(popup) = self.native_menu.as_ref() else {
            return false;
        };
        let window = popup.backend.window();
        let position = match window.outer_position() {
            Ok(position) => position,
            Err(error) => {
                self.disable_native_menus(format!("popup position is unknown: {error}"));
                return false;
            }
        };
        let parent_size = self.engine.window.as_ref().unwrap().surface_size();
        let size = window.surface_size();
        if size.width < popup.placement.size.width || size.height < popup.placement.size.height {
            if Instant::now() < popup.configure_deadline {
                return false;
            }
            self.disable_native_menus(
                "popup did not configure at the menu's requested size".to_owned(),
            );
            return false;
        }
        if popup_fits_parent(
            position,
            size,
            popup.placement,
            parent_size,
            self.engine.scale_factor,
        ) {
            if let Some(popup) = self.native_menu.as_mut()
                && !popup.ready
            {
                popup.ready = true;
                self.redraw();
            }
            return true;
        }
        self.disable_native_menus(
            "popup was shifted outside the parent viewport or constrained below the menu size"
                .to_owned(),
        );
        false
    }

    fn native_pointer_position(&mut self, position: PhysicalPosition<f64>) -> Option<Point> {
        if !self.native_menu_fits_parent() {
            return None;
        }
        let popup = self.native_menu.as_ref()?;
        // The compositor can slide a popup relative to the anchor. Input and
        // painting must both use the requested geometry, not mix its hit-test
        // coordinates with the compositor's final screen position.
        let parent_position = popup_pointer_in_parent(position, popup.placement.position);
        // Parent hover coverage must see the same pointer as popup hit testing,
        // especially when motion enters a transparent gap and repaints both.
        self.update_cursor_position(parent_position);
        Some(self.engine.last_mouse_pos)
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
                if self.native_pointer_position(position).is_none() {
                    return;
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
                let Some(cursor) = self.native_pointer_position(position) else {
                    return;
                };
                if let Err(error) = self.press_native_menu(cursor, button) {
                    self.terminate_for_error(event_loop, error);
                } else {
                    self.redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.native_menu.is_some() => {
                match self.refresh_dropdown_scroll_target() {
                    Ok(Some(target)) => {
                        self.scroll_open_dropdown(target, wheel_deltas(delta).1);
                        self.redraw();
                    }
                    Ok(None) => {}
                    Err(error) => self.terminate_for_error(event_loop, error),
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let Err(error) = self.refresh_layout_before_keyboard_input() {
                    self.terminate_for_error(event_loop, error);
                    return;
                }
                if self.handle_application_shortcut(&event) || self.handle_text_commit(&event) {
                    return;
                }
                self.handle_key(&event.logical_key, event.repeat);
            }
            WindowEvent::SurfaceResized(size) if size.width > 0 && size.height > 0 => {
                if let Some(popup) = self.native_menu.as_mut() {
                    if let Err(error) = popup.backend.resize(size) {
                        self.terminate_for_error(event_loop, error.into());
                    } else {
                        self.redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.native_menu_fits_parent();
                if self.native_menu.is_none() {
                    return;
                }
                let Some(popup) = self.native_menu.as_ref() else {
                    return;
                };
                let kind = popup.kind;
                let origin = popup.placement.position;
                let parent_origin = Point::new(
                    origin.x as f32 / self.engine.scale_factor,
                    origin.y as f32 / self.engine.scale_factor,
                );
                if let Some(popup) = self.native_menu.as_mut()
                    && let Err(error) = self.engine.try_redraw_native_menu(
                        popup.backend.as_mut(),
                        kind,
                        parent_origin,
                    )
                {
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
        let size = self.engine.window.as_ref().unwrap().surface_size();
        self.engine.try_ensure_widget_states()?;
        self.engine.try_ensure_layout(size)?;
        let viewport = (
            size.width as f32 / self.engine.scale_factor,
            size.height as f32 / self.engine.scale_factor,
        );
        let kind = self.native_menu.as_ref().unwrap().kind;
        let font_size = A::theme_for(&self.engine.app_state).font_body;
        let widget = A::view(&mut self.engine.app_state);
        validate_runtime_reconstruction(self.engine.widget_id_snapshot.as_ref(), &widget)?;
        match kind {
            NativeMenuKind::Dropdown(id) => {
                let overlays = collect_open_dropdown_overlays(
                    &widget,
                    &self.engine.taffy,
                    self.engine.last_root_node,
                    &self.engine.widget_states,
                    viewport,
                );
                let hit = hit_test_dropdown_menu_overlay(
                    &overlays,
                    cursor,
                    viewport,
                    A::locale().direction(),
                );
                drop(widget);
                if let Some(hit) = hit {
                    self.handle_dropdown_pointer_hit(hit, button);
                } else {
                    self.close_dropdown_menu(id, true);
                }
            }
            NativeMenuKind::Context(_) => {
                let hit = if button == MouseButton::Left {
                    let font = crate::render::text::get_cached_font(
                        &mut self.engine.font_cache,
                        "sans-serif",
                        font_size,
                    );
                    hit_test_context_menu_overlay_with_direction(
                        &widget,
                        cursor,
                        viewport,
                        &self.engine.widget_states,
                        &font,
                        A::locale().direction(),
                    )
                } else {
                    None
                };
                drop(widget);
                if let Some(hit) = hit {
                    self.handle_context_menu_hit(hit);
                } else {
                    self.engine.close_all_context_menus();
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::dropdown_menu::{DropdownMenuEntry, DropdownMenuState};

    fn context_hover_update(
        runtime: &crate::engine::DropdownMenuRuntime<u8>,
        state: &mut DropdownMenuState,
        previous: usize,
        next: usize,
    ) -> DropdownHoverUpdate {
        let hit = |index| {
            Some(
                crate::render::dropdown_menu_overlay::DropdownMenuOverlayHit::Entry {
                    id: 42,
                    path: vec![index],
                    kind: runtime.entry_kind(&[index]).unwrap(),
                    disabled: runtime.is_disabled(&[index]),
                },
            )
        };
        DropdownHoverUpdate {
            previous_hover: hit(previous),
            hover: hit(next),
            scroll_target: None,
            outcome: runtime.hover_entry(state, vec![next]),
        }
    }

    #[test]
    fn native_menu_settled_context_hover_skips_frames_and_local_highlight_is_popup_only() {
        let runtime = crate::engine::DropdownMenuRuntime::from_context_entries(&[
            crate::ContextMenuEntry::item("First", 1),
            crate::ContextMenuEntry::item("Second", 2),
        ]);
        let mut state = DropdownMenuState::default();
        state.open_at_index(Some(0));
        let kind = NativeMenuKind::Context(42);
        let same = context_hover_update(&runtime, &mut state, 0, 0);
        assert_eq!(
            native_hover_redraw(kind, true, false, &same),
            NativeHoverRedraw::None
        );
        let changed = context_hover_update(&runtime, &mut state, 0, 1);
        assert_eq!(
            native_hover_redraw(kind, true, false, &changed),
            NativeHoverRedraw::Popup
        );
        let settled = context_hover_update(&runtime, &mut state, 1, 1);
        assert_eq!(
            native_hover_redraw(kind, true, false, &settled),
            NativeHoverRedraw::None
        );
        assert_eq!(
            native_hover_redraw(kind, false, false, &settled),
            NativeHoverRedraw::Both
        );
        assert_eq!(
            native_hover_redraw(kind, true, true, &settled),
            NativeHoverRedraw::Both
        );
        assert_eq!(
            native_hover_redraw(NativeMenuKind::Dropdown(42), true, false, &settled),
            NativeHoverRedraw::Both
        );
    }

    #[test]
    fn native_menu_geometry_reveal_branch_and_lower_hover_transitions_redraw_both() {
        let runtime = crate::engine::DropdownMenuRuntime::from_context_entries(&[
            crate::ContextMenuEntry::item("First", 1),
            crate::ContextMenuEntry::submenu(
                "More",
                vec![crate::ContextMenuEntry::item("Child", 2)],
            ),
        ]);
        let kind = NativeMenuKind::Context(42);
        let mut state = DropdownMenuState::default();
        state.open_at_index(Some(0));
        state.scroll_level(0, 10.0, 100.0);
        let reveal = context_hover_update(&runtime, &mut state, 0, 0);
        assert_eq!(
            native_hover_redraw(kind, true, false, &reveal),
            NativeHoverRedraw::Both
        );
        let open = context_hover_update(&runtime, &mut state, 0, 1);
        assert_eq!(
            native_hover_redraw(kind, true, false, &open),
            NativeHoverRedraw::Both
        );
        let close = context_hover_update(&runtime, &mut state, 1, 0);
        assert_eq!(
            native_hover_redraw(kind, true, false, &close),
            NativeHoverRedraw::Both
        );
        let mut scrolled = context_hover_update(&runtime, &mut state, 0, 0);
        scrolled.outcome = MenuHoverOutcome::Changed {
            geometry_changed: false,
        };
        scrolled.scroll_target = Some(
            crate::render::dropdown_menu_overlay::DropdownMenuScrollTarget {
                id: 42,
                level: 0,
                current_scroll: 10.0,
                max_scroll: 100.0,
            },
        );
        assert_eq!(
            native_hover_redraw(kind, true, false, &scrolled),
            NativeHoverRedraw::Both
        );
        let mut entry = context_hover_update(&runtime, &mut state, 0, 0);
        entry.previous_hover = None;
        assert_eq!(
            native_hover_redraw(kind, true, false, &entry),
            NativeHoverRedraw::Both
        );
        entry.hover = None;
        assert_eq!(
            native_hover_redraw(kind, true, false, &entry),
            NativeHoverRedraw::Both
        );
    }

    #[test]
    fn native_popup_requires_alpha_presentation_for_gaps_between_panels() {
        let attributes = native_menu_attributes(MenuPlacement {
            kind: NativeMenuKind::Dropdown(7),
            position: PhysicalPosition::new(8, 44),
            size: PhysicalSize::new(316, 72),
        });
        assert!(attributes.transparent());
    }

    #[test]
    fn native_menu_rounds_outwards_and_translates_pointer_with_paint_origin() {
        let placement = physical_menu_rect(
            NativeMenuKind::Dropdown(4),
            Rect::from_xywh(10.25, -2.5, 20.5, 10.5),
            1.5,
        )
        .unwrap();
        assert_eq!(placement.position, PhysicalPosition::new(15, -4));
        assert_eq!(placement.size, PhysicalSize::new(32, 16));
        let parent_position = popup_pointer_in_parent(
            PhysicalPosition::new(9.0, 7.0),
            PhysicalPosition::new(17, -4),
        );
        assert_eq!(parent_position, PhysicalPosition::new(26.0, 3.0));
        assert_eq!(
            super::super::logical_cursor_position(parent_position, 1.5),
            Point::new(26.0 / 1.5, 2.0),
        );
        assert!(
            physical_menu_rect(
                NativeMenuKind::Context(1),
                Rect::from_xywh(0.0, 0.0, f32::NAN, 2.0),
                1.0
            )
            .is_none()
        );
    }

    #[test]
    fn compositor_adjustments_cannot_clip_a_native_dropdown() {
        let requested = MenuPlacement {
            kind: NativeMenuKind::Dropdown(7),
            position: PhysicalPosition::new(0, 44),
            size: PhysicalSize::new(316, 72),
        };
        let parent = PhysicalSize::new(400, 220);
        assert!(popup_fits_parent(
            PhysicalPosition::new(8, 44),
            requested.size,
            requested,
            parent,
            1.0,
        ));
        assert!(!popup_fits_parent(
            PhysicalPosition::new(-1, 44),
            requested.size,
            requested,
            parent,
            1.0,
        ));
        assert!(!popup_fits_parent(
            PhysicalPosition::new(40, 44),
            requested.size,
            requested,
            parent,
            1.0,
        ));
        assert!(!popup_fits_parent(
            PhysicalPosition::new(8, 44),
            PhysicalSize::new(150, 72),
            requested,
            parent,
            1.0,
        ));
    }

    #[test]
    fn dropdown_near_right_edge_clamps_before_native_placement() {
        let entries = [DropdownMenuEntry::item("Open", ())];
        let mut state = DropdownMenuState::default();
        state.open_at_index(Some(0));
        let viewport = Rect::from_xywh(0.0, 0.0, 400.0, 220.0);
        let surfaces = build_open_menu_surfaces(
            Rect::from_xywh(355.0, 20.0, 40.0, 40.0),
            &entries,
            &state,
            viewport,
            crate::i18n::LayoutDirection::Ltr,
        );
        let placement =
            physical_menu_rect(NativeMenuKind::Dropdown(7), surfaces[0].rect, 1.0).unwrap();
        assert_eq!(placement.position.x, 232);
        assert!(popup_fits_parent(
            placement.position,
            placement.size,
            placement,
            PhysicalSize::new(400, 220),
            1.0,
        ));
    }

    #[test]
    fn native_context_menu_union_uses_all_panels_and_same_pointer_origin() {
        let entries = [crate::ContextMenuEntry::submenu(
            "More",
            vec![crate::ContextMenuEntry::submenu(
                "Deeper",
                vec![crate::ContextMenuEntry::item("Leaf", ())],
            )],
        )];
        let mut state = DropdownMenuState::default();
        state.expand_submenu(vec![0, 0], None);
        let menu = crate::render::ContextMenuOverlay {
            id: 42,
            entries: &entries,
            anchor: Point::new(400.0, 100.0),
            state,
        };
        let font = crate::render::text::get_cached_font(
            &mut std::collections::HashMap::new(),
            "sans-serif",
            14.0,
        );
        for direction in [
            crate::i18n::LayoutDirection::Ltr,
            crate::i18n::LayoutDirection::Rtl,
        ] {
            let surfaces = crate::render::context_menu_overlay::context_surfaces(
                &menu,
                (900.0, 500.0),
                direction,
                &font,
            );
            assert_eq!(surfaces.len(), 3);
            let union = menu_surface_union(&surfaces).unwrap();
            assert!(union.width() > surfaces[0].rect.width());
            let placement = physical_menu_rect(NativeMenuKind::Context(42), union, 1.5).unwrap();
            for surface in surfaces {
                let center = surface.rect.center();
                let local = PhysicalPosition::new(
                    f64::from(center.x * 1.5) - f64::from(placement.position.x),
                    f64::from(center.y * 1.5) - f64::from(placement.position.y),
                );
                let parent = super::super::logical_cursor_position(
                    popup_pointer_in_parent(local, placement.position),
                    1.5,
                );
                assert!((parent.x - center.x).abs() < 0.001 && (parent.y - center.y).abs() < 0.001);
            }
        }
    }
}
