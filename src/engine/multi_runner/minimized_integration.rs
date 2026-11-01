//! Owned-compositor regression for minimized GL swaps blocking sibling surfaces.
//! Run only through tests/native/minimized_surface_smoke.py (external watchdog).
use super::*;
use crate::{InputState, LayerAnchors, LayerExclusiveZone, LayerShellConfig, Widget, spacing};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use taffy::prelude::{Dimension, Rect, Size, Style};
use winit::event::ElementState;
use winit::event_loop::EventLoop;
use winit::platform::wayland::EventLoopBuilderExtWayland;

const SETTINGS: SurfaceId = SurfaceId::new(1);
const PANEL: SurfaceId = SurfaceId::new(2);

#[derive(Clone, Default)]
struct Model {
    publication: u64,
    clicks: u64,
    draft: String,
}

#[derive(Clone, Debug)]
enum Message {
    Publish(u64),
    Click,
    Draft(String),
}

struct App;
impl MultiWindowAppLogic for App {
    type State = Model;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Model {
        Model::default()
    }

    fn view<'a>(state: &'a mut Model, surface: SurfaceId) -> Widget<'a, Message> {
        let full = Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::percent(1.0),
            },
            ..Default::default()
        };
        if surface == PANEL {
            // The whole 64px strip is a native pointer target. Its background
            // changes only on activation, independent of publication text.
            return Widget::Button {
                text: "Activate independent panel",
                on_press: Message::Click,
                style: full,
                color: Some(if state.clicks.is_multiple_of(2) {
                    skia_safe::Color::BLUE
                } else {
                    skia_safe::Color::GREEN
                }),
                variant: crate::ButtonVariant::Primary,
            }
            .with_id(902);
        }
        Widget::Column {
            style: Style {
                padding: Rect::length(spacing::XL),
                gap: Size::length(spacing::LG),
                ..full
            },
            children: vec![
                Widget::Text {
                    content: format!("Settings — latest model {}", state.publication),
                    color: None,
                    size: 20.0,
                    style: Style::default(),
                },
                Widget::TextInput {
                    id: 901,
                    model_value: Some(&state.draft),
                    on_change: Message::Draft,
                    on_submit: None,
                    style: Style {
                        size: Size {
                            width: Dimension::percent(1.0),
                            height: Dimension::length(72.0),
                        },
                        flex_shrink: 0.0,
                        ..Default::default()
                    },
                    label: "Persistent draft",
                    placeholder: "Type a draft before minimizing",
                    state: InputState::Idle,
                    error_msg: None,
                    is_password: false,
                },
            ],
        }
    }

    fn update(
        state: &mut Model,
        surface: SurfaceId,
        message: Message,
        _: &mut arboard::Clipboard,
    ) -> Vec<SurfaceCommand> {
        match message {
            Message::Publish(value) => state.publication = value,
            Message::Click => {
                assert_eq!(surface, PANEL);
                state.clicks += 1;
            }
            Message::Draft(value) => {
                assert_eq!(surface, SETTINGS);
                state.draft = value;
            }
        }
        Vec::new()
    }
}

struct Probe {
    runtime: MultiWindowRunner<App>,
    command_path: PathBuf,
    last_command: u64,
    deadline: Instant,
    frames: HashSet<WindowId>,
    ready: bool,
    native_panel_presses: u64,
}

impl Probe {
    fn healthy(&self) {
        assert!(
            self.runtime.fatal_error.is_none(),
            "{:?}",
            self.runtime.fatal_error
        );
        assert!(
            Instant::now() < self.deadline,
            "minimized fixture deadline exceeded"
        );
    }

