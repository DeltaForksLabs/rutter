use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, TryLockError},
};

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    ButtonVariant, DynamicIsland, HeadingLevel, MessageIngressError, MultiWindowAppLogic,
    MultiWindowMessageSender, MultiWindowRunner, SurfaceCommand, SurfaceId, Theme, Widget,
    activity::{
        ActivityAction, ActivityActionEvent, ActivityId, ActivityRegistry,
        ipc::{ActivityServer, DeliveryError, HostRequest},
        view,
    },
    spacing,
};
use taffy::prelude::*;

mod options;
mod overlay;

type Server = Arc<Mutex<ActivityServer>>;

#[derive(Clone)]
struct State {
    registry: ActivityRegistry,
    island: DynamicIsland,
    selected_summary: String,
    selector_labels: BTreeMap<ActivityId, String>,
    dark: bool,
    errors: BTreeMap<ActivityId, String>,
    ingress_error: Option<String>,
    // Shared references have no clone/drop lifecycle effects. The supervisor is
    // retained by run(), which always shuts it down, including failed startup.
    server: Option<Server>,
    last_registry: Arc<Mutex<ActivityRegistry>>,
    overlay: Option<overlay::Overlay>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            registry: ActivityRegistry::new(),
            island: DynamicIsland::interactive(),
            selected_summary: String::new(),
            selector_labels: BTreeMap::new(),
            dark: false,
            errors: BTreeMap::new(),
            ingress_error: None,
            server: None,
            last_registry: Arc::new(Mutex::new(ActivityRegistry::new())),
            overlay: None,
        }
    }
}

#[derive(Clone, Debug)]
enum Message {
    Incoming(HostRequest),
    Select(ActivityId),
    Action(ActivityId, ActivityAction),
    ToggleDetails,
    ToggleTheme,
    Collapse,
    Close,
}

impl State {
    fn apply_at(
        &mut self,
        surface: SurfaceId,
        message: Message,
        now: std::time::Instant,
        dispatch: impl FnOnce(&ActivityRegistry, ActivityActionEvent) -> std::io::Result<()>,
    ) -> Vec<SurfaceCommand> {
        if surface != SurfaceId::PRIMARY {
            return vec![];
        }
        if matches!(message, Message::Close) {
            return overlay::close(self);
        }
        let user_expansion = matches!(message, Message::ToggleDetails | Message::Select(_));
        let was_expanded = self.island.is_expanded();
        self.reduce(message, dispatch);
        let mut commands = overlay::focus::transition(self, was_expanded, user_expansion, now);
        commands.extend(overlay::publish(self));
        commands
    }

    fn reduce(
        &mut self,
        message: Message,
        dispatch: impl FnOnce(&ActivityRegistry, ActivityActionEvent) -> std::io::Result<()>,
    ) {
        match message {
            Message::Incoming(request) => {
                self.ingress_error = request
                    .apply(&mut self.registry)
                    .err()
                    .map(|error| format!("Activity update rejected ({:?}).", error.kind()));
                self.errors.retain(|id, _| self.registry.get(*id).is_some());
            }
            Message::Select(id) => {
                if self.registry.select(id).is_ok() {
                    self.island
                        .set_expanded(true)
                        .expect("host island is interactive");
                }
            }
            Message::Action(id, action) => {
                let result = self
                    .registry
                    .route_action(id, action)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
                    .and_then(|event| dispatch(&self.registry, event));
                match result {
                    Ok(()) => {
                        self.errors.remove(&id);
                    }
                    Err(error) => {
                        self.errors.insert(id, format!("{action:?} was not delivered ({:?}). Try again after the next producer update.", error.kind()));
                    }
                }
            }
            Message::ToggleDetails => self.island.toggle().expect("host island is interactive"),
            Message::ToggleTheme => self.dark = !self.dark,
            Message::Collapse => self.island.set_expanded(false).expect("interactive host"),
            Message::Close => {}
        }
        *self
            .last_registry
            .lock()
            .expect("registry snapshot lock poisoned") = self.registry.clone();
    }

