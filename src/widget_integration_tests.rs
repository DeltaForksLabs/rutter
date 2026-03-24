// ============================================================
// Testes de integração — Widget tree (Fase 2)
// TextInput não carrega mais &mut Editor.
// ============================================================

use taffy::prelude::Style;
use rutter::{ButtonVariant, InputState, Widget};
use rutter::render::hit_test::collect_input_ids;

#[derive(Debug, Clone, PartialEq)]
enum Msg { Changed(String), Submit, Clear }

fn input(id: u64) -> Widget<'static, Msg> {
    Widget::TextInput {
        on_change:   |s| Msg::Changed(s),
        on_submit:   Some(Msg::Submit),
        style:       Style::default(),
        id,
        label:       "Field",
        placeholder: "type here",
        state:       InputState::Idle,
        error_msg:   None,
        is_password: false,
    }
}

// ── Construção básica ────────────────────────────────────────

#[test]
fn text_widget_stores_content_and_size() {
    let w: Widget<()> = Widget::Text {
        content: "Hello!".into(),
        style:   Style::default(),
        color:   None,
        size:    18.0,
    };
    if let Widget::Text { content, size, .. } = w {
        assert_eq!(content, "Hello!");
        assert!((size - 18.0).abs() < f32::EPSILON);
    }
}

#[test]
fn text_input_no_longer_borrows_editor() {
    // Simplesmente compilar é o teste: não há lifetime de Editor
    let _w = input(1);
}

#[test]
fn text_input_stores_id_and_label() {
    let w = input(42);
    if let Widget::TextInput { id, label, .. } = w {
        assert_eq!(id, 42);
        assert_eq!(label, "Field");
    }
}

#[test]
fn text_input_placeholder_stored() {
    let w = input(1);
    if let Widget::TextInput { placeholder, .. } = w {
        assert_eq!(placeholder, "type here");
    }
}

#[test]
fn text_input_on_change_produces_message() {
    let w = input(1);
    if let Widget::TextInput { on_change, .. } = w {
        assert_eq!(on_change("hello".into()), Msg::Changed("hello".into()));
    }
}

#[test]
fn text_input_on_submit_is_some() {
    let w = input(1);
    if let Widget::TextInput { on_submit, .. } = w {
        assert_eq!(on_submit, Some(Msg::Submit));
    }
}

#[test]
fn button_primary_variant() {
    let w: Widget<Msg> = Widget::Button {
        text: "OK", on_press: Msg::Submit,
        style: Style::default(), color: None,
        variant: ButtonVariant::Primary,
    };
    if let Widget::Button { variant, on_press, .. } = w {
        assert_eq!(variant, ButtonVariant::Primary);
        assert_eq!(on_press, Msg::Submit);
    }
}

#[test]
fn button_ghost_variant() {
    let w: Widget<Msg> = Widget::Button {
        text: "Cancel", on_press: Msg::Clear,
        style: Style::default(), color: None,
        variant: ButtonVariant::Ghost,
    };
    if let Widget::Button { variant, .. } = w {
        assert_eq!(variant, ButtonVariant::Ghost);
    }
}

#[test]
fn button_text_variant() {
    let w: Widget<Msg> = Widget::Button {
        text: "More", on_press: Msg::Clear,
        style: Style::default(), color: None,
        variant: ButtonVariant::Text,
    };
    if let Widget::Button { variant, .. } = w {
        assert_eq!(variant, ButtonVariant::Text);
    }
}

// ── collect_input_ids ────────────────────────────────────────

#[test]
fn collect_single_input_id() {
    let mut ids = vec![];
    collect_input_ids(&input(7), &mut ids);
    assert_eq!(ids, vec![7]);
}

#[test]
fn collect_three_inputs_in_column() {
    let w = Widget::Column::<Msg> {
        style: Style::default(),
        children: vec![input(1), input(2), input(3)],
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn collect_in_row() {
    let w = Widget::Row::<Msg> {
        style: Style::default(),
        children: vec![input(10), input(20)],
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![10, 20]);
}

#[test]
fn collect_skips_button_and_text() {
    let w: Widget<Msg> = Widget::Column {
        style: Style::default(),
        children: vec![
            Widget::Text { content: "x".into(), style: Style::default(), color: None, size: 14.0 },
            Widget::Button { text: "b", on_press: Msg::Submit, style: Style::default(), color: None, variant: ButtonVariant::Primary },
        ],
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert!(ids.is_empty());
}

#[test]
fn collect_nested_in_container() {
    let w = Widget::Container::<Msg> {
        style: Style::default(), color: None, radius: 4.0,
        child: Box::new(input(99)),
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![99]);
}

#[test]
fn collect_deep_tree() {
    let w: Widget<Msg> = Widget::Column {
        style: Style::default(),
        children: vec![
            Widget::Container {
                style: Style::default(), color: None, radius: 4.0,
                child: Box::new(Widget::Row {
                    style: Style::default(),
                    children: vec![input(100), input(200)],
                }),
            },
            input(300),
        ],
    };
    let mut ids = vec![];
    collect_input_ids(&w, &mut ids);
    assert_eq!(ids, vec![100, 200, 300]);
}

// ── InputState ───────────────────────────────────────────────

#[test]
fn input_state_default_is_idle() {
    assert_eq!(InputState::default(), InputState::Idle);
}

#[test]
fn input_states_are_distinct() {
    assert_ne!(InputState::Error, InputState::Success);
    assert_ne!(InputState::Idle,  InputState::Focused);
    assert_ne!(InputState::Error, InputState::Idle);
}

#[test]
fn text_input_default_state_is_idle() {
    let w = input(1);
    if let Widget::TextInput { state, .. } = w {
        assert_eq!(state, InputState::Idle);
    }
}

#[test]
fn text_input_error_state() {
    let w: Widget<Msg> = Widget::TextInput {
        on_change: |_| Msg::Clear,
        on_submit: None,
        style: Style::default(),
        id: 1, label: "x", placeholder: "",
        state: InputState::Error,
        error_msg: Some("required".into()),
        is_password: false,
    };
    if let Widget::TextInput { state, error_msg, .. } = w {
        assert_eq!(state, InputState::Error);
        assert_eq!(error_msg, Some("required".into()));
    }
}

#[test]
fn text_input_password_flag() {
    let w: Widget<Msg> = Widget::TextInput {
        on_change: |_| Msg::Clear,
        on_submit: None,
        style: Style::default(),
        id: 2, label: "Password", placeholder: "",
        state: InputState::Idle,
        error_msg: None,
        is_password: true,
    };
    if let Widget::TextInput { is_password, .. } = w {
        assert!(is_password);
    }
}
