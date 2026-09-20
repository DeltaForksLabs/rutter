// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

use skia_safe::Canvas;
use taffy::prelude::Style;

use crate::app::ShortcutEvent;
use crate::i18n::LayoutDirection;
use crate::theme::Theme;

/// Version number for the supported custom-widget extension contract.
pub const CUSTOM_WIDGET_API_VERSION: u16 = 1;

/// Maximum bytes retained for one custom widget's event-loop runtime state.
pub const MAX_CUSTOM_WIDGET_STATE_BYTES: usize = 64 * 1024;

/// A logical point inside a custom widget's measured bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomPoint {
    pub x: f32,
    pub y: f32,
}

/// A logical size resolved by Rutter's layout engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomSize {
    pub width: f32,
    pub height: f32,
}

/// Layout information provided to a custom widget during painting.
#[derive(Clone, Copy)]
pub struct CustomLayout<'a> {
    /// The Taffy constraints declared when constructing [`Widget::custom`].
    pub style: &'a Style,
    /// The final logical size after parent constraints, direction, and scale handling.
    pub measured_size: CustomSize,
}

/// Event-loop-owned bytes retained for a custom widget's stable manual ID.
///
/// Store compact, application-defined state here when keeping it in the
/// application model would be inappropriate. The state is removed when the
/// custom widget ID disappears from the view tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomWidgetState {
    bytes: Vec<u8>,
}

/// Reports a rejected custom runtime-state mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomWidgetStateError {
    BytesExceeded { actual: usize, maximum: usize },
    AllocationFailed { requested: usize, maximum: usize },
}

impl fmt::Display for CustomWidgetStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BytesExceeded { actual, maximum } => write!(
                formatter,
                "custom widget state has {actual} bytes; expected at most {maximum} bytes"
            ),
            Self::AllocationFailed { requested, maximum } => write!(
                formatter,
                "custom widget state could not reserve {requested} bytes; expected an allocation up to {maximum} bytes"
            ),
        }
    }
}

impl std::error::Error for CustomWidgetStateError {}

impl CustomWidgetState {
    /// Returns the bytes retained for this custom widget.
    ///
    /// ```
    /// use rutter::CustomWidgetState;
    ///
    /// assert!(CustomWidgetState::default().bytes().is_empty());
    /// ```
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Replaces retained bytes after enforcing Rutter's per-widget limit.
    ///
    /// ```
    /// use rutter::CustomWidgetState;
    ///
    /// let mut state = CustomWidgetState::default();
    /// state.replace_bytes(&[1, 2]).unwrap();
    /// assert_eq!(state.bytes(), &[1, 2]);
    /// ```
    pub fn replace_bytes(&mut self, bytes: &[u8]) -> Result<(), CustomWidgetStateError> {
        if bytes.len() > MAX_CUSTOM_WIDGET_STATE_BYTES {
            return Err(CustomWidgetStateError::BytesExceeded {
                actual: bytes.len(),
                maximum: MAX_CUSTOM_WIDGET_STATE_BYTES,
            });
        }
        let mut replacement = Vec::new();
        replacement.try_reserve_exact(bytes.len()).map_err(|_| {
            CustomWidgetStateError::AllocationFailed {
                requested: bytes.len(),
                maximum: MAX_CUSTOM_WIDGET_STATE_BYTES,
            }
        })?;
        replacement.extend_from_slice(bytes);
        self.bytes = replacement;
        Ok(())
    }

    /// Clears all retained bytes for this custom widget.
    ///
    /// ```
    /// use rutter::CustomWidgetState;
    ///
    /// let mut state = CustomWidgetState::default();
    /// state.replace_bytes(&[1]).unwrap();
    /// state.clear();
    /// assert!(state.bytes().is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.bytes.clear();
    }
}

/// Declares which normal input routes a custom widget participates in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomInteraction {
    /// The node paints only and cannot receive pointer or keyboard input.
    #[default]
    VisualOnly,
    /// The node can receive primary-pointer input but cannot receive keyboard focus.
    Pointer,
    /// The node can receive keyboard focus and normalized pressed-key events.
    Keyboard,
    /// The node can receive primary-pointer input and keyboard focus.
    PointerAndKeyboard,
}

impl CustomInteraction {
    pub(crate) const fn supports_pointer(self) -> bool {
        matches!(self, Self::Pointer | Self::PointerAndKeyboard)
    }

    pub(crate) const fn supports_keyboard(self) -> bool {
        matches!(self, Self::Keyboard | Self::PointerAndKeyboard)
    }
}

/// A framework-routed primary-pointer event for a custom widget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CustomPointerEvent {
    Press {
        position: CustomPoint,
        bounds: CustomSize,
    },
    Move {
        position: CustomPoint,
        bounds: CustomSize,
    },
    Release {
        position: CustomPoint,
        bounds: CustomSize,
    },
}