    fn shutdown(&mut self) {
        if let Some(server) = &self.server {
            cleanup(
                &mut server.lock().expect("server lock poisoned"),
                &mut self.registry,
            );
            *self
                .last_registry
                .lock()
                .expect("registry snapshot lock poisoned") = self.registry.clone();
        }
    }

    fn refresh_labels(&mut self) {
        // Widget labels borrow these strings for the lifetime of the view.
        // Rebuild from live snapshots so replacement/removal cannot leave stale names.
        self.selector_labels = self
            .registry
            .iter()
            .map(|activity| {
                let prefix = if self.registry.selected() == Some(activity.id()) {
                    "Selected"
                } else {
                    "Select"
                };
                (
                    activity.id(),
                    format!("{prefix} · {}", view::summary(activity)),
                )
            })
            .collect();
        self.selected_summary = self
            .registry
            .selected()
            .and_then(|id| self.registry.get(id))
            .map(|activity| {
                format!(
                    "{} · {} details",
                    view::summary(activity),
                    if self.island.is_expanded() {
                        "Collapse"
                    } else {
                        "Expand"
                    }
                )
            })
            .unwrap_or_default();
    }
}

struct Host;

impl MultiWindowAppLogic for Host {
    type State = State;
    type Message = Message;

    fn new(_: &mut FontSystem) -> State {
        State::default()
    }

    fn theme_for(state: &State) -> Theme {
        if state.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }

