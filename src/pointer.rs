// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::{fmt, sync::Arc};

use crate::LogicalPointerPosition;
use crate::render::image::{ImageDecodeError, ImageDecodeLimits, decode_rutter_image_with_limits};
use skia_safe::{Color, Paint, surfaces};

/// The lifecycle phase of a pointer event delivered to an opted-in region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Pressed,
    Moved,
    Released,
    Cancelled,
}

/// Keyboard modifiers captured with a pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerModifiers {
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
}

impl PointerModifiers {
    /// Creates a modifier snapshot without exposing platform input types.
    pub const fn new(shift: bool, control: bool, alt: bool, meta: bool) -> Self {
        Self {
            shift,
            control,
            alt,
            meta,
        }
    }

    /// Reports whether Shift was pressed.
    pub const fn shift(self) -> bool {
        self.shift
    }

    /// Reports whether Control was pressed.
    pub const fn control(self) -> bool {
        self.control
    }

    /// Reports whether Alt was pressed.
    pub const fn alt(self) -> bool {
        self.alt
    }

    /// Reports whether the platform Meta key was pressed.
    pub const fn meta(self) -> bool {
        self.meta
    }
}

/// A logical, surface-local pointer event owned by Rutter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub phase: PointerPhase,
    pub position: LogicalPointerPosition,
    pub modifiers: PointerModifiers,
}

/// An opaque application-owned class used to match drag sources and targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragPayloadKind(u64);

impl DragPayloadKind {
    /// Creates a stable application-owned drag class.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the application-owned class value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// An opaque application-owned drag value; Rutter never dereferences it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragPayload {
    kind: DragPayloadKind,
    value: u64,
}

impl DragPayload {
    /// Creates a payload identifier suitable for a drag source.
    pub const fn new(kind: DragPayloadKind, value: u64) -> Self {
        Self { kind, value }
    }

    /// Returns the class matched against a drop target declaration.
    pub const fn kind(self) -> DragPayloadKind {
        self.kind
    }

    /// Returns the application-owned opaque value.
    pub const fn value(self) -> u64 {
        self.value
    }
}

/// The lifecycle phase of a typed in-surface drag operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Started,
    Entered,
    Moved,
    Exited,
    Dropped,
    Cancelled(DragCancelReason),
}

/// Explains why Rutter cancelled an active drag rather than completing a drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragCancelReason {
    CursorLeftSurface,
    FocusLost,
    BlockingOverlay,
    SourceRemoved,
    SurfaceClosed,
}

/// A typed drag event delivered through the application's normal update path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragEvent {
    pub phase: DragPhase,
    pub payload: DragPayload,
    pub source_id: u64,
    pub target_id: Option<u64>,
    pub position: LogicalPointerPosition,
    pub modifiers: PointerModifiers,
}

/// Declares an opt-in drag source for a [`PointerRegionConfig`].
pub struct DragSource<Msg> {
    pub payload: DragPayload,
    pub on_drag: fn(DragEvent) -> Msg,
}

impl<Msg> Copy for DragSource<Msg> {}

impl<Msg> Clone for DragSource<Msg> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Declares an explicit matching drop target for a [`PointerRegionConfig`].
pub struct DropTarget<Msg> {
    pub accepted_kind: DragPayloadKind,
    pub on_drag: fn(DragEvent) -> Msg,
}

impl<Msg> Copy for DropTarget<Msg> {}

impl<Msg> Clone for DropTarget<Msg> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A small built-in shape for a drag badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragBadgeIcon {
    /// Two overlapping rectangular outlines.
    Copy,
    /// Crossed strokes indicating movement.
    Move,
}

#[derive(Clone, Debug)]
pub(crate) enum DragBadgeContent {
    Status,
    Icon(DragBadgeIcon),
    Text(Arc<str>),
    Image(Arc<skia_safe::Image>),
}

/// Describes an invalid text label or encoded image supplied for a drag badge.
#[derive(Debug)]
pub struct DragBadgeError {
    cause: DragBadgeErrorCause,
}

#[derive(Debug)]
enum DragBadgeErrorCause {
    InvalidText { bytes: usize },
    Image(ImageDecodeError),
    ImageSurface,
}

