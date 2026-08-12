use std::cell::Cell;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use taffy::prelude::Style;

use super::*;
use crate::app::AppLogic;
use crate::theme::Theme;
use crate::widget::Widget;

#[derive(Clone, Debug)]
enum FakeMessage {
    Increment,
}

struct FakeMultiWindowApp;

impl MultiWindowAppLogic for FakeMultiWindowApp {
    type State = u64;
    type Message = FakeMessage;

    fn new(_: &mut FontSystem) -> Self::State {
        0
    }

    fn view<'a>(state: &'a mut Self::State, _: SurfaceId) -> Widget<'a, Self::Message> {
        let _ = state;
        Widget::Spacer {
            style: Style::default(),
        }
    }

    fn update(
        state: &mut Self::State,
        _: SurfaceId,
        message: Self::Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        match message {
            FakeMessage::Increment => *state += 1,
        }
        Vec::new()
    }

    fn theme_for(state: &Self::State) -> Theme {
        if *state == 41 {
            return Theme::dark();
        }
        Theme::light()
    }
}

#[test]
fn initial_surface_validation_rejects_empty_and_duplicate_registries() {
    assert!(matches!(
        validate_initial_surfaces(&[]),
        Err(MultiWindowRunError::EmptyInitialSurfaces)
    ));

    let duplicate = vec![
        SurfaceRequest::new(SurfaceId::new(4), WindowConfig::default()),
        SurfaceRequest::new(SurfaceId::new(4), WindowConfig::default()),
    ];
    assert!(matches!(
        validate_initial_surfaces(&duplicate),
        Err(MultiWindowRunError::DuplicateLogicalSurface(id)) if id == SurfaceId::new(4)
    ));
}

#[test]
fn initial_surface_validation_accepts_independent_surface_ids() {
    let surfaces = vec![
        SurfaceRequest::new(SurfaceId::new(1), WindowConfig::default()),
        SurfaceRequest::new(SurfaceId::new(2), WindowConfig::default()),
    ];

    assert!(validate_initial_surfaces(&surfaces).is_ok());
}

#[test]
fn surface_adapter_builds_the_requested_surface_view() {
    let _message = FakeMessage::Increment;
    let mut state = SurfaceAppState::<FakeMultiWindowApp>::new(SurfaceId::new(8), 41, 3);
    let widget = SurfaceAppAdapter::<FakeMultiWindowApp>::view(&mut state);

    assert!(matches!(widget, Widget::Spacer { .. }));
    drop(widget);
    assert_eq!(state.surface, SurfaceId::new(8));
    assert_eq!(state.model, 41);
    assert_eq!(state.revision, 3);
}

#[test]
fn surface_adapter_delegates_theme_resolution_to_the_shared_model() {
    let mut state = SurfaceAppState::<FakeMultiWindowApp>::new(SurfaceId::new(8), 41, 3);

    assert_eq!(
        SurfaceAppAdapter::<FakeMultiWindowApp>::theme_for(&state).surface,
        Theme::dark().surface
    );
    state.model = 0;
    assert_eq!(
        SurfaceAppAdapter::<FakeMultiWindowApp>::theme_for(&state).surface,
        Theme::light().surface
    );
}

#[test]
fn scheduler_uses_the_earliest_surface_deadline() {
    let now = Instant::now();
    let early = now + Duration::from_millis(10);
    let late = now + Duration::from_millis(30);

    assert_eq!(minimum_deadline(Some(late), Some(early)), Some(early));
    assert_eq!(minimum_deadline(None, Some(late)), Some(late));
    assert_eq!(minimum_deadline(Some(early), None), Some(early));
    assert_eq!(minimum_deadline(None, None), None);
}

#[test]
fn native_route_collision_is_a_surface_scoped_typed_error() {
    let surface = SurfaceId::new(12);
    let error = route_error(
        surface,
        SurfaceRouteRegistrationError::DuplicateNative(99_u64),
    );

    assert!(matches!(
        error,
        MultiWindowRunError::NativeRouteConflict { surface: id, native }
            if id == surface && native == "99"
    ));
}

#[test]
fn initialization_pauses_scheduled_work_until_native_resume() {
    let runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();

    assert_eq!(runtime.canonical_state, 0);
    assert_eq!(runtime.pending_surfaces.len(), 1);
    assert_eq!(runtime.pending_surfaces[0].surface, SurfaceId::PRIMARY);
    assert!(!runtime.native_surfaces_active);
    assert!(runtime.routes.is_empty());
}

