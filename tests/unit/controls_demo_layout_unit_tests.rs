use super::*;

#[test]
fn checkbox_row_uses_explicit_widths_and_horizontal_spacing() {
    let mut state = ControlsDemoState::default();
    let view = ControlsDemo::view(&mut state);
    let Widget::Column { children, .. } = view else {
        panic!("controls demo root must be a Column")
    };
    let (checkboxes, style) = checkbox_row(&children);

    assert_eq!(style.flex_wrap, FlexWrap::Wrap);
    assert_eq!(style.gap.width, LengthPercentage::length(24.0));
    assert_eq!(style.gap.height, LengthPercentage::length(12.0));
    assert_eq!(style.max_size.width, Dimension::length(384.0));
    assert_eq!(checkboxes.len(), 2);
    for checkbox in checkboxes {
        let Widget::Checkbox { style, .. } = checkbox else {
            unreachable!()
        };
        assert_eq!(style.size.width, Dimension::length(180.0));
    }
}

fn checkbox_row<'view, 'widget>(
    children: &'view [Widget<'widget, Msg>],
) -> (&'view [Widget<'widget, Msg>], &'view Style) {
    children
        .iter()
        .find_map(|child| match child {
            Widget::Row { children, style }
                if children
                    .iter()
                    .all(|child| matches!(child, Widget::Checkbox { .. })) =>
            {
                Some((children.as_slice(), style))
            }
            _ => None,
        })
        .expect("controls demo must contain one Row with only Checkbox children")
}
