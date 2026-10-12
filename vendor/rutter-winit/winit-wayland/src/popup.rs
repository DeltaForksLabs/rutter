// Modified for the local rutter-winit fork: popup placement and layer-parent lifecycle.
// See RUTTER_FORK.md in the fork root for provenance and validation.

use core::sync::atomic::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use dpi::{
    LogicalPosition, LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size,
};
use rwh_06::RawWindowHandle;
use sctk::compositor::SurfaceData;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::XdgPositioner;
use sctk::shell::xdg::popup::Popup as SctkPopup;
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::{Ime, WindowEvent};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAnchor, WindowAttributes, WindowButtons,
    WindowConstraintAdjustment, WindowGravity, WindowId, WindowLevel, WindowPositioner,
};

use super::ActiveEventLoop;
use super::output::MonitorHandle;
use crate::WindowAttributesWayland;
use crate::window::Handles;
use crate::window::handles::WindowRequests;
use crate::window::state::{WindowState, WindowType};

#[derive(Debug)]
pub struct Popup {
    /// The state of the popup.
    /// Retain the backing surface for renderer cleanup even after the event
    /// loop retires the role. `terminal` revokes all frontend access.
    popup_state: Arc<Mutex<WindowState>>,

    /// Window id.
    window_id: WindowId,

    /// The wayland display used solely for raw window handle.
    #[allow(dead_code)]
    display: WlDisplay,

    handles: Handles,
}

impl Popup {
    fn state(&self) -> Option<Arc<Mutex<WindowState>>> {
        if self.handles.window_requests.is_terminal() {
            return None;
        }
        Some(self.popup_state.clone())
    }
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        fn error(message: &'static str) -> RequestError {
            RequestError::NotSupported(NotSupportedError::new(message))
        }

