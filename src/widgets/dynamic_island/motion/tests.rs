use super::*;

fn host() -> Rect {
    Rect::from_xywh(0.0, 0.0, 420.0, 300.0)
}
fn compact() -> Rect {
    Rect::from_xywh(8.0, 8.0, 404.0, 76.0)
}

#[test]
fn dynamic_island_motion_all_eight_directions_have_exact_endpoints_and_midpoints() {
    use DynamicIslandSlide::*;
    // Explicit expected positions intentionally do not use outside() to compute
    // the oracle: InUp must start BELOW and InDown ABOVE.
    for (slide, outside) in [
        (SlideInLeft, Rect::from_xywh(-404.0, 8.0, 404.0, 76.0)),
        (SlideInRight, Rect::from_xywh(420.0, 8.0, 404.0, 76.0)),
        (SlideInUp, Rect::from_xywh(8.0, 300.0, 404.0, 76.0)),
        (SlideInDown, Rect::from_xywh(8.0, -76.0, 404.0, 76.0)),
        (SlideOutLeft, Rect::from_xywh(-404.0, 8.0, 404.0, 76.0)),
        (SlideOutRight, Rect::from_xywh(420.0, 8.0, 404.0, 76.0)),
        (SlideOutUp, Rect::from_xywh(8.0, -76.0, 404.0, 76.0)),
        (SlideOutDown, Rect::from_xywh(8.0, 300.0, 404.0, 76.0)),
    ] {
        let now = Instant::now();
        let mut motion = DynamicIslandMotion::new(host(), compact()).unwrap();
        motion.slide_at(slide, Duration::from_millis(280), now);
        assert_eq!(
            motion.bounds(),
            Some(if slide.is_entry() { outside } else { compact() })
        );
        let midpoint = motion.sample_at(now + Duration::from_millis(140)).unwrap();
        assert_eq!(
            midpoint,
            Rect::new(
                (compact().left + outside.left) / 2.0,
                (compact().top + outside.top) / 2.0,
                (compact().right + outside.right) / 2.0,
                (compact().bottom + outside.bottom) / 2.0
            )
        );
        let endpoint = motion.sample_at(now + Duration::from_millis(280));
        assert_eq!(endpoint, slide.is_entry().then_some(compact()));
        assert!(!motion.is_animating());
        assert_eq!(motion.sample_at(now + Duration::from_secs(30)), endpoint);
    }
}

#[test]
fn dynamic_island_motion_expansion_and_slide_reversals_sample_current_geometry() {
    let now = Instant::now();
    let mut motion = DynamicIslandMotion::new(host(), compact()).unwrap();
    let expanded = Rect::from_xywh(8.0, 8.0, 404.0, 252.0);
    motion
        .transition_to_at(expanded, Duration::from_millis(240), now)
        .unwrap();
    assert_eq!(
        motion.sample_at(now - Duration::from_millis(1)),
        Some(compact())
    );
    let half = now + Duration::from_millis(120);
    assert_eq!(motion.sample_at(half).unwrap().height(), 164.0);
    motion
        .transition_to_at(compact(), Duration::from_millis(240), half)
        .unwrap();
    assert_eq!(motion.bounds().unwrap().height(), 164.0);
    assert_eq!(
        motion.sample_at(half + Duration::from_secs(5)),
        Some(compact())
    );
    motion.slide_at(
        DynamicIslandSlide::SlideOutDown,
        Duration::from_millis(280),
        now,
    );
    let current = motion.sample_at(now + Duration::from_millis(100));
    motion.slide_at(
        DynamicIslandSlide::SlideInUp,
        Duration::from_millis(280),
        now + Duration::from_millis(100),
    );
    assert_eq!(motion.bounds(), current);
    assert_eq!(
        motion.sample_at(now + Duration::from_secs(10)),
        Some(compact())
    );
}

