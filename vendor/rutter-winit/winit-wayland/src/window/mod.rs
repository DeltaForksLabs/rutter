//! The Wayland window.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use dpi::{LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use rwh_06::RawWindowHandle;
use sctk::compositor::SurfaceData;
use sctk::reexports::client::Proxy;
use sctk::reexports::client::protocol::wl_display::WlDisplay;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::shell::xdg::window::{Window as SctkWindow, WindowDecorations};
use tracing::warn;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::{Ime, WindowEvent};
use winit_core::event_loop::AsyncRequestSerial;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImeCapabilities, ImeRequest, ImeRequestError, ResizeDirection, Theme,
    UserAttentionType, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use super::ActiveEventLoop;
use super::output::MonitorHandle;
use super::types::xdg_activation::XdgActivationTokenData;
use crate::window::state::WindowType;
use crate::{WindowAttributesWayland, output};
pub(crate) mod state;
pub use state::WindowState;
pub(crate) mod handles;
pub use handles::Handles;
use handles::WindowRequests;
mod lifetime;
mod retirement;
#[cfg(test)]
mod retirement_tests;

/// The Wayland window.
#[derive(Debug)]
pub struct Window {
    panel_role: bool,
    /// Reference to the underlying SCTK window.
    window: Option<SctkWindow>,
    surface: WlSurface,

    /// Window id.
    window_id: WindowId,

    /// The state of the window.
    window_state: Arc<Mutex<WindowState>>,

    /// The wayland display used solely for raw window handle.
    #[allow(dead_code)]
    display: WlDisplay,

    /// Common handles like queue, window requests, monitors and so on
    handles: Handles,
}

impl Window {
    pub(crate) fn new(
        event_loop_window_target: &ActiveEventLoop,
        mut attributes: WindowAttributes,
    ) -> Result<Self, RequestError> {
        let queue_handle = event_loop_window_target.queue_handle.clone();
        let mut state = event_loop_window_target.state.borrow_mut();

        let monitors = state.monitors.clone();

        let surface = state.compositor_state.create_surface(&queue_handle);
        let compositor = state.compositor_state.clone();
        let xdg_activation =
            state.xdg_activation.as_ref().map(|activation_state| activation_state.global().clone());
        let display = event_loop_window_target.handle.connection.display();

        let size: Size = if let Some(config) = &attributes.layer_shell {
            let (width, height) = config.logical_size();
            LogicalSize::new(width.max(1), height.max(1)).into()
        } else {
            attributes.surface_size.unwrap_or(LogicalSize::new(800., 600.).into())
        };

        // We prefer server side decorations, however to not have decorations we ask for client
        // side decorations instead.
        let default_decorations = if attributes.decorations {
            WindowDecorations::RequestServer
        } else {
            WindowDecorations::RequestClient
        };

        let (window, role) = if let Some(config) = &attributes.layer_shell {
            let role =
                crate::layer_shell::create_layer(&state, &queue_handle, surface.clone(), config)?;
            (None, role)
        } else {
            let window =
                state.xdg_shell.create_window(surface.clone(), default_decorations, &queue_handle);
            (Some(window.clone()), state::WindowType::Window { window, last_configure: None })
        };

        let WindowAttributesWayland { name: app_name, activation_token, prefer_csd, .. } =
            *attributes
                .platform
                .take()
                .and_then(|p| p.cast::<WindowAttributesWayland>().ok())
                .unwrap_or_default();

        let mut scale_factor = None;
        if let Some(RawWindowHandle::Wayland(handle)) = attributes.parent_window() {
            if let Some(s) =
                state.windows.borrow().get(&WindowId::from_raw(handle.surface.as_ptr() as usize))
            {
                scale_factor = Some(s.lock().unwrap().scale_factor());
            }
        }
        let scale_factor = scale_factor.unwrap_or(1.0);

        let mut window_state = WindowState::new(
            event_loop_window_target,
            &state,
            size,
            role,
            attributes.preferred_theme,
            prefer_csd,
            scale_factor,
            None,
        );

        window_state.set_window_icon(attributes.window_icon);

        // Set transparency hint.
        window_state.set_transparent(attributes.transparent);

        // Set blur.
        let _ = window_state.set_blur(attributes.blur);

        // Set the decorations hint.
        window_state.set_decorate(attributes.decorations);

        // Set the app_id.
        if let (Some(window), Some(name)) = (&window, app_name.map(|name| name.general)) {
            window.set_app_id(name);
        }

        // Set the window title.
        window_state.set_title(attributes.title);

        // Set the min and max sizes. We must set the hints upon creating a window, so
        // we use the default `1.` scaling...
        let min_size = attributes.min_surface_size.map(|size| size.to_logical(1.));
        let max_size = attributes.max_surface_size.map(|size| size.to_logical(1.));
        window_state.set_min_surface_size(min_size);
        window_state.set_max_surface_size(max_size);

        // Non-resizable implies that the min and max sizes are set to the same value.
        window_state.set_resizable(attributes.resizable);

        // Set startup mode.
        match attributes.fullscreen {
            Some(Fullscreen::Exclusive(..)) => {
                warn!("`Fullscreen::Exclusive` is ignored on Wayland");
            },
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                if let Some(window) = &window {
                    window.set_fullscreen(output)
                }
            },
            _ if attributes.maximized => {
                if let Some(window) = &window {
                    window.set_maximized()
                }
            },
            _ => (),
        };

