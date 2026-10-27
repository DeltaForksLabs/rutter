//! In-memory native windows and raster presentation shared by engine regressions.

use std::cell::Cell;
use std::rc::Rc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use skia_safe::{Canvas, Surface};
use winit::cursor::{Cursor, CustomCursor, CustomCursorSource};
use winit::dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use winit::error::RequestError;
use winit::event_loop::{
    ActiveEventLoop, ControlFlow, DeviceEvents, EventLoopProxy, OwnedDisplayHandle,
};
use winit::icon::Icon;
use winit::monitor::{Fullscreen, MonitorHandle};
use winit::window::*;

use crate::engine::gpu::{BackendType, GraphicsBackend, GraphicsError};

// Unused platform operations fail loudly rather than accidentally exercising a
// real display/GPU. Only the clipboard in RutterEngine's constructor needs the
// same isolated clipboard service as the existing runner unit tests.
macro_rules! unused_window_operations {
    ($(fn $name:ident(&self $(, $arg:ident: $ty:ty)*) $(-> $result:ty)?;)*) => {
        $(fn $name(&self $(, $arg: $ty)*) $(-> $result)? {
            $(let _ = $arg;)*
            panic!("unexpected native operation: {}", stringify!($name));
        })*
    };
}

#[derive(Debug)]
pub(super) struct MemoryWindow {
    pub size: PhysicalSize<u32>,
    pub scale: f64,
    pub popup: bool,
    pub popup_state: Option<std::sync::Arc<PopupState>>,
}

#[derive(Debug)]
pub(super) struct PopupState {
    pub geometry: SharedCell<PopupSurfaceGeometry>,
    pub asynchronous: bool,
    pub requests: SharedCell<usize>,
}

#[derive(Debug)]
pub(super) struct SharedCell<T>(std::sync::Mutex<T>);

impl<T: Copy> SharedCell<T> {
    pub fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }
    pub fn get(&self) -> T {
        *self.0.lock().unwrap()
    }
    pub fn set(&self, value: T) {
        *self.0.lock().unwrap() = value;
    }
}

impl HasDisplayHandle for MemoryWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasWindowHandle for MemoryWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl Window for MemoryWindow {
    fn window_type(&self) -> WindowType {
        if self.popup {
            WindowType::Popup
        } else {
            WindowType::Window
        }
    }
    fn id(&self) -> WindowId {
        WindowId::from_raw(if self.popup { 2 } else { 1 })
    }
    fn scale_factor(&self) -> f64 {
        self.scale
    }
    fn surface_size(&self) -> PhysicalSize<u32> {
        self.size
    }
    fn request_redraw(&self) {}
    fn pre_present_notify(&self) {}
    fn request_popup_layout(&self, _: WindowPositioner, _: Size) -> Option<PhysicalSize<u32>> {
        assert!(
            self.popup,
            "only popup windows accept placement transactions"
        );
        if let Some(state) = &self.popup_state {
            state.requests.set(state.requests.get() + 1);
            return (!state.asynchronous).then_some(state.geometry.get().surface_size);
        }
        Some(self.size)
    }
    fn popup_surface_geometry(&self, _: f64) -> Result<PopupSurfaceGeometry, RequestError> {
        if let Some(state) = &self.popup_state {
            return Ok(state.geometry.get());
        }
        Ok(PopupSurfaceGeometry {
            parent_position: winit::dpi::LogicalPosition::new(20.0, 60.0),
            surface_size: self.size,
            scale_factor: self.scale,
            configure_serial: Some(1),
        })
    }
    fn rwh_06_display_handle(&self) -> &dyn HasDisplayHandle {
        self
    }
    fn rwh_06_window_handle(&self) -> &dyn HasWindowHandle {
        self
    }

