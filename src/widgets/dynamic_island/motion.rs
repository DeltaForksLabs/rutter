// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Opt-in host geometry animation, independent of widget semantics and clocks.

use std::time::{Duration, Instant};

use skia_safe::Rect;

use super::DynamicIslandError;

/// Explicit capsule entry/exit directions relative to the host's edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicIslandSlide {
    /// Enter from the left edge.
    SlideInLeft,
    /// Enter from the right edge.
    SlideInRight,
    /// Travel upward into view, starting below the host.
    SlideInUp,
    /// Travel downward into view, starting above the host.
    SlideInDown,
    /// Exit toward the left edge.
    SlideOutLeft,
    /// Exit toward the right edge.
    SlideOutRight,
    /// Exit above the host.
    SlideOutUp,
    /// Exit below the host.
    SlideOutDown,
}

impl DynamicIslandSlide {
    /// Whether this transition reveals rather than hides the capsule.
    pub const fn is_entry(self) -> bool {
        matches!(
            self,
            Self::SlideInLeft | Self::SlideInRight | Self::SlideInUp | Self::SlideInDown
        )
    }

    fn outside(self, host: Rect, bounds: Rect) -> Rect {
        let (x, y) = match self {
            Self::SlideInLeft | Self::SlideOutLeft => (host.left - bounds.right, 0.0),
            Self::SlideInRight | Self::SlideOutRight => (host.right - bounds.left, 0.0),
            Self::SlideInUp | Self::SlideOutDown => (0.0, host.bottom - bounds.top),
            Self::SlideInDown | Self::SlideOutUp => (0.0, host.top - bounds.bottom),
        };
        Rect::new(
            bounds.left + x,
            bounds.top + y,
            bounds.right + x,
            bounds.bottom + y,
        )
    }
}

#[derive(Clone, Debug)]
struct Transition {
    from: Rect,
    to: Rect,
    start: Instant,
    duration: Duration,
    slide: Option<DynamicIslandSlide>,
}

/// Deterministic, interruptible capsule geometry controller in logical pixels.
///
/// Call `sample_at` once per host wakeup and share `bounds()` between layout,
/// painting and native input. No clock is read internally. Easing is finite
/// smoothstep (zero endpoint velocity); zero duration completes immediately.
/// Retargeting samples the old transition first, preserving position/size, not
/// velocity. Visible target bounds must fit the actual host. Translation may
/// cross its edges: clip painting/input to the host, **without re-rounding** the
/// clipped capsule. Hidden completion returns no rectangle; hosts must remove
/// the view and its focus/actions. Disable interaction while exiting.
#[derive(Clone, Debug)]
pub struct DynamicIslandMotion {
    host: Rect,
    resting: Rect,
    bounds: Rect,
    hidden: bool,
    transition: Option<Transition>,
}

impl DynamicIslandMotion {
    /// Creates a visible, idle controller. Geometry is bounded to 16384 logical
    /// pixels per host axis and absolute edge, preventing excessive host work.
    pub fn new(host: Rect, bounds: Rect) -> Result<Self, DynamicIslandError> {
        validate(host, bounds)?;
        Ok(Self {
            host,
            resting: bounds,
            bounds,
            hidden: false,
            transition: None,
        })
    }

    /// Last sampled rectangle, or `None` after an exit completes.
    pub fn bounds(&self) -> Option<Rect> {
        (!self.hidden).then_some(self.bounds)
    }

    /// Whether another sample is required. Idle controllers need no frame wakeups.
    pub fn is_animating(&self) -> bool {
        self.transition.is_some()
    }

    /// Samples elapsed time (late wakeups finish exactly at the endpoint).
    /// Times before the transition start sample its initial geometry.
    pub fn sample_at(&mut self, now: Instant) -> Option<Rect> {
        if let Some(transition) = &self.transition {
            let elapsed = now.saturating_duration_since(transition.start);
            let t = (elapsed.as_secs_f64() / transition.duration.as_secs_f64()).min(1.0) as f32;
            let eased = t * t * (3.0 - 2.0 * t);
            self.bounds = interpolate(transition.from, transition.to, eased);
            if elapsed >= transition.duration {
                self.bounds = transition.to;
                self.hidden = transition.slide.is_some_and(|slide| !slide.is_entry());
                self.transition = None;
            }
        }
        self.bounds()
    }