        let parent_window_handle =
            attributes.parent_window().ok_or(error("Popup without a parent is not supported!"))?;
        if let RawWindowHandle::Wayland(parent_window_handle) = parent_window_handle {
            let queue_handle = event_loop_window_target.queue_handle.clone();
            let mut state = event_loop_window_target.state.borrow_mut();
            let monitors = state.monitors.clone();
            let xdg_activation = state
                .xdg_activation
                .as_ref()
                .map(|activation_state| activation_state.global().clone());
            let xdg_positioner = XdgPositioner::new(&state.xdg_shell)
                .map_err(|_| error("Failed to create positioner"))?;
            let parent_window_id =
                WindowId::from_raw(parent_window_handle.surface.as_ptr() as usize);
            if state
                .window_requests
                .borrow()
                .get(&parent_window_id)
                .is_some_and(|requests| requests.is_terminal())
            {
                return Err(error("Popup parent native lifetime has retired"));
            }
            let (popup, popup_state) = if let Some(parent_window_state) =
                state.windows.borrow().get(&parent_window_id)
            {
                let requested_positioner = initial_popup_positioner(&attributes);
                let WindowPositioner {
                    anchor,
                    anchor_rect,
                    offset: positioner_offset,
                    gravity,
                    constraint_adjustment,
                    ..
                } = requested_positioner;
                let grab_keyboard = attributes.active;

                let mut parent_window_state = parent_window_state.lock().unwrap();

                // Use the scale factor and xdg geometry of the parent.
                let scale_factor = parent_window_state.scale_factor();
                let size: LogicalSize<i32> = attributes
                    .surface_size
                    .ok_or(error("Invalid size for popup"))?
                    .to_logical(scale_factor);
                if size.width == 0_i32 || size.height == 0_i32 {
                    return Err(error("The popups size must not be zero"));
                }

                // Anchoring
                // The anchor rect is relative to the parent window geometry, so we need to subtract
                // the geometry origin from the position to get the correct anchor rect.
                // This is important for client side decorations
                let geometry_origin = parent_window_state.content_surface_origin();
                let anchor_position = LogicalPosition::new(-geometry_origin.x, -geometry_origin.y);
                if xdg_positioner.version() >= 3 {
                    xdg_positioner.set_reactive();
                }
                xdg_positioner.set_anchor(from_anchor(anchor));
                xdg_positioner.set_gravity(from_gravity(gravity));
                xdg_positioner
                    .set_constraint_adjustment(from_constraint_adjustment(constraint_adjustment));

                let anchor_size = anchor_rect.1.to_logical::<i32>(scale_factor);
                let anchor_rect_position = anchor_rect.0.to_logical::<i32>(scale_factor);
                let anchor_rect_size =
                    LogicalSize::new(anchor_size.width.max(1), anchor_size.height.max(1));

                let anchor_rect = (
                    LogicalPosition::new(
                        anchor_rect_position.x + anchor_position.x,
                        anchor_rect_position.y + anchor_position.y,
                    ),
                    anchor_rect_size,
                );
                xdg_positioner.set_anchor_rect(
                    anchor_rect.0.x,
                    anchor_rect.0.y,
                    anchor_rect.1.width,
                    anchor_rect.1.height,
                );
                let offset: LogicalPosition<i32> = positioner_offset.to_logical(scale_factor);
                xdg_positioner.set_offset(offset.x, offset.y);
                xdg_positioner.set_size(size.width, size.height);

                let parent_surface = parent_window_state.window.xdg_surface();
                let surface = state.compositor_state.create_surface(&queue_handle);
                let popup = SctkPopup::from_surface(
                    parent_surface,
                    &xdg_positioner,
                    &queue_handle,
                    surface.clone(),
                    &state.xdg_shell,
                )
                .map_err(|_| error("Failed to create popup"))?;
                // Layer parents have no xdg_surface. Associate the null-parent
                // xdg_popup before its first commit (and before any grab).
                if let WindowType::Layer { layer, .. } = &parent_window_state.window {
                    layer.get_popup(popup.xdg_popup());
                }
                parent_window_state.add_child(super::make_wid(popup.wl_surface()));
                drop(parent_window_state);

                let mut popup_state = WindowState::new(
                    event_loop_window_target,
                    &state,
                    size.into(),
                    WindowType::Popup {
                        popup: popup.clone(),
                        xdg_positioner,
                        last_configure: None,
                        parent_origin: geometry_origin,
                        positioner: WindowPositioner::new(
                            anchor,
                            (anchor_rect_position.into(), anchor_rect_size.into()),
                            positioner_offset,
                            gravity,
                            constraint_adjustment,
                        ),
                    },
                    attributes.preferred_theme,
                    false,
                    scale_factor,
                    Some(parent_window_id),
                );

                // Set transparency hint.
                popup_state.set_transparent(attributes.transparent);

                // Set blur.
                let _ = popup_state.set_blur(attributes.blur);

                let WindowAttributesWayland { activation_token, .. } = *attributes
                    .platform
                    .take()
                    .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
                    .unwrap_or_default();

                // Activate the window when the token is passed.
                if let (Some(xdg_activation), Some(token)) =
                    (xdg_activation.as_ref(), activation_token)
                {
                    xdg_activation.activate(token.into_raw(), &surface);
                }

                // Request a keyboard grab so the compositor routes key events to
                // this popup rather than the parent window. Must happen before the
                // first commit that maps the surface.
                if grab_keyboard {
                    // Use the seat with the most recent event
                    let grab = state
                        .seat_state
                        .seats()
                        .filter_map(|seat| {
                            let serial = state.seats.get(&seat.id())?.latest_serial()?;
                            Some((seat, serial))
                        })
                        .max_by_key(|(_, serial)| *serial);

                    if let Some((seat, serial)) = grab {
                        popup.xdg_popup().grab(&seat, serial);
                    }
                }

                popup.wl_surface().commit();
                // popup.commit(); Trait not implemented in Sctk

                let popup_state = Arc::new(Mutex::new(popup_state));

                (popup, popup_state)
            } else {
                return Err(error("Parent window id unknown"));
            };

            let window_id = super::make_wid(popup.wl_surface());
            state.windows.get_mut().insert(window_id, popup_state.clone());

            let window_requests = WindowRequests {
                terminal: AtomicBool::new(false),
                redraw_requested: AtomicBool::new(true),
                closed: AtomicBool::new(false),
            };
            let window_requests = Arc::new(window_requests);
            state.window_requests.get_mut().insert(window_id, window_requests.clone());

            // Setup the event sync to insert `WindowEvents` right from the window.
            let window_events_sink = state.window_events_sink.clone();

            let mut wayland_source = event_loop_window_target.wayland_dispatcher.as_source_mut();
            let event_queue = wayland_source.queue();
            let configure_result = (|| {
                event_queue
                    .roundtrip(&mut state)
                    .map_err(|err| RequestError::from(os_error!(err)))?;
                loop {
                    if window_requests.is_terminal() {
                        return Err(error("Popup native lifetime retired before configure"));
                    }
                    if state
                        .window_compositor_updates
                        .iter()
                        .any(|u| u.window_id == window_id && u.close_window)
                    {
                        return Err(error(
                            "Popup was dismissed by the compositor before configure",
                        ));
                    }
                    if popup_state.lock().unwrap().is_configured() {
                        return Ok(());
                    }
                    event_queue
                        .blocking_dispatch(&mut state)
                        .map_err(|err| RequestError::from(os_error!(err)))?;
                }
            })();
            if let Err(error) = configure_result {
                if let Some(parent) = state.windows.get_mut().get(&parent_window_id) {
                    parent.lock().unwrap().remove_child(&window_id);
                }
                state.windows.get_mut().remove(&window_id);
                state.window_requests.get_mut().remove(&window_id);
                state.window_compositor_updates.retain(|update| update.window_id != window_id);
                return Err(error);
            }

            // Wake-up event loop, so it'll send initial redraw requested.
            let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
            event_loop_awakener.ping();

            Ok(Self {
                popup_state,
                window_id,
                display: event_loop_window_target.handle.connection.display().clone(),
                handles: Handles {
                    queue_handle,
                    window_requests,
                    monitors,
                    event_loop_awakener,
                    window_events_sink,

                    xdg_activation,
                    attention_requested: Arc::new(AtomicBool::new(false)),

                    compositor: state.compositor_state.clone(),
                },
            })
        } else {
            Err(RequestError::NotSupported(NotSupportedError::new(
                "A Popup requires a parent wayland window handle",
            )))
        }
    }
}

