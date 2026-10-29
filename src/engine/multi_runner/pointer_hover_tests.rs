//! Full host -> surface runner -> application routing, with deterministic native
//! windows. Xvfb supplies only the constructor clipboard, not pointer delivery.
use super::*;
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::{LogicalPointerPosition, Widget};
use taffy::prelude::*;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{
    ButtonSource, ElementState, MouseButton, MouseScrollDelta, PointerKind, PointerSource,
    TouchPhase,
};

#[derive(Clone, Default)]
pub(super) struct HoverModel {
    pub(super) reports: Vec<(SurfaceId, Option<LogicalPointerPosition>)>,
    pub(super) actions: Vec<(SurfaceId, u8)>,
    deadline: Option<Instant>,
    wakes: usize,
    close_on_leave: bool,
    pub(super) menu: bool,
    membership_only: bool,
    membership: BTreeMap<SurfaceId, bool>,
}

pub(super) struct HoverApp;
impl MultiWindowAppLogic for HoverApp {
    type State = HoverModel;
    type Message = u8;
    fn new(_: &mut FontSystem) -> HoverModel {
        HoverModel::default()
    }
    fn view<'a>(state: &'a mut HoverModel, _: SurfaceId) -> Widget<'a, u8> {
        if state.menu {
            return Widget::dropdown_menu(
                "Menu",
                vec![crate::DropdownMenuEntry::item("Child", 2)],
                Style {
                    size: Size::from_lengths(200.0, 44.0),
                    ..Style::default()
                },
            )
            .with_id(646);
        }
        let fixed = |height| Style {
            size: Size::from_lengths(200.0, height),
            flex_shrink: 0.0,
            ..Style::default()
        };
        let button = |text, on_press| Widget::Button {
            text,
            on_press,
            style: fixed(44.0),
            color: None,
            variant: crate::ButtonVariant::Primary,
        };
        Widget::ScrollView {
            id: 645,
            style: fixed(100.0),
            child: Box::new(Widget::Column {
                children: vec![
                    Widget::button_content(
                        "Header",
                        Widget::rich_text(crate::RichText::plain("Header"), Style::default()),
                        1,
                        fixed(44.0),
                        None,
                        crate::ButtonVariant::Text,
                    ),
                    Widget::Spacer {
                        style: fixed(100.0),
                    },
                    button("Child", 2),
                ],
                style: Style::default(),
            }),
        }
    }
    fn update(
        state: &mut HoverModel,
        surface: SurfaceId,
        message: u8,
        _: &mut arboard::Clipboard,
    ) -> Vec<SurfaceCommand> {
        state.actions.push((surface, message));
        vec![]
    }
    fn surface_pointer_hovered(
        state: &mut HoverModel,
        surface: SurfaceId,
        position: Option<LogicalPointerPosition>,
    ) -> Option<Vec<SurfaceCommand>> {
        if state.membership_only {
            let inside = position.is_some();
            if state.membership.get(&surface).copied().unwrap_or(false) == inside {
                return None;
            }
            state.membership.insert(surface, inside);
        }
        state.reports.push((surface, position));
        state.deadline = Some(Instant::now() + std::time::Duration::from_secs(1));
        if position.is_none() && state.close_on_leave {
            Some(vec![SurfaceCommand::Close(surface)])
        } else {
            Some(vec![])
        }
    }
    fn next_wakeup(state: &HoverModel, _: Instant) -> Option<Instant> {
        state.deadline
    }
    fn wakeup(state: &mut HoverModel, _: Instant) -> Vec<SurfaceCommand> {
        state.wakes += 1;
        state.deadline = None;
        vec![]
    }
}

struct DefaultHoverApp;

impl MultiWindowAppLogic for DefaultHoverApp {
    type State = HoverModel;
    type Message = u8;

    fn new(fonts: &mut FontSystem) -> HoverModel {
        HoverApp::new(fonts)
    }

    fn view<'a>(state: &'a mut HoverModel, surface: SurfaceId) -> Widget<'a, u8> {
        HoverApp::view(state, surface)
    }

    fn update(
        state: &mut HoverModel,
        surface: SurfaceId,
        message: u8,
        clipboard: &mut arboard::Clipboard,
    ) -> Vec<SurfaceCommand> {
        HoverApp::update(state, surface, message, clipboard)
    }
}

