use std::cell::Cell;

use arboard::Clipboard;
use taffy::prelude::Style;
use winit::dpi::PhysicalSize;

use super::*;
use crate::{
    ContextMenuEntry, PopupAnchor, PopupConstraints, PopupDirection, SurfaceMetrics, Widget,
};

// Reuse the runner's in-memory Window/raster backend; no graphical backend is created.
use crate::engine::test_surface as fixture;
use fixture::{MemoryWindow, RasterPopup};

#[derive(Clone, Default)]
struct PopupModel {
    closed: Vec<(SurfaceId, usize, usize)>,
    messages: BTreeMap<SurfaceId, usize>,
    parent_drops: Rc<Cell<usize>>,
    popup_drops: Rc<Cell<usize>>,
    metrics: BTreeMap<SurfaceId, SurfaceMetrics>,
    menu_requested_on: Option<SurfaceId>,
}

struct PopupApp;

impl MultiWindowAppLogic for PopupApp {
    type State = PopupModel;
    type Message = ();
    fn new(_: &mut FontSystem) -> PopupModel {
        PopupModel::default()
    }
    fn view<'a>(_: &'a mut PopupModel, _: SurfaceId) -> Widget<'a, ()> {
        const ENTRIES: &[ContextMenuEntry<'static, ()>] = &[ContextMenuEntry::Item {
            label: "Action",
            on_select: Some(()),
        }];
        Widget::context_menu(
            Widget::Spacer {
                style: Style::default(),
            },
            ENTRIES,
            Style::default(),
        )
        .with_id(7)
    }
    fn update(
        state: &mut PopupModel,
        surface: SurfaceId,
        _: (),
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        *state.messages.entry(surface).or_default() += 1;
        Vec::new()
    }
    fn surface_closed(state: &mut PopupModel, surface: SurfaceId) {
        state
            .closed
            .push((surface, state.parent_drops.get(), state.popup_drops.get()));
    }
    fn surface_metrics_changed(
        state: &mut PopupModel,
        surface: SurfaceId,
        metrics: SurfaceMetrics,
    ) {
        state.metrics.insert(surface, metrics);
    }
}

const CHILD: SurfaceId = SurfaceId::new(8);

fn config(parent: SurfaceId) -> PopupConfig {
    PopupConfig::new(
        parent,
        PopupAnchor::new(0, 0, 32, 32).unwrap(),
        320,
        360,
        PopupDirection::Above,
    )
    .unwrap()
}

fn install(runtime: &mut MultiWindowRunner<PopupApp>, surface: SurfaceId, popup: bool, scale: f64) {
    let window: Rc<dyn winit::window::Window> = Rc::new(MemoryWindow {
        size: if popup {
            PhysicalSize::new(300, 275)
        } else {
            LogicalSize::new(640, 32).to_physical(scale)
        },
        scale,
        popup,
        popup_state: None,
    });
    let drops = if popup {
        runtime.canonical_state.popup_drops.clone()
    } else {
        runtime.canonical_state.parent_drops.clone()
    };
    let window_config = if popup {
        WindowConfig::default().with_popup(config(SurfaceId::PRIMARY))
    } else {
        WindowConfig::default()
    };
    let state = SurfaceAppState::<PopupApp>::new(
        surface,
        runtime.canonical_state.clone(),
        runtime.revision,
    );
    let mut engine = RutterEngine::<SurfaceAppAdapter<PopupApp>>::with_shared_font_system(
        state,
        runtime.font_system.clone(),
        window_config.surface_config(),
    )
    .unwrap();
    engine.scale_factor = scale as f32;
    engine.window = Some(window.clone());
    engine.graphics_backend = Some(Box::new(RasterPopup {
        window: window.clone(),
        surface: skia_safe::surfaces::raster_n32_premul((1, 1)).unwrap(),
        frames: Rc::new(Cell::new(0)),
        drops,
        configure_on_frame: None,
    }));
    if runtime.canonical_state.menu_requested_on == Some(surface) {
        engine
            .try_ensure_layout_for_input(window.surface_size())
            .unwrap();
        engine
            .widget_states
            .get_mut(&7)
            .unwrap()
            .as_context_menu_mut()
            .unwrap()
            .open_at(4.0, 4.0);
    }
    runtime
        .register_committed_surface(
            SurfaceRequest::new(surface, window_config),
            window.id(),
            RutterRunner::with_engine(engine),
        )
        .unwrap();
}