    /// Expands/collapses to a visible target. Change semantic expansion separately
    /// at transition start; in particular, remove collapsed details immediately.
    pub fn transition_to_at(
        &mut self,
        bounds: Rect,
        duration: Duration,
        now: Instant,
    ) -> Result<(), DynamicIslandError> {
        validate(self.host, bounds)?;
        self.sample_at(now);
        self.resting = bounds;
        self.begin(bounds, duration, now, None);
        Ok(())
    }

    /// Starts/reverses a directional slide. An idle visible entry begins outside
    /// the host; an interrupted transition always begins at sampled geometry.
    pub fn slide_at(&mut self, slide: DynamicIslandSlide, duration: Duration, now: Instant) {
        self.sample_at(now);
        if slide.is_entry() && (self.hidden || !self.is_animating()) {
            self.bounds = slide.outside(self.host, self.resting);
        }
        let target = if slide.is_entry() {
            self.resting
        } else {
            slide.outside(self.host, self.resting)
        };
        self.begin(target, duration, now, Some(slide));
    }

    /// Reconciles actual host metrics and the new visible target. Active motion
    /// retains its remaining duration and slide direction, starting from the
    /// sampled rectangle mapped into the new host. Sampled dimensions are retained
    /// up to the new host dimensions, then interpolate to the new target. Position
    /// maps proportionally between the fully-offscreen leading and trailing edges,
    /// preserving offscreen entry origins even when dimensions change. This avoids
    /// scaling usable control bodies down with the host. Idle/hidden states stay idle.
    /// Invalid input leaves the controller untouched.
    pub fn reconcile_at(
        &mut self,
        host: Rect,
        bounds: Rect,
        now: Instant,
    ) -> Result<(), DynamicIslandError> {
        validate(host, bounds)?;
        self.sample_at(now);
        let previous = self.transition.clone();
        let width = self.bounds.width().min(host.width());
        let height = self.bounds.height().min(host.height());
        let mapped = Rect::from_xywh(
            host.left - width
                + (self.bounds.right - self.host.left) / (self.host.width() + self.bounds.width())
                    * (host.width() + width),
            host.top - height
                + (self.bounds.bottom - self.host.top)
                    / (self.host.height() + self.bounds.height())
                    * (host.height() + height),
            width,
            height,
        );
        self.host = host;
        self.resting = bounds;
        if let Some(previous) = previous {
            self.bounds = mapped;
            let target = previous
                .slide
                .filter(|slide| !slide.is_entry())
                .map_or(bounds, |slide| slide.outside(host, bounds));
            let remaining = previous
                .duration
                .saturating_sub(now.saturating_duration_since(previous.start));
            self.begin(target, remaining, now, previous.slide);
        } else {
            self.bounds = bounds;
        }
        Ok(())
    }

    fn begin(
        &mut self,
        to: Rect,
        duration: Duration,
        now: Instant,
        slide: Option<DynamicIslandSlide>,
    ) {
        self.hidden = false;
        if duration.is_zero() || self.bounds == to {
            self.bounds = to;
            self.hidden = slide.is_some_and(|slide| !slide.is_entry());
            self.transition = None;
        } else {
            self.transition = Some(Transition {
                from: self.bounds,
                to,
                start: now,
                duration,
                slide,
            });
        }
    }
}

fn validate(host: Rect, bounds: Rect) -> Result<(), DynamicIslandError> {
    let edges = [
        host.left,
        host.top,
        host.right,
        host.bottom,
        bounds.left,
        bounds.top,
        bounds.right,
        bounds.bottom,
    ];
    if edges
        .iter()
        .any(|edge| !edge.is_finite() || edge.abs() > 16384.0)
        || host.width() <= 0.0
        || host.height() <= 0.0
        || host.width() > 16384.0
        || host.height() > 16384.0
        || bounds.width() <= 0.0
        || bounds.height() <= 0.0
        || bounds.left < host.left
        || bounds.top < host.top
        || bounds.right > host.right
        || bounds.bottom > host.bottom
    {
        return Err(DynamicIslandError::InvalidGeometry);
    }
    Ok(())
}

fn interpolate(from: Rect, to: Rect, t: f32) -> Rect {
    Rect::new(
        from.left + (to.left - from.left) * t,
        from.top + (to.top - from.top) * t,
        from.right + (to.right - from.right) * t,
        from.bottom + (to.bottom - from.bottom) * t,
    )
}

#[cfg(test)]
mod tests;