fn runtime(scale: f64) -> MultiWindowRunner<HoverApp> {
    runtime_for::<HoverApp>(scale)
}

fn runtime_for<A: MultiWindowAppLogic + 'static>(scale: f64) -> MultiWindowRunner<A> {
    let mut runtime = MultiWindowRunner::<A>::initialize().unwrap();
    runtime.pending_surfaces.clear();
    for (surface, popup) in [(SurfaceId::PRIMARY, false), (SurfaceId::new(5), true)] {
        let mut engine = RutterEngine::<SurfaceAppAdapter<A>>::with_shared_font_system(
            SurfaceAppState::new(surface, runtime.canonical_state.clone(), runtime.revision),
            runtime.font_system.clone(),
            Default::default(),
        )
        .unwrap();
        let size = LogicalSize::new(200, 100).to_physical(scale);
        engine.scale_factor = scale as f32;
        engine.window = Some(Rc::new(MemoryWindow {
            size,
            scale,
            popup,
            popup_state: None,
            input_state: None,
        }));
        engine.try_ensure_layout_for_input(size).unwrap();
        let runner = RutterRunner::with_active_test_engine(engine);
        runtime
            .register_committed_surface(
                SurfaceRequest::new(surface, WindowConfig::default()),
                runner.native_window().unwrap().id(),
                runner,
            )
            .unwrap();
    }
    runtime.native_surfaces_active = true;
    runtime
}

fn moved(x: f64, y: f64, scale: f64) -> WindowEvent {
    WindowEvent::PointerMoved {
        device_id: None,
        position: PhysicalPosition::new(x * scale, y * scale),
        primary: true,
        source: PointerSource::Mouse,
    }
}

fn click(runtime: &mut MultiWindowRunner<HoverApp>, native: WindowId, y: f64, scale: f64) {
    for state in [ElementState::Pressed, ElementState::Released] {
        runtime.window_event(
            &MemoryEventLoop::default(),
            native,
            WindowEvent::PointerButton {
                device_id: None,
                state,
                position: PhysicalPosition::new(60.0 * scale, y * scale),
                primary: true,
                button: ButtonSource::Mouse(MouseButton::Left),
                is_macos_activation_click: false,
            },
        );
    }
}

#[test]
fn native_unpressed_motion_entry_leave_scale_publication_and_scheduler() {
    for scale in [1.0, 1.5, 2.0] {
        let mut runtime = runtime(scale);
        let event_loop = MemoryEventLoop::default();
        for surface in [SurfaceId::PRIMARY, SurfaceId::new(5)] {
            let native = runtime.routes.native_for(surface).unwrap();
            runtime.window_event(
                &event_loop,
                native,
                WindowEvent::PointerEntered {
                    device_id: None,
                    position: PhysicalPosition::new(60.0 * scale, 22.0 * scale),
                    primary: true,
                    kind: PointerKind::Mouse,
                },
            );
            runtime.window_event(&event_loop, native, moved(61.0, 23.0, scale));
            runtime.window_event(&event_loop, native, moved(62.0, 24.0, scale));
            runtime.window_event(
                &event_loop,
                native,
                WindowEvent::PointerLeft {
                    device_id: None,
                    position: None,
                    primary: true,
                    kind: PointerKind::Mouse,
                },
            );
            let reports = &runtime.canonical_state.reports;
            assert_eq!(
                &reports[reports.len() - 4..],
                &[
                    (surface, Some(LogicalPointerPosition::new(60.0, 22.0))),
                    (surface, Some(LogicalPointerPosition::new(61.0, 23.0))),
                    (surface, Some(LogicalPointerPosition::new(62.0, 24.0))),
                    (surface, None),
                ]
            );
            for runner in runtime.surface_runners.values_mut() {
                assert_eq!(runner.app_state_mut().model.reports, *reports);
                assert_eq!(runner.app_state_mut().revision, runtime.revision);
            }
            assert!(matches!(
                event_loop.control_flow(),
                ControlFlow::WaitUntil(_)
            ));
        }
        let due = runtime.canonical_state.deadline.unwrap();
        runtime.process_application_wakeup(&event_loop, due);
        assert_eq!(runtime.canonical_state.wakes, 1);
        assert_eq!(runtime.canonical_state.deadline, None);
        let reports = runtime.canonical_state.reports.clone();
        runtime.window_event(&event_loop, WindowId::from_raw(99), moved(1.0, 1.0, scale));
        runtime.close_surface(SurfaceId::new(5)).unwrap();
        runtime.window_event(&event_loop, WindowId::from_raw(2), moved(1.0, 1.0, scale));
        // Native-menu dispatch has a parent logical ID but a different native ID.
        // Exercise that same dispatch boundary without creating a native popup.
        runtime.dispatch_surface_event(
            &event_loop,
            SurfaceId::PRIMARY,
            WindowId::from_raw(2),
            moved(1.0, 1.0, scale),
        );
        assert_eq!(runtime.canonical_state.reports, reports);
        assert!(runtime.fatal_error.is_none());
    }
}

