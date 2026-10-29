use super::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use cosmic_text::SwashCache;
use rutter::engine::widget_state::{ScrollState, WidgetState};
use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
use rutter::render::{
    RichTextRenderer, draw_widgets,
    hit_test::{HitResult, hit_test},
    text::TextBufferCache,
};
use skia_safe::{Color, Point, surfaces};
use winit::dpi::PhysicalSize;

mod focus_tests;
mod hover_tests;
mod motion_tests;
mod region_tests;
mod text_tests;

fn metrics(width: u32, height: u32) -> SurfaceMetrics {
    SurfaceMetrics {
        physical_size: (width, height),
        logical_size: (width.into(), height.into()),
        scale_factor: 1.0,
        output_name: None,
        layer_shell_version: Some(4),
    }
}

fn configured(mode: DynamicIslandMode, width: u32, height: u32) -> Model {
    let now = Instant::now();
    let mut model = Model::new(mode, now);
    model.no_animation = true;
    IslandDemo::surface_metrics_changed(&mut model, SurfaceId::PRIMARY, metrics(width, height));
    IslandDemo::wakeup(&mut model, now);
    model
}

impl Model {
    // Existing immediate behavior checks explicitly opt out of host animation.
    fn apply(&mut self, message: Message) -> Vec<SurfaceCommand> {
        self.apply_at(message, Instant::now())
    }
}

fn defaults(mode: DynamicIslandMode) -> Options {
    Options {
        mode,
        output: None,
        enter: DynamicIslandSlide::SlideInDown,
        exit: DynamicIslandSlide::SlideOutUp,
        no_animation: false,
    }
}

#[test]
fn strict_cli_rejects_missing_unknown_extra_and_invalid_output_arguments() {
    for args in [
        vec![],
        vec!["active"],
        vec!["--help"],
        vec!["passive", ""],
        vec!["interactive", "--output"],
        vec!["passive", "DP-1", "extra"],
        vec!["interactive", "bad\0name"],
    ] {
        assert!(parse_args(args.into_iter().map(OsString::from)).is_err());
    }
    for (arg, mode) in [
        ("interactive", DynamicIslandMode::Interactive),
        ("passive", DynamicIslandMode::Passive),
    ] {
        assert_eq!(parse_args([arg.into()]).unwrap(), defaults(mode));
        assert_eq!(
            parse_args([arg.into(), "DP-2".into()]).unwrap(),
            Options {
                output: Some("DP-2".into()),
                ..defaults(mode)
            }
        );
    }
    assert!(parse_args(["passive".into(), "x".repeat(1025).into()]).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        assert!(parse_args(["passive".into(), OsString::from_vec(vec![0xff])]).is_err());
    }
}

#[test]
fn creation_is_transparent_top_centered_overlay_without_reservation_or_initial_input() {
    for mode in [DynamicIslandMode::Passive, DynamicIslandMode::Interactive] {
        let request = host_request(&Options {
            output: Some("DP-2".into()),
            ..defaults(mode)
        })
        .unwrap();
        let window = request.window;
        assert_eq!(request.surface, SurfaceId::PRIMARY);
        assert!(window.is_transparent());
        assert!(!window.has_decorations());
        assert!(!window.is_resizable());
        assert!(!window.closes_on_focus_loss());
        assert_eq!(window.input_region(), &SurfaceInputRegion::Empty);
        assert!(window.panel().is_none() && window.popup().is_none());
        let layer = window.layer_shell().unwrap();
        assert_eq!(layer.layer(), Layer::Overlay);
        assert_eq!(layer.anchors(), LayerAnchors::TOP);
        assert_eq!(layer.logical_size(), (420, 300));
        assert_eq!(layer.exclusive_zone(), LayerExclusiveZone::None);
        assert_eq!(layer.output(), &LayerOutput::Name("DP-2".into()));
        assert_eq!(
            layer.keyboard_interactivity(),
            if mode == DynamicIslandMode::Passive {
                LayerKeyboardInteractivity::None
            } else {
                LayerKeyboardInteractivity::OnDemand
            }
        );
        assert_eq!(
            layer.validate_protocol_version(3).is_ok(),
            mode == DynamicIslandMode::Passive
        );
    }
}

