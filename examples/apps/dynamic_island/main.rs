// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Standalone native layer-shell demo:
//! `cargo run --example dynamic_island -- interactive|passive [output-name]`
//! with optional `--enter SlideInDown --exit SlideOutUp --no-animation` flags.
//! Requires a compositor supporting layer-shell (v4 for on-demand keyboard focus)
//! and an alpha-capable renderer. Native failures propagate; no window fallback.

use std::{
    ffi::OsString,
    io,
    time::{Duration, Instant},
};

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::{
    ButtonVariant, DynamicIsland, DynamicIslandMode, DynamicIslandMotion, DynamicIslandSlide,
    Layer, LayerAnchors, LayerExclusiveZone, LayerKeyboardInteractivity, LayerOutput,
    LayerShellConfig, LogicalPointerPosition, MultiWindowAppLogic, MultiWindowRunner,
    ShortcutEvent, ShortcutKey, ShortcutNamedKey, ShortcutOutcome, SurfaceCommand, SurfaceId,
    SurfaceInputRegion, SurfaceMetrics, SurfaceRequest, Theme, Widget, WindowConfig,
    animation::{Animation, Easing, Slide},
    spacing,
};
use skia_safe::Rect as PaintRect;
use taffy::prelude::*;

const HOST_WIDTH: u32 = 420;
const HOST_HEIGHT: u32 = 300;
const DOCUMENT: u64 = 6403;
const INTERVAL: Duration = Duration::from_secs(3);
const FRAME: Duration = Duration::from_millis(16);
const FOCUS_GRANT_TIMEOUT: Duration = Duration::from_millis(750);

mod focus;
use focus::FocusFallbackReason;
const USAGE: &str = "usage: dynamic_island interactive|passive [output-name] [--enter SlideInLeft|SlideInRight|SlideInUp|SlideInDown] [--exit SlideOutLeft|SlideOutRight|SlideOutUp|SlideOutDown] [--no-animation]";

#[derive(Debug, PartialEq)]
struct Options {
    mode: DynamicIslandMode,
    output: Option<String>,
    enter: DynamicIslandSlide,
    exit: DynamicIslandSlide,
    no_animation: bool,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> io::Result<Options> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, USAGE);
    let mut args = args.into_iter();
    let mode = match args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .as_deref()
    {
        Some("interactive") => DynamicIslandMode::Interactive,
        Some("passive") => DynamicIslandMode::Passive,
        _ => return Err(invalid()),
    };
    let mut options = Options {
        mode,
        output: None,
        enter: DynamicIslandSlide::SlideInDown,
        exit: DynamicIslandSlide::SlideOutUp,
        no_animation: false,
    };
    let mut flags_started = false;
    let mut seen = std::collections::HashSet::new();
    while let Some(arg) = args.next() {
        let arg = arg.into_string().map_err(|_| invalid())?;
        if arg.starts_with('-') {
            flags_started = true;
            if !seen.insert(arg.clone()) {
                return Err(invalid());
            }
            match arg.as_str() {
                "--no-animation" => options.no_animation = true,
                "--enter" | "--exit" => {
                    let value = args
                        .next()
                        .and_then(|value| value.into_string().ok())
                        .ok_or_else(invalid)?;
                    use DynamicIslandSlide::*;
                    match (arg.as_str(), value.as_str()) {
                        ("--enter", "SlideInLeft") => options.enter = SlideInLeft,
                        ("--enter", "SlideInRight") => options.enter = SlideInRight,
                        ("--enter", "SlideInUp") => options.enter = SlideInUp,
                        ("--enter", "SlideInDown") => options.enter = SlideInDown,
                        ("--exit", "SlideOutLeft") => options.exit = SlideOutLeft,
                        ("--exit", "SlideOutRight") => options.exit = SlideOutRight,
                        ("--exit", "SlideOutUp") => options.exit = SlideOutUp,
                        ("--exit", "SlideOutDown") => options.exit = SlideOutDown,
                        _ => return Err(invalid()),
                    }
                }
                _ => return Err(invalid()),
            }
        } else if flags_started
            || options.output.is_some()
            || arg.is_empty()
            || arg.len() > 1024
            || arg.contains('\0')
        {
            return Err(invalid());
        } else {
            options.output = Some(arg);
        }
    }
    Ok(options)
}

