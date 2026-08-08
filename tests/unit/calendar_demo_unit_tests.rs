use super::*;

struct FixedCalendarTodaySource {
    today: CalendarDate,
}

impl CalendarTodaySource for FixedCalendarTodaySource {
    fn today(&self) -> CalendarDate {
        self.today
    }
}

#[test]
fn initial_calendar_state_selects_today_in_both_calendars() {
    let today = CalendarDate::new(2027, 3, 14).unwrap();
    let source = FixedCalendarTodaySource { today };
    let state = initial_calendar_demo_state(&source);

    assert_eq!(state.calendar_month, today.calendar_month());
    assert_eq!(state.calendar_date, Some(today));
    assert_eq!(state.picker_month, today.calendar_month());
    assert_eq!(state.picker_date, Some(today));
    assert_eq!(
        state.picker_accessibility_label,
        "Data do evento: 2027-03-14"
    );
}
