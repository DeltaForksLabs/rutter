// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use skia_safe::Rect;

use super::{DynamicIsland, DynamicIslandError, DynamicIslandMode};
use crate::{SurfaceInputRect, SurfaceInputRegion};

impl DynamicIsland {
    /// Constructs a native input policy from explicit surface-local **logical**
    /// painted bounds (not a viewport or an allocation including outer margins).
    ///
    /// Passive returns `Empty` after validating geometry. Interactive approximates
    /// the rounded rectangle with one-logical-pixel corner bands and one middle
    /// band, merging equal adjacent spans. Pixel centers select included cells:
    /// each cell can extend at most one logical pixel beyond the optical curve;
    /// no full-rectangle fallback swallows transparent host padding. Fractional
    /// edges round to nearest integer pixel boundaries (ties toward negative).
    /// Radius clamps to half the smaller extent, as for rounded painting.
    /// Effective radii above 127px are rejected to bound work and preserve the
    /// 256-rectangle limit; very thin geometry may normalize to `Empty`.
    ///
    /// This does **not** publish a native region or infer live widget bounds.
    /// The host must recompute after layout/expansion/resize and submit it through
    /// `SurfaceCommand::SetInputRegion`. Do not scale logical coordinates again.
    ///
    /// ```
    /// use rutter::{DynamicIsland, skia_safe::Rect};
    /// let island = DynamicIsland::interactive();
    /// let policy = island.input_region(Rect::from_xywh(8.0, 12.0, 320.0, 76.0),
    ///                                  DynamicIsland::RADIUS)?;
    /// # Ok::<(), rutter::DynamicIslandError>(())
    /// ```
    pub fn input_region(
        &self,
        bounds: Rect,
        radius: f32,
    ) -> Result<SurfaceInputRegion, DynamicIslandError> {
        let [left, top, right, bottom] =
            [bounds.left, bounds.top, bounds.right, bounds.bottom].map(f64::from);
        let radius = f64::from(radius);
        if ![left, top, right, bottom, radius]
            .iter()
            .all(|n| n.is_finite())
            || right <= left
            || bottom <= top
            || radius < 0.0
            || left < f64::from(i32::MIN)
            || top < f64::from(i32::MIN)
            || right > f64::from(i32::MAX)
            || bottom > f64::from(i32::MAX)
            || right - left > f64::from(i32::MAX)
            || bottom - top > f64::from(i32::MAX)
        {
            return Err(DynamicIslandError::InvalidGeometry);
        }
        let radius = radius.min((right - left) / 2.0).min((bottom - top) / 2.0);
        if self.mode() == DynamicIslandMode::Passive {
            return Ok(SurfaceInputRegion::Empty);
        }
        if radius > 127.0 {
            return Err(DynamicIslandError::GeometryTooComplex);
        }
        let first = pixel_edge(top);
        let end = pixel_edge(bottom);
        let mut rectangles: Vec<SurfaceInputRect> = Vec::new();
        let mut y = first;
        while y < end {
            let center = f64::from(y) + 0.5;
            let dy = (top + radius - center)
                .max(center - (bottom - radius))
                .max(0.0);
            let inset = if radius == 0.0 {
                0.0
            } else {
                radius - (radius * radius - dy * dy).max(0.0).sqrt()
            };
            let x = pixel_edge(left + inset);
            let width = pixel_edge(right - inset) - x;
            // Skip the flat middle in O(1), even for very tall documents.
            let next = if dy == 0.0 {
                pixel_edge(bottom - radius).max(y + 1).min(end)
            } else {
                y + 1
            };
            if width > 0 {
                if let Some(last) = rectangles.last_mut()
                    && let (lx, ly, lw, lh) = last.logical_rect()
                    && lx == x
                    && lw == width
                    && ly + lh == y
                {
                    *last = SurfaceInputRect::new(x, ly, width, next - ly)
                        .map_err(DynamicIslandError::InputRegion)?;
                } else {
                    rectangles.push(
                        SurfaceInputRect::new(x, y, width, next - y)
                            .map_err(DynamicIslandError::InputRegion)?,
                    );
                }
            }
            y = next;
        }
        SurfaceInputRegion::rectangles(rectangles).map_err(DynamicIslandError::InputRegion)
    }
}

fn pixel_edge(value: f64) -> i32 {
    (value - 0.5).ceil() as i32
}
