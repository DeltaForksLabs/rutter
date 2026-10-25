use super::*;
use crate::layout::build_taffy_tree;
use taffy::prelude::{Size, Style};

const FULL_ERROR: &str = "The channel must be a finite number between zero and one hundred. Correct the entire value before submitting this form.";
const SECRET: &str = "private-editor-value-917";

fn update_for(widget: &Widget<'_, ()>, sensitive: bool) -> TreeUpdate {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut input = InputWidgetState::new(&mut fonts.borrow_mut());
    input.set_sensitive(sensitive);
    input.try_set_text(&mut fonts.borrow_mut(), SECRET).unwrap();
    let inputs = HashMap::from([(17, input)]);
    let states = HashMap::new();
    let mut taffy = TaffyTree::new();
    let root = build_taffy_tree(&mut taffy, widget, fonts.clone(), &states);
    compute_layout(
        &mut taffy,
        root,
        PhysicalSize::new(160, 240),
        fonts,
        &RichTextRenderer::default(),
    );
    build_accessibility_update(
        &taffy,
        widget,
        root,
        AccessibilityInputs {
            input_states: &inputs,
            widget_states: &states,
            focused_widget_id: Some(17),
            viewport: (160.0, 240.0),
            direction: LayoutDirection::Ltr,
        },
    )
}

fn input_widget(
    multiline: bool,
    password: bool,
    state: InputState,
    error: Option<&str>,
) -> Widget<'static, ()> {
    let style = Style {
        size: Size::from_lengths(60.0, 40.0),
        ..Style::default()
    };
    if multiline {
        Widget::text_area(
            |_| (),
            None,
            style,
            "Channel",
            state,
            "",
            error.map(str::to_owned),
        )
    } else {
        // Keep a distinct controlled model value out of the description as well.
        Widget::controlled_text_input(
            "private-model-value-734",
            |_| (),
            None,
            style,
            "Channel",
            "",
            state,
            error.map(str::to_owned),
            password,
        )
    }
    .with_id(17)
}

#[test]
fn full_validation_message_and_invalid_state_are_accessible_including_disabled_fields() {
    for multiline in [false, true] {
        for enabled in [false, true] {
            let widget = input_widget(multiline, false, InputState::Error, Some(FULL_ERROR))
                .enabled(enabled);
            let update = update_for(&widget, false);
            let node = &update.nodes.iter().find(|(id, _)| id.0 == 17).unwrap().1;
            assert_eq!(node.description(), Some(FULL_ERROR));
            assert_eq!(node.invalid(), Some(Invalid::True));
            assert_eq!(node.is_disabled(), !enabled);
            assert_eq!(node.label(), Some("Channel"));
            assert_eq!(node.value(), Some(SECRET));
            assert!(!node.description().unwrap().contains(SECRET));
            assert!(
                !node
                    .description()
                    .unwrap()
                    .contains("private-model-value-734")
            );
            if !enabled {
                assert!(!node.supports_action(Action::Focus));
            }
        }
    }
}

#[test]
fn validation_metadata_never_exposes_password_or_sensitive_editor_values() {
    for (multiline, password, sensitive) in [
        (false, true, false),
        (false, false, true),
        (true, false, true),
    ] {
        for enabled in [false, true] {
            let widget = input_widget(multiline, password, InputState::Error, Some(FULL_ERROR))
                .enabled(enabled);
            let update = update_for(&widget, sensitive);
            let node = &update.nodes.iter().find(|(id, _)| id.0 == 17).unwrap().1;
            assert_eq!(node.description(), Some(FULL_ERROR));
            assert_eq!(node.invalid(), Some(Invalid::True));
            assert_eq!(node.value(), None);
            let serialized_debug = format!("{update:?}");
            assert!(!serialized_debug.contains(SECRET));
            assert!(!serialized_debug.contains("private-model-value-734"));
        }
    }
}

#[test]
fn validation_state_and_description_follow_current_feedback_without_stale_errors() {
    for multiline in [false, true] {
        for (state, error, expected_invalid, description) in [
            (InputState::Error, None, Some(Invalid::True), None),
            (
                InputState::Idle,
                Some(FULL_ERROR),
                Some(Invalid::True),
                Some(FULL_ERROR),
            ),
            (InputState::Idle, None, None, None),
            (InputState::Success, Some(""), None, None),
        ] {
            let update = update_for(&input_widget(multiline, false, state, error), false);
            let node = &update.nodes.iter().find(|(id, _)| id.0 == 17).unwrap().1;
            assert_eq!(node.invalid(), expected_invalid);
            assert_eq!(node.description(), description);
        }
    }
}