impl Popup {
    fn update_positioner(&self, new_positioner: WindowPositioner, requested_size: Option<Size>) {
        let Some(state) = self.state() else {
            return;
        };
        let mut state = state.lock().unwrap();
        let scale_factor = state.scale_factor();
        if let WindowType::Popup { popup, xdg_positioner, parent_origin, positioner, .. } =
            &mut state.window
        {
            *positioner = new_positioner;
            if let Some(size) = requested_size {
                let size: LogicalSize<i32> = size.to_logical(scale_factor);
                xdg_positioner.set_size(size.width.max(1), size.height.max(1));
            }
            xdg_positioner.set_anchor(from_anchor(new_positioner.anchor));
            xdg_positioner.set_gravity(from_gravity(new_positioner.gravity));
            xdg_positioner.set_constraint_adjustment(from_constraint_adjustment(
                new_positioner.constraint_adjustment,
            ));
            let (position, size) = new_positioner.anchor_rect;
            let size: LogicalSize<i32> = size.to_logical(scale_factor);
            let position: LogicalPosition<i32> = position.to_logical(scale_factor);
            xdg_positioner.set_anchor_rect(
                position.x - parent_origin.x,
                position.y - parent_origin.y,
                size.width.max(1),
                size.height.max(1),
            );
            let offset: LogicalPosition<i32> = new_positioner.offset.to_logical(scale_factor);
            xdg_positioner.set_offset(offset.x, offset.y);
            if popup.xdg_popup().version() >= 3 {
                popup.reposition(xdg_positioner, 0);
            }
        }
    }
}