fn runtime() -> MultiWindowRunner<PopupApp> {
    let mut runtime = MultiWindowRunner::<PopupApp>::initialize().unwrap();
    runtime.pending_surfaces.clear();
    install(&mut runtime, SurfaceId::PRIMARY, false, 1.25);
    runtime
}

#[test]
fn native_attributes_request_real_role_logical_geometry_exact_flags_and_required_grab() {
    let popup = config(SurfaceId::PRIMARY)
        .with_offset(0, -6)
        .unwrap()
        .with_constraints(PopupConstraints::SLIDE_X)
        .unwrap();
    let attributes = popup_attributes(&WindowConfig::default().with_popup(popup), popup);
    assert_eq!(attributes.window_type(), WindowType::Popup);
    assert!(attributes.active);
    assert_eq!(attributes.popup_grab, PopupGrab::Required);
    let non_grabbing = popup_attributes(
        &WindowConfig::default().with_popup(popup.with_grab(false)),
        popup.with_grab(false),
    );
    assert!(!non_grabbing.active);
    assert_eq!(non_grabbing.popup_grab, PopupGrab::None);
    assert!(!attributes.decorations);
    assert!(!attributes.resizable);
    assert!(attributes.layer_shell.is_none());
    assert!(attributes.panel.is_none());
    assert!(attributes.position.is_none());
    assert_eq!(
        attributes.positioner.unwrap().constraint_adjustment,
        PopupConstraints::SLIDE_X
    );
    for scale in [1.0, 1.25, 2.0] {
        assert_eq!(
            attributes.surface_size.unwrap().to_logical::<f64>(scale),
            LogicalSize::new(320.0, 360.0)
        );
    }
}

#[test]
fn live_parent_resolution_reports_unknown_suspended_retired_and_out_of_bounds_parents() {
    let mut runtime = runtime();
    for (parent, expected) in [
        (SurfaceId::new(99), "not registered"),
        (CHILD, "no live native surface"),
    ] {
        if parent == CHILD {
            runtime
                .surface_configs
                .insert(CHILD, WindowConfig::default());
        }
        let request = SurfaceRequest::new(
            SurfaceId::new(20),
            WindowConfig::default().with_popup(config(parent)),
        );
        let error = runtime.surface_attributes(&request).unwrap_err();
        assert!(matches!(error, MultiWindowRunError::Popup { parent: id, .. } if id == parent));
        assert!(error.to_string().contains(expected));
    }
    let outside = PopupConfig::new(
        SurfaceId::PRIMARY,
        PopupAnchor::new(620, 0, 32, 32).unwrap(),
        320,
        360,
        PopupDirection::Below,
    )
    .unwrap();
    let request = SurfaceRequest::new(
        SurfaceId::new(20),
        WindowConfig::default().with_popup(outside),
    );
    assert!(
        runtime
            .surface_attributes(&request)
            .unwrap_err()
            .to_string()
            .contains("configured logical viewport")
    );
    let request = SurfaceRequest::new(
        SurfaceId::new(20),
        WindowConfig::default().with_popup(config(SurfaceId::PRIMARY)),
    );
    assert!(
        runtime
            .surface_attributes(&request)
            .unwrap_err()
            .to_string()
            .contains("resolve live native parent handle")
    );
    runtime.close_surface(SurfaceId::PRIMARY).unwrap();
    assert!(
        runtime
            .surface_attributes(&request)
            .unwrap_err()
            .to_string()
            .contains("not registered")
    );
}

#[test]
fn request_budget_is_checked_at_actual_parent_scale_before_native_creation() {
    let popup = PopupConfig::new(
        SurfaceId::PRIMARY,
        PopupAnchor::new(0, 0, 1, 1).unwrap(),
        2048,
        2048,
        PopupDirection::Below,
    )
    .unwrap();
    assert!(validate_requested_allocation(popup, 1.0).is_ok());
    for scale in [1.25, 2.0, f64::INFINITY, f64::NAN, 0.0, -1.0] {
        assert!(validate_requested_allocation(popup, scale).is_err());
    }
}

