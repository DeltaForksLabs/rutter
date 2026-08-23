use super::*;

#[test]
fn time_demo_starts_with_a_closed_sao_paulo_picker() {
    let state = initial_time_demo_state();

    assert!(!state.picker_open);
    assert_eq!(state.selected_time, TimeOfDay::new(9, 30, 0).unwrap());
    assert_eq!(TIME_ZONES[state.selected_time_zone], "America/Sao_Paulo");
}

#[test]
fn time_picker_messages_update_only_the_selected_time_part() {
    let mut state = initial_time_demo_state();

    apply_time_demo_message(&mut state, Msg::MinuteChanged(45));
    apply_time_demo_message(&mut state, Msg::TimeZoneChanged(3));

    assert_eq!(state.selected_time, TimeOfDay::new(9, 45, 0).unwrap());
    assert_eq!(TIME_ZONES[state.selected_time_zone], "Asia/Tokyo");
}