    unused_window_operations! {
        fn reset_dead_keys(&self);
        fn surface_position(&self) -> PhysicalPosition<i32>;
        fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError>;
        fn set_outer_position(&self, position: Position);
        fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>>;
        fn outer_size(&self) -> PhysicalSize<u32>;
        fn safe_area(&self) -> PhysicalInsets<u32>;
        fn set_min_surface_size(&self, size: Option<Size>);
        fn set_max_surface_size(&self, size: Option<Size>);
        fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>>;
        fn set_surface_resize_increments(&self, size: Option<Size>);
        fn set_title(&self, title: &str);
        fn set_transparent(&self, transparent: bool);
        fn set_blur(&self, blur: bool);
        fn set_visible(&self, visible: bool);
        fn is_visible(&self) -> Option<bool>;
        fn set_resizable(&self, resizable: bool);
        fn is_resizable(&self) -> bool;
        fn set_enabled_buttons(&self, buttons: WindowButtons);
        fn enabled_buttons(&self) -> WindowButtons;
        fn set_minimized(&self, minimized: bool);
        fn is_minimized(&self) -> Option<bool>;
        fn set_maximized(&self, maximized: bool);
        fn is_maximized(&self) -> bool;
        fn set_fullscreen(&self, fullscreen: Option<Fullscreen>);
        fn fullscreen(&self) -> Option<Fullscreen>;
        fn set_decorations(&self, decorated: bool);
        fn is_decorated(&self) -> bool;
        fn set_window_level(&self, level: WindowLevel);
        fn set_window_icon(&self, icon: Option<Icon>);
        fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError>;
        fn ime_capabilities(&self) -> Option<ImeCapabilities>;
        fn focus_window(&self);
        fn has_focus(&self) -> bool;
        fn request_user_attention(&self, attention: Option<UserAttentionType>);
        fn set_theme(&self, theme: Option<Theme>);
        fn theme(&self) -> Option<Theme>;
        fn set_content_protected(&self, protected: bool);
        fn title(&self) -> String;
        fn set_cursor(&self, cursor: Cursor);
        fn set_cursor_position(&self, position: Position) -> Result<(), RequestError>;
        fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError>;
        fn set_cursor_visible(&self, visible: bool);
        fn drag_window(&self) -> Result<(), RequestError>;
        fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), RequestError>;
        fn show_window_menu(&self, position: Position);
        fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError>;
        fn current_monitor(&self) -> Option<MonitorHandle>;
        fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>>;
        fn primary_monitor(&self) -> Option<MonitorHandle>;
    }
}

#[derive(Debug, Default)]
pub(super) struct MemoryEventLoop {
    exited: Cell<bool>,
    flow: Cell<Option<ControlFlow>>,
}

impl ActiveEventLoop for MemoryEventLoop {
    fn exit(&self) {
        self.exited.set(true);
    }
    fn exiting(&self) -> bool {
        self.exited.get()
    }
    fn set_control_flow(&self, flow: ControlFlow) {
        self.flow.set(Some(flow));
    }
    fn control_flow(&self) -> ControlFlow {
        self.flow.get().unwrap_or(ControlFlow::Wait)
    }
    unused_window_operations! {
        fn create_proxy(&self) -> EventLoopProxy;
        fn create_window(&self, attributes: WindowAttributes) -> Result<Box<dyn Window>, RequestError>;
        fn create_custom_cursor(&self, source: CustomCursorSource) -> Result<CustomCursor, RequestError>;
        fn available_monitors(&self) -> Box<dyn Iterator<Item = MonitorHandle>>;
        fn primary_monitor(&self) -> Option<MonitorHandle>;
        fn listen_device_events(&self, allowed: DeviceEvents);
        fn system_theme(&self) -> Option<Theme>;
        fn owned_display_handle(&self) -> OwnedDisplayHandle;
        fn rwh_06_handle(&self) -> &dyn HasDisplayHandle;
    }
}

pub(super) struct RasterPopup {
    pub window: Rc<dyn Window>,
    pub surface: Surface,
    pub frames: Rc<Cell<usize>>,
    pub drops: Rc<Cell<usize>>,
    pub configure_on_frame: Option<(std::sync::Arc<PopupState>, PopupSurfaceGeometry)>,
}

impl GraphicsBackend for RasterPopup {
    fn backend_type(&self) -> BackendType {
        BackendType::CpuSoftbuffer
    }
    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError> {
        Ok(self.surface.canvas())
    }
    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        self.frames.set(self.frames.get() + 1);
        if let Some((state, geometry)) = self.configure_on_frame.take() {
            state.geometry.set(geometry);
        }
        Ok(())
    }
    fn resize(&mut self, _: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        panic!("fixed in-memory configure must not resize");
    }
    fn window(&self) -> &Rc<dyn Window> {
        &self.window
    }
}

impl Drop for RasterPopup {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