/// A supported custom accessibility action requested through AccessKit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomAccessibilityAction {
    Click,
    Increment,
    Decrement,
}

/// Actions a custom accessibility node exposes to assistive technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CustomAccessibilityActions {
    pub click: bool,
    pub increment: bool,
    pub decrement: bool,
}

impl CustomAccessibilityActions {
    pub(crate) const fn supports(self, action: CustomAccessibilityAction) -> bool {
        match action {
            CustomAccessibilityAction::Click => self.click,
            CustomAccessibilityAction::Increment => self.increment,
            CustomAccessibilityAction::Decrement => self.decrement,
        }
    }

    pub(crate) const fn is_empty(self) -> bool {
        !self.click && !self.increment && !self.decrement
    }
}

/// Framework-approved AccessKit roles for a custom widget's single semantic node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomAccessibilityRole {
    Button,
    CheckBox,
    Switch,
    RadioButton,
    Slider,
    ProgressIndicator,
    Image,
    StaticText,
}

/// One supported state exposed by a custom accessibility node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomAccessibilityState {
    #[default]
    Default,
    Toggled(bool),
    Selected(bool),
    Expanded(bool),
    Busy,
}

/// Declarative metadata for one framework-owned custom accessibility node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomAccessibilityNode {
    pub role: CustomAccessibilityRole,
    pub name: String,
    pub value: Option<String>,
    pub state: CustomAccessibilityState,
    pub actions: CustomAccessibilityActions,
}

/// Accessibility participation for a custom widget.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CustomAccessibility {
    /// Explicitly omit a semantic node. Visual-only widgets must use this or a node without actions.
    #[default]
    VisualOnly,
    /// Emit one Rutter-owned AccessKit node using the custom widget's stable ID.
    Node(CustomAccessibilityNode),
}

/// The result of a custom input or accessibility callback.
#[must_use = "custom event outcomes must be returned to the Rutter runtime"]
pub enum CustomEventOutcome<Message> {
    /// Leave the event to the framework's remaining routing when applicable.
    Ignored,
    /// Consume the event without dispatching an application message.
    Consumed,
    /// Dispatch the message through the normal application update callback.
    Message(Message),
    /// Retain primary-pointer capture until a callback returns `ReleasePointer`.
    CapturePointer,
    /// End a previously granted primary-pointer capture.
    ReleasePointer,
    /// Dispatch a message and retain primary-pointer capture.
    MessageAndCapture(Message),
}

/// A confined drawing context for a custom widget's current frame.
///
/// `canvas` records into a private picture; Rutter replays that picture through
/// the node's clip after the callback returns. It is never the window canvas.
pub struct CustomPaintContext<'a> {
    pub canvas: &'a Canvas,
    pub layout: CustomLayout<'a>,
    pub theme: &'a Theme,
    pub scale_factor: f32,
    pub direction: LayoutDirection,
    pub is_focused: bool,
    pub is_hovered: bool,
    pub runtime_state: &'a CustomWidgetState,
}

/// Version 1 of Rutter's supported custom leaf-widget contract.
///
/// Implementations are rebuilt from `AppLogic::view`; only
/// [`CustomWidgetState`] is retained by Rutter, keyed by the manual ID passed
/// to [`Widget::custom`]. Callbacks run on the event-loop thread and can only
/// return typed application messages or mutate that bounded runtime state.
///
/// ```
/// use rutter::{CustomPaintContext, CustomWidgetV1};
///
/// struct Line;
/// impl CustomWidgetV1<()> for Line {
///     fn paint(&self, _: CustomPaintContext<'_>) {}
/// }
/// ```
pub trait CustomWidgetV1<Message> {
    /// Paints this leaf into Rutter's isolated, local recording canvas.
    fn paint(&self, context: CustomPaintContext<'_>);

    /// Declares the standard input channels this leaf needs.
    fn interaction(&self) -> CustomInteraction {
        CustomInteraction::VisualOnly
    }

    /// Receives declared primary-pointer events on the event-loop thread.
    fn pointer_event(
        &self,
        _: CustomPointerEvent,
        _: &mut CustomWidgetState,
    ) -> CustomEventOutcome<Message> {
        CustomEventOutcome::Ignored
    }

    /// Receives declared, normalized pressed-key events while this leaf is focused.
    fn keyboard_event(
        &self,
        _: ShortcutEvent,
        _: &mut CustomWidgetState,
    ) -> CustomEventOutcome<Message> {
        CustomEventOutcome::Ignored
    }

    /// Declares one validated, framework-owned accessibility node or explicit visual-only status.
    fn accessibility(&self) -> CustomAccessibility {
        CustomAccessibility::VisualOnly
    }

    /// Handles a declared AccessKit action on the event-loop thread.
    fn accessibility_action(
        &self,
        _: CustomAccessibilityAction,
        _: &mut CustomWidgetState,
    ) -> CustomEventOutcome<Message> {
        CustomEventOutcome::Ignored
    }
}