#[test]
fn injected_bootstrap_uses_factory_state_and_dynamic_surface_order() {
    let factory_calls = Cell::new(0);
    let surfaces = vec![
        SurfaceRequest::new(
            SurfaceId::new(2),
            WindowConfig::default().with_title("Second panel"),
        ),
        SurfaceRequest::new(
            SurfaceId::new(1),
            WindowConfig::default().with_title("First panel"),
        ),
    ];

    let runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize_with(
        |_| {
            factory_calls.set(factory_calls.get() + 1);
            Ok::<_, std::convert::Infallible>(41)
        },
        surfaces.clone(),
    )
    .unwrap();

    assert_eq!(factory_calls.get(), 1);
    assert_eq!(runtime.canonical_state, 41);
    assert_eq!(runtime.pending_surfaces, surfaces);
}

#[test]
fn injected_bootstrap_reuses_initial_surface_validation() {
    let factory_calls = Cell::new(0);
    let empty = MultiWindowRunner::<FakeMultiWindowApp>::initialize_with(
        |_| {
            factory_calls.set(factory_calls.get() + 1);
            Ok::<_, std::convert::Infallible>(41)
        },
        Vec::new(),
    );
    assert!(matches!(
        empty,
        Err(MultiWindowRunError::EmptyInitialSurfaces)
    ));
    assert_eq!(factory_calls.get(), 0);

    let duplicate_id = SurfaceId::new(7);
    let duplicate = vec![
        SurfaceRequest::new(duplicate_id, WindowConfig::default().with_title("One")),
        SurfaceRequest::new(duplicate_id, WindowConfig::default().with_title("Two")),
    ];
    let result = MultiWindowRunner::<FakeMultiWindowApp>::initialize_with(
        |_| Ok::<_, std::convert::Infallible>(41),
        duplicate,
    );

    assert!(matches!(
        result,
        Err(MultiWindowRunError::DuplicateLogicalSurface(id)) if id == duplicate_id
    ));
}

#[test]
fn injected_bootstrap_preserves_factory_failure() {
    let startup_error =
        std::io::Error::new(std::io::ErrorKind::NotFound, "missing panel configuration");
    let result = MultiWindowRunner::<FakeMultiWindowApp>::initialize_with(
        |_| Err::<u64, _>(startup_error),
        vec![SurfaceRequest::new(
            SurfaceId::PRIMARY,
            WindowConfig::default(),
        )],
    );

    assert!(matches!(
        result,
        Err(MultiWindowRunError::Startup(error))
            if error.to_string() == "missing panel configuration"
    ));
}

#[test]
fn injected_bootstrap_accepts_an_already_boxed_failure() {
    let result = MultiWindowRunner::<FakeMultiWindowApp>::initialize_with(
        |_| {
            let error: Box<dyn std::error::Error + Send + Sync> =
                Box::new(std::io::Error::other("boxed startup failure"));
            Err::<u64, _>(error)
        },
        vec![SurfaceRequest::new(
            SurfaceId::PRIMARY,
            WindowConfig::default(),
        )],
    );

    assert!(matches!(
        result,
        Err(MultiWindowRunError::Startup(error))
            if error.to_string() == "boxed startup failure"
    ));
}

#[test]
fn injected_factory_mutates_the_retained_font_system() {
    let runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize_with(
        |font_system| {
            font_system.db_mut().set_sans_serif_family("Injected Sans");
            Ok::<_, std::convert::Infallible>(41)
        },
        vec![SurfaceRequest::new(
            SurfaceId::PRIMARY,
            WindowConfig::default(),
        )],
    )
    .unwrap();

    assert_eq!(
        runtime
            .font_system
            .borrow()
            .db()
            .family_name(&cosmic_text::fontdb::Family::SansSerif),
        "Injected Sans"
    );
}

#[test]
fn suspended_logical_close_does_not_require_a_native_route() {
    let surface = SurfaceId::PRIMARY;
    let mut runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();
    runtime
        .backend_preference
        .retain_committed(BackendType::OpenGl);
    runtime
        .surface_configs
        .insert(surface, WindowConfig::default());

    assert!(runtime.close_surface(surface).is_ok());
    assert!(runtime.surface_configs.is_empty());
    assert!(runtime.routes.is_empty());
    assert_eq!(
        runtime.backend_preference.required_backend(),
        Some(BackendType::OpenGl)
    );
}

#[test]
fn retained_backend_preference_keeps_first_commit_across_later_commits() {
    let mut preference = MultiWindowBackendPreference::default();

    preference.retain_committed(BackendType::OpenGl);
    preference.retain_committed(BackendType::Vulkan);

    assert_eq!(preference.required_backend(), Some(BackendType::OpenGl));
}
