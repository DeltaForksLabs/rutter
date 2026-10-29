//! Exercise the actual launcher composition through normal runner routes. No
//! clock thread, native event loop or layer-shell policy participates.
use super::*;
use crate::MultiWindowAppLogic;
use crate::engine::test_surface::{MemoryEventLoop, MemoryWindow};
use crate::{SurfaceId, Widget};
use arboard::Clipboard;
use std::time::{Duration, Instant};
use winit::dpi::PhysicalSize;
use winit::keyboard::{KeyCode, KeyLocation, PhysicalKey};

#[path = "../../../examples/widgets/dynamic_island_demo.rs"]
#[expect(
    dead_code,
    reason = "Runner regressions embed the real view without starting its native event loop"
)]
mod dynamic_island_demo;
#[path = "../../../examples/widgets/layout.rs"]
#[expect(
    dead_code,
    reason = "Runner regressions reuse page composition, not the flow-row helper"
)]
mod layout;
#[path = "../../../examples/widgets/theme_selector.rs"]
#[expect(
    dead_code,
    reason = "Runner regressions embed only the multi-window demo, not the single-window theme test helper"
)]
mod theme_selector;
use dynamic_island_demo::{DynamicIslandDemo, IslandState, Message};

struct Showcase;
struct State {
    model: IslandState,
    now: Instant,
    messages: Vec<Message>,
}

impl AppLogic for Showcase {
    type State = State;
    type Message = Message;
    fn new(_: &mut FontSystem) -> State {
        let now = Instant::now();
        State {
            model: dynamic_island_demo::controlled_state(now),
            now,
            messages: vec![],
        }
    }
    fn view<'a>(state: &'a mut State) -> Widget<'a, Message> {
        DynamicIslandDemo::view(&mut state.model, SurfaceId::PRIMARY)
    }
    fn update(state: &mut State, message: Message, clipboard: &mut Clipboard) {
        state.messages.push(message.clone());
        DynamicIslandDemo::update(&mut state.model, SurfaceId::PRIMARY, message, clipboard);
        dynamic_island_demo::advance(&mut state.model, state.now);
    }
    fn shortcut(state: &State, event: crate::ShortcutEvent) -> crate::ShortcutOutcome<Message> {
        DynamicIslandDemo::shortcut(&state.model, SurfaceId::PRIMARY, event)
    }
}

fn runner(width: u32, height: u32, scale: f64) -> RutterRunner<Showcase> {
    let mut engine = RutterEngine::<Showcase>::new().unwrap();
    engine.window = Some(Rc::new(MemoryWindow {
        size: PhysicalSize::new(
            (f64::from(width) * scale) as u32,
            (f64::from(height) * scale) as u32,
        ),
        scale,
        popup: false,
        popup_state: None,
        input_state: None,
    }));
    engine.scale_factor = scale as f32;
    DynamicIslandDemo::surface_metrics_changed(
        &mut engine.app_state.model,
        SurfaceId::PRIMARY,
        crate::SurfaceMetrics {
            physical_size: (
                (f64::from(width) * scale) as u32,
                (f64::from(height) * scale) as u32,
            ),
            logical_size: (width.into(), height.into()),
            scale_factor: scale,
            output_name: None,
            layer_shell_version: None,
        },
    );
    let mut runner = RutterRunner::with_engine(engine);
    runner.active_window_id = Some(WindowId::from_raw(1));
    publish(&mut runner);
    runner
}

fn publish(runner: &mut RutterRunner<Showcase>) {
    runner.refresh_layout_before_keyboard_input().unwrap();
    assert!(runner.fatal_error.is_none());
}

fn key(runner: &mut RutterRunner<Showcase>, key: NamedKey) {
    runner.window_event(
        &MemoryEventLoop::default(),
        WindowId::from_raw(1),
        WindowEvent::KeyboardInput {
            device_id: None,
            is_synthetic: false,
            event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::Tab),
                logical_key: Key::Named(key),
                text: None,
                location: KeyLocation::Standard,
                state: ElementState::Pressed,
                repeat: false,
                text_with_all_modifiers: None,
                key_without_modifiers: Key::Named(key),
            },
        },
    );
    publish(runner);
}

fn button(runner: &RutterRunner<Showcase>, message: Message) -> u64 {
    *runner
        .engine
        .runtime_caches
        .buttons
        .iter()
        .find(|(_, value)| **value == message)
        .unwrap()
        .0
}

fn settle(runner: &mut RutterRunner<Showcase>) {
    runner.engine.app_state.now += Duration::from_millis(240);
    dynamic_island_demo::advance(
        &mut runner.engine.app_state.model,
        runner.engine.app_state.now,
    );
    runner.engine.layout_dirty = true;
    publish(runner);
}

