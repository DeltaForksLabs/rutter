use std::error::Error;

use rutter::{
    AUTO_ID, InputState, RutterRunError, Widget, WidgetId, WidgetIdError, WidgetIdSnapshot,
};
use taffy::prelude::Style;

type TestWidget = Widget<'static, ()>;
const AUTOMATIC_NAMESPACE_BIT: u64 = 1 << 63;

fn password_input(id: u64) -> TestWidget {
    Widget::text_input(
        |_| (),
        None,
        Style::default(),
        "Password",
        "secret",
        InputState::Idle,
        None,
        true,
    )
    .with_id(id)
}

fn plain_input(id: u64) -> TestWidget {
    Widget::text_input(
        |_| (),
        None,
        Style::default(),
        "Name",
        "name",
        InputState::Idle,
        None,
        false,
    )
    .with_id(id)
}

fn text_area(id: u64) -> TestWidget {
    Widget::text_area(
        |_| (),
        None,
        Style::default(),
        "Notes",
        InputState::Idle,
        "notes",
        None,
    )
    .with_id(id)
}

fn column(children: Vec<TestWidget>) -> TestWidget {
    Widget::Column {
        children,
        style: Style::default(),
    }
}

fn tab_bar(id: u64) -> TestWidget {
    Widget::tab_bar(&["One", "Two"], 0, |_| (), Style::default()).with_id(id)
}

#[test]
fn regular_and_strong_text_are_transition_compatible() {
    let regular: TestWidget = Widget::Text {
        content: "2025".into(),
        style: Style::default(),
        color: None,
        size: 16.0,
    };
    let strong: TestWidget = Widget::strong_text("2026", Style::default(), None, 16.0);
    let regular_snapshot = WidgetIdSnapshot::capture(&regular).unwrap();
    let strong_snapshot = WidgetIdSnapshot::capture(&strong).unwrap();

    regular_snapshot
        .validate_transition_to(&strong_snapshot)
        .unwrap();
    regular_snapshot
        .validate_reconstruction(&strong_snapshot)
        .unwrap();
}

#[test]
fn manual_id_accepts_only_the_manual_namespace() {
    assert_eq!(WidgetId::manual(42).unwrap().get(), 42);
    assert!(matches!(
        WidgetId::manual(0),
        Err(WidgetIdError::ReservedValue { value: 0 })
    ));
    assert!(WidgetId::manual(AUTOMATIC_NAMESPACE_BIT).is_err());
    assert!(password_input(AUTO_ID).try_with_id(0).is_err());
    let assigned = password_input(AUTO_ID)
        .with_widget_id(WidgetId::manual(9).unwrap())
        .unwrap();
    assert!(matches!(assigned, Widget::TextInput { id: 9, .. }));
}

#[test]
fn typed_id_assignment_rejects_widgets_without_an_explicit_id_field() {
    let button = Widget::Button {
        text: "Save",
        on_press: (),
        style: Style::default(),
        color: None,
        variant: rutter::ButtonVariant::Primary,
    };

    assert!(matches!(
        button.try_with_id(9),
        Err(WidgetIdError::UnsupportedWidget { value: 9 })
    ));
}

#[test]
fn duplicate_password_and_text_area_id_reports_both_owners() {
    let tree = column(vec![password_input(42), text_area(42)]);
    let error = WidgetIdSnapshot::capture(&tree).unwrap_err();

    assert!(matches!(
        error,
        WidgetIdError::Duplicate {
            value: 42,
            first_type: "PasswordTextInput",
            first_path,
            second_type: "TextArea",
            second_path,
        } if first_path == vec![0] && second_path == vec![1]
    ));
}

#[test]
fn distinct_automatic_owners_are_accepted() {
    let tree = column(vec![password_input(AUTO_ID), text_area(AUTO_ID)]);
    assert!(WidgetIdSnapshot::capture(&tree).is_ok());
}

#[test]
fn transition_rejects_manual_id_reused_by_another_type() {
    let previous = WidgetIdSnapshot::capture(&password_input(17)).unwrap();
    let next = WidgetIdSnapshot::capture(&text_area(17)).unwrap();

    assert!(matches!(
        previous.validate_transition_to(&next),
        Err(WidgetIdError::IncompatibleReuse { value: 17, .. })
    ));
}