impl CoreWindow for Popup {
    fn window_type(&self) -> winit_core::window::WindowType {
        winit_core::window::WindowType::Popup
    }

    fn positioner(&self) -> WindowPositioner {
        let Some(state) = self.state() else { return WindowPositioner::default() };
        if let WindowType::Popup { positioner, .. } = &state.lock().unwrap().window {
            *positioner
        } else {
            WindowPositioner::default()
        }
    }

    fn set_positioner(&self, new_positioner: WindowPositioner) {
        self.update_positioner(new_positioner, None);
    }

    fn request_popup_layout(
        &self,
        positioner: WindowPositioner,
        size: Size,
    ) -> Option<PhysicalSize<u32>> {
        self.update_positioner(positioner, Some(size));
        self.request_redraw();
        None
    }
    fn id(&self) -> WindowId {
        self.window_id
    }

    fn request_redraw(&self) {
        self.handles.request_redraw();
    }

    #[inline]
    fn title(&self) -> String {
        let Some(s) = self.state() else { return String::new() };
        s.lock().unwrap().title().to_owned()
    }

    fn pre_present_notify(&self) {
        let Some(s) = self.state() else { return };
        s.lock().unwrap().request_frame_callback();
    }

    fn reset_dead_keys(&self) {
        winit_common::xkb::reset_dead_keys()
    }

