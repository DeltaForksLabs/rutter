// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! App-driven scalar animation. Sampling does not schedule frames or transform
//! widgets: applications must apply values to layout or supported properties and
//! keep painting, hit testing and accessibility consistent.

use std::{
    fmt,
    time::{Duration, Instant},
};

/// Interpolation curve with fixed endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    /// Constant speed.
    Linear,
    /// Smoothstep, with zero velocity at each endpoint.
    SmoothStep,
}

impl Easing {
    fn apply(self, t: f64) -> f64 {
        match self {
            Self::Linear => t,
            Self::SmoothStep => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// A non-finite animation value or invalid slide extent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationError;

impl fmt::Display for AnimationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("animation values must be finite and slide extents nonnegative")
    }
}

impl std::error::Error for AnimationError {}

#[derive(Clone, Debug)]
struct Transition {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
    easing: Easing,
}

/// A scalar whose clock is supplied explicitly by the application.
#[derive(Clone, Debug)]
pub struct Animation {
    value: f32,
    transition: Option<Transition>,
}

impl Animation {
    /// Creates an idle animation with a finite initial value.
    pub fn new(value: f32) -> Result<Self, AnimationError> {
        if !value.is_finite() {
            return Err(AnimationError);
        }
        Ok(Self {
            value,
            transition: None,
        })
    }

    /// Last sampled value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Whether the host needs to schedule another sample.
    pub fn is_animating(&self) -> bool {
        self.transition.is_some()
    }

    /// Samples elapsed time, clamping before the start and after the end.
    pub fn sample_at(&mut self, now: Instant) -> f32 {
        if let Some(transition) = &self.transition {
            let elapsed = now.saturating_duration_since(transition.start);
            let t = (elapsed.as_secs_f64() / transition.duration.as_secs_f64()).min(1.0);
            let t = transition.easing.apply(t);
            self.value =
                ((1.0 - t) * f64::from(transition.from) + t * f64::from(transition.to)) as f32;
            if elapsed >= transition.duration {
                self.value = transition.to;
                self.transition = None;
            }
        }
        self.value
    }

    /// Retargets from the current sample, preserving position but not velocity.
    /// Invalid targets leave the animation untouched. Zero duration completes now.
    pub fn retarget_at(
        &mut self,
        target: f32,
        duration: Duration,
        easing: Easing,
        now: Instant,
    ) -> Result<(), AnimationError> {
        if !target.is_finite() {
            return Err(AnimationError);
        }
        self.sample_at(now);
        if duration.is_zero() || self.value == target {
            self.value = target;
            self.transition = None;
        } else {
            self.transition = Some(Transition {
                from: self.value,
                to: target,
                start: now,
                duration,
                easing,
            });
        }
        Ok(())
    }
}

/// Physical slide directions. Entry up starts below; entry down starts above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slide {
    /// Enter from the left.
    InLeft,
    /// Enter from the right.
    InRight,
    /// Enter upward from below.
    InUp,
    /// Enter downward from above.
    InDown,
    /// Exit toward the left.
    OutLeft,
    /// Exit toward the right.
    OutRight,
    /// Exit upward.
    OutUp,
    /// Exit downward.
    OutDown,
}