fn host_request(options: &Options) -> Result<SurfaceRequest, rutter::LayerShellConfigError> {
    let keyboard = match options.mode {
        DynamicIslandMode::Passive => LayerKeyboardInteractivity::None,
        DynamicIslandMode::Interactive => LayerKeyboardInteractivity::OnDemand,
    };
    let layer = LayerShellConfig::new(
        "rutter-dynamic-island",
        LayerAnchors::TOP,
        HOST_WIDTH,
        HOST_HEIGHT,
    )?
    .with_layer(Layer::Overlay)
    .with_exclusive_zone(LayerExclusiveZone::None)?
    .with_keyboard_interactivity(keyboard)
    .with_output(
        options
            .output
            .clone()
            .map_or(LayerOutput::Compositor, LayerOutput::Name),
    )?;
    Ok(SurfaceRequest::new(
        SurfaceId::PRIMARY,
        WindowConfig::default()
            .with_title("Dynamic Island")
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_input_region(SurfaceInputRegion::Empty)
            .with_layer_shell(layer),
    ))
}

#[derive(Clone)]
struct Model {
    island: DynamicIsland,
    metrics: Option<SurfaceMetrics>,
    pending_region: bool,
    region_bounds: Option<PaintRect>,
    current_region: SurfaceInputRegion,
    published_region: Option<SurfaceInputRegion>,
    #[cfg(test)]
    region_builds: usize,
    paused: bool,
    alternate: bool,
    deadline: Option<Instant>,
    motion: Option<DynamicIslandMotion>,
    enter: DynamicIslandSlide,
    exit: DynamicIslandSlide,
    no_animation: bool,
    closing: bool,
    frame: Option<Instant>,
    queued: Vec<Message>,
    text_x: Animation,
    text_y: Animation,
    text_exit_end: Option<Instant>,
    text_slide: DynamicIslandSlide,
    displayed_summary: Option<&'static str>,
    pointer_inside: bool,
    focused: bool,
    focus_deadline: Option<Instant>,
    keyboard_policy: LayerKeyboardInteractivity,
    focus_fallback: Option<FocusFallbackReason>,
}

#[derive(Clone, Debug, PartialEq)]
enum Message {
    Expand,
    PauseResume,
    Collapse,
    Close,
}

impl Model {
    fn new(mode: DynamicIslandMode, now: Instant) -> Self {
        Self {
            island: match mode {
                DynamicIslandMode::Passive => DynamicIsland::passive(),
                DynamicIslandMode::Interactive => DynamicIsland::interactive(),
            },
            metrics: None,
            pending_region: false,
            region_bounds: None,
            current_region: SurfaceInputRegion::Empty,
            published_region: None,
            #[cfg(test)]
            region_builds: 0,
            paused: false,
            alternate: false,
            deadline: (mode == DynamicIslandMode::Passive).then_some(now + INTERVAL),
            motion: None,
            enter: DynamicIslandSlide::SlideInDown,
            exit: DynamicIslandSlide::SlideOutUp,
            no_animation: false,
            closing: false,
            frame: None,
            queued: vec![],
            text_x: Animation::new(0.0).unwrap(),
            text_y: Animation::new(0.0).unwrap(),
            text_exit_end: None,
            text_slide: DynamicIslandSlide::SlideInDown,
            displayed_summary: None,
            pointer_inside: false,
            focused: false,
            focus_deadline: None,
            keyboard_policy: if mode == DynamicIslandMode::Passive {
                LayerKeyboardInteractivity::None
            } else {
                LayerKeyboardInteractivity::OnDemand
            },
            focus_fallback: None,
        }
    }

