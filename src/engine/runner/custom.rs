// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::Point;
use winit::keyboard::Key;

use super::RutterRunner;
use crate::app::{AppLogic, shortcut_event_from_winit};
use crate::widget::custom::find_custom_widget;
use crate::widget::{
    CustomAccessibility, CustomAccessibilityAction, CustomEventOutcome, CustomPoint,
    CustomPointerEvent, CustomSize,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct CustomPointerCapture {
    pub(super) id: u64,
    origin: Point,
    bounds: CustomSize,
}

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn dispatch_custom_pointer(&mut self, id: u64, event: CustomPointerEvent) -> bool {
        let Some(outcome) = self.custom_pointer_outcome(id, event) else {
            return false;
        };
        self.apply_custom_outcome(id, outcome, Some(event))
    }

    pub(super) fn dispatch_captured_custom_move(&mut self) -> bool {
        let Some(capture) = self.custom_pointer_capture else {
            return false;
        };
        let event = CustomPointerEvent::Move {
            position: self.captured_custom_position(capture),
            bounds: capture.bounds,
        };
        self.dispatch_custom_pointer(capture.id, event)
    }

    pub(super) fn dispatch_captured_custom_release(&mut self) -> bool {
        let Some(capture) = self.custom_pointer_capture.take() else {
            return false;
        };
        let event = CustomPointerEvent::Release {
            position: self.captured_custom_position(capture),
            bounds: capture.bounds,
        };
        self.dispatch_custom_pointer(capture.id, event)
    }

    pub(super) fn dispatch_custom_keyboard(&mut self, id: u64, key: &Key, repeat: bool) -> bool {
        let event = shortcut_event_from_winit(key, self.engine.modifiers.state(), repeat);
        let Some(outcome) = self.custom_keyboard_outcome(id, event) else {
            return false;
        };
        self.apply_custom_outcome(id, outcome, None)
    }

    pub(super) fn dispatch_custom_accessibility_action(
        &mut self,
        id: u64,
        action: CustomAccessibilityAction,
    ) -> bool {
        let Some(outcome) = self.custom_accessibility_outcome(id, action) else {
            return false;
        };
        self.apply_custom_outcome(id, outcome, None)
    }

    fn custom_pointer_outcome(
        &mut self,
        id: u64,
        event: CustomPointerEvent,
    ) -> Option<CustomEventOutcome<A::Message>> {
        let widget_tree = A::view(&mut self.engine.app_state);
        let widget = find_custom_widget(&widget_tree, id)?;
        let state = self.engine.custom_widget_states.get_mut(&id)?;
        Some(widget.pointer_event(event, state))
    }

    fn custom_keyboard_outcome(
        &mut self,
        id: u64,
        event: crate::ShortcutEvent,
    ) -> Option<CustomEventOutcome<A::Message>> {
        let widget_tree = A::view(&mut self.engine.app_state);
        let widget = find_custom_widget(&widget_tree, id)?;
        let state = self.engine.custom_widget_states.get_mut(&id)?;
        Some(widget.keyboard_event(event, state))
    }

    fn custom_accessibility_outcome(
        &mut self,
        id: u64,
        action: CustomAccessibilityAction,
    ) -> Option<CustomEventOutcome<A::Message>> {
        let widget_tree = A::view(&mut self.engine.app_state);
        let widget = find_custom_widget(&widget_tree, id)?;
        let CustomAccessibility::Node(node) = widget.accessibility() else {
            return None;
        };
        if !node.actions.supports(action) {
            return None;
        }
        let state = self.engine.custom_widget_states.get_mut(&id)?;
        Some(widget.accessibility_action(action, state))
    }

    fn apply_custom_outcome(
        &mut self,
        id: u64,
        outcome: CustomEventOutcome<A::Message>,
        pointer_event: Option<CustomPointerEvent>,
    ) -> bool {
        match outcome {
            CustomEventOutcome::Ignored => false,
            CustomEventOutcome::Consumed => self.finish_custom_event(),
            CustomEventOutcome::Message(message) => self.dispatch_custom_message(message),
            CustomEventOutcome::CapturePointer => self.capture_custom_pointer(id, pointer_event),
            CustomEventOutcome::ReleasePointer => self.release_custom_pointer(),
            CustomEventOutcome::MessageAndCapture(message) => {
                self.dispatch_custom_message(message);
                self.capture_custom_pointer(id, pointer_event)
            }
        }
    }

    fn dispatch_custom_message(&mut self, message: A::Message) -> bool {
        A::update(
            &mut self.engine.app_state,
            message,
            &mut self.engine.clipboard,
        );
        self.finish_custom_event()
    }

    fn finish_custom_event(&mut self) -> bool {
        self.engine.layout_dirty = true;
        self.redraw();
        true
    }

    fn capture_custom_pointer(&mut self, id: u64, event: Option<CustomPointerEvent>) -> bool {
        let Some(CustomPointerEvent::Press { position, bounds }) = event else {
            return self.finish_custom_event();
        };
        let cursor = self.logical_cursor_position();
        self.custom_pointer_capture = Some(CustomPointerCapture {
            id,
            origin: Point::new(cursor.x - position.x, cursor.y - position.y),
            bounds,
        });
        self.finish_custom_event()
    }

    fn release_custom_pointer(&mut self) -> bool {
        self.custom_pointer_capture = None;
        self.finish_custom_event()
    }

    fn captured_custom_position(&self, capture: CustomPointerCapture) -> CustomPoint {
        let cursor = self.logical_cursor_position();
        CustomPoint {
            x: cursor.x - capture.origin.x,
            y: cursor.y - capture.origin.y,
        }
    }

    fn logical_cursor_position(&self) -> Point {
        Point::new(
            self.cursor_pos.x / self.engine.scale_factor,
            self.cursor_pos.y / self.engine.scale_factor,
        )
    }
}

