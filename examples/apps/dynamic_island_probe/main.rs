//! Native integration fixture for tests/native/dynamic_island_smoke.py.
//! Fixed geometry deliberately makes compositor input and pixel probes repeatable.

use std::time::Instant;

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    ButtonVariant, DynamicIsland, DynamicIslandMode, Layer, LayerAnchors, LayerExclusiveZone,
    LayerKeyboardInteractivity, LayerShellConfig, MultiWindowAppLogic, MultiWindowRunner,
    ShortcutEvent, ShortcutKey, ShortcutNamedKey, ShortcutOutcome, SurfaceCommand, SurfaceId,
    SurfaceInputRegion, SurfaceMetrics, SurfaceRequest, Theme, Widget, WindowConfig, spacing,
};
use skia_safe::{Color, Rect as PaintRect};
use taffy::prelude::*;

const OVERLAY: SurfaceId = SurfaceId::new(1);

#[derive(Clone)]
struct Model {
    island: DynamicIsland,
    pending_region: bool,
    clicks: u32,
}

impl Model {
    fn bounds(&self) -> PaintRect {
        PaintRect::from_xywh(
            8.0,
            8.0,
            404.0,
            if self.island.is_expanded() {
                160.0
            } else {
                76.0
            },
        )
    }

    fn publish(&mut self) -> Vec<SurfaceCommand> {
        self.pending_region = false;
        let region = self
            .island
            .input_region(self.bounds(), DynamicIsland::RADIUS)
            .unwrap();
        println!(
            "PROBE_REGION expanded={} mode={:?}",
            self.island.is_expanded(),
            self.island.mode()
        );
        vec![
            SurfaceCommand::SetInputRegion {
                surface: OVERLAY,
                region,
            },
            SurfaceCommand::RequestRedraw(OVERLAY),
        ]
    }
}

#[derive(Clone, Debug)]
enum Message {
    Underlying,
    Toggle,
    Action,
}

fn fill() -> Style {
    Style {
        size: Size::percent(1.0_f32),
        ..Style::default()
    }
}

struct Probe;

impl MultiWindowAppLogic for Probe {
    type State = Model;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Model {
        Model {
            island: DynamicIsland::interactive(),
            pending_region: false,
            clicks: 0,
        }
    }

    fn view<'a>(state: &'a mut Model, surface: SurfaceId) -> Widget<'a, Message> {
        if surface == SurfaceId::PRIMARY {
            return Widget::button_content(
                "Underlying click target",
                Widget::Container {
                    child: Box::new(Widget::Column {
                        children: vec![],
                        style: fill(),
                    }),
                    style: fill(),
                    color: Some(Color::from_rgb(32, 160, 96)),
                    radius: 0.0,
                },
                Message::Underlying,
                fill(),
                None,
                ButtonVariant::Text,
            );
        }
        let mut theme = Theme::dark();
        // Half-alpha blue over the green ordinary window proves real composition.
        theme.surface = Color::from_argb(128, 32, 64, 224);
        let mut capsule = if state.island.mode() == DynamicIslandMode::Passive {
            state
                .island
                .passive_view("Passive painted capsule", &theme)
                .unwrap()
        } else {
            state
                .island
                .interactive_view(
                    "Toggle details",
                    Message::Toggle,
                    Widget::Button {
                        text: "Capsule action",
                        on_press: Message::Action,
                        style: Style {
                            size: Size {
                                width: Dimension::percent(1.0),
                                height: Dimension::length(44.0),
                            },
                            ..Style::default()
                        },
                        color: None,
                        variant: ButtonVariant::Primary,
                    },
                    &theme,
                )
                .unwrap()
        };
        let bounds = state.bounds();
        let Widget::Container { style, .. } = &mut capsule else {
            unreachable!()
        };
        *style = Style {
            position: Position::Absolute,
            inset: Rect {
                left: LengthPercentageAuto::length(bounds.left),
                top: LengthPercentageAuto::length(bounds.top),
                ..Rect::auto()
            },
            size: Size::from_lengths(bounds.width(), bounds.height()),
            padding: Rect::length(spacing::LG),
            ..Style::default()
        };
        Widget::Container {
            child: Box::new(capsule),
            style: fill(),
            color: None,
            radius: 0.0,
        }
    }

    fn update(
        state: &mut Model,
        surface: SurfaceId,
        message: Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        match message {
            Message::Underlying => {
                assert_eq!(surface, SurfaceId::PRIMARY);
                state.clicks += 1;
                println!("PROBE_UNDERLYING {}", state.clicks);
                vec![]
            }
            Message::Toggle => {
                assert_eq!(surface, OVERLAY);
                state.island.toggle().unwrap();
                state.publish()
            }
            Message::Action => {
                assert_eq!(surface, OVERLAY);
                println!("PROBE_ACTION");
                vec![]
            }
        }
    }

    fn surface_metrics_changed(state: &mut Model, surface: SurfaceId, metrics: SurfaceMetrics) {
        println!("PROBE_METRICS {} {:?}", surface.get(), metrics);
        if surface == OVERLAY {
            state.pending_region = true;
        }
    }

    fn next_wakeup(state: &Model, now: Instant) -> Option<Instant> {
        state.pending_region.then_some(now)
    }

    fn wakeup(state: &mut Model, _: Instant) -> Vec<SurfaceCommand> {
        if state.pending_region {
            state.publish()
        } else {
            vec![]
        }
    }

    fn shortcut(_: &Model, surface: SurfaceId, event: ShortcutEvent) -> ShortcutOutcome<Message> {
        if event.key == ShortcutKey::Named(ShortcutNamedKey::Escape) {
            println!("PROBE_KEY {}", surface.get());
        }
        ShortcutOutcome::Ignored
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = match std::env::args().nth(1).as_deref() {
        Some("passive") => DynamicIslandMode::Passive,
        Some("interactive") => DynamicIslandMode::Interactive,
        _ => return Err("expected passive or interactive".into()),
    };
    let keyboard = if mode == DynamicIslandMode::Passive {
        LayerKeyboardInteractivity::None
    } else {
        LayerKeyboardInteractivity::OnDemand
    };
    let layer = LayerShellConfig::new("rutter-input-probe", LayerAnchors::TOP, 420, 300)?
        .with_layer(Layer::Overlay)
        .with_exclusive_zone(LayerExclusiveZone::None)?
        .with_keyboard_interactivity(keyboard);
    MultiWindowRunner::<Probe>::try_run_with_state(
        Model {
            island: if mode == DynamicIslandMode::Passive {
                DynamicIsland::passive()
            } else {
                DynamicIsland::interactive()
            },
            pending_region: false,
            clicks: 0,
        },
        vec![
            SurfaceRequest::new(
                SurfaceId::PRIMARY,
                WindowConfig::default()
                    .with_title("Rutter input probe underlying")
                    .with_decorations(false)
                    .with_inner_size(1100, 800)?,
            ),
            SurfaceRequest::new(
                OVERLAY,
                WindowConfig::default()
                    .with_title("Rutter input probe overlay")
                    .with_transparent(true)
                    .with_decorations(false)
                    .with_input_region(SurfaceInputRegion::Empty)
                    .with_layer_shell(layer),
            ),
        ],
    )?;
    Ok(())
}