#[test]
fn popup_creation_rejects_self_nested_hidden_parents_and_top_level_backend_fallback() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    for (surface, parent, expected) in [
        (
            SurfaceId::PRIMARY,
            SurfaceId::PRIMARY,
            "cannot parent itself",
        ),
        (SurfaceId::new(20), CHILD, "nested"),
    ] {
        let request =
            SurfaceRequest::new(surface, WindowConfig::default().with_popup(config(parent)));
        assert!(
            runtime
                .surface_attributes(&request)
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
    }
    let request = SurfaceRequest::new(
        CHILD,
        WindowConfig::default().with_popup(config(SurfaceId::PRIMARY)),
    );
    assert!(
        MultiWindowRunner::<PopupApp>::validate_committed_popup(
            &request,
            &runtime.surface_runners[&SurfaceId::PRIMARY]
        )
        .unwrap_err()
        .to_string()
        .contains("no top-level fallback")
    );
    assert!(
        MultiWindowRunner::<PopupApp>::validate_committed_popup(
            &request,
            &runtime.surface_runners[&CHILD]
        )
        .is_ok()
    );
    // Reopening the same ID validates its new native parent before discarding
    // the old popup. The memory parent's raw handle deliberately fails here.
    let error = runtime
        .open_surface(&fixture::MemoryEventLoop::default(), request)
        .unwrap_err();
    assert!(matches!(
        error,
        MultiWindowRunError::Popup { surface: CHILD, .. }
    ));
    assert_eq!(runtime.arbitrary_popup(), Some(CHILD));
    runtime
        .surface_configs
        .get_mut(&SurfaceId::PRIMARY)
        .unwrap()
        .set_visible(false);
    let request = SurfaceRequest::new(
        SurfaceId::new(20),
        WindowConfig::default().with_popup(config(SurfaceId::PRIMARY)),
    );
    assert!(
        runtime
            .surface_attributes(&request)
            .unwrap_err()
            .to_string()
            .contains("parent is hidden")
    );
}

#[test]
fn popup_configured_size_and_scale_are_published_instead_of_requested_dimensions() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    runtime.notify_surface_metrics(CHILD);
    let metrics = &runtime.canonical_state.metrics[&CHILD];
    assert_eq!(metrics.physical_size, (300, 275));
    assert_eq!(metrics.logical_size, (240.0, 220.0));
    assert_eq!(metrics.scale_factor, 1.25);
}

#[test]
fn close_parent_releases_child_graphics_and_routes_before_parent_and_notifies_once() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    runtime.close_surface(SurfaceId::PRIMARY).unwrap();
    assert_eq!(
        runtime.canonical_state.closed,
        vec![(CHILD, 0, 1), (SurfaceId::PRIMARY, 1, 1)]
    );
    assert!(runtime.routes.is_empty());
    assert!(runtime.surface_runners.is_empty());
    assert!(runtime.surface_configs.is_empty());
}

#[test]
fn suspension_discards_transient_but_preserves_parent_and_reopen_routes_are_fresh() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    runtime.release_native_surfaces();
    assert_eq!(runtime.canonical_state.closed, vec![(CHILD, 0, 1)]);
    assert!(runtime.surface_configs.contains_key(&SurfaceId::PRIMARY));
    assert!(!runtime.surface_configs.contains_key(&CHILD));
    assert!(runtime.routes.is_empty());
    install(&mut runtime, CHILD, true, 2.0);
    assert_eq!(
        runtime.routes.surface_for(WindowId::from_raw(2)),
        Some(CHILD)
    );
    runtime.close_surface(CHILD).unwrap();
    assert_eq!(runtime.canonical_state.popup_drops.get(), 2);
}

#[test]
fn parent_geometry_change_closes_popup_and_source_aware_messages_skip_retired_popup() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    runtime.deliver_external_message(CHILD, ()).unwrap();
    runtime
        .deliver_external_message(SurfaceId::PRIMARY, ())
        .unwrap();
    assert_eq!(runtime.canonical_state.messages[&CHILD], 1);
    assert_eq!(runtime.canonical_state.messages[&SurfaceId::PRIMARY], 1);
    assert!(
        !runtime
            .popup_lifecycle_event(
                SurfaceId::PRIMARY,
                &WindowEvent::SurfaceResized(PhysicalSize::new(800, 40))
            )
            .unwrap()
    );
    assert!(!runtime.surface_configs.contains_key(&CHILD));
    assert_eq!(runtime.deliver_external_message(CHILD, ()).unwrap(), None);
    assert_eq!(runtime.canonical_state.closed.len(), 1);
}

#[test]
fn replacement_and_hiding_release_popup_without_hiding_its_live_parent() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    runtime.prepare_popup_open();
    assert_eq!(runtime.arbitrary_popup(), None);
    assert!(runtime.surface_configs[&SurfaceId::PRIMARY].is_visible());
    install(&mut runtime, CHILD, true, 1.0);
    runtime.set_surface_visibility(CHILD, false).unwrap();
    assert_eq!(runtime.arbitrary_popup(), None);
    assert_eq!(runtime.canonical_state.closed.len(), 2);
}

