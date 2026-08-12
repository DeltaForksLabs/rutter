// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.
use crate::engine::run_error::RutterRunError;
use crate::i18n::Locale;
use crate::input_limits::{InputKind, InputLimits};
use crate::render::text::TextShapeCacheLimits;
use crate::widget::Widget;
use arboard::Clipboard;
use cosmic_text::FontSystem;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::Hash;
use winit::dpi::PhysicalSize;
use winit::error::EventLoopError;
use winit::window::{Window, WindowAttributes};
/// Stable application-owned identity for one logical surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(u64);
impl SurfaceId {
    /// Identity of the default initial surface.
    /// ```
    /// assert_eq!(rutter::multi_window::SurfaceId::PRIMARY.get(), 0);
    /// ```
    pub const PRIMARY: Self = Self(0);
    /// Creates a logical surface identity from an application-defined value.
    /// ```
    /// assert_eq!(rutter::multi_window::SurfaceId::new(7).get(), 7);
    /// ```
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Returns the application-defined identity value.
    /// ```
    /// assert_eq!(rutter::multi_window::SurfaceId::new(9).get(), 9);
    /// ```
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Positive physical dimensions requested for a window's inner surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    width: u32,
    height: u32,
}
impl WindowSize {
    /// Validates positive width and height dimensions.
    /// ```
    /// let size = rutter::multi_window::WindowSize::new(800, 600).unwrap();
    /// assert_eq!((size.width(), size.height()), (800, 600));
    /// ```
    pub const fn new(width: u32, height: u32) -> Result<Self, WindowConfigError> {
        if width == 0 || height == 0 {
            return Err(WindowConfigError::InvalidSize { width, height });
        }
        Ok(Self { width, height })
    }
    /// Returns the validated width.
    /// ```
    /// assert_eq!(rutter::multi_window::WindowSize::new(320, 240).unwrap().width(), 320);
    /// ```
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Returns the validated height.
    /// ```
    /// assert_eq!(rutter::multi_window::WindowSize::new(320, 240).unwrap().height(), 240);
    /// ```
    pub const fn height(self) -> u32 {
        self.height
    }
}
/// Determines what a close request does to the application registry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CloseBehavior {
    /// Removes only the requested surface.
    #[default]
    CloseSurface,
    /// Exits the entire application.
    ExitApplication,
}

/// Startup settings for one native window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    title: String,
    transparent: bool,
    decorations: bool,
    resizable: bool,
    inner_size: Option<WindowSize>,
    close_behavior: CloseBehavior,
}
impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Rutter".to_string(),
            transparent: false,
            decorations: true,
            resizable: true,
            inner_size: None,
            close_behavior: CloseBehavior::default(),
        }
    }
}
macro_rules! bool_window_setting {
    ($builder:ident, $accessor:ident, $field:ident, $description:literal) => {
        #[doc = concat!("Sets whether ", $description, ".\n```\nlet value = rutter::multi_window::WindowConfig::default().", stringify!($builder), "(true);\nassert!(value.", stringify!($accessor), "());\n```")]
        pub const fn $builder(mut self, value: bool) -> Self {
            self.$field = value;
            self
        }
        #[doc = concat!("Reports whether ", $description, ".\n```\nlet value = rutter::multi_window::WindowConfig::default().", stringify!($builder), "(true);\nassert!(value.", stringify!($accessor), "());\n```")]
        pub const fn $accessor(&self) -> bool {
            self.$field
        }
    };
}
impl WindowConfig {
    /// Sets the native window title.
    /// ```
    /// assert_eq!(rutter::multi_window::WindowConfig::default().with_title("Editor").title(), "Editor");
    /// ```
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
    /// Returns the configured native window title.
    /// ```
    /// assert_eq!(rutter::multi_window::WindowConfig::default().title(), "Rutter");
    /// ```
    pub fn title(&self) -> &str {
        &self.title
    }
    bool_window_setting!(
        with_transparent,
        is_transparent,
        transparent,
        "compositor transparency is requested"
    );
    bool_window_setting!(
        with_decorations,
        has_decorations,
        decorations,
        "native decorations are requested"
    );
    bool_window_setting!(
        with_resizable,
        is_resizable,
        resizable,
        "the native window may be resized"
    );
    /// Validates and sets the requested inner dimensions.
    /// ```
    /// assert_eq!(rutter::multi_window::WindowConfig::default().with_inner_size(800, 600).unwrap().inner_size().unwrap().width(), 800);
    /// ```
    pub fn with_inner_size(mut self, width: u32, height: u32) -> Result<Self, WindowConfigError> {
        self.inner_size = Some(WindowSize::new(width, height)?);
        Ok(self)
    }
    /// Returns the requested inner dimensions, if specified.
    /// ```
    /// assert!(rutter::multi_window::WindowConfig::default().inner_size().is_none());
    /// ```
    pub const fn inner_size(&self) -> Option<WindowSize> {
        self.inner_size
    }
    /// Sets how the runtime handles a close request.
    /// ```
    /// use rutter::multi_window::{CloseBehavior, WindowConfig};
    /// assert_eq!(WindowConfig::default().with_close_behavior(CloseBehavior::ExitApplication).close_behavior(), CloseBehavior::ExitApplication);
    /// ```
    pub const fn with_close_behavior(mut self, close_behavior: CloseBehavior) -> Self {
        self.close_behavior = close_behavior;
        self
    }
    /// Returns the configured close behavior.
    /// ```
    /// use rutter::multi_window::{CloseBehavior, WindowConfig};
    /// assert_eq!(WindowConfig::default().close_behavior(), CloseBehavior::CloseSurface);
    /// ```
    pub const fn close_behavior(&self) -> CloseBehavior {
        self.close_behavior
    }
    pub(crate) fn window_attributes(&self) -> WindowAttributes {
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_visible(false)
            .with_transparent(self.transparent)
            .with_decorations(self.decorations)
            .with_resizable(self.resizable);
        match self.inner_size {
            Some(size) => {
                attributes.with_inner_size(PhysicalSize::new(size.width(), size.height()))
            }
            None => attributes,
        }
    }
    pub(crate) fn surface_config(&self) -> crate::app::SurfaceConfig {
        if self.transparent {
            return crate::app::SurfaceConfig::transparent();
        }
        crate::app::SurfaceConfig::default()
    }
}

