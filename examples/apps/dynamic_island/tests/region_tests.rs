use super::*;

#[test]
fn text_frames_and_hover_reuse_bands_and_redraw_without_native_region_updates() {
    for mode in [DynamicIslandMode::Interactive, DynamicIslandMode::Passive] {
        let mut state = configured(mode, 320, 300);
        let region = state.input_region().clone();
        let builds = state.region_builds;
        for _ in 0..120 {
            assert_eq!(
                state.publish_region(),
                [SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
            );
            state.hover(Some(LogicalPointerPosition::new(160.0, 40.0)));
            assert_eq!(state.input_region(), &region);
        }
        assert_eq!(state.region_builds, builds);
    }
}

#[test]
fn metrics_and_native_lifetime_force_publication_and_close_clears_input() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    let original = state.input_region().clone();
    let builds = state.region_builds;
    let mut scaled = metrics(320, 300);
    scaled.scale_factor = 1.5;
    IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, scaled);
    assert_eq!(published(&IslandDemo::wakeup(&mut state, now)), &original);
    assert_eq!(
        state.region_builds, builds,
        "logical bands unchanged by scaling"
    );
    IslandDemo::surface_created(&mut state, SurfaceId::PRIMARY);
    assert_eq!(published(&IslandDemo::wakeup(&mut state, now)), &original);
    let commands = state.apply_at(Message::Close, now);
    assert_eq!(published(&commands), &SurfaceInputRegion::Empty);
    assert_eq!(
        commands.last(),
        Some(&SurfaceCommand::Close(SurfaceId::PRIMARY))
    );
    assert_eq!(
        state.publish_region(),
        [SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
    );
    IslandDemo::surface_closed(&mut state, SurfaceId::PRIMARY);
    assert!(state.published_region.is_none());
    assert_eq!(state.current_region, SurfaceInputRegion::Empty);
    assert!(state.region_bounds.is_none());
}

#[test]
fn changed_geometry_with_identical_integer_bands_deduplicates_native_submission() {
    let mut state = configured(DynamicIslandMode::Interactive, 320, 300);
    let original = state.input_region().clone();
    // The native region is integer-banded. A subpixel geometry change need not
    // change those bands, but still has to be checked against the current shape.
    let bounds = capsule_bounds(&state).unwrap();
    let host = PaintRect::from_xywh(0.0, 0.0, 320.0, 300.0);
    for delta in [0.00001, 0.00002] {
        let mut target = bounds;
        target.bottom += delta;
        state.motion = Some(DynamicIslandMotion::new(host, target).unwrap());
        let expected = state
            .island
            .input_region(target, DynamicIsland::RADIUS)
            .unwrap();
        assert_eq!(expected, original, "fixture must retain integer bands");
        let commands = state.publish_region();
        assert_eq!(
            commands,
            [SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
        );
    }
}