#[test]
fn observation_preserves_primary_child_clicks_and_wheel_scrolling() {
    for scale in [1.0, 1.5, 2.0] {
        let mut runtime = runtime(scale);
        let native = runtime.routes.native_for(SurfaceId::PRIMARY).unwrap();
        let event_loop = MemoryEventLoop::default();
        runtime.window_event(&event_loop, native, moved(60.0, 22.0, scale));
        click(&mut runtime, native, 22.0, scale);
        runtime.window_event(
            &event_loop,
            native,
            WindowEvent::MouseWheel {
                device_id: None,
                delta: MouseScrollDelta::LineDelta(0.0, -10.0),
                phase: TouchPhase::Moved,
            },
        );
        runtime.window_event(&event_loop, native, moved(60.0, 78.0, scale));
        click(&mut runtime, native, 78.0, scale);
        assert_eq!(
            runtime.canonical_state.actions,
            [(SurfaceId::PRIMARY, 1), (SurfaceId::PRIMARY, 2)]
        );
        assert_eq!(runtime.canonical_state.reports.len(), 2);
        assert!(runtime.fatal_error.is_none());
    }
}

#[test]
fn observed_header_retains_keyboard_enter_and_space_activation() {
    use winit::event::KeyEvent;
    use winit::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};

    let mut runtime = runtime(1.5);
    let native = runtime.routes.native_for(SurfaceId::PRIMARY).unwrap();
    let event_loop = MemoryEventLoop::default();
    runtime.window_event(&event_loop, native, moved(60.0, 22.0, 1.5));
    assert!(
        runtime.canonical_state.actions.is_empty(),
        "hover does not activate or focus the header"
    );
    for key in [
        Key::Named(NamedKey::Tab),
        Key::Named(NamedKey::Enter),
        Key::Character(" ".into()),
    ] {
        runtime.window_event(
            &event_loop,
            native,
            WindowEvent::KeyboardInput {
                device_id: None,
                is_synthetic: false,
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::Tab),
                    logical_key: key.clone(),
                    text: None,
                    location: KeyLocation::Standard,
                    state: ElementState::Pressed,
                    repeat: false,
                    text_with_all_modifiers: None,
                    key_without_modifiers: key,
                },
            },
        );
    }
    assert_eq!(
        runtime.canonical_state.actions,
        [(SurfaceId::PRIMARY, 1), (SurfaceId::PRIMARY, 1)]
    );
    assert_eq!(runtime.canonical_state.reports.len(), 1);
}

#[test]
fn observer_commands_retire_only_the_source_and_late_native_events_are_ignored() {
    let mut runtime = runtime(1.0);
    runtime.canonical_state.close_on_leave = true;
    runtime.publish_wakeup_state(runtime.canonical_state.clone(), runtime.revision + 1);
    let native = runtime.routes.native_for(SurfaceId::PRIMARY).unwrap();
    let event_loop = MemoryEventLoop::default();
    runtime.window_event(
        &event_loop,
        native,
        WindowEvent::PointerLeft {
            device_id: None,
            position: None,
            primary: true,
            kind: PointerKind::Mouse,
        },
    );
    assert!(runtime.routes.surface_for(native).is_none());
    assert!(!runtime.surface_configs.contains_key(&SurfaceId::PRIMARY));
    assert!(runtime.surface_configs.contains_key(&SurfaceId::new(5)));
    assert_eq!(
        runtime.canonical_state.reports,
        [(SurfaceId::PRIMARY, None)]
    );
    runtime.window_event(&event_loop, native, moved(60.0, 22.0, 1.0));
    assert_eq!(
        runtime.canonical_state.reports,
        [(SurfaceId::PRIMARY, None)]
    );
    assert!(runtime.fatal_error.is_none());
}