        match attributes.cursor {
            Cursor::Icon(icon) => window_state.set_cursor(icon),
            Cursor::Custom(cursor) => window_state.set_custom_cursor(cursor),
        }

        // Apply resize increments.
        if let Some(increments) = attributes.surface_resize_increments {
            let increments = increments.to_logical(window_state.scale_factor());
            window_state.set_resize_increments(Some(increments));
        }

        // Activate the window when the token is passed.
        if let (Some(xdg_activation), Some(token)) = (xdg_activation.as_ref(), activation_token) {
            xdg_activation.activate(token.into_raw(), &surface);
        }

        // XXX Do initial commit.
        surface.commit();

        // Add the window and window requests into the state.
        let window_state = Arc::new(Mutex::new(window_state));
        let window_id = super::make_wid(&surface);
        state.windows.get_mut().insert(window_id, window_state.clone());

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

        // Do a roundtrip.
        let configure_result = (|| {
            event_queue.roundtrip(&mut state).map_err(|err| RequestError::from(os_error!(err)))?;
            loop {
                if window_requests.is_terminal() {
                    return Err(
                        NotSupportedError::new("surface retired before initial configure").into()
                    );
                }
                if state
                    .window_compositor_updates
                    .iter()
                    .any(|update| update.window_id == window_id && update.close_window)
                {
                    return Err(
                        NotSupportedError::new("surface closed before initial configure").into()
                    );
                }
                if window_state.lock().unwrap().is_configured() {
                    return Ok(());
                }
                event_queue
                    .blocking_dispatch(&mut state)
                    .map_err(|err| RequestError::from(os_error!(err)))?;
            }
        })();
        if let Err(error) = configure_result {
            state.windows.get_mut().remove(&window_id);
            state.window_requests.get_mut().remove(&window_id);
            state.window_compositor_updates.retain(|update| update.window_id != window_id);
            return Err(error);
        }

        // Wake-up event loop, so it'll send initial redraw requested.
        let event_loop_awakener = event_loop_window_target.event_loop_awakener.clone();
        event_loop_awakener.ping();

        Ok(Self {
            panel_role: attributes.panel.is_some(),
            window,
            surface,
            display,

            window_id,
            window_state,

            handles: Handles {
                queue_handle,
                window_requests,
                monitors,
                event_loop_awakener,
                window_events_sink,

                compositor,

                xdg_activation,
                attention_requested: Arc::new(AtomicBool::new(false)),
            },
        })
    }

    pub(crate) fn xdg_toplevel(&self) -> Option<NonNull<c_void>> {
        NonNull::new(self.window.as_ref()?.xdg_toplevel().id().as_ptr().cast())
    }
}

impl Window {
    pub fn request_activation_token(&self) -> Result<AsyncRequestSerial, RequestError> {
        if !self.native_available() {
            return Err(RequestError::Ignored);
        }
        let xdg_activation = match self.handles.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => return Err(NotSupportedError::new("xdg_activation_v1 is not available").into()),
        };

        let serial = AsyncRequestSerial::get();

        let data = XdgActivationTokenData::Obtain((self.window_id, serial));
        let xdg_activation_token =
            xdg_activation.get_activation_token(&self.handles.queue_handle, data);
        xdg_activation_token.set_surface(self.surface());
        xdg_activation_token.commit();

        Ok(serial)
    }

    #[inline]
    /// The backing Wayland surface. After terminal layer closure this is retained
    /// only for renderer cleanup; do not attach, commit or render through it.
    /// Use `window_handle()` to check availability of a renderable native lifetime.
    pub fn surface(&self) -> &WlSurface {
        // After terminal closure this is a cleanup-only backing allocation.
        // Do not commit/render through it; window_handle() is unavailable.
        &self.surface
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.handles.window_requests.closed.store(true, Ordering::Relaxed);
        self.handles.event_loop_awakener.ping();
    }
}

impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        if !self.native_available() {
            return Err(rwh_06::HandleError::Unavailable);
        }
        let raw = rwh_06::WaylandWindowHandle::new({
            let ptr = self.surface.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_surface will never be null")
        });

        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw.into())) }
    }
}

impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        if !self.native_available() {
            return Err(rwh_06::HandleError::Unavailable);
        }
        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.display.id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_proxy should never be null")
        });

        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw.into())) }
    }
}

impl CoreWindow for Window {
    fn surface_logical_size(&self) -> LogicalSize<f64> {
        self.with_state(LogicalSize::default(), |state| state.surface_size().cast())
    }
    fn layer_shell_version(&self) -> Option<u32> {
        self.with_state(None, |state| {
            if let WindowType::Layer { layer, .. } = &state.window {
                if let sctk::shell::wlr_layer::SurfaceKind::Wlr(role) = layer.kind() {
                    return Some(role.version());
                }
            }
            None
        })
    }
    fn layer_output_name(&self) -> Option<String> {
        if !self.native_available() {
            return None;
        }
        let data = self.surface.data::<SurfaceData<()>>()?;
        let mut outputs = data.outputs();
        let output = outputs.next()?;
        if outputs.next().is_some() {
            return None;
        }
        let data = output.data::<sctk::output::OutputData>()?;
        data.with_output_info(|info| info.name.clone())
    }
    fn window_type(&self) -> winit_core::window::WindowType {
        if self.panel_role {
            winit_core::window::WindowType::Panel
        } else if self.window.is_none() {
            winit_core::window::WindowType::LayerShell
        } else {
            winit_core::window::WindowType::Window
        }
    }

    fn id(&self) -> WindowId {
        self.window_id
    }

    fn request_redraw(&self) {
        self.handles.request_redraw();
    }

    #[inline]
    fn title(&self) -> String {
        self.with_state(String::new(), |state| state.title().to_owned())
    }

    fn pre_present_notify(&self) {
        self.with_state((), WindowState::request_frame_callback);
    }

    fn reset_dead_keys(&self) {
        winit_common::xkb::reset_dead_keys()
    }

