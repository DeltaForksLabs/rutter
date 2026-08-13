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

    fn surface_event(
        state: &mut Self::State,
        surface: SurfaceId,
        event: SurfaceEvent,
    ) -> Vec<SurfaceCommand> {
        if event == SurfaceEvent::FocusChanged(false) {
            return Vec::new();
        }
        *state += 1;
        vec![SurfaceCommand::RequestRedraw(surface)]
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
fn surface_iteration_stops_for_exit_or_fatal_error() {
    assert!(!schedule_iteration_stops(false, false));
    assert!(schedule_iteration_stops(true, false));
    assert!(schedule_iteration_stops(false, true));
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

#[test]
fn native_focus_events_translate_without_exposing_other_winit_events() {
    assert_eq!(
        surface_events::translate_surface_event(&WindowEvent::Focused(true)),
        Some(SurfaceEvent::FocusChanged(true))
    );
    assert_eq!(
        surface_events::translate_surface_event(&WindowEvent::Focused(false)),
        Some(SurfaceEvent::FocusChanged(false))
    );
    assert_eq!(
        surface_events::translate_surface_event(&WindowEvent::CloseRequested),
        None
    );
}

#[test]
fn application_focus_event_publishes_state_and_returns_commands() {
    let mut runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();

    let commands =
        runtime.notify_surface_event(SurfaceId::PRIMARY, SurfaceEvent::FocusChanged(true));

    assert_eq!(runtime.canonical_state, 1);
    assert_eq!(runtime.revision, 1);
    assert_eq!(
        commands,
        vec![SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
    );
}

#[test]
fn suspended_visibility_updates_are_persisted_for_native_resume() {
    let surface = SurfaceId::PRIMARY;
    let mut runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();
    runtime
        .surface_configs
        .insert(surface, WindowConfig::default());
    runtime.surface_runners.insert(
        surface,
        runtime
            .build_surface_runner(&SurfaceRequest::new(surface, WindowConfig::default()))
            .unwrap(),
    );

    runtime.set_surface_visibility(surface, false).unwrap();

    assert!(!runtime.config_for(surface).unwrap().is_visible());
    assert!(
        !runtime
            .config_for(surface)
            .unwrap()
            .window_attributes()
            .visible
    );
}

#[test]
fn suspended_redraw_requests_are_safe_and_unknown_targets_are_rejected() {
    let surface = SurfaceId::PRIMARY;
    let unknown = SurfaceId::new(99);
    let mut runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();
    runtime
        .surface_configs
        .insert(surface, WindowConfig::default());
    runtime.surface_runners.insert(
        surface,
        runtime
            .build_surface_runner(&SurfaceRequest::new(surface, WindowConfig::default()))
            .unwrap(),
    );

    assert!(runtime.request_surface_redraw(surface).is_ok());
    assert!(matches!(
        runtime.request_surface_redraw(unknown),
        Err(MultiWindowRunError::UnknownLogicalSurface(id)) if id == unknown
    ));
    assert!(matches!(
        runtime.set_surface_visibility(unknown, false),
        Err(MultiWindowRunError::UnknownLogicalSurface(id)) if id == unknown
    ));
}

#[test]
fn focus_loss_close_requires_a_prior_focus_gain() {
    let surface = SurfaceId::new(7);
    let mut runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();
    runtime.surface_configs.insert(
        surface,
        WindowConfig::default().with_close_on_focus_loss(true),
    );

    assert!(
        !runtime
            .focus_loss_closes_surface(surface, SurfaceEvent::FocusChanged(false))
            .unwrap()
    );
    assert!(
        !runtime
            .focus_loss_closes_surface(surface, SurfaceEvent::FocusChanged(true))
            .unwrap()
    );
    assert!(
        runtime
            .focus_loss_closes_surface(surface, SurfaceEvent::FocusChanged(false))
            .unwrap()
    );
}

#[test]
fn temporary_surface_is_removed_after_focus_gain_then_loss() {
    let surface = SurfaceId::new(7);
    let mut runtime = MultiWindowRunner::<FakeMultiWindowApp>::initialize().unwrap();
    runtime.surface_configs.insert(
        surface,
        WindowConfig::default().with_close_on_focus_loss(true),
    );

    assert!(
        !runtime
            .focus_loss_closes_surface(surface, SurfaceEvent::FocusChanged(true))
            .unwrap()
    );
    assert!(runtime.surface_configs.contains_key(&surface));
    let closes_automatically = runtime
        .focus_loss_closes_surface(surface, SurfaceEvent::FocusChanged(false))
        .unwrap();
    assert!(
        runtime
            .apply_automatic_surface_close(surface, closes_automatically)
            .unwrap()
    );
    assert!(!runtime.surface_configs.contains_key(&surface));
    assert!(!runtime.focus_acquired_surfaces.contains(&surface));
}

#[test]
fn explicit_lifecycle_commands_suppress_automatic_focus_close() {
    let surface = SurfaceId::new(7);
    assert!(!surface_events::commands_end_surface_lifecycle(
        &[],
        surface
    ));
    assert!(surface_events::commands_end_surface_lifecycle(
        &[SurfaceCommand::Close(surface)],
        surface
    ));
    assert!(surface_events::commands_end_surface_lifecycle(
        &[SurfaceCommand::Exit],
        surface
    ));
}