fn cache_states<A: MultiWindowAppLogic + 'static>(
    runtime: &MultiWindowRunner<A>,
) -> Vec<(bool, u64)> {
    runtime
        .surface_runners
        .values()
        .map(|runner| runner.test_layout_cache_state())
        .collect()
}

fn settle_layouts<A: MultiWindowAppLogic + 'static>(runtime: &mut MultiWindowRunner<A>) {
    for runner in runtime.surface_runners.values_mut() {
        runner
            .sync_native_menu(&MemoryEventLoop::default())
            .unwrap();
        assert!(!runner.test_layout_cache_state().0);
    }
}

fn left() -> WindowEvent {
    WindowEvent::PointerLeft {
        device_id: None,
        position: None,
        primary: true,
        kind: PointerKind::Mouse,
    }
}

#[test]
fn default_hover_preserves_revisions_and_cached_layouts_on_every_surface() {
    let mut runtime = runtime_for::<DefaultHoverApp>(1.5);
    let event_loop = MemoryEventLoop::default();
    settle_layouts(&mut runtime);
    let revision = runtime.revision;
    let caches = cache_states(&runtime);
    for surface in [SurfaceId::PRIMARY, SurfaceId::new(5)] {
        let native = runtime.routes.native_for(surface).unwrap();
        for event in [
            WindowEvent::PointerEntered {
                device_id: None,
                position: PhysicalPosition::new(90.0, 33.0),
                primary: true,
                kind: PointerKind::Mouse,
            },
            moved(60.0, 22.0, 1.5),
            moved(61.0, 23.0, 1.5),
            left(),
            left(),
        ] {
            runtime.window_event(&event_loop, native, event);
            assert_eq!(runtime.revision, revision);
            assert_eq!(cache_states(&runtime), caches);
            settle_layouts(&mut runtime);
            assert_eq!(cache_states(&runtime), caches, "no deferred cache rebuild");
            for runner in runtime.surface_runners.values_mut() {
                assert_eq!(runner.app_state_mut().revision, revision);
                assert!(runner.app_state_mut().model.reports.is_empty());
            }
        }
    }
    assert!(runtime.canonical_state.reports.is_empty());
    assert!(runtime.fatal_error.is_none());
}

#[test]
fn changed_membership_publishes_but_repeated_hover_and_leave_keep_cached_layouts() {
    let mut runtime = runtime(1.5);
    runtime.canonical_state.membership_only = true;
    runtime.publish_wakeup_state(runtime.canonical_state.clone(), runtime.revision + 1);
    let event_loop = MemoryEventLoop::default();
    for surface in [SurfaceId::PRIMARY, SurfaceId::new(5)] {
        let native = runtime.routes.native_for(surface).unwrap();
        for inside in [true, false] {
            settle_layouts(&mut runtime);
            let revision = runtime.revision;
            let event = if inside {
                moved(60.0, 22.0, 1.5)
            } else {
                left()
            };
            runtime.window_event(&event_loop, native, event);
            assert_eq!(runtime.revision, revision + 1);
            assert!(cache_states(&runtime).iter().all(|(dirty, _)| *dirty));
            for runner in runtime.surface_runners.values_mut() {
                let published = runner.app_state_mut();
                assert_eq!(published.revision, revision + 1);
                assert_eq!(published.model.membership[&surface], inside);
                assert_eq!(published.model.reports, runtime.canonical_state.reports);
            }
            settle_layouts(&mut runtime);
            let caches = cache_states(&runtime);
            for _ in 0..3 {
                let event = if inside {
                    moved(61.0, 23.0, 1.5)
                } else {
                    left()
                };
                runtime.window_event(&event_loop, native, event);
                assert_eq!(runtime.revision, revision + 1);
                assert_eq!(cache_states(&runtime), caches);
                settle_layouts(&mut runtime);
                assert_eq!(cache_states(&runtime), caches);
                for runner in runtime.surface_runners.values_mut() {
                    assert_eq!(runner.app_state_mut().revision, revision + 1);
                }
            }
        }
    }
    assert_eq!(runtime.canonical_state.reports.len(), 4);
    assert!(runtime.fatal_error.is_none());
}