#[test]
fn dynamic_island_motion_expired_entry_is_independent_of_prior_sampling() {
    let now = Instant::now();
    let duration = Duration::from_millis(280);
    for elapsed in [duration, Duration::from_secs(1)] {
        let mut unsampled = DynamicIslandMotion::new(host(), compact()).unwrap();
        unsampled.slide_at(DynamicIslandSlide::SlideInDown, duration, now);
        let mut sampled = unsampled.clone();
        sampled.sample_at(now + elapsed);
        for motion in [&mut sampled, &mut unsampled] {
            motion.slide_at(DynamicIslandSlide::SlideInUp, duration, now + elapsed);
            assert_eq!(motion.bounds().unwrap().top, host().bottom);
            assert!(motion.is_animating());
        }
        for offset in [Duration::ZERO, duration / 2, duration] {
            assert_eq!(
                sampled.sample_at(now + elapsed + offset),
                unsampled.sample_at(now + elapsed + offset)
            );
        }
        assert_eq!(unsampled.bounds(), Some(compact()));
        assert!(!unsampled.is_animating());
    }
}

#[test]
fn dynamic_island_motion_resize_preserves_extents_offscreen_origins_and_endpoints() {
    use DynamicIslandSlide::*;
    let now = Instant::now();
    let duration = Duration::from_millis(280);
    let resized_host = Rect::from_xywh(20.0, 30.0, 320.0, 110.0);
    let resized = Rect::from_xywh(28.0, 38.0, 304.0, 76.0);
    for slide in [SlideInLeft, SlideInRight, SlideInUp, SlideInDown] {
        let mut motion = DynamicIslandMotion::new(host(), compact()).unwrap();
        motion.slide_at(slide, duration, now);
        motion.reconcile_at(resized_host, resized, now).unwrap();
        let origin = motion.bounds().unwrap();
        assert_eq!(origin.width(), 320.0);
        assert_eq!(origin.height(), 76.0);
        match slide {
            SlideInLeft => assert_eq!(origin.right, resized_host.left),
            SlideInRight => assert_eq!(origin.left, resized_host.right),
            SlideInUp => assert_eq!(origin.top, resized_host.bottom),
            SlideInDown => assert_eq!(origin.bottom, resized_host.top),
            _ => unreachable!(),
        }
        assert_eq!(motion.sample_at(now + duration / 2).unwrap().height(), 76.0);
        assert_eq!(motion.sample_at(now + duration), Some(resized));
        assert!(!motion.is_animating());
    }
}

#[test]
fn dynamic_island_motion_zero_duration_resize_and_invalid_geometry_are_bounded() {
    let now = Instant::now();
    let mut motion = DynamicIslandMotion::new(host(), compact()).unwrap();
    motion.slide_at(DynamicIslandSlide::SlideOutUp, Duration::ZERO, now);
    assert_eq!(motion.bounds(), None);
    assert!(!motion.is_animating());
    let resized_host = Rect::from_xywh(0.0, 0.0, 320.0, 110.0);
    let resized = Rect::from_xywh(8.0, 8.0, 304.0, 94.0);
    motion.reconcile_at(resized_host, resized, now).unwrap();
    assert_eq!(motion.bounds(), None);
    motion.slide_at(DynamicIslandSlide::SlideInDown, Duration::ZERO, now);
    assert_eq!(motion.bounds(), Some(resized));
    motion
        .transition_to_at(resized, Duration::ZERO, now)
        .unwrap();
    assert!(!motion.is_animating());
    motion.slide_at(
        DynamicIslandSlide::SlideOutRight,
        Duration::from_millis(280),
        now,
    );
    motion
        .reconcile_at(host(), compact(), now + Duration::from_millis(140))
        .unwrap();
    assert_eq!(motion.sample_at(now + Duration::from_millis(280)), None);
    for invalid in [
        Rect::from_xywh(0.0, 0.0, f32::NAN, 300.0),
        Rect::from_xywh(0.0, 0.0, 0.0, 300.0),
        Rect::from_xywh(0.0, 0.0, 20000.0, 300.0),
        Rect::from_xywh(0.0, 0.0, f32::INFINITY, 300.0),
    ] {
        assert!(DynamicIslandMotion::new(invalid, compact()).is_err());
        assert!(motion.reconcile_at(invalid, compact(), now).is_err());
        assert_eq!(motion.bounds(), None);
    }
    for invalid in [
        Rect::from_xywh(-1.0, 0.0, 404.0, 76.0),
        Rect::from_xywh(8.0, 8.0, 404.0, 400.0),
        Rect::from_xywh(8.0, 8.0, -1.0, 76.0),
    ] {
        assert!(
            motion
                .transition_to_at(invalid, Duration::ZERO, now)
                .is_err()
        );
        assert_eq!(motion.bounds(), None);
    }
}