    fn surface_position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        let s =
            self.state().ok_or_else(|| NotSupportedError::new("the popup has been destroyed"))?;
        let state = s.lock().unwrap();
        if let WindowType::Popup { last_configure: Some(configure), parent_origin, .. } =
            &state.window
        {
            let (x, y) = configure.position;
            // xdg_popup.configure is relative to the parent's xdg geometry;
            // This legacy API reports parent-relative content coordinates in
            // popup-scale physical units, not parent-buffer physical units.
            // popup_surface_geometry exposes the logical configure directly.
            return Ok(LogicalPosition::new(x + parent_origin.x, y + parent_origin.y)
                .to_physical(state.scale_factor()));
        }
        Err(NotSupportedError::new("the popup has not been configured yet").into())
    }

    fn popup_surface_geometry(
        &self,
        _parent_scale_factor: f64,
    ) -> Result<winit_core::window::PopupSurfaceGeometry, RequestError> {
        let state =
            self.state().ok_or_else(|| NotSupportedError::new("the popup has been destroyed"))?;
        let state = state.lock().unwrap();
        if let WindowType::Popup { last_configure: Some(configure), parent_origin, .. } =
            &state.window
        {
            return Ok(configured_popup_geometry(
                configure.position,
                *parent_origin,
                state.surface_size(),
                state.scale_factor(),
                configure.serial,
            ));
        }
        Err(NotSupportedError::new("the popup has not been configured yet").into())
    }

    fn set_outer_position(&self, position: Position) {
        let mut positioner = self.positioner();
        positioner.anchor = WindowAnchor::TopLeft;
        positioner.gravity = WindowGravity::BottomRight;
        positioner.anchor_rect = (position, PhysicalSize::new(1, 1).into());
        positioner.offset = PhysicalPosition::new(0, 0).into();
        positioner.constraint_adjustment = WindowConstraintAdjustment::empty();
        self.set_positioner(positioner);
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        let Some(s) = self.state() else { return PhysicalSize::default() };
        let popup_state = s.lock().unwrap();
        let scale_factor = popup_state.scale_factor();
        super::logical_to_physical_rounded(popup_state.surface_size(), scale_factor)
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let s = self.state()?;
        let mut popup_state = s.lock().unwrap();
        let new_size = popup_state.request_surface_size(size);
        self.request_redraw();
        Some(new_size)
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        let Some(s) = self.state() else { return PhysicalSize::default() };
        let popup_state = s.lock().unwrap();
        let scale_factor = popup_state.scale_factor();
        super::logical_to_physical_rounded(popup_state.outer_size(), scale_factor)
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        let Some(s) = self.state() else { return };
        s.lock().unwrap().set_min_surface_size(min_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    /// Set the maximum surface size for the window.
    #[inline]
    fn set_max_surface_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        let Some(s) = self.state() else { return };
        s.lock().unwrap().set_max_surface_size(max_size);
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        let s = self.state()?;
        let popup_state = s.lock().unwrap();
        let scale_factor = popup_state.scale_factor();
        popup_state
            .resize_increments()
            .map(|size| super::logical_to_physical_rounded(size, scale_factor))
    }

    fn set_surface_resize_increments(&self, increments: Option<Size>) {
        let Some(s) = self.state() else { return };
        let mut popup_state = s.lock().unwrap();
        let scale_factor = popup_state.scale_factor();
        let increments = increments.map(|size| size.to_logical(scale_factor));
        popup_state.set_resize_increments(increments);
    }

    fn set_title(&self, title: &str) {
        let Some(s) = self.state() else { return };
        s.lock().unwrap().set_title(title.to_owned());
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        let Some(s) = self.state() else { return };
        s.lock().unwrap().set_transparent(transparent);
    }

    fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, _resizable: bool) {
        // A popup cannot be resized with the mouse
    }

    fn is_resizable(&self) -> bool {
        // A popup cannot be resized with the mouse
        false
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
    }

    fn enabled_buttons(&self) -> WindowButtons {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
        WindowButtons::all()
    }

    fn set_minimized(&self, _minimized: bool) {
        // Not possible for popups
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, _maximized: bool) {
        // Not possible for popups
    }

    fn is_maximized(&self) -> bool {
        // Not possible for popups
        false
    }

    fn set_fullscreen(&self, _fullscreen: Option<Fullscreen>) {
        // Not possible for popups
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        let Some(s) = self.state() else { return 1.0 };
        s.lock().unwrap().scale_factor()
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        let Some(s) = self.state() else { return };
        if s.lock().unwrap().set_blur(blur) {
            self.request_redraw();
        }
    }

    #[inline]
    fn set_decorations(&self, _decorate: bool) {
        // Popup does not support decorations
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        // Popup does not support decorations
        false
    }

    fn set_window_level(&self, _level: WindowLevel) {
        // Popup does not have a window level
    }

    fn set_window_icon(&self, _window_icon: Option<winit_core::icon::Icon>) {
        // Popup does not have a window icon
    }

    #[inline]
    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let Some(s) = self.state() else { return Ok(()) };
        let state_changed = s.lock().unwrap().request_ime_update(request)?;

        if let Some(allowed) = state_changed {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.handles.push_window_event(event, self.window_id);
        }

        Ok(())
    }

    #[inline]
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        let s = self.state()?;
        s.lock().unwrap().ime_allowed()
    }

    fn focus_window(&self) {}

    fn has_focus(&self) -> bool {
        let Some(s) = self.state() else { return false };
        s.lock().unwrap().has_focus()
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        if let Some(state) = self.state() {
            let state = state.lock().unwrap();
            let surface = state.window.wl_surface();
            self.handles.request_user_attention(surface, request_type);
        }
    }

    fn set_theme(&self, _theme: Option<Theme>) {
        // A popup does not have a frame
    }

    fn theme(&self) -> Option<Theme> {
        // A popup does not have a frame
        None
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn set_cursor(&self, cursor: Cursor) {
        let Some(s) = self.state() else { return };
        let mut popup_state = s.lock().unwrap();
        match cursor {
            Cursor::Icon(icon) => popup_state.set_cursor(icon),
            Cursor::Custom(cursor) => popup_state.set_custom_cursor(cursor),
        }
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        let Some(s) = self.state() else { return Err(RequestError::Ignored) };
        let scale_factor = s.lock().unwrap().scale_factor();
        let position = position.to_logical(scale_factor);
        s.lock()
            .unwrap()
            .set_cursor_position(position)
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        let Some(s) = self.state() else { return Err(RequestError::Ignored) };
        s.lock().unwrap().set_cursor_grab(mode)
    }

    fn set_cursor_visible(&self, visible: bool) {
        let Some(s) = self.state() else { return };
        s.lock().unwrap().set_cursor_visible(visible);
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        // Popup does not support dragging
        Err(RequestError::Ignored)
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), RequestError> {
        // Popup does not support dragging
        Err(RequestError::Ignored)
    }

    fn show_window_menu(&self, _position: Position) {
        // A popup does not have a menu
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        let Some(state) = self.state() else {
            return Err(RequestError::Ignored);
        };

        self.handles.set_cursor_hittest(state.lock().unwrap().window.wl_surface(), hittest)
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        let state = self.state()?;
        let state = state.lock().unwrap();
        let data = state.window.wl_surface().data::<SurfaceData<()>>()?;
        data.outputs()
            .next()
            .map(MonitorHandle::new)
            .map(|monitor| CoreMonitorHandle(Arc::new(monitor)))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        self.handles.available_monitors()
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        // NOTE: There's no such concept on Wayland.
        None
    }

    /// Get the raw-window-handle v0.6 display handle.
    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    /// Get the raw-window-handle v0.6 window handle.
    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        self.handles.window_requests.closed.store(true, Ordering::Relaxed);
        self.handles.event_loop_awakener.ping();
    }
}

