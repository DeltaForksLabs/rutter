// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

mod sealed {
    pub trait Sealed {}
    impl Sealed for i64 {}
    impl Sealed for f64 {}
}

/// A supported numeric input to [`super::Widget::counter`].
///
/// Sealed to `i64` and `f64`. All four numeric arguments and the callback use
/// the same type; integer values never pass through floating-point arithmetic.
pub trait CounterInput: sealed::Sealed + Copy + fmt::Debug + fmt::Display + PartialOrd {
    #[doc(hidden)]
    fn finite(self) -> bool;
    #[doc(hidden)]
    fn positive(self) -> bool;
    #[doc(hidden)]
    fn configure<Msg>(parameters: CounterParameters<Self, Msg>) -> CounterConfig<Msg>;
}

impl CounterInput for i64 {
    fn finite(self) -> bool {
        true
    }
    fn positive(self) -> bool {
        self > 0
    }
    fn configure<Msg>(parameters: CounterParameters<Self, Msg>) -> CounterConfig<Msg> {
        CounterConfig::Integer(parameters)
    }
}

impl CounterInput for f64 {
    fn finite(self) -> bool {
        self.is_finite()
    }
    fn positive(self) -> bool {
        self > 0.0
    }
    fn configure<Msg>(parameters: CounterParameters<Self, Msg>) -> CounterConfig<Msg> {
        CounterConfig::Float(parameters)
    }
}

/// Typed numeric parameters for the single [`super::Widget::Counter`] widget.
#[derive(Debug, Clone)]
pub struct CounterParameters<N, Msg> {
    /// Current application-controlled value.
    pub value: N,
    /// Inclusive lower bound.
    pub min: N,
    /// Inclusive upper bound.
    pub max: N,
    /// Positive relative adjustment, not a grid anchored at the minimum.
    pub step: N,
    /// Produces an application message without changing the numeric type.
    pub on_change: fn(N) -> Msg,
}

/// Numeric configuration of a Counter, not separate widget kinds or ID families.
///
/// Prefer [`super::Widget::counter`] or [`super::Widget::try_counter`] to constructing
/// this enum directly. Keeping each mode's parameters together prevents mixed
/// integer/float bounds or a callback with the wrong numeric type.
#[derive(Debug, Clone)]
pub enum CounterConfig<Msg> {
    Integer(CounterParameters<i64, Msg>),
    Float(CounterParameters<f64, Msg>),
}

impl<Msg> CounterConfig<Msg> {
    /// The current value, preserving its numeric representation.
    pub fn value(&self) -> CounterValue {
        match self {
            Self::Integer(parameters) => CounterValue::Integer(parameters.value),
            Self::Float(parameters) => CounterValue::Float(parameters.value),
        }
    }
}

/// A Counter value without its callback or bounds, used by intrinsic layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterValue {
    Integer(i64),
    Float(f64),
}

impl fmt::Display for CounterValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(value) => value.fmt(formatter),
            Self::Float(value) => value.fmt(formatter),
        }
    }
}

impl From<i64> for CounterValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for CounterValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

/// Reports invalid Counter parameters; the default payload type remains `i64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterConfigError<N = i64> {
    InvalidRange { min: N, max: N },
    ValueOutOfRange { value: N, min: N, max: N },
    InvalidStep { step: N },
}

impl<N: fmt::Display> fmt::Display for CounterConfigError<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { min, max } => write!(
                formatter,
                "invalid counter range {min}..={max}, expected finite bounds with min <= max"
            ),
            Self::ValueOutOfRange { value, min, max } => write!(
                formatter,
                "invalid counter value {value}, expected a finite value within {min}..={max}"
            ),
            Self::InvalidStep { step } => write!(
                formatter,
                "invalid counter step {step}, expected a finite step > 0"
            ),
        }
    }
}

impl<N: fmt::Debug + fmt::Display> std::error::Error for CounterConfigError<N> {}

/// Validates integer or floating-point Counter parameters without converting them.
///
/// Bounds are inclusive and ordered, all numbers must be finite, and the step
/// must be positive. Values need not align to a step grid.
///
/// ```rust
/// use rutter::validate_counter;
/// assert!(validate_counter(1, 0, 9, 1).is_ok());
/// assert!(validate_counter(0.5, 0.0, 1.0, 0.1).is_ok());
/// assert!(validate_counter(f64::NAN, 0.0, 1.0, 0.1).is_err());
/// ```
pub fn validate_counter<N: CounterInput>(
    value: N,
    min: N,
    max: N,
    step: N,
) -> Result<(), CounterConfigError<N>> {
    if !min.finite() || !max.finite() || min > max {
        return Err(CounterConfigError::InvalidRange { min, max });
    }
    if !value.finite() || value < min || value > max {
        return Err(CounterConfigError::ValueOutOfRange { value, min, max });
    }
    if !step.finite() || !step.positive() {
        return Err(CounterConfigError::InvalidStep { step });
    }
    Ok(())
}
