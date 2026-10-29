//! Connection-independent, bounded native surface input policies.

use std::error::Error;
use std::fmt;

/// Maximum rectangles in a surface input region, bounding allocation and protocol work.
pub const MAX_SURFACE_INPUT_RECTANGLES: usize = 256;

/// A positive rectangle in surface-local **logical** integer coordinates.
///
/// Negative origins are valid (including `i32::MIN`). Width and height must be
/// positive `i32`s, and each origin plus extent must fit in `i32`. Rectangles
/// may extend outside the surface; the compositor intersects them with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceInputRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl SurfaceInputRect {
    /// Validates positive extents and checked right/bottom coordinate sums.
    pub const fn new(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Self, SurfaceInputRegionError> {
        if width <= 0 || height <= 0 {
            return Err(SurfaceInputRegionError::NonPositiveExtent { width, height });
        }
        if x.checked_add(width).is_none() || y.checked_add(height).is_none() {
            return Err(SurfaceInputRegionError::CoordinateOverflow { x, y, width, height });
        }
        Ok(Self { x, y, width, height })
    }

    /// Returns `(x, y, width, height)` in logical pixels; never scale these again.
    pub const fn logical_rect(self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.width, self.height)
    }
}

/// An owned, validated rectangle union. Construct through [`SurfaceInputRegion::rectangles`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceInputRectangles(Vec<SurfaceInputRect>);

impl SurfaceInputRectangles {
    /// Returns the validated rectangles in caller order. Overlap is permitted.
    pub fn as_slice(&self) -> &[SurfaceInputRect] {
        &self.0
    }
}

/// Native pointer/touch input region, independent of opacity and keyboard focus.
///
/// This changes neither alpha/opaque regions nor popup grabs, shell roles or
/// keyboard interactivity. On Wayland it is pending surface state applied by the
/// next surface commit; the setter requests a frame for that purpose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfaceInputRegion {
    /// Restore the native default (the entire surface); backward-compatible default.
    #[default]
    Full,
    /// Pass through surface input. This does not release grabs or keyboard focus.
    Empty,
    /// Union of bounded surface-local logical rectangles.
    Rectangles(SurfaceInputRectangles),
}

impl SurfaceInputRegion {
    /// Owns at most [`MAX_SURFACE_INPUT_RECTANGLES`] validated rectangles.
    /// An empty iterator is normalized to [`Self::Empty`]. Iteration stops after
    /// the first excess rectangle, even for an unbounded iterator.
    pub fn rectangles(
        rectangles: impl IntoIterator<Item = SurfaceInputRect>,
    ) -> Result<Self, SurfaceInputRegionError> {
        let mut owned = Vec::new();
        for rectangle in rectangles {
            if owned.len() == MAX_SURFACE_INPUT_RECTANGLES {
                return Err(SurfaceInputRegionError::TooManyRectangles);
            }
            owned.push(rectangle);
        }
        if owned.is_empty() {
            return Ok(Self::Empty);
        }
        Ok(Self::Rectangles(SurfaceInputRectangles(owned)))
    }
}

/// Invalid surface input geometry or excessive region complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfaceInputRegionError {
    /// Both extents must be strictly positive.
    NonPositiveExtent { width: i32, height: i32 },
    /// Right or bottom exceeds the signed protocol coordinate range.
    CoordinateOverflow { x: i32, y: i32, width: i32, height: i32 },
    /// More than [`MAX_SURFACE_INPUT_RECTANGLES`] rectangles were supplied.
    TooManyRectangles,
}

impl fmt::Display for SurfaceInputRegionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositiveExtent { width, height } => write!(
                f,
                "invalid surface input extent {width}x{height}; expected positive logical i32 \
                 extents"
            ),
            Self::CoordinateOverflow { x, y, width, height } => write!(
                f,
                "surface input rectangle ({x}, {y}, {width}, {height}) overflows; expected i32 \
                 right/bottom coordinates"
            ),
            Self::TooManyRectangles => {
                write!(f, "surface input region exceeds {MAX_SURFACE_INPUT_RECTANGLES} rectangles")
            },
        }
    }
}

impl Error for SurfaceInputRegionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_rect_checks_extents_and_signed_coordinate_boundaries() {
        for (width, height) in [(0, 1), (1, 0), (-1, 2), (2, i32::MIN)] {
            assert!(matches!(
                SurfaceInputRect::new(0, 0, width, height),
                Err(SurfaceInputRegionError::NonPositiveExtent { .. })
            ));
        }
        for (x, y) in [(i32::MAX, 0), (0, i32::MAX)] {
            assert!(matches!(
                SurfaceInputRect::new(x, y, 1, 1),
                Err(SurfaceInputRegionError::CoordinateOverflow { .. })
            ));
        }
        let negative = SurfaceInputRect::new(i32::MIN, -20, i32::MAX, 40).unwrap();
        assert_eq!(negative.logical_rect(), (i32::MIN, -20, i32::MAX, 40));
        assert!(SurfaceInputRect::new(i32::MAX - 1, i32::MAX - 1, 1, 1).is_ok());
    }

    #[test]
    fn input_region_is_owned_bounded_and_empty_is_pass_through() {
        assert_eq!(SurfaceInputRegion::default(), SurfaceInputRegion::Full);
        assert_eq!(SurfaceInputRegion::rectangles([]).unwrap(), SurfaceInputRegion::Empty);
        let rect = SurfaceInputRect::new(-4, 8, 20, 10).unwrap();
        let mut source = vec![rect; MAX_SURFACE_INPUT_RECTANGLES];
        let region = SurfaceInputRegion::rectangles(source.iter().copied()).unwrap();
        source.clear();
        let SurfaceInputRegion::Rectangles(rectangles) = region.clone() else { panic!() };
        assert_eq!(rectangles.as_slice(), vec![rect; MAX_SURFACE_INPUT_RECTANGLES]);
        assert_eq!(
            SurfaceInputRegion::rectangles(std::iter::repeat(rect)),
            Err(SurfaceInputRegionError::TooManyRectangles)
        );
    }
}