impl rwh_06::HasWindowHandle for Popup {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let state = self.state().ok_or(rwh_06::HandleError::Unavailable)?;
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = state.lock().unwrap().window.wl_surface().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl rwh_06::HasDisplayHandle for Popup {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        if self.state().is_none() {
            return Err(rwh_06::HandleError::Unavailable);
        };
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}

fn configured_popup_geometry(
    position: (i32, i32),
    parent_origin: LogicalPosition<i32>,
    size: LogicalSize<u32>,
    popup_scale: f64,
    serial: u32,
) -> winit_core::window::PopupSurfaceGeometry {
    winit_core::window::PopupSurfaceGeometry {
        parent_position: LogicalPosition::new(
            f64::from(position.0 + parent_origin.x),
            f64::from(position.1 + parent_origin.y),
        ),
        surface_size: super::logical_to_physical_rounded(size, popup_scale),
        scale_factor: popup_scale,
        configure_serial: Some(serial),
    }
}

fn initial_popup_positioner(attributes: &WindowAttributes) -> WindowPositioner {
    let mut positioner = attributes.positioner.unwrap_or_default();
    if attributes.positioner.is_none()
        && let Some(position) = attributes.position
    {
        // `with_position` specifies the popup's top-left in parent coordinates;
        // the default center gravity would shift it by half the popup size.
        positioner.anchor = WindowAnchor::TopLeft;
        positioner.gravity = WindowGravity::BottomRight;
        positioner.anchor_rect = (position, PhysicalSize::new(1, 1).into());
    }
    positioner
}

fn from_gravity(
    gravity: WindowGravity,
) -> wayland_protocols::xdg::shell::client::xdg_positioner::Gravity {
    use wayland_protocols::xdg::shell::client::xdg_positioner::Gravity;
    match gravity {
        WindowGravity::Center => Gravity::None,
        WindowGravity::Top => Gravity::Top,
        WindowGravity::Bottom => Gravity::Bottom,
        WindowGravity::Left => Gravity::Left,
        WindowGravity::Right => Gravity::Right,
        WindowGravity::TopLeft => Gravity::TopLeft,
        WindowGravity::BottomLeft => Gravity::BottomLeft,
        WindowGravity::TopRight => Gravity::TopRight,
        WindowGravity::BottomRight => Gravity::BottomRight,
        _ => Gravity::None,
    }
}

fn from_anchor(
    value: WindowAnchor,
) -> wayland_protocols::xdg::shell::client::xdg_positioner::Anchor {
    use wayland_protocols::xdg::shell::client::xdg_positioner::Anchor;
    match value {
        WindowAnchor::Center => Anchor::None,
        WindowAnchor::Top => Anchor::Top,
        WindowAnchor::Bottom => Anchor::Bottom,
        WindowAnchor::Left => Anchor::Left,
        WindowAnchor::Right => Anchor::Right,
        WindowAnchor::TopLeft => Anchor::TopLeft,
        WindowAnchor::BottomLeft => Anchor::BottomLeft,
        WindowAnchor::TopRight => Anchor::TopRight,
        WindowAnchor::BottomRight => Anchor::BottomRight,
        _ => Anchor::None,
    }
}

fn from_constraint_adjustment(
    value: WindowConstraintAdjustment,
) -> wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment {
    use wayland_protocols::xdg::shell::client::xdg_positioner::ConstraintAdjustment;

    const _: () = {
        assert!(WindowConstraintAdjustment::SLIDE_X.bits() == ConstraintAdjustment::SlideX.bits());
        assert!(WindowConstraintAdjustment::SLIDE_Y.bits() == ConstraintAdjustment::SlideY.bits());
        assert!(WindowConstraintAdjustment::FLIP_X.bits() == ConstraintAdjustment::FlipX.bits());
        assert!(WindowConstraintAdjustment::FLIP_Y.bits() == ConstraintAdjustment::FlipY.bits());
        assert!(
            WindowConstraintAdjustment::RESIZE_X.bits() == ConstraintAdjustment::ResizeX.bits()
        );
        assert!(
            WindowConstraintAdjustment::RESIZE_Y.bits() == ConstraintAdjustment::ResizeY.bits()
        );
    };

    ConstraintAdjustment::from_bits_retain(value.bits())
}

#[cfg(test)]
mod positioner_tests {
    use super::*;

