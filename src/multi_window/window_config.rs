// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{Window, WindowAttributes, WindowLevel as WinitWindowLevel};

/// Positive physical dimensions requested for a window's inner surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    width: u32,
    height: u32,
}

impl WindowSize {
    /// Validates positive width and height dimensions.
    /// ```
    /// let size = rutter::WindowSize::new(800, 600).unwrap();
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
    /// assert_eq!(rutter::WindowSize::new(320, 240).unwrap().width(), 320);
    /// ```
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the validated height.
    /// ```
    /// assert_eq!(rutter::WindowSize::new(320, 240).unwrap().height(), 240);
    /// ```
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Requested physical desktop coordinates for a native window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPosition {
    x: i32,
    y: i32,
}

impl WindowPosition {
    /// Creates a position; negative coordinates support monitors left or above the origin.
    /// ```
    /// let position = rutter::WindowPosition::new(-120, 80);
    /// assert_eq!((position.x(), position.y()), (-120, 80));
    /// ```
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Returns the requested horizontal coordinate.
    /// ```
    /// assert_eq!(rutter::WindowPosition::new(40, 20).x(), 40);
    /// ```
    pub const fn x(self) -> i32 {
        self.x
    }

    /// Returns the requested vertical coordinate.
    /// ```
    /// assert_eq!(rutter::WindowPosition::new(40, 20).y(), 20);
    /// ```
    pub const fn y(self) -> i32 {
        self.y
    }
}

/// Requested stacking level relative to other native windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WindowLevel {
    /// Requests placement below normal windows.
    AlwaysOnBottom,
    /// Uses the platform's normal window level.
    #[default]
    Normal,
    /// Requests placement above normal windows.
    AlwaysOnTop,
}

impl From<WindowLevel> for WinitWindowLevel {
    fn from(level: WindowLevel) -> Self {
        match level {
            WindowLevel::AlwaysOnBottom => Self::AlwaysOnBottom,
            WindowLevel::Normal => Self::Normal,
            WindowLevel::AlwaysOnTop => Self::AlwaysOnTop,
        }
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

/// Startup and lifecycle settings for one native window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    title: String,
    transparent: bool,
    decorations: bool,
    resizable: bool,
    visible: bool,
    close_on_focus_loss: bool,
    inner_size: Option<WindowSize>,
    min_inner_size: Option<WindowSize>,
    max_inner_size: Option<WindowSize>,
    position: Option<WindowPosition>,
    window_level: WindowLevel,
    close_behavior: CloseBehavior,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Rutter".to_string(),
            transparent: false,
            decorations: true,
            resizable: true,
            visible: true,
            close_on_focus_loss: false,
            inner_size: None,
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            window_level: WindowLevel::default(),
            close_behavior: CloseBehavior::default(),
        }
    }
}

macro_rules! bool_window_setting {
    ($builder:ident, $accessor:ident, $field:ident, $description:literal) => {
        #[doc = concat!("Sets whether ", $description, ".\n```\nlet value = rutter::WindowConfig::default().", stringify!($builder), "(true);\nassert!(value.", stringify!($accessor), "());\n```")]
        pub const fn $builder(mut self, value: bool) -> Self {
            self.$field = value;
            self
        }
        #[doc = concat!("Reports whether ", $description, ".\n```\nlet value = rutter::WindowConfig::default().", stringify!($builder), "(true);\nassert!(value.", stringify!($accessor), "());\n```")]
        pub const fn $accessor(&self) -> bool {
            self.$field
        }
    };
}

impl WindowConfig {
    /// Sets the native window title.
    /// ```
    /// assert_eq!(rutter::WindowConfig::default().with_title("Editor").title(), "Editor");
    /// ```
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Returns the configured native window title.
    /// ```
    /// assert_eq!(rutter::WindowConfig::default().title(), "Rutter");
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
    bool_window_setting!(
        with_visible,
        is_visible,
        visible,
        "the native window is initially visible"
    );
    bool_window_setting!(
        with_close_on_focus_loss,
        closes_on_focus_loss,
        close_on_focus_loss,
        "the surface closes after gaining and then losing native focus"
    );

    /// Validates and sets the requested inner dimensions.
    /// ```
    /// let config = rutter::WindowConfig::default().with_inner_size(800, 600).unwrap();
    /// assert_eq!(config.inner_size().unwrap().width(), 800);
    /// ```
    pub fn with_inner_size(mut self, width: u32, height: u32) -> Result<Self, WindowConfigError> {
        self.inner_size = Some(WindowSize::new(width, height)?);
        Ok(self)
    }

    /// Returns the requested inner dimensions, if specified.
    /// ```
    /// assert!(rutter::WindowConfig::default().inner_size().is_none());
    /// ```
    pub const fn inner_size(&self) -> Option<WindowSize> {
        self.inner_size
    }

    /// Validates and sets the minimum inner dimensions.
    /// ```
    /// let config = rutter::WindowConfig::default().with_min_inner_size(320, 200).unwrap();
    /// assert_eq!(config.min_inner_size().unwrap().height(), 200);
    /// ```
    pub fn with_min_inner_size(
        mut self,
        width: u32,
        height: u32,
    ) -> Result<Self, WindowConfigError> {
        self.min_inner_size = Some(WindowSize::new(width, height)?);
        validate_size_bounds(self.min_inner_size, self.max_inner_size)?;
        Ok(self)
    }

    /// Returns the minimum inner dimensions, if specified.
    /// ```
    /// assert!(rutter::WindowConfig::default().min_inner_size().is_none());
    /// ```
    pub const fn min_inner_size(&self) -> Option<WindowSize> {
        self.min_inner_size
    }