    fn view<'a>(state: &'a mut State, _: SurfaceId) -> Widget<'a, Message> {
        state.refresh_labels();
        let theme = Self::theme_for(state);
        if state.overlay.is_some() {
            return overlay::view(state, &theme);
        }
        let header = vec![
            Widget::Heading {
                content: "Dynamic Island activities".into(),
                level: HeadingLevel::H1,
                style: fluid(),
                color: None,
            },
            text(
                "External producers publish plain snapshots. Controls return requests to the producer; these examples simulate transfers and media.",
                &theme,
            ),
        ];
        let mut children = vec![
            Widget::Column {
                children: header,
                style: Style {
                    gap: Size::length(spacing::SM),
                    ..fluid()
                },
            },
            button("Switch light / dark theme", Message::ToggleTheme),
        ];
        if let Some(error) = &state.ingress_error {
            children.push(text(error, &theme));
        }
        if state.registry.iter().len() == 0 {
            children.push(text("No activities. Start dynamic_island_client with this host's activity.sock path and download, upload, or media.", &theme));
        } else {
            let selectors = state
                .registry
                .iter()
                .map(|activity| {
                    let label = state
                        .selector_labels
                        .get(&activity.id())
                        .expect("live activity label");
                    Widget::button_content(
                        label,
                        text(label, &theme),
                        Message::Select(activity.id()),
                        Style {
                            min_size: Size::from_lengths(0.0, 44.0),
                            ..fluid()
                        },
                        None,
                        if state.registry.selected() == Some(activity.id()) {
                            ButtonVariant::Ghost
                        } else {
                            ButtonVariant::Text
                        },
                    )
                })
                .collect();
            children.push(Widget::Column {
                children: selectors,
                style: Style {
                    gap: Size::length(spacing::SM),
                    ..fluid()
                },
            });
            if let Some(activity) = state
                .registry
                .selected()
                .and_then(|id| state.registry.get(id))
            {
                let mut details = vec![view::details(activity, Message::Action, &theme)];
                if let Some(error) = state.errors.get(&activity.id()) {
                    details.push(text(error, &theme));
                }
                children.push(
                    state
                        .island
                        .interactive_view(
                            &state.selected_summary,
                            Message::ToggleDetails,
                            Widget::Column {
                                children: details,
                                style: Style {
                                    gap: Size::length(spacing::MD),
                                    ..fluid()
                                },
                            },
                            &theme,
                        )
                        .expect("host island is interactive"),
                );
            }
        }
        Widget::ScrollView {
            id: 9_000_000,
            style: Style {
                size: Size::percent(1.0_f32),
                min_size: Size::from_lengths(0.0, 0.0),
                ..Style::default()
            },
            child: Box::new(Widget::Column {
                children,
                style: Style {
                    padding: Rect::length(spacing::XL),
                    gap: Size::length(spacing::XL),
                    ..fluid()
                },
            }),
        }
    }

    fn update(
        state: &mut State,
        surface: SurfaceId,
        message: Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        let server = state.server.clone();
        state.apply_at(
            surface,
            message,
            std::time::Instant::now(),
            |registry, event| {
                let server = server
                    .as_ref()
                    .ok_or_else(|| std::io::Error::other("server unavailable"))?;
                server
                    .try_lock()
                    .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))?
                    .dispatch(registry, event)
            },
        )
    }

    fn surface_closed(state: &mut State, surface: SurfaceId) {
        if surface == SurfaceId::PRIMARY {
            if let Some(overlay) = &mut state.overlay {
                overlay.closed = true;
                overlay.pending = false;
                overlay.focus = Default::default();
            }
            state.shutdown();
        }
    }

    fn surface_metrics_changed(
        state: &mut State,
        surface: SurfaceId,
        metrics: rutter::SurfaceMetrics,
    ) {
        if surface == SurfaceId::PRIMARY
            && let Some(overlay) = &mut state.overlay
        {
            overlay.metrics = Some(metrics);
            overlay.invalidate();
        }
    }

    fn surface_created(state: &mut State, surface: SurfaceId) {
        if surface == SurfaceId::PRIMARY
            && let Some(overlay) = &mut state.overlay
        {
            overlay.invalidate();
        }
    }

    fn next_wakeup(state: &State, now: std::time::Instant) -> Option<std::time::Instant> {
        overlay::next_wakeup(state, now)
    }

    fn wakeup(state: &mut State, now: std::time::Instant) -> Vec<SurfaceCommand> {
        overlay::wakeup(state, now)
    }

    fn surface_event(
        state: &mut State,
        surface: SurfaceId,
        event: rutter::SurfaceEvent,
    ) -> Vec<SurfaceCommand> {
        overlay::focus::event(state, surface, event)
    }

    fn shortcut(
        state: &State,
        surface: SurfaceId,
        event: rutter::ShortcutEvent,
    ) -> rutter::ShortcutOutcome<Message> {
        overlay::shortcut(state, surface, event)
    }
}

fn fluid() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        min_size: Size::from_lengths(0.0, 0.0),
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn text<'a>(content: &str, theme: &Theme) -> Widget<'a, Message> {
    Widget::Text {
        content: content.into(),
        style: fluid(),
        size: theme.font_body,
        color: Some(theme.on_surface),
    }
}

fn button<'a>(text: &'static str, on_press: Message) -> Widget<'a, Message> {
    Widget::Button {
        text,
        on_press,
        style: Style {
            min_size: Size::from_lengths(0.0, 40.0),
            ..fluid()
        },
        color: None,
        variant: ButtonVariant::Ghost,
    }
}