impl fmt::Display for DragBadgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            DragBadgeErrorCause::InvalidText { bytes } => write!(
                formatter,
                "drag badge text has {bytes} UTF-8 bytes or contains controls; expected 1 to 64 printable bytes"
            ),
            DragBadgeErrorCause::Image(error) => {
                write!(formatter, "drag badge image is invalid: {error}")
            }
            DragBadgeErrorCause::ImageSurface => {
                formatter.write_str("drag badge image could not allocate a 20×20 raster copy")
            }
        }
    }
}

impl std::error::Error for DragBadgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            DragBadgeErrorCause::InvalidText { .. } | DragBadgeErrorCause::ImageSurface => None,
            DragBadgeErrorCause::Image(error) => Some(error),
        }
    }
}

/// A non-interactive badge alongside the native pointer during a drag.
///
/// By default it shows a dot in transit and a plus above matching drop targets.
/// Choose contrasting colors for its background and mark in both themes.
#[derive(Clone, Debug)]
pub struct DragBadge {
    pub background: Color,
    pub foreground: Color,
    pub(crate) content: DragBadgeContent,
}

impl DragBadge {
    /// Defines the badge's background and contrasting mark/outline color.
    pub const fn new(background: Color, foreground: Color) -> Self {
        Self {
            background,
            foreground,
            content: DragBadgeContent::Status,
        }
    }

    /// Uses a built-in geometric icon in place of the status dot/plus.
    pub fn with_icon(mut self, icon: DragBadgeIcon) -> Self {
        self.content = DragBadgeContent::Icon(icon);
        self
    }