fn published(commands: &[SurfaceCommand]) -> &SurfaceInputRegion {
    let [
        SurfaceCommand::SetInputRegion {
            surface: SurfaceId::PRIMARY,
            region,
        },
        SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY),
        ..,
    ] = commands
    else {
        panic!("expected exact input publication and redraw, got {commands:?}");
    };
    region
}

fn escape() -> ShortcutEvent {
    ShortcutEvent {
        key: ShortcutKey::Named(ShortcutNamedKey::Escape),
        control: false,
        alt: false,
        shift: false,
        super_key: false,
        repeat: false,
    }
}

#[test]
fn metrics_are_reconciled_on_immediate_wakeup_and_expansion_publishes_current_bounds() {
    let now = Instant::now();
    let mut state = Model::new(DynamicIslandMode::Interactive, now);
    state.no_animation = true;
    assert_eq!(
        published(&state.publish_region()),
        &SurfaceInputRegion::Empty
    );
    assert_eq!(IslandDemo::next_wakeup(&state, now), None);
    IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(320, 300));
    assert_eq!(IslandDemo::next_wakeup(&state, now), Some(now));
    let collapsed = IslandDemo::wakeup(&mut state, now);
    assert_eq!(
        published(&collapsed),
        &state
            .island
            .input_region(capsule_bounds(&state).unwrap(), DynamicIsland::RADIUS)
            .unwrap()
    );
    assert_eq!(IslandDemo::next_wakeup(&state, now), None);
    assert_eq!(
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape()),
        ShortcutOutcome::Ignored
    );
    let expanded = state.apply(Message::Expand);
    assert!(state.island.is_expanded());
    assert_ne!(published(&expanded), published(&collapsed));
    assert_eq!(
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape()),
        ShortcutOutcome::Message(Message::Collapse)
    );
    assert_eq!(
        IslandDemo::shortcut(&state, SurfaceId::new(1), escape()),
        ShortcutOutcome::Ignored
    );
    state.apply(Message::PauseResume);
    assert!(state.paused && state.summary().starts_with("Paused"));
    state.apply(Message::PauseResume);
    assert!(!state.paused && state.summary().starts_with("Playing"));
    let collapse = state.apply(Message::Collapse);
    assert_eq!(published(&collapse), published(&collapsed));
    assert!(
        collapse.contains(&SurfaceCommand::SetLayerKeyboardInteractivity {
            surface: SurfaceId::PRIMARY,
            interactivity: LayerKeyboardInteractivity::OnDemand,
        })
    );
    assert_eq!(state.island.mode(), DynamicIslandMode::Interactive);
    IslandDemo::surface_metrics_changed(&mut state, SurfaceId::PRIMARY, metrics(300, 110));
    let resized = IslandDemo::wakeup(&mut state, now);
    assert_ne!(published(&resized), published(&collapsed));
    assert_eq!(
        state.apply(Message::Close),
        vec![
            SurfaceCommand::SetInputRegion {
                surface: SurfaceId::PRIMARY,
                region: SurfaceInputRegion::Empty
            },
            SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY),
            SurfaceCommand::Close(SurfaceId::PRIMARY)
        ]
    );
}

