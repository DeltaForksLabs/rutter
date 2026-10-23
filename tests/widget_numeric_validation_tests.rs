use rutter::{
    CounterConfigError, Widget, WidgetConfigError, validate_counter, validate_slider,
    validate_virtual_grid, validate_virtual_list,
};

#[test]
fn counter_validation_rejects_invalid_ranges_values_and_steps() {
    assert_eq!(
        validate_counter(1, 3, 2, 1),
        Err(CounterConfigError::InvalidRange { min: 3, max: 2 })
    );
    assert_eq!(
        validate_counter(5, 0, 3, 1),
        Err(CounterConfigError::ValueOutOfRange {
            value: 5,
            min: 0,
            max: 3,
        })
    );
    assert_eq!(
        validate_counter(1, 0, 3, 0),
        Err(CounterConfigError::InvalidStep { step: 0 })
    );
    assert!(
        validate_counter(5, 0, 3, 1)
            .unwrap_err()
            .to_string()
            .contains("value 5")
    );
    assert!(validate_counter(-1, -3, 3, 2).is_ok());
}

#[test]
fn slider_validation_rejects_non_finite_and_reversed_ranges() {
    assert!(matches!(
        validate_slider(f32::NAN, 0.0, 1.0, 0.1),
        Err(WidgetConfigError::InvalidSliderRange)
    ));
    assert!(matches!(
        validate_slider(0.0, 2.0, 1.0, 0.1),
        Err(WidgetConfigError::InvalidSliderRange)
    ));
    assert!(validate_slider(0.5, 0.0, 1.0, 0.1).is_ok());
}

#[test]
fn float_counter_validation_rejects_nonfinite_numbers_in_every_argument() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for index in 0..4 {
            let mut args = [0.5, 0.0, 1.0, 0.25];
            args[index] = invalid;
            let [value, min, max, step] = args;
            let error = validate_counter(value, min, max, step).unwrap_err();
            match index {
                0 => assert!(matches!(error, CounterConfigError::ValueOutOfRange { .. })),
                1 | 2 => assert!(matches!(error, CounterConfigError::InvalidRange { .. })),
                3 => assert!(matches!(error, CounterConfigError::InvalidStep { .. })),
                _ => unreachable!(),
            }
            let widget: Result<Widget<'_, ()>, _> = Widget::try_counter(
                value,
                min,
                max,
                step,
                |_| (),
                taffy::Style::default(),
                "Amount",
            );
            assert!(
                widget.is_err(),
                "nonfinite argument {index} must be rejected"
            );
        }
    }
}

#[test]
fn float_counter_validation_preserves_inclusive_bounds_and_positive_fractional_steps() {
    assert_eq!(
        validate_counter(0.0, 1.0, -1.0, 0.1),
        Err(CounterConfigError::InvalidRange {
            min: 1.0,
            max: -1.0
        })
    );
    for value in [-1.25, 1.25] {
        assert_eq!(
            validate_counter(value, -1.0, 1.0, 0.25),
            Err(CounterConfigError::ValueOutOfRange {
                value,
                min: -1.0,
                max: 1.0
            })
        );
    }
    for step in [0.0, -0.0, -0.25] {
        let error = validate_counter(0.0, -1.0, 1.0, step).unwrap_err();
        assert_eq!(error, CounterConfigError::InvalidStep { step });
        assert!(error.to_string().contains("finite step > 0"));
    }
    for value in [-1.0, -0.125, 1.0] {
        assert!(validate_counter(value, -1.0, 1.0, 0.1).is_ok());
    }
    assert!(validate_counter(0.5, 0.5, 0.5, 0.125).is_ok());
    assert!(validate_counter(0.0, -f64::MAX, f64::MAX, f64::from_bits(1)).is_ok());
}

#[test]
fn virtual_item_validation_rejects_non_positive_structure() {
    assert!(matches!(
        validate_virtual_list(f32::INFINITY),
        Err(WidgetConfigError::InvalidVirtualItemHeight)
    ));
    assert!(matches!(
        validate_virtual_grid(0, 24.0),
        Err(WidgetConfigError::InvalidVirtualGridColumns)
    ));
    assert!(validate_virtual_grid(2, 24.0).is_ok());
}