    #[test]
    fn popup_snapshot_keeps_parent_logical_configure_separate_from_popup_buffer_scale() {
        for popup_scale in [1.0, 1.5, 2.0] {
            let snapshot = configured_popup_geometry(
                (-100, 30),
                LogicalPosition::new(8, 24),
                LogicalSize::new(160, 72),
                popup_scale,
                42,
            );
            assert_eq!(snapshot.parent_position, LogicalPosition::new(-92.0, 54.0));
            assert_eq!(snapshot.surface_size, LogicalSize::new(160, 72).to_physical(popup_scale));
            assert_eq!(snapshot.scale_factor, popup_scale);
            assert_eq!(snapshot.configure_serial, Some(42));
        }
    }

    #[test]
    fn implicit_popup_position_is_anchored_at_its_top_left_corner() {
        let attributes = WindowAttributes::default().with_position(PhysicalPosition::new(230, 44));
        let positioner = initial_popup_positioner(&attributes);
        assert_eq!(positioner.anchor, WindowAnchor::TopLeft);
        assert_eq!(positioner.gravity, WindowGravity::BottomRight);
        assert_eq!(
            positioner.anchor_rect.0.to_physical::<i32>(1.0),
            PhysicalPosition::new(230, 44)
        );
    }

    #[test]
    fn explicit_positioner_is_preserved() {
        let centered = WindowPositioner::default();
        let attributes = WindowAttributes::default()
            .with_position(PhysicalPosition::new(230, 44))
            .with_positioner(centered);
        assert_eq!(initial_popup_positioner(&attributes), centered);
    }
}
