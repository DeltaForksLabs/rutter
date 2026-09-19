// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use winit::event::KeyEvent;

use crate::app::{AppLogic, ShortcutEvent, ShortcutOutcome, shortcut_event_from_winit};

use super::RutterRunner;

impl<A: AppLogic + 'static> RutterRunner<A> {
    pub(super) fn handle_application_shortcut(&mut self, key_event: &KeyEvent) -> bool {
        let shortcut_event = shortcut_event_from_winit(
            &key_event.logical_key,
            self.engine.modifiers.state(),
            key_event.repeat,
        );
        if preserves_focused_input_text(
            self.engine.focused_input_id().is_some(),
            key_event.text.as_deref(),
            &shortcut_event,
        ) {
            return false;
        }

        self.dispatch_application_shortcut(shortcut_event)
    }

    fn dispatch_application_shortcut(&mut self, shortcut_event: ShortcutEvent) -> bool {
        match A::shortcut(&self.engine.app_state, shortcut_event) {
            ShortcutOutcome::Ignored => false,
            ShortcutOutcome::Consumed => true,
            ShortcutOutcome::Message(message) => {
                A::update(
                    &mut self.engine.app_state,
                    message,
                    &mut self.engine.clipboard,
                );
                self.engine.layout_dirty = true;
                self.redraw();
                true
            }
        }
    }
}

fn preserves_focused_input_text(
    input_is_focused: bool,
    committed_text: Option<&str>,
    event: &ShortcutEvent,
) -> bool {
    input_is_focused
        && committed_text.is_some_and(is_printable_text)
        && !event.control
        && !event.alt
        && !event.super_key
}

fn is_printable_text(text: &str) -> bool {
    !text.is_empty() && !text.chars().all(char::is_control)
}

#[cfg(test)]
mod tests {
    use arboard::Clipboard;
    use cosmic_text::FontSystem;

    use super::{ShortcutEvent, ShortcutOutcome, preserves_focused_input_text};
    use crate::app::{AppLogic, ShortcutKey};
    use crate::widget::Widget;

    #[derive(Clone, Debug, PartialEq)]
    enum ShortcutMessage {
        OpenLauncher,
    }

    struct ShortcutApp;

    impl AppLogic for ShortcutApp {
        type State = ();
        type Message = ShortcutMessage;

        fn new(_: &mut FontSystem) -> Self::State {}

        fn view<'a>(_: &'a mut Self::State) -> Widget<'a, Self::Message> {
            Widget::Spacer {
                style: Default::default(),
            }
        }

        fn update(_: &mut Self::State, _: Self::Message, _: &mut Clipboard) {}

        fn shortcut(_: &Self::State, event: ShortcutEvent) -> ShortcutOutcome<Self::Message> {
            match (&event.key, event.control, event.repeat) {
                (ShortcutKey::Character(key), true, false) if key.eq_ignore_ascii_case("p") => {
                    ShortcutOutcome::Message(ShortcutMessage::OpenLauncher)
                }
                (ShortcutKey::Character(key), true, true) if key.eq_ignore_ascii_case("p") => {
                    ShortcutOutcome::Consumed
                }
                _ => ShortcutOutcome::Ignored,
            }
        }
    }

    fn character_event(control: bool, repeat: bool) -> ShortcutEvent {
        ShortcutEvent {
            key: ShortcutKey::Character("p".into()),
            control,
            alt: false,
            shift: false,
            super_key: false,
            repeat,
        }
    }

    #[test]
    fn printable_input_text_bypasses_application_shortcuts() {
        assert!(preserves_focused_input_text(
            true,
            Some("p"),
            &character_event(false, false),
        ));
        assert!(preserves_focused_input_text(
            true,
            Some("P"),
            &ShortcutEvent {
                shift: true,
                ..character_event(false, false)
            },
        ));
        assert!(!preserves_focused_input_text(
            true,
            Some("p"),
            &character_event(true, false),
        ));
    }

    #[test]
    fn matched_modified_shortcut_returns_one_typed_message() {
        let outcome = ShortcutApp::shortcut(&(), character_event(true, false));

        assert_eq!(
            outcome,
            ShortcutOutcome::Message(ShortcutMessage::OpenLauncher)
        );
    }

    #[test]
    fn repeated_shortcut_is_observable_and_can_be_suppressed() {
        let outcome = ShortcutApp::shortcut(&(), character_event(true, true));

        assert_eq!(outcome, ShortcutOutcome::Consumed);
    }

    #[test]
    fn unmatched_shortcut_preserves_existing_toolkit_routing() {
        let mut event = character_event(true, false);
        event.key = ShortcutKey::Character("x".into());

        assert_eq!(ShortcutApp::shortcut(&(), event), ShortcutOutcome::Ignored);
    }
}
