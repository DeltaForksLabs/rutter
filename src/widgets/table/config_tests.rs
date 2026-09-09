use super::*;

#[test]
fn widths_reject_non_positive_and_non_finite_values() {
    assert!(matches!(
        TableColumnWidth::fixed(0.0),
        Err(TableConfigError::InvalidFixedWidth { value: 0.0 })
    ));
    assert!(matches!(
        TableColumnWidth::flex(f32::NAN, 1),
        Err(TableConfigError::InvalidFlexMinimumWidth { .. })
    ));
    assert!(matches!(
        TableColumnWidth::flex(80.0, 0),
        Err(TableConfigError::ZeroFlexWeight { value: 0 })
    ));
}

#[test]
fn metrics_reject_invalid_heights() {
    assert!(matches!(
        TableMetrics::new(-1.0, 44.0),
        Err(TableConfigError::InvalidHeaderHeight { value: -1.0 })
    ));
    assert!(matches!(
        TableMetrics::new(44.0, f32::INFINITY),
        Err(TableConfigError::InvalidRowHeight { .. })
    ));
}

#[test]
fn options_preserve_configured_values() {
    let metrics = TableMetrics::new(52.0, 36.0).unwrap();
    let options = TableOptions::<()>::default()
        .with_empty_label("No results")
        .with_metrics(metrics);

    assert_eq!(options.empty_label(), "No results");
    assert_eq!(options.metrics(), metrics);
}

#[test]
fn configuration_errors_include_offending_values_and_expectations() {
    let error = TableColumnWidth::fixed(-2.0).unwrap_err().to_string();

    assert!(error.contains("-2.0"));
    assert!(error.contains("finite value > 0"));
}