    /// Validates and sets the maximum inner dimensions.
    /// ```
    /// let config = rutter::WindowConfig::default().with_max_inner_size(1280, 900).unwrap();
    /// assert_eq!(config.max_inner_size().unwrap().width(), 1280);
    /// ```
    pub fn with_max_inner_size(
        mut self,
        width: u32,
        height: u32,
    ) -> Result<Self, WindowConfigError> {
        self.max_inner_size = Some(WindowSize::new(width, height)?);
        validate_size_bounds(self.min_inner_size, self.max_inner_size)?;
        Ok(self)
    }

    /// Returns the maximum inner dimensions, if specified.
    /// ```
    /// assert!(rutter::WindowConfig::default().max_inner_size().is_none());
    /// ```
    pub const fn max_inner_size(&self) -> Option<WindowSize> {
        self.max_inner_size
    }

    /// Sets the requested physical desktop position.
    /// ```
    /// let config = rutter::WindowConfig::default().with_position(-40, 120);
    /// assert_eq!(config.position().unwrap(), rutter::WindowPosition::new(-40, 120));
    /// ```
    pub const fn with_position(mut self, x: i32, y: i32) -> Self {
        self.position = Some(WindowPosition::new(x, y));
        self
    }

    /// Returns the requested desktop position, if specified.
    /// ```
    /// assert!(rutter::WindowConfig::default().position().is_none());
    /// ```
    pub const fn position(&self) -> Option<WindowPosition> {
        self.position
    }

    /// Sets the requested native stacking level.
    /// ```
    /// let config = rutter::WindowConfig::default()
    ///     .with_window_level(rutter::WindowLevel::AlwaysOnTop);
    /// assert_eq!(config.window_level(), rutter::WindowLevel::AlwaysOnTop);
    /// ```
    pub const fn with_window_level(mut self, window_level: WindowLevel) -> Self {
        self.window_level = window_level;
        self
    }

    /// Returns the requested native stacking level.
    /// ```
    /// assert_eq!(rutter::WindowConfig::default().window_level(), rutter::WindowLevel::Normal);
    /// ```
    pub const fn window_level(&self) -> WindowLevel {
        self.window_level
    }

    /// Sets how the runtime handles a close request.
    /// ```
    /// let config = rutter::WindowConfig::default()
    ///     .with_close_behavior(rutter::CloseBehavior::ExitApplication);
    /// assert_eq!(config.close_behavior(), rutter::CloseBehavior::ExitApplication);
    /// ```
    pub const fn with_close_behavior(mut self, close_behavior: CloseBehavior) -> Self {
        self.close_behavior = close_behavior;
        self
    }

    /// Returns the configured close behavior.
    /// ```
    /// assert_eq!(rutter::WindowConfig::default().close_behavior(), rutter::CloseBehavior::CloseSurface);
    /// ```
    pub const fn close_behavior(&self) -> CloseBehavior {
        self.close_behavior
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub(crate) fn window_attributes(&self) -> WindowAttributes {
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_visible(self.visible)
            .with_transparent(self.transparent)
            .with_decorations(self.decorations)
            .with_resizable(self.resizable)
            .with_window_level(self.window_level.into());
        self.apply_geometry(attributes)
    }

    pub(crate) fn surface_config(&self) -> crate::app::SurfaceConfig {
        if self.transparent {
            return crate::app::SurfaceConfig::transparent();
        }
        crate::app::SurfaceConfig::default()
    }

    fn apply_geometry(&self, mut attributes: WindowAttributes) -> WindowAttributes {
        attributes.inner_size = self.inner_size.map(window_size_to_winit);
        attributes.min_inner_size = self.min_inner_size.map(window_size_to_winit);
        attributes.max_inner_size = self.max_inner_size.map(window_size_to_winit);
        attributes.position = self.position.map(window_position_to_winit);
        attributes
    }
}

fn window_size_to_winit(size: WindowSize) -> winit::dpi::Size {
    PhysicalSize::new(size.width(), size.height()).into()
}

fn window_position_to_winit(position: WindowPosition) -> winit::dpi::Position {
    PhysicalPosition::new(position.x(), position.y()).into()
}

fn validate_size_bounds(
    minimum: Option<WindowSize>,
    maximum: Option<WindowSize>,
) -> Result<(), WindowConfigError> {
    let (Some(minimum), Some(maximum)) = (minimum, maximum) else {
        return Ok(());
    };
    if minimum.width <= maximum.width && minimum.height <= maximum.height {
        return Ok(());
    }
    Err(WindowConfigError::InvalidSizeBounds { minimum, maximum })
}

/// Reports invalid native-window configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowConfigError {
    /// One or both dimensions are zero.
    InvalidSize { width: u32, height: u32 },
    /// A minimum dimension exceeds its corresponding maximum.
    InvalidSizeBounds {
        minimum: WindowSize,
        maximum: WindowSize,
    },
}

impl Display for WindowConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => write!(
                formatter,
                "invalid window dimensions {width}x{height}; expected width > 0 and height > 0"
            ),
            Self::InvalidSizeBounds { minimum, maximum } => write!(
                formatter,
                "invalid window size bounds {}x{}..={}x{}; expected each minimum dimension to be less than or equal to its maximum",
                minimum.width(),
                minimum.height(),
                maximum.width(),
                maximum.height()
            ),
        }
    }
}

impl Error for WindowConfigError {}