fn cleanup(server: &mut ActivityServer, registry: &mut ActivityRegistry) {
    let report = server.shutdown();
    for request in report.cleanup {
        if let Err(error) = request.apply(registry) {
            eprintln!("Activity shutdown cleanup failed: {error}");
        }
    }
    if let Some(error) = report.endpoint_error {
        eprintln!("Activity endpoint cleanup failed: {error}");
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = options::parse(std::env::args_os().skip(1))?;
    let (request, ingress) = options::configuration(options.overlay)?;
    let slot = Arc::new(Mutex::new(None::<MultiWindowMessageSender<Message>>));
    let delivery_slot = slot.clone();
    let server = Arc::new(Mutex::new(ActivityServer::bind(
        options.directory,
        move |request| {
            let slot = match delivery_slot.try_lock() {
                Ok(slot) => slot,
                Err(TryLockError::WouldBlock) => return Err(DeliveryError::Full),
                Err(TryLockError::Poisoned(_)) => return Err(DeliveryError::Closed),
            };
            let sender = slot.as_ref().ok_or(DeliveryError::Full)?;
            sender
                .try_send(SurfaceId::PRIMARY, Message::Incoming(request))
                .map_err(|error| match error {
                    MessageIngressError::Full { .. } | MessageIngressError::Busy => {
                        DeliveryError::Full
                    }
                    MessageIngressError::Closed => DeliveryError::Closed,
                })
        },
    )?));
    println!(
        "Activity host socket: {}",
        server.lock().unwrap().socket_path().display()
    );
    let last_registry = Arc::new(Mutex::new(ActivityRegistry::new()));
    let mut state = State {
        server: Some(server.clone()),
        last_registry: last_registry.clone(),
        overlay: options.overlay.then(overlay::Overlay::default),
        ..State::default()
    };
    state
        .island
        .set_expanded(!options.overlay)
        .expect("host island is interactive");
    let result = MultiWindowRunner::<Host>::try_run_with_state_and_message_ingress(
        state,
        vec![request],
        ingress,
        |sender| {
            *slot.lock().unwrap() = Some(sender);
            Ok::<_, std::io::Error>(())
        },
    );
    // Runtime has disposed its registry. Retain the last authoritative snapshot
    // for mandatory cleanup even if startup/renderer failure skipped surface_closed.
    // Keep the closed sender installed until shutdown so late delivery sees Closed.
    cleanup(
        &mut server.lock().unwrap(),
        &mut last_registry.lock().unwrap(),
    );
    slot.lock().unwrap().take();
    result?;
    Ok(())
}

#[cfg(test)]
mod presentation_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use rutter::activity::{Transfer, TransferDirection, TransferState};

    #[test]
    fn host_document_keeps_natural_height_and_section_bounds_in_short_windows() {
        use std::{cell::RefCell, collections::HashMap, rc::Rc};
        for dark in [false, true] {
            for width in [320, 800] {
                for populated in [false, true] {
                    let mut state = State {
                        dark,
                        ..State::default()
                    };
                    state.island.set_expanded(true).unwrap();
                    if populated {
                        let owner = state.registry.register_owner().unwrap();
                        let id = state
                            .registry
                            .start(
                                owner,
                                Transfer::new(
                                    "A long simulated download title for localized narrow layouts",
                                    TransferDirection::Download,
                                    20,
                                    Some(100),
                                    TransferState::Running,
                                )
                                .unwrap(),
                            )
                            .unwrap();
                        state.errors.insert(
                            id,
                            "Pause was not delivered. Retry after the next producer update.".into(),
                        );
                    }
                    let widget = Host::view(&mut state, SurfaceId::PRIMARY);
                    rutter::WidgetIdSnapshot::capture(&widget).unwrap();
                    let mut tree = TaffyTree::new();
                    let fonts = Rc::new(RefCell::new(FontSystem::new()));
                    let root = rutter::layout::build_taffy_tree(
                        &mut tree,
                        &widget,
                        fonts.clone(),
                        &HashMap::new(),
                    );
                    rutter::layout::compute_layout(
                        &mut tree,
                        root,
                        winit::dpi::PhysicalSize::new(width, 120),
                        fonts,
                        &rutter::render::RichTextRenderer::default(),
                    );
                    assert_eq!(tree.layout(root).unwrap().size.height, 120.0);
                    let document = tree.children(root).unwrap()[0];
                    let bounds = tree.layout(document).unwrap();
                    assert!(bounds.size.height > 120.0);
                    let mut previous_bottom = spacing::XL;
                    for child in tree.children(document).unwrap() {
                        let child = tree.layout(child).unwrap();
                        assert!(child.location.y >= previous_bottom - 1.0);
                        assert!((child.location.x - spacing::XL).abs() <= 1.0);
                        assert!(
                            child.location.x + child.size.width
                                <= bounds.size.width - spacing::XL + 1.0
                        );
                        previous_bottom = child.location.y + child.size.height;
                    }
                    assert!(previous_bottom + spacing::XL <= bounds.size.height + 1.0);
                }
            }
        }
    }

    #[test]
    fn reducer_revalidates_latest_snapshot_and_routes_selected_activity() {
        for overlay in [false, true] {
            revalidate_actions(overlay);
        }
    }

    fn revalidate_actions(overlay: bool) {
        let mut state = State {
            overlay: overlay.then(overlay::Overlay::default),
            ..State::default()
        };
        let owner = state.registry.register_owner().unwrap();
        let snapshot = || {
            Transfer::new(
                "Simulation",
                TransferDirection::Download,
                0,
                Some(100),
                TransferState::Running,
            )
            .unwrap()
        };
        let first = state.registry.start(owner, snapshot()).unwrap();
        let second = state.registry.start(owner, snapshot()).unwrap();
        state.reduce(Message::Select(second), |_, _| unreachable!());
        assert_eq!(state.registry.selected(), Some(second));
        state.reduce(
            Message::Action(second, ActivityAction::Pause),
            |registry, event| {
                assert_eq!(event.id(), second);
                assert!(registry.get(first).is_some());
                Ok(())
            },
        );
        state
            .registry
            .replace(
                owner,
                second,
                Transfer::new(
                    "Simulation",
                    TransferDirection::Download,
                    100,
                    Some(100),
                    TransferState::Completed,
                )
                .unwrap(),
            )
            .unwrap();
        state.reduce(Message::Action(second, ActivityAction::Pause), |_, _| {
            panic!("stale action dispatched")
        });
        assert!(state.errors.contains_key(&second));
        state.registry.remove(owner, second).unwrap();
        state.reduce(Message::Action(second, ActivityAction::Cancel), |_, _| {
            panic!("removed activity dispatched")
        });
        assert_eq!(state.registry.selected(), Some(first));
    }

    #[test]
    fn external_producer_receives_controls_and_shutdown_cleans_retained_registry() {
        for overlay in [false, true] {
            producer_lifecycle(overlay);
        }
    }

    fn producer_lifecycle(overlay: bool) {
        use rutter::activity::{ActivityContent, Media, MediaState, ipc::ActivityClient};
        use std::{
            os::unix::fs::DirBuilderExt,
            sync::{
                atomic::{AtomicU64, Ordering},
                mpsc,
            },
            time::Duration,
        };

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rutter-host-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let (sender, receiver) = mpsc::sync_channel(32);
        let mut server = ActivityServer::bind(&directory, move |request| {
            sender.try_send(request).map_err(|error| match error {
                mpsc::TrySendError::Full(_) => DeliveryError::Full,
                mpsc::TrySendError::Disconnected(_) => DeliveryError::Closed,
            })
        })
        .unwrap();
        let socket = server.socket_path().to_owned();
        let producer = std::thread::spawn(move || {
            let mut client = ActivityClient::connect(socket).unwrap();
            let transfer = |state| {
                Transfer::new(
                    "Simulation",
                    TransferDirection::Download,
                    0,
                    Some(100),
                    state,
                )
                .unwrap()
            };
            let id = client.start(transfer(TransferState::Running)).unwrap();
            let expect = |client: &mut ActivityClient, expected| {
                assert_eq!(
                    client
                        .wait_action(Duration::from_secs(1))
                        .unwrap()
                        .unwrap()
                        .action,
                    expected
                );
            };
            expect(&mut client, ActivityAction::Pause);
            client.replace(id, transfer(TransferState::Paused)).unwrap();
            expect(&mut client, ActivityAction::Resume);
            client
                .replace(id, transfer(TransferState::Running))
                .unwrap();
            expect(&mut client, ActivityAction::Cancel);
            client.remove(id).unwrap();
            let media = |title| {
                Media::new(
                    title,
                    "Demo artist",
                    Duration::ZERO,
                    Some(Duration::from_secs(180)),
                    MediaState::Playing,
                )
                .unwrap()
            };
            let id = client.start(media("Initial")).unwrap();
            expect(&mut client, ActivityAction::Previous);
            client.replace(id, media("After previous")).unwrap();
            expect(&mut client, ActivityAction::Next);
            client.replace(id, media("After next")).unwrap();
            // Leave an activity live for explicit supervisor shutdown cleanup.
            client
        });
        let mut state = State {
            overlay: overlay.then(overlay::Overlay::default),
            ..State::default()
        };
        let now = std::time::Instant::now();
        if overlay {
            state.apply_at(
                SurfaceId::PRIMARY,
                Message::ToggleDetails,
                now,
                |_, _| unreachable!(),
            );
        }
        let mut focus_wakeup = Host::next_wakeup(&state, now);
        let mut stage = 0;
        while !producer.is_finished() {
            let request = receiver
                .recv_timeout(Duration::from_secs(3))
                .expect("producer request deadline");
            let commands = state.apply_at(
                SurfaceId::PRIMARY,
                Message::Incoming(request),
                now + Duration::from_millis(100),
                |_, _| unreachable!(),
            );
            assert!(
                commands.is_empty(),
                "IPC must not regrab focus or republish unchanged input"
            );
            assert_eq!(
                Host::next_wakeup(&state, now),
                focus_wakeup,
                "IPC must not restart a focus deadline"
            );
            assert_eq!(
                state.island.is_expanded(),
                overlay,
                "IPC must not change expansion"
            );
            let request = state.registry.iter().find_map(|activity| {
                let action = match (stage, activity.content()) {
                    (0, ActivityContent::Transfer(t)) if t.state() == TransferState::Running => {
                        ActivityAction::Pause
                    }
                    (1, ActivityContent::Transfer(t)) if t.state() == TransferState::Paused => {
                        ActivityAction::Resume
                    }
                    (2, ActivityContent::Transfer(t)) if t.state() == TransferState::Running => {
                        ActivityAction::Cancel
                    }
                    (3, ActivityContent::Media(_)) => ActivityAction::Previous,
                    (4, ActivityContent::Media(m)) if m.title() == "After previous" => {
                        ActivityAction::Next
                    }
                    _ => return None,
                };
                Some((activity.id(), action))
            });
            if let Some((id, action)) = request {
                let commands = state.apply_at(
                    SurfaceId::PRIMARY,
                    Message::Action(id, action),
                    now + Duration::from_millis(100),
                    |registry, event| server.dispatch(registry, event),
                );
                assert!(
                    commands.is_empty(),
                    "producer actions must not regrab focus"
                );
                assert_eq!(Host::next_wakeup(&state, now), focus_wakeup);
                assert!(!state.errors.contains_key(&id));
                stage += 1;
                if let Some(deadline) = focus_wakeup.take() {
                    assert!(matches!(
                        Host::wakeup(&mut state, deadline).as_slice(),
                        [SurfaceCommand::SetLayerKeyboardInteractivity {
                            interactivity: rutter::LayerKeyboardInteractivity::OnDemand,
                            ..
                        }]
                    ));
                    assert!(state.island.is_expanded());
                }
            }
            if stage == 5 && state.registry.iter().any(|activity| matches!(activity.content(), ActivityContent::Media(m) if m.title() == "After next")) { break; }
        }
        let client = producer.join().unwrap();
        assert_eq!(stage, 5);
        assert_eq!(state.registry.iter().len(), 1);
        let mut retained = state.last_registry.lock().unwrap();
        cleanup(&mut server, &mut retained);
        assert_eq!(retained.iter().len(), 0);
        assert!(!server.socket_path().exists());
        drop(client);
        drop(server);
        std::fs::remove_dir(directory).unwrap();
    }
}