    fn summary(&self) -> &'static str {
        match (
            self.island.mode(),
            self.island.is_expanded(),
            self.paused,
            self.alternate,
        ) {
            (DynamicIslandMode::Passive, _, _, false) => "Passive · Syncing library",
            (DynamicIslandMode::Passive, _, _, true) => "Passive · Library up to date",
            (_, true, true, _) => "Paused · Details",
            (_, true, false, _) => "Playing · Details",
            (_, false, true, _) => "Paused · Show details",
            (_, false, false, _) => "Playing · Show details",
        }
    }

    fn input_region(&mut self) -> &SurfaceInputRegion {
        let bounds =
            capsule_bounds(self).filter(|_| self.island.mode() == DynamicIslandMode::Interactive);
        if bounds != self.region_bounds {
            #[cfg(test)]
            {
                self.region_builds += 1;
            }
            self.current_region = bounds.map_or(SurfaceInputRegion::Empty, |bounds| {
                self.island
                    .input_region(bounds, DynamicIsland::RADIUS)
                    .expect("bounded demo geometry fits the native rectangle budget")
            });
            self.region_bounds = bounds;
        }
        &self.current_region
    }

    fn hover(&mut self, position: Option<LogicalPointerPosition>) -> bool {
        if self.island.mode() == DynamicIslandMode::Passive || self.closing {
            return false;
        }
        // Use the very same bounded logical bands as native input, including the
        // details body. Transparent rounded corners are not capsule membership.
        let inside = position.is_some_and(|point| match self.input_region() {
            SurfaceInputRegion::Rectangles(rects) => rects.as_slice().iter().any(|rect| {
                let (x, y, width, height) = rect.logical_rect();
                point.x() >= x as f32
                    && point.x() < (x + width) as f32
                    && point.y() >= y as f32
                    && point.y() < (y + height) as f32
            }),
            _ => false,
        });
        if inside == self.pointer_inside {
            return false;
        }
        if inside {
            self.queued.push(Message::Expand);
        }
        self.pointer_inside = inside;
        true
    }

    fn publish_region(&mut self) -> Vec<SurfaceCommand> {
        self.pending_region = false;
        self.input_region();
        let mut commands = Vec::new();
        if self.published_region.as_ref() != Some(&self.current_region) {
            self.published_region = Some(self.current_region.clone());
            commands.push(SurfaceCommand::SetInputRegion {
                surface: SurfaceId::PRIMARY,
                region: self.current_region.clone(),
            });
        }
        commands.push(SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY));
        commands
    }

    fn apply_at(&mut self, message: Message, now: Instant) -> Vec<SurfaceCommand> {
        if self.island.mode() == DynamicIslandMode::Passive || self.closing {
            return Vec::new();
        }
        if matches!(message, Message::Expand) && self.island.is_expanded()
            || matches!(message, Message::Collapse) && !self.island.is_expanded()
        {
            return Vec::new();
        }
        if let Some(motion) = &mut self.motion {
            motion.sample_at(now);
        }
        let old_summary = self.summary();
        let geometry_changed = matches!(message, Message::Expand | Message::Collapse);
        match message {
            Message::Expand => self
                .island
                .set_expanded(true)
                .expect("interactive mode is fixed at startup"),
            Message::Collapse => self
                .island
                .set_expanded(false)
                .expect("interactive mode is fixed at startup"),
            Message::PauseResume => self.paused = !self.paused,
            Message::Close => {
                self.closing = true;
                self.focus_deadline = None;
                self.text_x = Animation::new(0.0).unwrap();
                self.text_y = Animation::new(0.0).unwrap();
                self.text_exit_end = None;
                self.displayed_summary = None;
                self.motion = None;
                self.frame = None;
                self.queued.clear();
                let mut commands = self.publish_region();
                commands.push(SurfaceCommand::Close(SurfaceId::PRIMARY));
                return commands;
            }
        }
        if old_summary != self.summary() {
            self.start_text_entry(now);
        }
        if geometry_changed {
            let duration = if self.no_animation {
                Duration::ZERO
            } else {
                Duration::from_millis(240)
            };
            if let (Some(bounds), Some(motion)) = (resting_bounds(self), &mut self.motion) {
                motion
                    .transition_to_at(bounds, duration, now)
                    .expect("validated demo bounds");
            }
        }
        self.schedule_frame(now);
        let mut commands = self.publish_region();
        if geometry_changed {
            commands.extend(self.expansion_focus_changed(now));
        }
        commands
    }

    fn slide_duration(&self) -> Duration {
        if self.no_animation {
            Duration::ZERO
        } else {
            Duration::from_millis(280)
        }
    }

    fn schedule_frame(&mut self, now: Instant) {
        if self.text_is_animating()
            || self
                .motion
                .as_ref()
                .is_some_and(|motion| motion.is_animating())
        {
            self.frame.get_or_insert(now + FRAME);
        } else {
            self.frame = None;
        }
    }

    fn start_text_exit(&mut self, old: &'static str, now: Instant) {
        self.sample_text(now);
        if self.no_animation {
            self.displayed_summary = None;
            self.text_exit_end = None;
            self.retarget_text((0.0, 0.0), now);
            return;
        }
        // Keep the outgoing payload and phase boundary while newer summaries
        // coalesce in application state. Accessibility always uses that state.
        if self.displayed_summary.is_some() {
            return;
        }
        self.displayed_summary = Some(old);
        self.text_slide = self.exit;
        self.text_exit_end = Some(now + self.slide_duration());
        self.retarget_text(slide_offset(self.exit, 1.0), now);
        self.schedule_frame(now);
    }

    fn start_text_entry(&mut self, now: Instant) {
        self.sample_text(now);
        if !self.text_is_animating() && !self.no_animation {
            self.seed_text_entry();
        }
        self.displayed_summary = None;
        self.text_exit_end = None;
        self.text_slide = self.enter;
        self.retarget_text((0.0, 0.0), now);
        self.schedule_frame(now);
    }

    fn sample_text(&mut self, now: Instant) {
        if let Some(boundary) = self.text_exit_end.filter(|end| now >= *end) {
            self.text_exit_end = None;
            self.displayed_summary = None;
            self.text_slide = self.enter;
            self.seed_text_entry();
            // A delayed frame advances both phases using the real exit boundary.
            self.retarget_text((0.0, 0.0), boundary);
        }
        self.text_x.sample_at(now);
        self.text_y.sample_at(now);
    }

    fn seed_text_entry(&mut self) {
        let (x, y) = slide_offset(self.enter, 0.0);
        self.text_x = Animation::new(x).unwrap();
        self.text_y = Animation::new(y).unwrap();
    }

    fn retarget_text(&mut self, target: (f32, f32), now: Instant) {
        let duration = self.slide_duration();
        self.text_x
            .retarget_at(target.0, duration, Easing::SmoothStep, now)
            .unwrap();
        self.text_y
            .retarget_at(target.1, duration, Easing::SmoothStep, now)
            .unwrap();
    }

    fn text_is_animating(&self) -> bool {
        self.text_exit_end.is_some() || self.text_x.is_animating() || self.text_y.is_animating()
    }

    fn text_offset(&self) -> (f32, f32) {
        (self.text_x.value(), self.text_y.value())
    }

    fn reconcile_at(&mut self, now: Instant) {
        let Some(bounds) = resting_bounds(self) else {
            self.motion = None;
            self.frame = None;
            return;
        };
        let (width, height) = self.metrics.as_ref().unwrap().logical_size;
        let host = PaintRect::from_xywh(0.0, 0.0, width as f32, height as f32);
        if let Some(motion) = &mut self.motion {
            motion
                .reconcile_at(host, bounds, now)
                .expect("validated demo bounds");
        } else {
            let motion = DynamicIslandMotion::new(host, bounds).expect("validated demo bounds");
            self.motion = Some(motion);
            if !self.no_animation {
                self.start_text_entry(now);
            }
        }
        self.schedule_frame(now);
    }
}