fn key_event(key: NamedKey) -> WindowEvent {
    WindowEvent::KeyboardInput {
        device_id: None,
        is_synthetic: false,
        event: winit::event::KeyEvent {
            physical_key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape),
            logical_key: Key::Named(key),
            text: None,
            location: winit::keyboard::KeyLocation::Standard,
            state: ElementState::Pressed,
            repeat: false,
            text_with_all_modifiers: None,
            key_without_modifiers: Key::Named(key),
        },
    }
}

#[test]
fn keyboard_routes_only_from_invoking_parent_and_escape_closes_through_runtime_boundary() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    let keyboard = key_event(NamedKey::Tab);
    assert_eq!(
        runtime.popup_keyboard_target(SurfaceId::PRIMARY, &keyboard),
        Some((CHILD, WindowId::from_raw(2)))
    );
    assert_eq!(
        runtime.popup_keyboard_target(SurfaceId::new(99), &keyboard),
        None
    );
    assert_eq!(runtime.popup_keyboard_target(CHILD, &keyboard), None);
    assert!(
        !runtime
            .popup_lifecycle_event(SurfaceId::new(99), &key_event(NamedKey::Escape))
            .unwrap()
    );
    assert_eq!(runtime.arbitrary_popup(), Some(CHILD));
    runtime.window_event(
        &fixture::MemoryEventLoop::default(),
        WindowId::from_raw(1),
        key_event(NamedKey::Escape),
    );
    assert_eq!(runtime.arbitrary_popup(), None);
    assert_eq!(runtime.canonical_state.closed, vec![(CHILD, 0, 1)]);
}

#[test]
fn native_dismissal_and_destroy_events_notify_once_and_reopening_uses_fresh_route() {
    for event in [WindowEvent::CloseRequested, WindowEvent::Destroyed] {
        let mut runtime = runtime();
        install(&mut runtime, CHILD, true, 1.25);
        let event_loop = fixture::MemoryEventLoop::default();
        runtime.window_event(&event_loop, WindowId::from_raw(2), event);
        runtime.window_event(&event_loop, WindowId::from_raw(2), WindowEvent::Destroyed);
        assert_eq!(runtime.canonical_state.closed, vec![(CHILD, 0, 1)]);
        install(&mut runtime, CHILD, true, 2.0);
        assert_eq!(
            runtime.routes.surface_for(WindowId::from_raw(2)),
            Some(CHILD)
        );
    }
}

#[test]
fn menu_arbitration_closes_popup_on_other_parent_but_suppresses_nested_menu() {
    let mut runtime = MultiWindowRunner::<PopupApp>::initialize().unwrap();
    runtime.pending_surfaces.clear();
    runtime.canonical_state.menu_requested_on = Some(SurfaceId::PRIMARY);
    install(&mut runtime, SurfaceId::PRIMARY, false, 1.0);
    install(&mut runtime, CHILD, true, 1.0);
    runtime.coordinate_native_transients().unwrap();
    assert_eq!(runtime.arbitrary_popup(), None);
    assert!(
        runtime
            .runner_for_mut(SurfaceId::PRIMARY)
            .unwrap()
            .has_transient_menu_request()
            .unwrap()
    );

    runtime.prepare_popup_open();
    runtime.canonical_state.menu_requested_on = Some(CHILD);
    install(&mut runtime, CHILD, true, 1.0);
    runtime.coordinate_native_transients().unwrap();
    assert_eq!(runtime.arbitrary_popup(), Some(CHILD));
    assert!(
        !runtime
            .runner_for_mut(CHILD)
            .unwrap()
            .has_transient_menu_request()
            .unwrap()
    );
}

#[test]
fn parent_pointer_outside_anchor_dismisses_but_anchor_remains_available_for_toggle() {
    let mut runtime = runtime();
    install(&mut runtime, CHILD, true, 1.25);
    for (x, remains_open) in [(20.0, true), (50.0, false)] {
        let event = WindowEvent::PointerButton {
            device_id: None,
            state: ElementState::Pressed,
            position: winit::dpi::PhysicalPosition::new(x, 20.0),
            primary: true,
            button: winit::event::ButtonSource::Mouse(winit::event::MouseButton::Left),
            is_macos_activation_click: false,
        };
        runtime
            .popup_lifecycle_event(SurfaceId::PRIMARY, &event)
            .unwrap();
        assert_eq!(runtime.arbitrary_popup().is_some(), remains_open);
    }
}
