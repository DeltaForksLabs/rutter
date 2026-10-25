// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use super::{PickerColor, paint};
use crate::{
    CustomAccessibility, CustomAccessibilityAction, CustomAccessibilityActions,
    CustomAccessibilityNode, CustomAccessibilityRole, CustomAccessibilityState, CustomEventOutcome,
    CustomInteraction, CustomPaintContext, CustomPointerEvent, CustomWidgetState, CustomWidgetV1,
    ShortcutEvent, ShortcutKey, ShortcutNamedKey,
};

pub(super) enum ColorControl<Msg> {
    Plane {
        hue: f32,
        saturation: f32,
        value: f32,
        on_change: fn((f32, f32)) -> Msg,
    },
    Hue {
        hue: f32,
        on_change: fn(f32) -> Msg,
    },
    Alpha {
        color: PickerColor,
        on_change: fn(f32) -> Msg,
    },
    Preview {
        color: PickerColor,
    },
}

impl<Msg> ColorControl<Msg> {
    fn adjusted(&self, horizontal: f32, vertical: f32, endpoint: Option<f32>) -> Option<Msg> {
        match *self {
            Self::Plane {
                saturation,
                value,
                on_change,
                ..
            } => {
                let s = endpoint.unwrap_or(saturation + horizontal).clamp(0.0, 1.0);
                let v = (value + vertical).clamp(0.0, 1.0);
                ((s, v) != (saturation, value)).then(|| on_change((s, v)))
            }
            Self::Hue { hue, on_change } => {
                let next = endpoint
                    .map_or(hue + (horizontal + vertical) * 360.0, |v| v * 360.0)
                    .clamp(0.0, 360.0);
                (next != hue).then(|| on_change(next))
            }
            Self::Alpha { color, on_change } => {
                let alpha = color.rgba()[3];
                let next = endpoint
                    .unwrap_or(alpha + horizontal + vertical)
                    .clamp(0.0, 1.0);
                (next != alpha).then(|| on_change(next))
            }
            Self::Preview { .. } => None,
        }
    }

    fn at_position(&self, x: f32, y: f32) -> Option<Msg> {
        match *self {
            Self::Plane {
                saturation,
                value,
                on_change,
                ..
            } => ((x, 1.0 - y) != (saturation, value)).then(|| on_change((x, 1.0 - y))),
            Self::Hue { hue, on_change } => (x * 360.0 != hue).then(|| on_change(x * 360.0)),
            Self::Alpha { color, on_change } => (x != color.rgba()[3]).then(|| on_change(x)),
            Self::Preview { .. } => None,
        }
    }
}

impl<Msg> CustomWidgetV1<Msg> for ColorControl<Msg> {
    fn paint(&self, context: CustomPaintContext<'_>) {
        paint::paint_control(self, context);
    }

    fn interaction(&self) -> CustomInteraction {
        if matches!(self, Self::Preview { .. }) {
            CustomInteraction::VisualOnly
        } else {
            CustomInteraction::PointerAndKeyboard
        }
    }

    fn pointer_event(
        &self,
        event: CustomPointerEvent,
        _: &mut CustomWidgetState,
    ) -> CustomEventOutcome<Msg> {
        if matches!(self, Self::Preview { .. }) {
            return CustomEventOutcome::Ignored;
        }
        let (position, bounds) = match event {
            CustomPointerEvent::Press { position, bounds }
            | CustomPointerEvent::Move { position, bounds }
            | CustomPointerEvent::Release { position, bounds } => (position, bounds),
        };
        if !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || !position.x.is_finite()
            || !position.y.is_finite()
        {
            return CustomEventOutcome::Ignored;
        }
        let message = self.at_position(
            (position.x / bounds.width).clamp(0.0, 1.0),
            (position.y / bounds.height).clamp(0.0, 1.0),
        );
        match (event, message) {
            (CustomPointerEvent::Press { .. }, Some(message)) => {
                CustomEventOutcome::MessageAndCapture(message)
            }
            (CustomPointerEvent::Press { .. }, None) => CustomEventOutcome::CapturePointer,
            (CustomPointerEvent::Release { .. }, None) => CustomEventOutcome::ReleasePointer,
            (_, Some(message)) => CustomEventOutcome::Message(message),
            (_, None) => CustomEventOutcome::Consumed,
        }
    }

    fn keyboard_event(
        &self,
        event: ShortcutEvent,
        _: &mut CustomWidgetState,
    ) -> CustomEventOutcome<Msg> {
        if event.control || event.alt || event.super_key || matches!(self, Self::Preview { .. }) {
            return CustomEventOutcome::Ignored;
        }
        let step = if event.shift { 0.1 } else { 0.01 };
        let (horizontal, vertical, endpoint) = match event.key {
            ShortcutKey::Named(ShortcutNamedKey::ArrowLeft) => (-step, 0.0, None),
            ShortcutKey::Named(ShortcutNamedKey::ArrowRight) => (step, 0.0, None),
            ShortcutKey::Named(ShortcutNamedKey::ArrowUp) => (0.0, step, None),
            ShortcutKey::Named(ShortcutNamedKey::ArrowDown) => (0.0, -step, None),
            ShortcutKey::Named(ShortcutNamedKey::PageUp) => (0.0, 0.1, None),
            ShortcutKey::Named(ShortcutNamedKey::PageDown) => (0.0, -0.1, None),
            ShortcutKey::Named(ShortcutNamedKey::Home) => (0.0, 0.0, Some(0.0)),
            ShortcutKey::Named(ShortcutNamedKey::End) => (0.0, 0.0, Some(1.0)),
            _ => return CustomEventOutcome::Ignored,
        };
        self.adjusted(horizontal, vertical, endpoint)
            .map_or(CustomEventOutcome::Consumed, CustomEventOutcome::Message)
    }

    fn accessibility(&self) -> CustomAccessibility {
        let (name, value, role) = match *self {
            Self::Plane {
                saturation, value, ..
            } => (
                "Saturation and value",
                format!(
                    "Saturation {:.1}%, value {:.1}%",
                    saturation * 100.0,
                    value * 100.0
                ),
                CustomAccessibilityRole::Slider,
            ),
            Self::Hue { hue, .. } => (
                "Hue",
                format!("{hue:.1} degrees"),
                CustomAccessibilityRole::Slider,
            ),
            Self::Alpha { color, .. } => (
                "Alpha",
                format!("{:.1}%", color.rgba()[3] * 100.0),
                CustomAccessibilityRole::Slider,
            ),
            Self::Preview { color } => (
                "Selected color",
                color.to_hex(),
                CustomAccessibilityRole::Image,
            ),
        };
        let interactive = !matches!(self, Self::Preview { .. });
        CustomAccessibility::Node(CustomAccessibilityNode {
            role,
            name: name.into(),
            value: Some(value),
            state: CustomAccessibilityState::Default,
            actions: CustomAccessibilityActions {
                click: false,
                increment: interactive,
                decrement: interactive,
            },
        })
    }

    fn accessibility_action(
        &self,
        action: CustomAccessibilityAction,
        _: &mut CustomWidgetState,
    ) -> CustomEventOutcome<Msg> {
        if matches!(self, Self::Preview { .. }) {
            return CustomEventOutcome::Ignored;
        }
        let step = match action {
            CustomAccessibilityAction::Increment => 0.01,
            CustomAccessibilityAction::Decrement => -0.01,
            _ => return CustomEventOutcome::Ignored,
        };
        self.adjusted(0.0, step, None)
            .map_or(CustomEventOutcome::Consumed, CustomEventOutcome::Message)
    }
}