    fn surface_position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        Err(NotSupportedError::new("window position information is not available on Wayland")
            .into())
    }

    fn set_outer_position(&self, _position: Position) {
        // Not possible.
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        self.with_state(PhysicalSize::default(), |state| {
            super::logical_to_physical_rounded(state.surface_size(), state.scale_factor())
        })
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let new_size = self.with_state(None, |state| {
            if matches!(state.window, WindowType::Layer { .. }) {
                return None;
            }
            Some(state.request_surface_size(size))
        })?;
        self.request_redraw();
        Some(new_size)
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        self.with_state(PhysicalSize::default(), |state| {
            super::logical_to_physical_rounded(state.outer_size(), state.scale_factor())
        })
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let min_size = min_size.map(|size| size.to_logical(scale_factor));
        self.with_state((), |state| state.set_min_surface_size(min_size));
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    /// Set the maximum surface size for the window.
    #[inline]
    fn set_max_surface_size(&self, max_size: Option<Size>) {
        let scale_factor = self.scale_factor();
        let max_size = max_size.map(|size| size.to_logical(scale_factor));
        self.with_state((), |state| state.set_max_surface_size(max_size));
        // NOTE: Requires commit to be applied.
        self.request_redraw();
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.with_state(None, |state| {
            state
                .resize_increments()
                .map(|size| super::logical_to_physical_rounded(size, state.scale_factor()))
        })
    }

    fn set_surface_resize_increments(&self, increments: Option<Size>) {
        self.with_state((), |state| {
            let increments = increments.map(|size| size.to_logical(state.scale_factor()));
            state.set_resize_increments(increments);
        });
    }

    fn set_title(&self, title: &str) {
        let new_title = title.to_string();
        self.with_state((), |state| state.set_title(new_title));
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        self.with_state((), |state| state.set_transparent(transparent));
    }

    fn set_visible(&self, _visible: bool) {
        // Not possible on Wayland.
    }

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, resizable: bool) {
        if self.with_state(false, |state| state.set_resizable(resizable)) {
            // NOTE: Requires commit to be applied.
            self.request_redraw();
        }
    }

    fn is_resizable(&self) -> bool {
        self.with_state(false, |state| state.resizable())
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
    }

    fn enabled_buttons(&self) -> WindowButtons {
        // TODO(kchibisov) v5 of the xdg_shell allows that.
        WindowButtons::all()
    }

    fn set_minimized(&self, minimized: bool) {
        // You can't unminimize the window on Wayland.
        if !minimized {
            warn!("Unminimizing is ignored on Wayland.");
            return;
        }

        if let Some(window) = &self.window {
            window.set_minimized();
        }
    }

    fn is_minimized(&self) -> Option<bool> {
        // XXX clients don't know whether they are minimized or not.
        None
    }

    fn set_maximized(&self, maximized: bool) {
        if let Some(window) = &self.window {
            if maximized { window.set_maximized() } else { window.unset_maximized() }
        }
    }

    fn is_maximized(&self) -> bool {
        self.with_state(false, |state| match &state.window {
            WindowType::Window { last_configure, .. } => {
                last_configure.as_ref().is_some_and(|configure| configure.is_maximized())
            },
            _ => false,
        })
    }

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        match fullscreen {
            Some(Fullscreen::Borderless(monitor)) => {
                let output = monitor.as_ref().and_then(|monitor| {
                    monitor.cast_ref::<output::MonitorHandle>().map(|handle| &handle.proxy)
                });

                if let Some(window) = &self.window {
                    window.set_fullscreen(output)
                }
            },
            Some(_) => {
                warn!("this fullscreen mode is ignored on Wayland");
            },
            None => {
                if let Some(window) = &self.window {
                    window.unset_fullscreen()
                }
            },
        }
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        let is_fullscreen = self.with_state(false, |state| match &state.window {
            WindowType::Window { last_configure, .. } => {
                last_configure.as_ref().is_some_and(|configure| configure.is_fullscreen())
            },
            _ => false,
        });

        if is_fullscreen {
            let current_monitor = self.current_monitor();
            Some(Fullscreen::Borderless(current_monitor))
        } else {
            None
        }
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        self.with_state(1.0, |state| state.scale_factor())
    }

    #[inline]
    fn set_blur(&self, blur: bool) {
        if self.with_state(false, |state| state.set_blur(blur)) {
            self.request_redraw();
        }
    }

    #[inline]
    fn set_decorations(&self, decorate: bool) {
        self.with_state((), |state| state.set_decorate(decorate))
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        self.with_state(false, WindowState::is_decorated)
    }

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_window_icon(&self, window_icon: Option<winit_core::icon::Icon>) {
        self.with_state((), |state| state.set_window_icon(window_icon))
    }

    #[inline]
    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let unavailable = if matches!(request, ImeRequest::Disable) {
            Ok(None)
        } else {
            Err(ImeRequestError::NotSupported)
        };
        let state_changed =
            self.with_state(unavailable, |state| state.request_ime_update(request))?;

        if let Some(allowed) = state_changed {
            let event = WindowEvent::Ime(if allowed { Ime::Enabled } else { Ime::Disabled });
            self.handles.push_window_event(event, self.window_id);
        }

        Ok(())
    }

    #[inline]
    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        self.with_state(None, |state| state.ime_allowed())
    }

    fn focus_window(&self) {}

    fn has_focus(&self) -> bool {
        self.with_state(false, |state| state.has_focus())
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        if !self.native_available() {
            return;
        }
        self.handles.request_user_attention(self.surface(), request_type);
    }

    fn set_theme(&self, theme: Option<Theme>) {
        self.with_state((), |state| state.set_theme(theme))
    }

    fn theme(&self) -> Option<Theme> {
        self.with_state(None, |state| state.theme())
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn set_cursor(&self, cursor: Cursor) {
        self.with_state((), |state| match cursor {
            Cursor::Icon(icon) => state.set_cursor(icon),
            Cursor::Custom(cursor) => state.set_custom_cursor(cursor),
        });
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        self.with_state(Err(RequestError::Ignored), |state| state.set_cursor_position(position))
            // Request redraw on success, since the state is double buffered.
            .map(|_| self.request_redraw())
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        self.with_state(Err(RequestError::Ignored), |state| state.set_cursor_grab(mode))
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.with_state((), |state| state.set_cursor_visible(visible));
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        self.with_state(Err(RequestError::Ignored), |state| state.drag_window())
    }

    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError> {
        self.with_state(Err(RequestError::Ignored), |state| state.drag_resize_window(direction))
    }

    fn show_window_menu(&self, position: Position) {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        self.with_state((), |state| state.show_window_menu(position));
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        if !self.native_available() {
            return Err(RequestError::Ignored);
        }
        self.handles.set_cursor_hittest(self.surface(), hittest)
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        if !self.native_available() {
            return None;
        }
        let data = self.surface.data::<SurfaceData<()>>()?;
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