fn slide_offset(direction: DynamicIslandSlide, progress: f32) -> (f32, f32) {
    use DynamicIslandSlide::*;
    let slide = match direction {
        SlideInLeft => Slide::InLeft,
        SlideInRight => Slide::InRight,
        SlideInUp => Slide::InUp,
        SlideInDown => Slide::InDown,
        SlideOutLeft => Slide::OutLeft,
        SlideOutRight => Slide::OutRight,
        SlideOutUp => Slide::OutUp,
        SlideOutDown => Slide::OutDown,
    };
    slide.offset(progress, 1.0, 1.0).unwrap()
}

// DEMO ONLY: these explicit bounded dimensions are shared by painting and native
// input publication, never inferred from fonts. The library remains natural-height.
fn capsule_bounds(state: &Model) -> Option<PaintRect> {
    if state.closing {
        return None;
    }
    state.motion.as_ref()?.bounds()
}

fn resting_bounds(state: &Model) -> Option<PaintRect> {
    let (width, height) = state.metrics.as_ref()?.logical_size;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 2.0 * f64::from(spacing::SM + spacing::LG)
        || height <= 2.0 * f64::from(spacing::SM + spacing::LG)
        || width > 16384.0
        || height > 16384.0
    {
        return None;
    }
    // 240px leaves 208px for the wrapped summary and its 44px header body.
    // Expansion grows both axes; passive presentation retains its original width.
    let desired_width =
        if state.island.mode() == DynamicIslandMode::Interactive && !state.island.is_expanded() {
            240.0
        } else {
            404.0
        };
    let width = (width as f32 - 2.0 * spacing::SM).min(desired_width);
    let desired_height = match (state.island.mode(), state.island.is_expanded()) {
        (DynamicIslandMode::Passive, _) => 64.0,
        (_, false) => 76.0,
        (_, true) => 252.0,
    };
    let height = (height as f32 - 2.0 * spacing::SM).min(desired_height);
    Some(PaintRect::from_xywh(
        ((state.metrics.as_ref()?.logical_size.0 as f32 - width) / 2.0).floor(),
        spacing::SM,
        width,
        height,
    ))
}