impl Slide {
    /// Offset at progress 0..=1 for a viewport's logical width and height.
    /// The caller must clip the visual content to that viewport.
    pub fn offset(
        self,
        progress: f32,
        width: f32,
        height: f32,
    ) -> Result<(f32, f32), AnimationError> {
        if !progress.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width < 0.0
            || height < 0.0
        {
            return Err(AnimationError);
        }
        let t = progress.clamp(0.0, 1.0);
        Ok(match self {
            Self::InLeft => (-width * (1.0 - t), 0.0),
            Self::InRight => (width * (1.0 - t), 0.0),
            Self::InUp => (0.0, height * (1.0 - t)),
            Self::InDown => (0.0, -height * (1.0 - t)),
            Self::OutLeft => (-width * t, 0.0),
            Self::OutRight => (width * t, 0.0),
            Self::OutUp => (0.0, -height * t),
            Self::OutDown => (0.0, height * t),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_have_correct_endpoints_and_midpoints() {
        for (slide, start, end) in [
            (Slide::InLeft, (-100.0, 0.0), (0.0, 0.0)),
            (Slide::InRight, (100.0, 0.0), (0.0, 0.0)),
            (Slide::InUp, (0.0, 40.0), (0.0, 0.0)),
            (Slide::InDown, (0.0, -40.0), (0.0, 0.0)),
            (Slide::OutLeft, (0.0, 0.0), (-100.0, 0.0)),
            (Slide::OutRight, (0.0, 0.0), (100.0, 0.0)),
            (Slide::OutUp, (0.0, 0.0), (0.0, -40.0)),
            (Slide::OutDown, (0.0, 0.0), (0.0, 40.0)),
        ] {
            assert_eq!(slide.offset(0.0, 100.0, 40.0).unwrap(), start);
            assert_eq!(slide.offset(1.0, 100.0, 40.0).unwrap(), end);
            assert_eq!(
                slide.offset(0.5, 100.0, 40.0).unwrap(),
                ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0)
            );
            assert!(slide.offset(f32::NAN, 100.0, 40.0).is_err());
        }
    }

    #[test]
    fn interruption_validation_and_idle_are_deterministic() {
        let now = Instant::now();
        let mut a = Animation::new(0.0).unwrap();
        assert!(Animation::new(f32::INFINITY).is_err());
        a.retarget_at(10.0, Duration::from_secs(2), Easing::Linear, now)
            .unwrap();
        assert_eq!(a.sample_at(now + Duration::from_secs(1)), 5.0);
        assert!(
            a.retarget_at(f32::NAN, Duration::ZERO, Easing::Linear, now)
                .is_err()
        );
        a.retarget_at(
            15.0,
            Duration::from_secs(2),
            Easing::SmoothStep,
            now + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(a.sample_at(now + Duration::from_secs(2)), 10.0);
        assert_eq!(a.sample_at(now + Duration::from_secs(10)), 15.0);
        assert!(!a.is_animating());
        a.retarget_at(4.0, Duration::ZERO, Easing::Linear, now)
            .unwrap();
        assert_eq!(a.value(), 4.0);
        assert!(!a.is_animating());
    }

    #[test]
    fn interpolation_between_extreme_finite_values_stays_finite() {
        let now = Instant::now();
        for easing in [Easing::Linear, Easing::SmoothStep] {
            let mut animation = Animation::new(-f32::MAX).unwrap();
            animation
                .retarget_at(f32::MAX, Duration::from_secs(4), easing, now)
                .unwrap();
            for second in 0..=4 {
                assert!(
                    animation
                        .sample_at(now + Duration::from_secs(second))
                        .is_finite()
                );
            }
            assert_eq!(animation.value(), f32::MAX);
        }
    }

    #[test]
    fn sampled_value_changes_an_ordinary_button_layout() {
        use crate::{
            ButtonVariant, Widget,
            layout::{build_taffy_tree, compute_layout},
            render::RichTextRenderer,
        };
        use cosmic_text::FontSystem;
        use std::{cell::RefCell, collections::HashMap, rc::Rc};
        use taffy::prelude::*;
        use winit::dpi::PhysicalSize;

        let now = Instant::now();
        let mut animation = Animation::new(40.0).unwrap();
        animation
            .retarget_at(120.0, Duration::from_secs(2), Easing::Linear, now)
            .unwrap();
        let width = animation.sample_at(now + Duration::from_secs(1));
        let button = Widget::Button {
            text: "Action",
            on_press: (),
            color: None,
            variant: ButtonVariant::Primary,
            style: Style {
                size: Size::from_lengths(width, 44.0),
                ..Style::default()
            },
        };
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut tree = TaffyTree::new();
        let root = build_taffy_tree(&mut tree, &button, fonts.clone(), &HashMap::new());
        compute_layout(
            &mut tree,
            root,
            PhysicalSize::new(320, 100),
            fonts,
            &RichTextRenderer::default(),
        );
        assert_eq!(tree.layout(root).unwrap().size.width, 80.0);
    }
}
