// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Parent-content-relative native popup requests. No desktop coordinates or raw handles.

use std::fmt;

use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{WindowAnchor, WindowGravity, WindowPositioner};

use super::SurfaceId;

/// Exact allowed native positioner adjustments; no flags are added by Rutter.
pub use winit::window::WindowConstraintAdjustment as PopupConstraints;

const MAX_COORDINATE: i32 = 1_000_000;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: u64 = 4 * 1024 * 1024;

/// Positive, integral logical rectangle inside the parent's configured content area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupAnchor {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl PopupAnchor {
    /// Validates nonnegative coordinates, positive dimensions and bounded protocol arithmetic.
    /// The runtime additionally checks containment in the live parent's logical viewport.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Result<Self, PopupConfigError> {
        if x < 0
            || y < 0
            || width == 0
            || height == 0
            || i64::from(x) + i64::from(width) > i64::from(MAX_COORDINATE)
            || i64::from(y) + i64::from(height) > i64::from(MAX_COORDINATE)
        {
            return Err(PopupConfigError::InvalidAnchor {
                x,
                y,
                width,
                height,
            });
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    /// Returns `(x, y, width, height)` in parent-content logical pixels.
    pub const fn rectangle(self) -> (i32, i32, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }

    pub(crate) fn fits(self, viewport: (f64, f64)) -> bool {
        viewport.0.is_finite()
            && viewport.1.is_finite()
            && f64::from(self.x) + f64::from(self.width) <= viewport.0
            && f64::from(self.y) + f64::from(self.height) <= viewport.1
    }
}

/// Preferred direction away from the center of the corresponding anchor edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupDirection {
    Above,
    Below,
    Left,
    Right,
}

/// Validated request for an arbitrary-content native popup.
///
/// Defaults to Winit `PopupGrab::Required` and `active = true`. On Wayland, open
/// synchronously inside the actual parent's pointer-button or keyboard press callback.
/// Programmatic, release, synthetic, repeat, and deferred opens cannot authorize a grab;
/// the backend rejects missing/stale input before allocating a popup role.
/// [`Self::with_grab(false)`](Self::with_grab) explicitly selects `PopupGrab::None`
/// and `active = false`, allowing programmatic opens without outside-click guarantees.
/// Escape on the popup or its invoking parent dismisses either mode.
/// See `docs/NATIVE_POPUPS.md` for platform and dismissal limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupConfig {
    parent: SurfaceId,
    anchor: PopupAnchor,
    width: u32,
    height: u32,
    direction: PopupDirection,
    offset: (i32, i32),
    constraints: PopupConstraints,
    grab: bool,
}

impl PopupConfig {
    /// Creates a grab-required popup request in logical pixels, with no adjustments or offset.
    /// Physical allocation is also checked at the parent's actual scale before creation.
    /// On Wayland, return the open command synchronously from an actual parent input press.
    pub fn new(
        parent: SurfaceId,
        anchor: PopupAnchor,
        width: u32,
        height: u32,
        direction: PopupDirection,
    ) -> Result<Self, PopupConfigError> {
        if width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_PIXELS
        {
            return Err(PopupConfigError::InvalidSize { width, height });
        }
        Ok(Self {
            parent,
            anchor,
            width,
            height,
            direction,
            offset: (0, 0),
            constraints: PopupConstraints::empty(),
            grab: true,
        })
    }

    /// Selects required input grabbing (the default) or explicit non-grabbing behavior.
    ///
    /// `true` maps exactly to Winit `PopupGrab::Required` with `active = true`;
    /// `false` maps to `PopupGrab::None` with `active = false`. The legacy `Auto`
    /// policy is never used. Required Wayland grabs need a live, unconsumed press
    /// on this popup's actual parent during the same native callback. A release,
    /// timer, accessibility activation, or queued application message is insufficient.
    /// Non-grabbing Wayland popups can open programmatically, keep native focus
    /// with the parent, and have no outside-client dismissal guarantee. X11 retains
    /// its native pointer grabs in both modes; this setting controls activation there.
    pub const fn with_grab(mut self, grab: bool) -> Self {
        self.grab = grab;
        self
    }

    /// Whether this request requires a Wayland input grab (default: `true`).
    pub const fn requires_input_grab(self) -> bool {
        self.grab
    }