fn fluid_document() -> Style {
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

fn details<'a>(paused: bool) -> Widget<'a, Message> {
    let action = |text, on_press| Widget::Button {
        text,
        on_press,
        style: Style {
            size: Size {
                width: Dimension::percent(1.0),
                height: Dimension::length(44.0),
            },
            ..fluid_document()
        },
        color: None,
        variant: ButtonVariant::Primary,
    };
    Widget::Column {
        children: vec![
            Widget::rich_text(
                rutter::RichText::plain(if paused {
                    "Playback paused. Resume when you are ready."
                } else {
                    "Now playing: Evening walk. Pause playback or close this island."
                }),
                fluid_document(),
            ),
            action(
                if paused { "Resume" } else { "Pause" },
                Message::PauseResume,
            ),
            action("Close island", Message::Close),
        ],
        style: Style {
            gap: Size::length(spacing::SM),
            ..fluid_document()
        },
    }
}

fn island_view<'a>(state: &'a Model, summary: &'a str, theme: &Theme) -> Widget<'a, Message> {
    let Some(bounds) = capsule_bounds(state) else {
        return Widget::Column {
            children: vec![],
            style: Style::default(),
        };
    };
    let mut capsule = match state.island.mode() {
        DynamicIslandMode::Passive => state.island.passive_view(summary, theme),
        DynamicIslandMode::Interactive => {
            state
                .island
                .interactive_view(summary, Message::Expand, details(state.paused), theme)
        }
    }
    .expect("view matches startup mode");
    let Widget::Container { style, child, .. } = &mut capsule else {
        unreachable!()
    };
    let Widget::Column { children, .. } = child.as_mut() else {
        unreachable!()
    };
    let summary_widget = if state.island.mode() == DynamicIslandMode::Interactive {
        let Widget::ButtonContent { label, child, .. } = &mut children[0] else {
            unreachable!()
        };
        *label = state.summary();
        child.as_mut()
    } else {
        &mut children[0]
    };
    let Widget::RichText { content, .. } = summary_widget else {
        unreachable!()
    };
    let (x, y) = state.text_offset();
    *content = content
        .clone()
        .with_bounds_relative_visual_offset(x, y)
        .with_accessibility_label(state.summary());
    // The entire document scrolls: even an unusually long localized header must
    // remain reachable, rather than consuming a fixed details allocation.
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
    let document = std::mem::replace(
        child,
        Box::new(Widget::Column {
            children: vec![],
            style: Style::default(),
        }),
    );
    **child = Widget::ScrollView {
        id: DOCUMENT,
        style: Style {
            size: Size::percent(1.0_f32),
            min_size: Size::from_lengths(0.0, 0.0),
            ..Style::default()
        },
        child: document,
    };
    let host = Widget::Container {
        child: Box::new(capsule),
        style: Style {
            size: Size::percent(1.0_f32),
            ..Style::default()
        },
        color: None,
        radius: 0.0,
    };
    if state.island.mode() == DynamicIslandMode::Passive || state.closing {
        host.enabled(false)
    } else {
        host
    }
}

struct IslandDemo;

impl MultiWindowAppLogic for IslandDemo {
    type State = Model;
    type Message = Message;

    fn new(_: &mut FontSystem) -> Model {
        Model::new(DynamicIslandMode::Interactive, Instant::now())
    }

    fn view<'a>(state: &'a mut Model, _: SurfaceId) -> Widget<'a, Message> {
        island_view(
            state,
            state.displayed_summary.unwrap_or_else(|| state.summary()),
            &Self::theme(),
        )
    }

    fn update(
        state: &mut Model,
        surface: SurfaceId,
        message: Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        if surface != SurfaceId::PRIMARY {
            return vec![];
        }
        state.queued.push(message);
        vec![]
    }

    fn surface_metrics_changed(state: &mut Model, surface: SurfaceId, metrics: SurfaceMetrics) {
        if surface == SurfaceId::PRIMARY {
            state.metrics = Some(metrics);
            state.pending_region = true;
            // Native scaling/configuration may change even at identical logical bounds.
            state.published_region = None;
        }
    }

    fn surface_created(state: &mut Model, surface: SurfaceId) {
        if surface == SurfaceId::PRIMARY {
            state.published_region = None;
            state.pending_region = true;
        }
    }

    fn surface_pointer_hovered(
        state: &mut Model,
        surface: SurfaceId,
        position: Option<LogicalPointerPosition>,
    ) -> Option<Vec<SurfaceCommand>> {
        (surface == SurfaceId::PRIMARY && state.hover(position)).then(Vec::new)
    }

    fn surface_event(
        state: &mut Model,
        surface: SurfaceId,
        event: rutter::SurfaceEvent,
    ) -> Vec<SurfaceCommand> {
        if surface != SurfaceId::PRIMARY
            || state.closing
            || state.island.mode() == DynamicIslandMode::Passive
        {
            return vec![];
        }
        let rutter::SurfaceEvent::FocusChanged(focused) = event;
        state.focused = focused;
        if focused {
            state.focus_deadline = None;
        } else if state.island.is_expanded() {
            return Self::focus_by_click_fallback(state, FocusFallbackReason::FocusLost);
        }
        vec![]
    }

    fn surface_closed(state: &mut Model, surface: SurfaceId) {
        if surface == SurfaceId::PRIMARY {
            state.closing = true;
            state.focus_deadline = None;
            state.deadline = None;
            state.frame = None;
            state.pending_region = false;
            state.region_bounds = None;
            state.current_region = SurfaceInputRegion::Empty;
            state.published_region = None;
            state.queued.clear();
        }
    }

    fn next_wakeup(state: &Model, now: Instant) -> Option<Instant> {
        if state.pending_region || !state.queued.is_empty() {
            Some(now)
        } else {
            state
                .deadline
                .into_iter()
                .chain(state.frame)
                .chain(state.focus_deadline)
                .min()
        }
    }

    fn wakeup(state: &mut Model, now: Instant) -> Vec<SurfaceCommand> {
        if state.pending_region {
            state.reconcile_at(now);
        }
        let mut commands = vec![];
        for message in std::mem::take(&mut state.queued) {
            commands.extend(state.apply_at(message, now));
        }
        if state.focus_deadline.is_some_and(|deadline| now >= deadline) {
            commands.extend(Self::focus_by_click_fallback(
                state,
                FocusFallbackReason::GrantDeadlineElapsed,
            ));
        }
        if state.frame.is_some_and(|deadline| now >= deadline) {
            let deadline = state.frame.unwrap();
            if let Some(motion) = &mut state.motion {
                motion.sample_at(now);
            }
            state.sample_text(now);
            // Keep the absolute 16ms cadence; skip missed frames, never time-step
            // geometry. Sampling uses actual elapsed time even after a late wake.
            state.frame = (state.text_is_animating()
                || state
                    .motion
                    .as_ref()
                    .is_some_and(|motion| motion.is_animating()))
            .then(|| {
                now + (FRAME
                    - Duration::from_nanos(
                        (now.duration_since(deadline).as_nanos() % FRAME.as_nanos()) as u64,
                    ))
            });
            state.pending_region = true;
        }
        if state.deadline.is_some_and(|deadline| now >= deadline) {
            let old = state.summary();
            state.alternate = !state.alternate;
            state.start_text_exit(old, now);
            state.deadline = Some(now + INTERVAL);
            state.pending_region = true;
        }
        if state.pending_region {
            commands.extend(state.publish_region());
        }
        commands
    }

    fn shortcut(
        state: &Model,
        surface: SurfaceId,
        event: ShortcutEvent,
    ) -> ShortcutOutcome<Message> {
        if surface == SurfaceId::PRIMARY
            && state.island.is_expanded()
            && !state.closing
            && event.key == ShortcutKey::Named(ShortcutNamedKey::Escape)
            && !event.control
            && !event.alt
            && !event.super_key
        {
            ShortcutOutcome::Message(Message::Collapse)
        } else {
            ShortcutOutcome::Ignored
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_args(std::env::args_os().skip(1))?;
    let request = host_request(&options)?;
    let mut state = Model::new(options.mode, Instant::now());
    state.enter = options.enter;
    state.exit = options.exit;
    state.no_animation = options.no_animation;
    MultiWindowRunner::<IslandDemo>::try_run_with_state(state, vec![request])?;
    Ok(())
}

#[cfg(test)]
mod tests;