#[test]
fn transition_rejects_downgrading_a_password_input() {
    let previous = WidgetIdSnapshot::capture(&password_input(17)).unwrap();
    let next = WidgetIdSnapshot::capture(&plain_input(17)).unwrap();

    assert!(matches!(
        previous.validate_transition_to(&next),
        Err(WidgetIdError::IncompatibleReuse { value: 17, .. })
    ));
}

#[test]
fn transition_allows_manual_id_to_move_with_the_same_family() {
    let previous = WidgetIdSnapshot::capture(&password_input(17)).unwrap();
    let moved = WidgetIdSnapshot::capture(&column(vec![password_input(17)])).unwrap();

    assert_eq!(previous.validate_transition_to(&moved), Ok(()));
}

#[test]
fn manual_tab_bar_move_keeps_derived_ids_compatible() {
    let previous = WidgetIdSnapshot::capture(&tab_bar(23)).unwrap();
    let moved = WidgetIdSnapshot::capture(&column(vec![tab_bar(23)])).unwrap();

    assert_eq!(previous.validate_transition_to(&moved), Ok(()));
}

#[test]
fn strict_reconstruction_detects_a_different_owner_set() {
    let validated = WidgetIdSnapshot::capture(&column(vec![password_input(17)])).unwrap();
    let rebuilt =
        WidgetIdSnapshot::capture(&column(vec![password_input(17), text_area(AUTO_ID)])).unwrap();

    assert!(matches!(
        validated.validate_reconstruction(&rebuilt),
        Err(WidgetIdError::InconsistentTree { .. })
    ));
}

#[test]
fn strict_reconstruction_detects_non_identified_structure_changes() {
    let validated = WidgetIdSnapshot::capture(&column(vec![password_input(17)])).unwrap();
    let rebuilt = WidgetIdSnapshot::capture(&column(vec![
        password_input(17),
        Widget::Spacer {
            style: Style::default(),
        },
    ]))
    .unwrap();

    assert!(matches!(
        validated.validate_reconstruction(&rebuilt),
        Err(WidgetIdError::InconsistentStructure { .. })
    ));
}

#[test]
fn closed_popover_reserves_ids_for_its_declared_content() {
    let tree = Widget::popover(
        false,
        password_input(42),
        text_area(42),
        None,
        Style::default(),
        Style::default(),
    )
    .with_id(100);

    assert!(matches!(
        WidgetIdSnapshot::capture(&tree),
        Err(WidgetIdError::Duplicate { value: 42, .. })
    ));
}

#[test]
fn manual_id_cannot_collide_with_accessibility_leaf_path() {
    const SYNTHETIC_TEXT_ID_AT_PATH_1_0: u64 = 0x2314_bd40_01b6_b321;
    let tree = column(vec![
        password_input(SYNTHETIC_TEXT_ID_AT_PATH_1_0),
        Widget::Row {
            children: vec![Widget::Text {
                content: "Label".into(),
                style: Style::default(),
                color: None,
                size: 14.0,
            }],
            style: Style::default(),
        },
    ]);

    assert!(matches!(
        WidgetIdSnapshot::capture(&tree),
        Err(WidgetIdError::Duplicate {
            value: SYNTHETIC_TEXT_ID_AT_PATH_1_0,
            ..
        })
    ));
}

#[test]
fn direct_reserved_raw_id_is_rejected_by_tree_validation() {
    let widget: TestWidget = Widget::Spinner {
        id: AUTOMATIC_NAMESPACE_BIT | 23,
        style: Style::default(),
    };

    assert!(matches!(
        WidgetIdSnapshot::capture(&widget),
        Err(WidgetIdError::ReservedValue { value })
            if value == AUTOMATIC_NAMESPACE_BIT | 23
    ));
}

#[test]
fn runtime_override_error_reports_the_cache_and_offending_id() {
    let error = WidgetIdError::RuntimeOverride {
        value: 42,
        cache: "inputs",
    };

    assert!(error.to_string().contains("value 42"));
    assert!(error.to_string().contains("inputs"));
}

#[test]
fn runner_error_preserves_widget_validation_details() {
    let error = RutterRunError::from(WidgetIdError::ReservedValue { value: 0 });

    assert!(error.to_string().contains("value 0"));
    assert!(error.source().is_some());
}
