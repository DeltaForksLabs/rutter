use rutter::{WidgetConfigError, validate_slider, validate_virtual_grid, validate_virtual_list};

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
