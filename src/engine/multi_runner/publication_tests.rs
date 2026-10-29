//! Publication boundary regressions; no parent redraw is delivered between
//! replacing the canonical model and resolving a native-menu request.

use arboard::Clipboard;
use taffy::prelude::{Size, Style};
use winit::dpi::PhysicalSize;

use super::*;
use crate::{ContextMenuEntry, DropdownMenuEntry, Widget};

use crate::engine::test_surface as fixture;

#[derive(Clone)]
struct PublishedMenus {
    owners: Vec<u64>,
    context: bool,
    deadline: Option<Instant>,
    live_clock: bool,
}

struct PublishedMenuApp;

impl MultiWindowAppLogic for PublishedMenuApp {
    type State = PublishedMenus;
    type Message = ();

    fn new(_: &mut FontSystem) -> PublishedMenus {
        PublishedMenus {
            owners: vec![7],
            context: true,
            deadline: None,
            live_clock: false,
        }
    }

    fn view<'a>(state: &'a mut PublishedMenus, surface: SurfaceId) -> Widget<'a, ()> {
        const ENTRIES: &[ContextMenuEntry<'static, ()>] = &[ContextMenuEntry::Item {
            label: "Run",
            on_select: Some(()),
        }];
        let owner_style = Style {
            size: Size::length(100.0_f32),
            flex_shrink: 0.0,
            ..Style::default()
        };
        let mut children: Vec<_> = state
            .owners
            .iter()
            .map(|id| {
                if state.context {
                    Widget::context_menu(
                        Widget::Spacer {
                            style: owner_style.clone(),
                        },
                        ENTRIES,
                        owner_style.clone(),
                    )
                    .with_id(*id)
                } else {
                    Widget::dropdown_menu(
                        "Run",
                        vec![DropdownMenuEntry::item("Run", ())],
                        owner_style.clone(),
                    )
                    .with_id(*id)
                }
            })
            .collect();
        if state.live_clock && surface == SurfaceId::new(5) {
            children.push(Widget::clock_with_config(
                crate::TimeZone::UTC,
                crate::ClockConfig::new(crate::ClockFormat::twenty_four_hour(), 14.0).unwrap(),
                Style::default(),
                "Test clock",
            ));
        }
        Widget::Row {
            children,
            style: Style::default(),
        }
    }

    fn update(
        state: &mut PublishedMenus,
        _: SurfaceId,
        _: (),
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        state.owners.insert(0, 9);
        Vec::new()
    }

    fn next_wakeup(state: &PublishedMenus, _: Instant) -> Option<Instant> {
        state.deadline
    }

    fn wakeup(state: &mut PublishedMenus, _: Instant) -> Vec<SurfaceCommand> {
        state.owners.insert(0, 9);
        state.deadline = None;
        Vec::new()
    }
}

fn runtime(context: bool) -> MultiWindowRunner<PublishedMenuApp> {
    runtime_with_clock(context, false)
}

fn runtime_with_clock(context: bool, live_clock: bool) -> MultiWindowRunner<PublishedMenuApp> {
    let mut runtime = MultiWindowRunner::<PublishedMenuApp>::initialize().unwrap();
    runtime.pending_surfaces.clear();
    runtime.canonical_state.context = context;
    runtime.canonical_state.live_clock = live_clock;
    for surface in [SurfaceId::PRIMARY, SurfaceId::new(5)] {
        let state =
            SurfaceAppState::new(surface, runtime.canonical_state.clone(), runtime.revision);
        let mut engine =
            RutterEngine::<SurfaceAppAdapter<PublishedMenuApp>>::with_shared_font_system(
                state,
                runtime.font_system.clone(),
                WindowConfig::default().surface_config(),
            )
            .unwrap();
        let size = PhysicalSize::new(640, 32);
        engine.window = Some(Rc::new(fixture::MemoryWindow {
            size,
            scale: 1.0,
            popup: false,
            popup_state: None,
            input_state: None,
        }));
        engine.try_ensure_layout_for_input(size).unwrap();
        if context {
            engine
                .widget_states
                .get_mut(&7)
                .unwrap()
                .as_context_menu_mut()
                .unwrap()
                .open_at(8.0, 8.0);
        } else {
            engine
                .widget_states
                .get_mut(&7)
                .unwrap()
                .as_dropdown_menu_mut()
                .unwrap()
                .open_at_index(Some(0));
        }
        engine.layout_dirty = true;
        runtime
            .surface_configs
            .insert(surface, WindowConfig::default());
        runtime
            .surface_runners
            .insert(surface, RutterRunner::with_engine(engine));
    }
    // Prove the old trees have finished their initial layout before publication.
    assert_requests_resolve(&mut runtime);
    runtime
}

fn assert_requests_resolve(runtime: &mut MultiWindowRunner<PublishedMenuApp>) {
    for runner in runtime.surface_runners.values_mut() {
        assert!(runner.has_transient_menu_request().unwrap());
        assert!(runner.take_fatal_error().is_none());
    }
}

#[test]
fn wakeup_without_redraw_commands_reconciles_each_surface_before_menu_resolution() {
    for context in [true, false] {
        let mut runtime = runtime(context);
        let now = Instant::now();
        runtime.canonical_state.deadline = Some(now);
        assert_eq!(
            runtime.process_application_wakeup(&fixture::MemoryEventLoop::default(), now),
            None
        );
        assert_eq!(runtime.canonical_state.owners, [9, 7]);
        assert_requests_resolve(&mut runtime);
    }
}

#[test]
fn cross_surface_and_external_publications_reconcile_other_surface_before_redraw() {
    for context in [true, false] {
        for external in [false, true] {
            let mut runtime = runtime(context);
            let source = SurfaceId::new(5);
            if external {
                assert_eq!(
                    runtime.deliver_external_message(source, ()).unwrap(),
                    Some(Vec::new())
                );
            } else {
                runtime.runner_for_mut(source).unwrap().deliver_message(());
                assert!(
                    runtime
                        .synchronize_surface_state(source)
                        .unwrap()
                        .is_empty()
                );
            }
            assert_eq!(runtime.canonical_state.owners, [9, 7]);
            assert_requests_resolve(&mut runtime);
        }
    }
}

#[test]
fn a_live_clock_keeps_its_wakeup_when_event_scheduling_returns_to_wait() {
    let mut runtime = runtime(true);
    let event_loop = fixture::MemoryEventLoop::default();
    runtime.native_surfaces_active = true;
    runtime.schedule_after_event(&event_loop);
    assert_eq!(event_loop.control_flow(), ControlFlow::Wait);

    let mut runtime = runtime_with_clock(true, true);
    runtime.native_surfaces_active = true;
    runtime.schedule_after_event(&event_loop);
    assert!(matches!(
        event_loop.control_flow(),
        ControlFlow::WaitUntil(_)
    ));

    runtime.close_surface(SurfaceId::new(5)).unwrap();
    runtime.schedule_after_event(&event_loop);
    assert_eq!(event_loop.control_flow(), ControlFlow::Wait);
}