/// Describes one logical surface requested from the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceRequest {
    pub surface: SurfaceId,
    pub window: WindowConfig,
}
impl SurfaceRequest {
    /// Creates a request pairing a logical identity with window settings.
    /// ```
    /// use rutter::multi_window::{SurfaceId, SurfaceRequest, WindowConfig};
    /// assert_eq!(SurfaceRequest::new(SurfaceId::PRIMARY, WindowConfig::default()).surface, SurfaceId::PRIMARY);
    /// ```
    pub fn new(surface: SurfaceId, window: WindowConfig) -> Self {
        Self { surface, window }
    }
}
/// Runtime operations emitted by application updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceCommand {
    Open(SurfaceRequest),
    Close(SurfaceId),
    Exit,
}
/// Elm-style application contract for a runtime with multiple native windows.
pub trait MultiWindowAppLogic {
    type State: Clone;
    type Message: Clone + Debug;
    /// Creates the state used by [`MultiWindowRunner::run`](crate::MultiWindowRunner::run).
    /// Injected startup through [`MultiWindowRunner::run_with`](crate::MultiWindowRunner::run_with)
    /// or [`MultiWindowRunner::run_with_state`](crate::MultiWindowRunner::run_with_state) bypasses
    /// this hook.
    fn new(font_system: &mut FontSystem) -> Self::State;
    /// Returns surfaces for the default runner startup path.
    ///
    /// The default contains only [`SurfaceId::PRIMARY`]. Injected startup bypasses this hook.
    fn initial_surfaces() -> Vec<SurfaceRequest> {
        vec![SurfaceRequest::new(
            SurfaceId::PRIMARY,
            WindowConfig::default(),
        )]
    }
    /// Produces the widget tree for one logical surface.
    fn view<'a>(state: &'a mut Self::State, surface: SurfaceId) -> Widget<'a, Self::Message>;
    /// Observes a surface after its native window and backend are committed.
    fn surface_created(_state: &mut Self::State, _surface: SurfaceId) {}
    /// Observes logical removal after the native route has been retired.
    fn surface_closed(_state: &mut Self::State, _surface: SurfaceId) {}
    /// Processes a source-aware message and returns requested runtime operations.
    fn update(
        state: &mut Self::State,
        surface: SurfaceId,
        message: Self::Message,
        clipboard: &mut Clipboard,
    ) -> Vec<SurfaceCommand>;
    /// Returns the application theme.
    fn theme() -> crate::theme::Theme {
        crate::theme::Theme::default()
    }
    /// Resolves the application theme from its current shared state.
    ///
    /// The default delegates to [`Self::theme`], preserving existing implementations.
    ///
    /// ```rust
    /// use rutter::{MultiWindowAppLogic, Theme};
    ///
    /// fn active_theme<A: MultiWindowAppLogic>(state: &A::State) -> Theme {
    ///     A::theme_for(state)
    /// }
    /// ```
    fn theme_for(_state: &Self::State) -> crate::theme::Theme {
        Self::theme()
    }
    /// Returns the locale used for layout direction and catalogs.
    fn locale() -> Locale {
        Locale::default()
    }
    /// Returns limits for one resolved input.
    fn input_limits(_id: u64, kind: InputKind) -> InputLimits {
        InputLimits::for_kind(kind)
    }
    /// Returns the runtime's shaping-cache budget.
    fn text_shape_cache_limits() -> TextShapeCacheLimits {
        TextShapeCacheLimits::default()
    }
}
/// Reports invalid native-window configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowConfigError {
    InvalidSize { width: u32, height: u32 },
}
impl Display for WindowConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => write!(
                formatter,
                "invalid window dimensions {width}x{height}; expected width > 0 and height > 0"
            ),
        }
    }
}
impl Error for WindowConfigError {}
/// Describes controlled failures in the multi-window runtime.
#[derive(Debug)]
pub enum MultiWindowRunError {
    EventLoop(EventLoopError),
    Surface(SurfaceId, RutterRunError),
    DuplicateLogicalSurface(SurfaceId),
    NativeRouteConflict { surface: SurfaceId, native: String },
    UnknownLogicalSurface(SurfaceId),
    EmptyInitialSurfaces,
    Startup(Box<dyn Error + Send + Sync>),
}
impl Display for MultiWindowRunError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(error) => write!(formatter, "event loop failed: {error}"),
            Self::Surface(surface, error) => write!(
                formatter,
                "surface {surface:?} failed: {error}; expected successful initialization or an operation on its isolated runtime"
            ),
            Self::DuplicateLogicalSurface(surface) => write!(
                formatter,
                "surface {surface:?} is already registered; expected each logical surface ID to have one owner"
            ),
            Self::NativeRouteConflict { surface, native } => write!(
                formatter,
                "surface {surface:?} received existing native window route {native}; expected each committed Winit WindowId to be unique"
            ),
            Self::UnknownLogicalSurface(surface) => write!(
                formatter,
                "surface {surface:?} is not registered; expected a committed logical surface"
            ),
            Self::EmptyInitialSurfaces => formatter.write_str(
                "initial surface registry is empty; expected at least one logical surface",
            ),
            Self::Startup(error) => write!(
                formatter,
                "multi-window startup failed: {error}; expected the injected state factory to initialize successfully"
            ),
        }
    }
}
impl Error for MultiWindowRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EventLoop(error) => Some(error),
            Self::Surface(_, error) => Some(error),
            Self::Startup(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
impl From<EventLoopError> for MultiWindowRunError {
    fn from(error: EventLoopError) -> Self {
        Self::EventLoop(error)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceRouteRegistrationError<NativeId> {
    DuplicateLogical(SurfaceId),
    DuplicateNative(NativeId),
}
pub(crate) struct SurfaceRoutes<NativeId> {
    native_by_surface: HashMap<SurfaceId, NativeId>,
    surface_by_native: HashMap<NativeId, SurfaceId>,
}
impl<NativeId: Copy + Eq + Hash> SurfaceRoutes<NativeId> {
    pub(crate) fn new() -> Self {
        Self {
            native_by_surface: HashMap::new(),
            surface_by_native: HashMap::new(),
        }
    }
    pub(crate) fn register_committed(
        &mut self,
        surface: SurfaceId,
        native: NativeId,
    ) -> Result<(), SurfaceRouteRegistrationError<NativeId>> {
        if self.native_by_surface.contains_key(&surface) {
            return Err(SurfaceRouteRegistrationError::DuplicateLogical(surface));
        }
        if self.surface_by_native.contains_key(&native) {
            return Err(SurfaceRouteRegistrationError::DuplicateNative(native));
        }
        self.native_by_surface.insert(surface, native);
        self.surface_by_native.insert(native, surface);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn native_for(&self, surface: SurfaceId) -> Option<NativeId> {
        self.native_by_surface.get(&surface).copied()
    }
    pub(crate) fn surface_for(&self, native: NativeId) -> Option<SurfaceId> {
        self.surface_by_native.get(&native).copied()
    }
    pub(crate) fn remove_surface(&mut self, surface: SurfaceId) -> Option<NativeId> {
        let native = self.native_by_surface.remove(&surface)?;
        self.surface_by_native.remove(&native);
        Some(native)
    }
    pub(crate) fn remove_native(&mut self, native: NativeId) -> Option<SurfaceId> {
        let surface = self.surface_by_native.remove(&native)?;
        self.native_by_surface.remove(&surface);
        Some(surface)
    }
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.native_by_surface.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.native_by_surface.clear();
        self.surface_by_native.clear();
    }
}
#[cfg(test)]
#[path = "../tests/unit/multi_window_unit_tests.rs"]
mod tests;