#[cfg(test)]
mod tests {
    use arboard::Clipboard;
    use cosmic_text::FontSystem;
    use taffy::prelude::Style;
    use winit::keyboard::{Key, NamedKey};

    use super::RutterRunner;
    use crate::WidgetId;
    use crate::app::AppLogic;
    use crate::engine::RutterEngine;
    use crate::widget::{
        CustomAccessibility, CustomAccessibilityAction, CustomAccessibilityActions,
        CustomAccessibilityNode, CustomAccessibilityRole, CustomAccessibilityState,
        CustomEventOutcome, CustomInteraction, CustomPaintContext, CustomPointerEvent,
        CustomWidgetState, CustomWidgetV1, Widget,
    };

    #[derive(Clone, Debug, PartialEq)]
    enum CustomMessage {
        Pressed,
        Activated,
    }

    struct InteractiveCustom;

    impl CustomWidgetV1<CustomMessage> for InteractiveCustom {
        fn paint(&self, _: CustomPaintContext<'_>) {}

        fn interaction(&self) -> CustomInteraction {
            CustomInteraction::PointerAndKeyboard
        }

        fn pointer_event(
            &self,
            event: CustomPointerEvent,
            _: &mut CustomWidgetState,
        ) -> CustomEventOutcome<CustomMessage> {
            match event {
                CustomPointerEvent::Press { .. } => {
                    CustomEventOutcome::MessageAndCapture(CustomMessage::Pressed)
                }
                CustomPointerEvent::Release { .. } => CustomEventOutcome::ReleasePointer,
                CustomPointerEvent::Move { .. } => CustomEventOutcome::Consumed,
            }
        }

        fn keyboard_event(
            &self,
            event: crate::ShortcutEvent,
            _: &mut CustomWidgetState,
        ) -> CustomEventOutcome<CustomMessage> {
            match event.key {
                crate::ShortcutKey::Named(crate::ShortcutNamedKey::Escape) => {
                    CustomEventOutcome::Message(CustomMessage::Activated)
                }
                _ => CustomEventOutcome::Ignored,
            }
        }

        fn accessibility(&self) -> CustomAccessibility {
            CustomAccessibility::Node(CustomAccessibilityNode {
                role: CustomAccessibilityRole::Button,
                name: String::from("Activate"),
                value: None,
                state: CustomAccessibilityState::Default,
                actions: CustomAccessibilityActions {
                    click: true,
                    ..Default::default()
                },
            })
        }

        fn accessibility_action(
            &self,
            action: CustomAccessibilityAction,
            _: &mut CustomWidgetState,
        ) -> CustomEventOutcome<CustomMessage> {
            match action {
                CustomAccessibilityAction::Click => {
                    CustomEventOutcome::Message(CustomMessage::Activated)
                }
                _ => CustomEventOutcome::Ignored,
            }
        }
    }

    struct CustomApp;

    impl AppLogic for CustomApp {
        type State = Vec<CustomMessage>;
        type Message = CustomMessage;

        fn new(_: &mut FontSystem) -> Self::State {
            Vec::new()
        }

        fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
            Widget::custom(
                WidgetId::manual(601).unwrap(),
                InteractiveCustom,
                Style::default(),
            )
        }

        fn update(state: &mut Self::State, message: Self::Message, _: &mut Clipboard) {
            state.push(message);
        }
    }

    fn custom_runner() -> RutterRunner<CustomApp> {
        let engine = RutterEngine::<CustomApp>::new().unwrap();
        let mut runner = RutterRunner::with_engine(engine);
        runner.engine.try_ensure_widget_states().unwrap();
        runner
    }

    #[test]
    fn custom_pointer_message_reaches_application_update_and_captures_pointer() {
        let mut runner = custom_runner();
        let event = CustomPointerEvent::Press {
            position: crate::CustomPoint { x: 4.0, y: 6.0 },
            bounds: crate::CustomSize {
                width: 80.0,
                height: 30.0,
            },
        };

        assert!(runner.dispatch_custom_pointer(601, event));
        assert_eq!(runner.engine.app_state, vec![CustomMessage::Pressed]);
        assert_eq!(
            runner.custom_pointer_capture.map(|capture| capture.id),
            Some(601)
        );
    }

    #[test]
    fn custom_accessibility_action_reaches_application_update() {
        let mut runner = custom_runner();

        assert!(runner.dispatch_custom_accessibility_action(601, CustomAccessibilityAction::Click));
        assert_eq!(runner.engine.app_state, vec![CustomMessage::Activated]);
    }

    #[test]
    fn custom_keyboard_message_reaches_application_update() {
        let mut runner = custom_runner();

        assert!(runner.dispatch_custom_keyboard(601, &Key::Named(NamedKey::Escape), false));
        assert_eq!(runner.engine.app_state, vec![CustomMessage::Activated]);
    }
}