#[test]
fn passive_policy_is_fixed_empty_nonfocusable_and_uses_sparse_timed_wakeups() {
    let now = Instant::now();
    let mut state = configured(DynamicIslandMode::Passive, 320, 300);
    state.pending_region = true;
    assert_eq!(
        IslandDemo::wakeup(&mut state, now),
        [SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
    );
    let deadline = IslandDemo::next_wakeup(&state, now).unwrap();
    assert!(deadline > now);
    assert!(IslandDemo::wakeup(&mut state, deadline - Duration::from_millis(1)).is_empty());
    let before = state.summary();
    assert_eq!(
        IslandDemo::wakeup(&mut state, deadline),
        [SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
    );
    assert_ne!(before, state.summary());
    assert_eq!(
        IslandDemo::next_wakeup(&state, deadline),
        Some(deadline + INTERVAL)
    );
    for message in [
        Message::Expand,
        Message::Collapse,
        Message::PauseResume,
        Message::Close,
    ] {
        assert!(state.apply(message).is_empty());
    }
    assert_eq!(state.island.mode(), DynamicIslandMode::Passive);
    assert!(!state.island.is_expanded() && !state.paused);
    assert_eq!(
        IslandDemo::shortcut(&state, SurfaceId::PRIMARY, escape()),
        ShortcutOutcome::Ignored
    );
    let view = island_view(&state, state.summary(), &Theme::dark());
    let (tree, root) = layout(&view, 320, 300);
    assert!(
        hit_test(
            &view,
            &tree,
            root,
            Point::new(160.0, 40.0),
            Point::default(),
            &HashMap::new()
        )
        .is_none()
    );
}

fn layout(
    view: &Widget<'_, Message>,
    width: u32,
    height: u32,
) -> (TaffyTree<RutterContext>, NodeId) {
    let fonts = Rc::new(RefCell::new(FontSystem::new()));
    let mut tree = TaffyTree::new();
    let root = build_taffy_tree(&mut tree, view, fonts.clone(), &HashMap::new());
    compute_layout(
        &mut tree,
        root,
        PhysicalSize::new(width, height),
        fonts,
        &RichTextRenderer::default(),
    );
    (tree, root)
}

fn bounds(tree: &TaffyTree<RutterContext>, root: NodeId, path: &[usize]) -> PaintRect {
    let mut node = root;
    let mut origin = Point::default();
    for &index in path {
        let layout = tree.layout(node).unwrap();
        origin.x += layout.location.x;
        origin.y += layout.location.y;
        node = tree.children(node).unwrap()[index];
    }
    let layout = tree.layout(node).unwrap();
    PaintRect::from_xywh(
        origin.x + layout.location.x,
        origin.y + layout.location.y,
        layout.size.width,
        layout.size.height,
    )
}

#[test]
fn actual_configured_geometry_matches_native_shape_and_scrolling_reaches_child_actions() {
    for (width, height) in [(420, 300), (320, 300), (800, 300), (320, 110)] {
        let mut state = configured(DynamicIslandMode::Interactive, width, height);
        state.apply(Message::Expand);
        let view = island_view(&state, state.summary(), &Theme::dark());
        let (tree, root) = layout(&view, width, height);
        let capsule = bounds(&tree, root, &[0]);
        assert_eq!(capsule, capsule_bounds(&state).unwrap());
        let region = state
            .island
            .input_region(capsule, DynamicIsland::RADIUS)
            .unwrap();
        let SurfaceInputRegion::Rectangles(rects) = region else {
            panic!("interactive shape")
        };
        for rect in rects.as_slice() {
            let (x, y, w, h) = rect.logical_rect();
            assert!(x as f32 >= capsule.left && (x + w) as f32 <= capsule.right);
            assert!(y as f32 >= capsule.top && (y + h) as f32 <= capsule.bottom);
        }
        let viewport = bounds(&tree, root, &[0, 0]);
        let document = bounds(&tree, root, &[0, 0, 0]);
        let header = bounds(&tree, root, &[0, 0, 0, 0]);
        let details = bounds(&tree, root, &[0, 0, 0, 1]);
        assert_eq!(header.height(), 44.0);
        assert_eq!(header.left, capsule.left + spacing::LG);
        assert!(details.top >= header.bottom + spacing::MD - 1.0);
        for (path, message) in [
            (&[0, 0, 0, 0][..], Message::Expand),
            (&[0, 0, 0, 1, 1][..], Message::PauseResume),
            (&[0, 0, 0, 1, 2][..], Message::Close),
        ] {
            let action = bounds(&tree, root, path);
            let mut scroll = ScrollState {
                offset_y: 0.0,
                content_height: document.height(),
                viewport_h: viewport.height(),
            };
            scroll.scroll_by((action.bottom - viewport.bottom).max(0.0));
            let point = Point::new(action.center_x(), action.center_y() - scroll.offset_y);
            assert!(point.y >= viewport.top && point.y < viewport.bottom);
            let states = HashMap::from([(DOCUMENT, WidgetState::Scroll(scroll))]);
            assert!(
                matches!(hit_test(&view, &tree, root, point, Point::default(), &states),
                Some(HitResult::Message { msg, focus_id: Some(_), .. }) if msg == message)
            );
        }
        if height == 110 {
            assert!(document.height() > viewport.height());
        }
    }
}

#[test]
fn long_summary_scrolls_without_consuming_controls_and_raster_stays_in_capsule() {
    const LONG: &str = "Playing — Details anzeigen — عرض التفاصيل — A deliberately long localized summary stays reachable by scrolling rather than stealing the space allocated to controls.";
    for theme in [Theme::light(), Theme::dark()] {
        for width in [320, 800] {
            let mut state = configured(DynamicIslandMode::Interactive, width, 300);
            state.apply(Message::Expand);
            let view = island_view(&state, LONG, &theme);
            let (tree, root) = layout(&view, width, 300);
            let capsule = bounds(&tree, root, &[0]);
            let header = bounds(&tree, root, &[0, 0, 0, 0]);
            let text = bounds(&tree, root, &[0, 0, 0, 0, 0]);
            let details = bounds(&tree, root, &[0, 0, 0, 1]);
            assert!(header.height() >= text.height());
            assert!(details.top >= header.bottom + spacing::MD - 1.0);
            for scale in [1.0_f32, 1.5, 2.0] {
                let mut raster = surfaces::raster_n32_premul((
                    (width as f32 * scale) as i32,
                    (300.0 * scale) as i32,
                ))
                .unwrap();
                raster.canvas().clear(Color::TRANSPARENT);
                raster.canvas().scale((scale, scale));
                draw_widgets(
                    raster.canvas(),
                    &tree,
                    root,
                    &view,
                    &mut FontSystem::new(),
                    &mut SwashCache::new(),
                    Point::new(-1.0, -1.0),
                    None,
                    &HashMap::new(),
                    &HashMap::new(),
                    &mut HashMap::new(),
                    &mut TextBufferCache::default(),
                    true,
                    &theme,
                    scale,
                );
                let pixels = raster.peek_pixels().unwrap();
                assert_eq!(pixels.get_color((0, 0)).a(), 0);
                assert_eq!(
                    pixels.get_color((
                        (capsule.center_x() * scale) as i32,
                        ((capsule.top + 4.0) * scale) as i32
                    )),
                    theme.surface
                );
                assert_eq!(
                    pixels
                        .get_color((
                            (capsule.center_x() * scale) as i32,
                            ((capsule.bottom + 2.0) * scale) as i32
                        ))
                        .a(),
                    0
                );
                let mut ink = 0;
                for y in (text.top * scale).ceil() as i32
                    ..(text.bottom.min(capsule.bottom - spacing::LG) * scale).floor() as i32
                {
                    for x in (text.left * scale).ceil() as i32..(text.right * scale).floor() as i32
                    {
                        let color = pixels.get_color((x, y));
                        if color.a() > 0 && color != theme.surface {
                            ink += 1;
                        }
                    }
                }
                assert!(ink > 20, "summary must actually paint");
            }
        }
    }
}
