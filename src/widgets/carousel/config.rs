// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

const MIN_UNCONTAINED_EXTENT: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CarouselSizing {
    Uncontained { item_extent: f32 },
    Weighted { flex_weights: Box<[u16]> },
}

/// Validated layout and interaction options for [`Widget::carousel_view`](crate::Widget::carousel_view).
#[derive(Debug, Clone, PartialEq)]
pub struct CarouselConfig {
    pub(crate) sizing: CarouselSizing,
    pub(crate) item_snapping: bool,
    pub(crate) accessibility_label: String,
}

/// Reports an invalid value supplied while configuring a carousel.
#[derive(Debug, Clone, PartialEq)]
pub enum CarouselConfigError {
    InvalidItemExtent { value: f32 },
    EmptyFlexWeights,
    ZeroFlexWeight { index: usize, value: u16 },
}

impl fmt::Display for CarouselConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidItemExtent { value } => write!(
                formatter,
                "invalid carousel item extent {value:?}, expected a finite value >= {MIN_UNCONTAINED_EXTENT} logical pixel"
            ),
            Self::EmptyFlexWeights => formatter.write_str(
                "invalid carousel flex weights [], expected at least one positive integer",
            ),
            Self::ZeroFlexWeight { index, value } => write!(
                formatter,
                "invalid carousel flex_weights[{index}] = {value}, expected every weight > 0"
            ),
        }
    }
}

impl std::error::Error for CarouselConfigError {}

impl CarouselConfig {
    /// Creates an uncontained layout whose items keep one fixed horizontal extent.
    ///
    /// ```rust
    /// use rutter::CarouselConfig;
    /// let config = CarouselConfig::uncontained(240.0).unwrap();
    /// ```
    pub fn uncontained(item_extent: f32) -> Result<Self, CarouselConfigError> {
        if !item_extent.is_finite() || item_extent < MIN_UNCONTAINED_EXTENT {
            return Err(CarouselConfigError::InvalidItemExtent { value: item_extent });
        }
        Ok(Self::new(CarouselSizing::Uncontained { item_extent }))
    }

    /// Creates a layout whose visible items interpolate between relative weights.
    ///
    /// ```rust
    /// use rutter::CarouselConfig;
    /// let config = CarouselConfig::weighted([1, 6, 1]).unwrap();
    /// ```
    pub fn weighted(flex_weights: impl Into<Vec<u16>>) -> Result<Self, CarouselConfigError> {
        let flex_weights = flex_weights.into();
        validate_flex_weights(&flex_weights)?;
        Ok(Self::new(CarouselSizing::Weighted {
            flex_weights: flex_weights.into_boxed_slice(),
        }))
    }

    /// Enables or disables settling on item boundaries after each scroll input.
    ///
    /// ```rust
    /// use rutter::CarouselConfig;
    /// let config = CarouselConfig::weighted([1, 4, 1]).unwrap().with_item_snapping(true);
    /// ```
    #[must_use]
    pub fn with_item_snapping(mut self, enabled: bool) -> Self {
        self.item_snapping = enabled;
        self
    }

    /// Sets the collection label announced by accessibility clients.
    ///
    /// ```rust
    /// use rutter::CarouselConfig;
    /// let config = CarouselConfig::uncontained(200.0).unwrap()
    ///     .with_accessibility_label("Featured projects");
    /// ```
    #[must_use]
    pub fn with_accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = label.into();
        self
    }

    fn new(sizing: CarouselSizing) -> Self {
        Self {
            sizing,
            item_snapping: false,
            accessibility_label: "Carousel".into(),
        }
    }
}

fn validate_flex_weights(flex_weights: &[u16]) -> Result<(), CarouselConfigError> {
    if flex_weights.is_empty() {
        return Err(CarouselConfigError::EmptyFlexWeights);
    }
    match flex_weights.iter().position(|weight| *weight == 0) {
        Some(index) => Err(CarouselConfigError::ZeroFlexWeight {
            index,
            value: flex_weights[index],
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncontained_rejects_non_finite_extent_with_value() {
        let error = CarouselConfig::uncontained(f32::NAN).unwrap_err();
        assert!(error.to_string().contains("NaN"));
    }

    #[test]
    fn uncontained_rejects_subpixel_extent_with_value() {
        let error = CarouselConfig::uncontained(0.5).unwrap_err();
        assert!(error.to_string().contains("0.5"));
        assert!(error.to_string().contains(">= 1"));
    }

    #[test]
    fn uncontained_accepts_positive_extent() {
        let config = CarouselConfig::uncontained(240.0).unwrap();
        assert_eq!(
            config.sizing,
            CarouselSizing::Uncontained { item_extent: 240.0 }
        );
    }

    #[test]
    fn weighted_rejects_empty_weights() {
        let error = CarouselConfig::weighted(Vec::<u16>::new()).unwrap_err();
        assert_eq!(error, CarouselConfigError::EmptyFlexWeights);
    }

    #[test]
    fn weighted_identifies_zero_weight_index() {
        let error = CarouselConfig::weighted([1, 0, 3]).unwrap_err();
        assert_eq!(
            error,
            CarouselConfigError::ZeroFlexWeight { index: 1, value: 0 }
        );
    }

    #[test]
    fn weighted_owns_validated_weights() {
        let config = CarouselConfig::weighted([1, 6, 1]).unwrap();
        let CarouselSizing::Weighted { flex_weights } = config.sizing else {
            panic!("expected weighted carousel sizing");
        };
        assert_eq!(flex_weights.as_ref(), &[1, 6, 1]);
    }

    #[test]
    fn builders_update_snapping_and_accessibility_label() {
        let config = CarouselConfig::uncontained(180.0)
            .unwrap()
            .with_item_snapping(true)
            .with_accessibility_label("Recent files");
        assert!(config.item_snapping);
        assert_eq!(config.accessibility_label, "Recent files");
    }
}