    /// Uses printable text; rendering truncates long labels within the badge.
    pub fn with_text(mut self, text: impl Into<Arc<str>>) -> Result<Self, DragBadgeError> {
        let text = text.into();
        let contains_control = text.chars().any(|character| {
            character.is_control()
                || matches!(character, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        });
        if text.is_empty() || text.len() > 64 || contains_control {
            return Err(DragBadgeError {
                cause: DragBadgeErrorCause::InvalidText { bytes: text.len() },
            });
        }
        self.content = DragBadgeContent::Text(text);
        Ok(self)
    }

    /// Decodes an embedded raster image and stores only a 20×20 copy.
    ///
    /// The maximum encoded size is 128 KiB and decoded dimensions are at most
    /// 128×128 pixels. Prepare the badge outside `AppLogic::view` when possible
    /// to avoid repeated decoding. No file paths or network URLs are accepted.
    pub fn with_image(mut self, encoded: &[u8]) -> Result<Self, DragBadgeError> {
        let limits = ImageDecodeLimits {
            max_encoded_bytes: 128 * 1024,
            max_width: 128,
            max_height: 128,
            max_alloc_bytes: 128 * 128 * 4,
        };
        let decoded =
            decode_rutter_image_with_limits(encoded, limits).map_err(|error| DragBadgeError {
                cause: DragBadgeErrorCause::Image(error),
            })?;
        let mut surface = surfaces::raster_n32_premul((20, 20)).ok_or(DragBadgeError {
            cause: DragBadgeErrorCause::ImageSurface,
        })?;
        surface.canvas().clear(Color::TRANSPARENT);
        surface
            .canvas()
            .scale((20.0 / decoded.width as f32, 20.0 / decoded.height as f32));
        surface
            .canvas()
            .draw_image(&decoded.image, (0.0, 0.0), Some(&Paint::default()));
        self.content = DragBadgeContent::Image(Arc::new(surface.image_snapshot()));
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveDragBadge {
    pub(crate) appearance: DragBadge,
    pub(crate) can_drop: bool,
}

/// Configures typed pointer, capture, and drag/drop behavior for one region.
pub struct PointerRegionConfig<Msg> {
    pub on_pointer: fn(PointerEvent) -> Msg,
    pub capture_on_press: bool,
    pub drag_source: Option<DragSource<Msg>>,
    pub drop_target: Option<DropTarget<Msg>>,
    pub drag_badge: Option<DragBadge>,
}

impl<Msg> PointerRegionConfig<Msg> {
    /// Creates a region that reports pointer events without capturing them.
    ///
    /// ```
    /// # use rutter::{PointerEvent, PointerRegionConfig};
    /// # enum Msg { Pointer(PointerEvent) }
    /// let config = PointerRegionConfig::new(Msg::Pointer);
    /// ```
    pub const fn new(on_pointer: fn(PointerEvent) -> Msg) -> Self {
        Self {
            on_pointer,
            capture_on_press: false,
            drag_source: None,
            drop_target: None,
            drag_badge: None,
        }
    }

    /// Captures move and release events after the region receives a primary press.
    pub const fn with_pointer_capture(mut self) -> Self {
        self.capture_on_press = true;
        self
    }

    /// Declares this region as a drag source with an opaque payload.
    pub const fn with_drag_source(mut self, source: DragSource<Msg>) -> Self {
        self.drag_source = Some(source);
        self
    }

    /// Enables an in-surface badge while this region is an active drag source.
    ///
    /// Without a drag source, this option has no visual effect. Badges do not
    /// change hit testing, accessibility semantics, or the native OS cursor.
    pub fn with_drag_badge(mut self, badge: DragBadge) -> Self {
        self.drag_badge = Some(badge);
        self
    }

    /// Declares this region as a target for one application-owned payload class.
    pub const fn with_drop_target(mut self, target: DropTarget<Msg>) -> Self {
        self.drop_target = Some(target);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BADGE_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x18, 0x08, 0x02, 0x00, 0x00, 0x00, 0x6f,
        0x15, 0xaa, 0xaf, 0x00, 0x00, 0x00, 0x1f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xb8,
        0xe3, 0xe0, 0x41, 0x15, 0xc4, 0x30, 0x6a, 0xd0, 0xa8, 0x41, 0xa3, 0x06, 0x8d, 0x1a, 0x34,
        0x6a, 0xd0, 0xa8, 0x41, 0x03, 0x6f, 0x10, 0x00, 0xe0, 0x82, 0x21, 0x2e, 0x6a, 0xcd, 0x6f,
        0xbc, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn pointer_message(_: PointerEvent) {}

    #[test]
    fn payload_retains_its_application_owned_identity() {
        let payload = DragPayload::new(DragPayloadKind::new(12), 99);

        assert_eq!(payload.kind().get(), 12);
        assert_eq!(payload.value(), 99);
    }

    #[test]
    fn modifiers_preserve_each_platform_independent_flag() {
        let modifiers = PointerModifiers::new(true, false, true, false);

        assert!(modifiers.shift());
        assert!(!modifiers.control());
        assert!(modifiers.alt());
        assert!(!modifiers.meta());
    }

    #[test]
    fn drag_badge_is_opt_in_on_a_pointer_region() {
        let without_badge = PointerRegionConfig::new(pointer_message);
        assert!(without_badge.drag_badge.is_none());

        let badge = DragBadge::new(Color::RED, Color::WHITE);
        let with_badge = without_badge.with_drag_badge(badge.clone());
        assert_eq!(
            with_badge.drag_badge.as_ref().map(|badge| badge.background),
            Some(badge.background)
        );
        assert!(!with_badge.capture_on_press);
    }

    #[test]
    fn badge_text_accepts_short_or_truncatable_text_but_rejects_controls_and_excess_bytes() {
        let badge = DragBadge::new(Color::RED, Color::WHITE);
        let long = badge.clone().with_text("A long badge label").unwrap();
        assert!(matches!(long.content, DragBadgeContent::Text(_)));
        assert!(badge.clone().with_text("\u{202e}fake").is_err());
        assert!(badge.clone().with_text("\n").is_err());
        assert!(badge.clone().with_text(" ".repeat(65)).is_err());
        assert!(badge.with_text("").is_err());
    }

    #[test]
    fn badge_keeps_only_a_small_decoded_copy_of_an_embedded_image() {
        let badge = DragBadge::new(Color::RED, Color::WHITE);
        let with_image = badge.clone().with_image(BADGE_PNG).unwrap();
        assert!(
            matches!(with_image.content, DragBadgeContent::Image(ref image)
            if image.width() == 20 && image.height() == 20)
        );
        let invalid = badge.clone().with_image(&[0, 1, 2]).unwrap_err();
        assert!(std::error::Error::source(&invalid).is_some());
        assert!(badge.with_image(&vec![0; 128 * 1024 + 1]).is_err());
    }
}