#[test]
fn actual_showcase_keyboard_reveals_header_and_details_and_escape_collapses() {
    for (width, height) in [(320, 260), (800, 700)] {
        for scale in [1.0, 1.5, 2.0] {
            let mut runner = runner(width, height, scale);
            assert_eq!(
                runner.engine.runtime_caches.buttons.len(),
                2,
                "theme and header only"
            );
            let header = button(&runner, Message::Expand);
            key(&mut runner, NamedKey::Tab);
            key(&mut runner, NamedKey::Tab);
            assert_eq!(
                runner.engine.focused_widget_id,
                Some(header),
                "passive clock is not a Tab stop"
            );
            key(&mut runner, NamedKey::Enter);
            settle(&mut runner);
            let pause = button(&runner, Message::PauseResume);
            key(&mut runner, NamedKey::Tab);
            assert_eq!(runner.engine.focused_widget_id, Some(pause));
            key(&mut runner, NamedKey::Enter);
            assert!(
                runner
                    .engine
                    .app_state
                    .messages
                    .contains(&Message::PauseResume)
            );
            key(&mut runner, NamedKey::Tab);
            assert_eq!(
                runner.engine.focused_widget_id,
                Some(button(&runner, Message::Collapse))
            );
            key(&mut runner, NamedKey::Escape);
            assert!(
                !runner
                    .engine
                    .runtime_caches
                    .buttons
                    .values()
                    .any(|msg| *msg == Message::PauseResume)
            );
            assert!(
                runner
                    .engine
                    .app_state
                    .messages
                    .contains(&Message::Collapse)
            );
            assert_eq!(button(&runner, Message::Expand), header);
            use accesskit::ActionHandler;
            runner
                .engine
                .accessibility_actions
                .handler()
                .do_action(accesskit::ActionRequest {
                    action: accesskit::Action::Click,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: accesskit::NodeId(header),
                    data: None,
                });
            runner.process_accessibility_actions();
            publish(&mut runner);
            assert!(
                runner
                    .engine
                    .runtime_caches
                    .buttons
                    .values()
                    .any(|msg| *msg == Message::Collapse),
                "accessible header activation expands"
            );
            assert_eq!(button(&runner, Message::PauseResume), pause);
        }
    }
}

#[test]
fn hover_uses_live_scrolled_layout_and_does_not_restart_or_consume_detail_controls() {
    for scale in [1.0, 1.5, 2.0] {
        let mut runner = runner(320, 260, scale);
        let (&region_id, &before_scroll) = runner
            .engine
            .runtime_caches
            .viewport_geometry
            .pointer_regions
            .iter()
            .next()
            .unwrap();
        assert!(before_scroll.is_empty(), "clipped-out region cannot hover");
        runner
            .engine
            .widget_states
            .get_mut(&layout::PAGE_SCROLL_ID)
            .unwrap()
            .as_scroll_mut()
            .unwrap()
            .scroll_by(f32::MAX);
        publish(&mut runner);
        let frame = runner
            .engine
            .runtime_caches
            .viewport_geometry
            .pointer_regions[&region_id];
        assert!(!frame.is_empty());
        let point = skia_safe::Point::new(frame.center_x(), frame.center_y());
        runner.window_event(
            &MemoryEventLoop::default(),
            WindowId::from_raw(1),
            WindowEvent::PointerMoved {
                device_id: None,
                position: PhysicalPosition::new(
                    f64::from(point.x) * scale,
                    f64::from(point.y) * scale,
                ),
                primary: true,
                source: PointerSource::Mouse,
            },
        );
        assert_eq!(
            runner.engine.app_state.messages.last(),
            Some(&Message::Hover(true))
        );
        let count = runner.engine.app_state.messages.len();
        runner.refresh_pointer_region_hover(false).unwrap();
        assert_eq!(runner.engine.app_state.messages.len(), count);
        settle(&mut runner);
        assert!(
            runner
                .engine
                .runtime_caches
                .buttons
                .values()
                .any(|msg| *msg == Message::PauseResume)
        );
        runner.refresh_pointer_region_hover(true).unwrap();
        assert_eq!(
            runner.engine.app_state.messages.last(),
            Some(&Message::Hover(false))
        );
        let pause = button(&runner, Message::PauseResume);
        runner.engine.focused_widget_id = Some(pause);
        key(&mut runner, NamedKey::Enter);
        assert_eq!(
            runner.engine.app_state.messages.last(),
            Some(&Message::PauseResume)
        );
    }
}