    fn command(&mut self, event_loop: &dyn ActiveEventLoop) {
        let command = match std::fs::read_to_string(&self.command_path) {
            Ok(command) => command,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("read owned minimized fixture command: {error}"),
        };
        let fields: Vec<_> = command.split_whitespace().collect();
        assert!(fields.len() >= 2, "expected sequence and command");
        let sequence: u64 = fields[0].parse().unwrap();
        if sequence <= self.last_command {
            return;
        }
        assert_eq!(sequence, self.last_command + 1);
        match fields[1] {
            "publish" => {
                assert_eq!(fields.len(), 3);
                let commands = self
                    .runtime
                    .deliver_external_message(
                        SETTINGS,
                        Message::Publish(fields[2].parse().unwrap()),
                    )
                    .unwrap()
                    .unwrap();
                assert!(commands.is_empty());
            }
            "check" => {
                assert_eq!(fields.len(), 5);
                assert_eq!(
                    self.runtime.canonical_state.publication,
                    fields[2].parse::<u64>().unwrap()
                );
                assert_eq!(
                    self.runtime.canonical_state.clicks,
                    fields[3].parse::<u64>().unwrap()
                );
                assert_eq!(self.runtime.canonical_state.draft, fields[4]);
                assert!(self.native_panel_presses >= self.runtime.canonical_state.clicks);
            }
            "quit" => {
                println!("MINIMIZED_EXIT");
                event_loop.exit();
                return;
            }
            unknown => panic!("unknown minimized fixture command: {unknown}"),
        }
        // Force the real runner's redraw path even when the compositor withholds
        // frame callbacks. Settings first: a blocked EGL swap must prevent ACK.
        // Only redraws are injected here; pointer/keyboard input is always native.
        for surface in [SETTINGS, PANEL] {
            let native = self.runtime.routes.native_for(surface).unwrap();
            let size = self.runtime.surface_runners[&surface]
                .native_window()
                .unwrap()
                .surface_size();
            assert!(
                size.width > 0 && size.height > 0,
                "redraw must reach end_frame, not zero-size early return"
            );
            self.runtime
                .window_event(event_loop, native, WindowEvent::RedrawRequested);
            self.healthy();
        }
        self.last_command = sequence;
        let model = &self.runtime.canonical_state;
        println!(
            "MINIMIZED_DONE {sequence} {} {} {}",
            model.publication,
            model.clicks,
            if model.draft.is_empty() {
                "-"
            } else {
                &model.draft
            }
        );
        std::io::stdout().flush().unwrap();
    }
}

impl ApplicationHandler for Probe {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(event_loop.layer_shell_version().is_some());
        self.runtime.can_create_surfaces(event_loop);
        self.healthy();
        assert_eq!(
            self.runtime.backend_preference.retained,
            Some(BackendType::OpenGl)
        );
        for (surface, runner) in &self.runtime.surface_runners {
            assert_eq!(
                runner
                    .native_window()
                    .unwrap()
                    .layer_shell_version()
                    .is_some(),
                *surface == PANEL
            );
        }
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.destroy_surfaces(event_loop);
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        self.healthy();
        self.runtime.new_events(event_loop, cause);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        native: WindowId,
        event: WindowEvent,
    ) {
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        let before = (
            self.runtime.canonical_state.clicks,
            self.runtime.canonical_state.draft.clone(),
        );
        if self.runtime.routes.native_for(PANEL) == Some(native)
            && matches!(
                event,
                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    ..
                }
            )
        {
            self.native_panel_presses += 1;
        }
        self.runtime.window_event(event_loop, native, event);
        self.healthy();
        let model = &self.runtime.canonical_state;
        if before != (model.clicks, model.draft.clone()) {
            println!(
                "MINIMIZED_INPUT {} {}",
                model.clicks,
                if model.draft.is_empty() {
                    "-"
                } else {
                    &model.draft
                }
            );
            std::io::stdout().flush().unwrap();
        }
        if redraw {
            self.frames.insert(native);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.runtime.about_to_wait(event_loop);
        self.healthy();
        if !self.ready && self.frames.len() == 2 {
            self.ready = true;
            println!("MINIMIZED_READY OpenGl");
            std::io::stdout().flush().unwrap();
        }
        if self.ready {
            self.command(event_loop);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(20),
        ));
    }
}

#[test]
#[ignore = "requires tests/native/minimized_surface_smoke.py and its owned compositor/watchdog"]
fn native_minimized_settings_does_not_stall_shared_gl_runtime() {
    let command_path = PathBuf::from(
        std::env::var_os("RUTTER_MINIMIZED_COMMAND").expect("owned driver command path"),
    );
    let layer = LayerShellConfig::new(
        "rutter-minimized-panel",
        LayerAnchors::TOP | LayerAnchors::LEFT | LayerAnchors::RIGHT,
        0,
        64,
    )
    .unwrap()
    .with_exclusive_zone(LayerExclusiveZone::Reserve(64))
    .unwrap();
    let requests = vec![
        SurfaceRequest::new(
            SETTINGS,
            WindowConfig::default()
                .with_title("Rutter minimized Settings")
                .with_decorations(false)
                .with_inner_size(640, 360)
                .unwrap(),
        ),
        SurfaceRequest::new(PANEL, WindowConfig::default().with_layer_shell(layer)),
    ];
    let mut runtime = MultiWindowRunner::<App>::initialize_with(
        |fonts| Ok::<_, std::convert::Infallible>(App::new(fonts)),
        requests,
    )
    .unwrap();
    runtime.backend_preference.retained = Some(BackendType::OpenGl);
    let mut builder = EventLoop::builder();
    builder.with_wayland().with_any_thread(true);
    builder
        .build()
        .unwrap()
        .run_app(Probe {
            runtime,
            command_path,
            last_command: 0,
            deadline: Instant::now() + Duration::from_secs(90),
            frames: HashSet::new(),
            ready: false,
            native_panel_presses: 0,
        })
        .unwrap();
}
