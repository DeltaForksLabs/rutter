use std::collections::HashMap;

use super::*;
use winit::dpi::Size;

#[derive(Default)]
struct FakeAccessibilityAdapter {
    routed_events: usize,
}

fn route_fake_accessibility_event(
    routes: &SurfaceRoutes<u64>,
    adapters: &mut HashMap<SurfaceId, FakeAccessibilityAdapter>,
    native: u64,
) {
    let Some(surface) = routes.surface_for(native) else {
        return;
    };
    adapters.entry(surface).or_default().routed_events += 1;
}

#[test]
fn config_defaults_and_attributes_are_independent() {
    let defaults = WindowConfig::default();
    assert_eq!(defaults.title(), "Rutter");
    assert!(!defaults.is_transparent());
    assert!(defaults.has_decorations());
    assert!(defaults.is_resizable());
    assert_eq!(defaults.inner_size(), None);
    assert_eq!(defaults.close_behavior(), CloseBehavior::CloseSurface);

    let configured = defaults
        .clone()
        .with_title("Inspector")
        .with_transparent(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_inner_size(900, 700)
        .unwrap()
        .with_close_behavior(CloseBehavior::ExitApplication);
    let attributes = configured.window_attributes();

    assert_eq!(defaults.title(), "Rutter");
    assert_eq!(attributes.title, "Inspector");
    assert!(!attributes.visible);
    assert!(attributes.transparent);
    assert!(!attributes.decorations);
    assert!(!attributes.resizable);
    assert_eq!(
        attributes.inner_size,
        Some(Size::Physical(PhysicalSize::new(900, 700)))
    );
    assert!(configured.surface_config().is_transparent());
    assert!(!defaults.surface_config().is_transparent());
}

#[test]
fn zero_window_dimension_is_rejected_with_context() {
    let error = WindowSize::new(0, 480).unwrap_err();
    let expected = WindowConfigError::InvalidSize {
        width: 0,
        height: 480,
    };

    assert_eq!(error, expected);
    assert!(error.to_string().contains("0x480"));
    assert!(WindowConfig::default().with_inner_size(640, 0).is_err());
}

#[test]
fn committed_routes_ignore_unknown_probe_ids() {
    let mut routes = SurfaceRoutes::new();
    routes
        .register_committed(SurfaceId::PRIMARY, 10_u64)
        .unwrap();

    assert_eq!(routes.native_for(SurfaceId::PRIMARY), Some(10));
    assert_eq!(routes.surface_for(10), Some(SurfaceId::PRIMARY));
    assert_eq!(routes.surface_for(99), None);
    assert!(!routes.is_empty());
}

#[test]
fn accessibility_events_reach_only_the_matching_surface_adapter() {
    let first = SurfaceId::new(1);
    let second = SurfaceId::new(2);
    let mut routes = SurfaceRoutes::new();
    routes.register_committed(first, 10_u64).unwrap();
    routes.register_committed(second, 20_u64).unwrap();
    let mut adapters = HashMap::new();

    route_fake_accessibility_event(&routes, &mut adapters, 10);
    route_fake_accessibility_event(&routes, &mut adapters, 99);
    route_fake_accessibility_event(&routes, &mut adapters, 20);

    assert_eq!(adapters.get(&first).unwrap().routed_events, 1);
    assert_eq!(adapters.get(&second).unwrap().routed_events, 1);
    assert_eq!(adapters.len(), 2);
}

#[test]
fn removing_one_route_preserves_the_other() {
    let mut routes = SurfaceRoutes::new();
    routes
        .register_committed(SurfaceId::new(1), 10_u64)
        .unwrap();
    routes
        .register_committed(SurfaceId::new(2), 20_u64)
        .unwrap();

    assert_eq!(routes.remove_surface(SurfaceId::new(1)), Some(10));
    assert_eq!(routes.surface_for(10), None);
    assert_eq!(routes.surface_for(20), Some(SurfaceId::new(2)));
    assert_eq!(routes.remove_native(20), Some(SurfaceId::new(2)));
    assert!(routes.is_empty());
}

#[test]
fn duplicate_logical_and_native_routes_are_rejected() {
    let mut routes = SurfaceRoutes::new();
    routes
        .register_committed(SurfaceId::new(1), 10_u64)
        .unwrap();

    let logical = routes.register_committed(SurfaceId::new(1), 20);
    let native = routes.register_committed(SurfaceId::new(2), 10);

    assert_eq!(
        logical,
        Err(SurfaceRouteRegistrationError::DuplicateLogical(
            SurfaceId::new(1)
        ))
    );
    assert_eq!(
        native,
        Err(SurfaceRouteRegistrationError::DuplicateNative(10))
    );
}

#[test]
fn clearing_routes_retires_every_native_identity() {
    let mut routes = SurfaceRoutes::new();
    routes
        .register_committed(SurfaceId::new(1), 10_u64)
        .unwrap();
    routes.clear();

    assert!(routes.is_empty());
    assert_eq!(routes.surface_for(10), None);
}

struct FakeMultiWindowLogic;

impl MultiWindowAppLogic for FakeMultiWindowLogic {
    type State = ();
    type Message = ();

    fn new(_: &mut FontSystem) -> Self::State {}

    fn view<'a>(_: &'a mut Self::State, _: SurfaceId) -> Widget<'a, Self::Message> {
        unreachable!()
    }

    fn update(
        _: &mut Self::State,
        _: SurfaceId,
        _: Self::Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        Vec::new()
    }
}

#[test]
fn default_initial_registry_contains_primary_surface() {
    let requests = FakeMultiWindowLogic::initial_surfaces();
    let expected = SurfaceRequest::new(SurfaceId::PRIMARY, WindowConfig::default());

    assert_eq!(requests, vec![expected]);
    assert_eq!(FakeMultiWindowLogic::locale(), Locale::default());
    assert_eq!(
        FakeMultiWindowLogic::input_limits(0, InputKind::SearchBar).max_lines,
        1
    );
    assert!(FakeMultiWindowLogic::text_shape_cache_limits().max_entries > 0);
    assert_eq!(FakeMultiWindowLogic::theme().font_body, 16.0);
}