    /// Adds a signed logical offset. For a gap use positive Y below, negative Y above,
    /// positive X right, or negative X left. Popup origins may be negative.
    pub fn with_offset(mut self, x: i32, y: i32) -> Result<Self, PopupConfigError> {
        if !(-MAX_COORDINATE..=MAX_COORDINATE).contains(&x)
            || !(-MAX_COORDINATE..=MAX_COORDINATE).contains(&y)
        {
            return Err(PopupConfigError::InvalidOffset { x, y });
        }
        self.offset = (x, y);
        Ok(self)
    }

    /// Sets exactly the permitted slide/flip/resize flags, rejecting unknown bits.
    pub fn with_constraints(
        mut self,
        constraints: PopupConstraints,
    ) -> Result<Self, PopupConfigError> {
        if constraints.bits() & !PopupConstraints::all().bits() != 0 {
            return Err(PopupConfigError::UnknownConstraints(constraints.bits()));
        }
        self.constraints = constraints;
        Ok(self)
    }

    /// Registered logical parent, resolved by the runtime at creation.
    pub const fn parent(self) -> SurfaceId {
        self.parent
    }
    /// Anchor in parent-content logical pixels.
    pub const fn anchor(self) -> PopupAnchor {
        self.anchor
    }
    /// Requested logical dimensions; use `SurfaceMetrics` for actual configured geometry.
    pub const fn logical_size(self) -> (u32, u32) {
        (self.width, self.height)
    }
    /// Requested placement direction.
    pub const fn direction(self) -> PopupDirection {
        self.direction
    }
    /// Signed logical offset.
    pub const fn offset(self) -> (i32, i32) {
        self.offset
    }
    /// Exact permitted native adjustments.
    pub const fn constraints(self) -> PopupConstraints {
        self.constraints
    }

    pub(crate) fn positioner(self) -> WindowPositioner {
        let (anchor, gravity) = match self.direction {
            PopupDirection::Above => (WindowAnchor::Top, WindowGravity::Top),
            PopupDirection::Below => (WindowAnchor::Bottom, WindowGravity::Bottom),
            PopupDirection::Left => (WindowAnchor::Left, WindowGravity::Left),
            PopupDirection::Right => (WindowAnchor::Right, WindowGravity::Right),
        };
        WindowPositioner::new(
            anchor,
            (
                LogicalPosition::new(f64::from(self.anchor.x), f64::from(self.anchor.y)).into(),
                LogicalSize::new(f64::from(self.anchor.width), f64::from(self.anchor.height))
                    .into(),
            ),
            LogicalPosition::new(f64::from(self.offset.0), f64::from(self.offset.1)).into(),
            gravity,
            self.constraints,
        )
    }
}

/// Invalid popup configuration, detected before any native allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupConfigError {
    InvalidAnchor {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    InvalidSize {
        width: u32,
        height: u32,
    },
    InvalidOffset {
        x: i32,
        y: i32,
    },
    UnknownConstraints(u32),
    ConflictingWindowHints,
}

impl fmt::Display for PopupConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAnchor { x, y, width, height } => write!(f,
                "invalid popup anchor ({x}, {y}, {width}, {height}); expected a positive rectangle within 0..={MAX_COORDINATE} logical pixels"),
            Self::InvalidSize { width, height } => write!(f,
                "invalid popup size {width}x{height}; expected positive dimensions <= {MAX_DIMENSION} and area <= {MAX_PIXELS} logical pixels"),
            Self::InvalidOffset { x, y } => write!(f,
                "invalid popup offset ({x}, {y}); expected each coordinate within +/-{MAX_COORDINATE} logical pixels"),
            Self::UnknownConstraints(bits) => write!(f, "invalid popup constraint bits {bits:#x}; expected only slide, flip and resize flags"),
            Self::ConflictingWindowHints => f.write_str("native popup conflicts with panel/layer role, desktop position, window size bounds, stacking, hidden state or application-exit close behavior; use PopupConfig geometry and CloseSurface"),
        }
    }
}

impl std::error::Error for PopupConfigError {}

#[cfg(test)]
#[path = "../../tests/unit/native_popup_config_tests.rs"]
mod tests;
