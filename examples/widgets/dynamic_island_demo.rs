// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Ordinary-window composition showcase; the independent native example owns
//! layer-shell policy. Only active transitions request animation frames here.
use std::time::{Duration, Instant};

use arboard::Clipboard;
use cosmic_text::FontSystem;
use rutter::spacing;
use rutter::{
    ButtonVariant, DynamicIsland, HeadingLevel, MultiWindowAppLogic, MultiWindowRunner,
    PointerEvent, PointerRegionConfig, RichText, ShortcutEvent, ShortcutKey, ShortcutNamedKey,
    ShortcutOutcome, SurfaceCommand, SurfaceId, SurfaceMetrics, SurfaceRequest, Theme, Widget,
    WidgetId, WindowConfig,
    animation::{Animation, Easing},
};
use taffy::prelude::*;

use super::{
    layout,
    theme_selector::{ExampleTheme, example_theme_selector},
};
mod clock {
    use std::time::{Duration, Instant};

    use chrono::{DateTime, Local, TimeZone, Timelike};
    use rutter::animation::{Animation, Easing};

    pub(super) const CYCLE: Duration = Duration::from_secs(3);
    pub(super) const SLIDE: Duration = Duration::from_millis(240);

    /// All presentation strings come from one local wall-clock sample. Tests inject
    /// a fixed-offset timestamp; animation itself only uses the supplied Instant.
    #[derive(Clone)]
    pub(super) struct Snapshot {
        pub parts: [String; 3],
        pub label: String,
        until_minute: Duration,
    }

    impl Snapshot {
        pub fn local() -> Self {
            Self::at(Local::now())
        }

        pub fn at<Tz: TimeZone>(timestamp: DateTime<Tz>) -> Self
        where
            Tz::Offset: std::fmt::Display,
        {
            let parts = [
                timestamp.format("%H:%M").to_string(),
                timestamp.format("%d %B %Y").to_string(),
                timestamp.format("%A").to_string(),
            ];
            let label = parts.join(" · ");
            let until_minute = Duration::from_secs(60 - u64::from(timestamp.second()))
                - Duration::from_nanos(u64::from(timestamp.nanosecond()));
            Self {
                parts,
                label,
                until_minute,
            }
        }
    }

    #[derive(Clone)]
    pub(super) struct Clock {
        pub snapshot: Snapshot,
        pub part: usize,
        pub cycle: Instant,
        minute: Instant,
        outgoing: Option<String>,
        exit_end: Option<Instant>,
        offset: Animation,
    }

    impl Clock {
        pub fn new(now: Instant, snapshot: Snapshot) -> Self {
            Self {
                minute: now + snapshot.until_minute,
                snapshot,
                part: 0,
                cycle: now + CYCLE,
                outgoing: None,
                exit_end: None,
                offset: Animation::new(0.0).unwrap(),
            }
        }

        pub fn deadline(&self) -> Instant {
            self.cycle.min(self.minute)
        }

        pub fn visual(&self) -> &str {
            self.outgoing
                .as_deref()
                .unwrap_or(&self.snapshot.parts[self.part])
        }

        pub fn offset(&self) -> f32 {
            self.offset.value()
        }

        pub fn active(&self) -> bool {
            self.exit_end.is_some() || self.offset.is_animating()
        }

        pub fn refresh(&mut self, now: Instant, snapshot: Snapshot) {
            let old = self.visual().to_owned();
            if now >= self.cycle {
                // Coalesce missed cycles in constant work, never replay old frames.
                let skipped = now.duration_since(self.cycle).as_nanos() / CYCLE.as_nanos() + 1;
                self.part = (self.part + (skipped % 3) as usize) % 3;
                let remainder = now.duration_since(self.cycle).as_nanos() % CYCLE.as_nanos();
                self.cycle = now + CYCLE - Duration::from_nanos(remainder as u64);
            }
            self.minute = now + snapshot.until_minute;
            self.snapshot = snapshot;
            if old != self.snapshot.parts[self.part] && self.outgoing.is_none() {
                self.outgoing = Some(old);
                self.exit_end = Some(now + SLIDE);
                self.offset
                    .retarget_at(-1.0, SLIDE, Easing::SmoothStep, now)
                    .unwrap();
            }
        }

        pub fn sample(&mut self, now: Instant) {
            if let Some(boundary) = self.exit_end.filter(|end| now >= *end) {
                self.exit_end = None;
                self.outgoing = None;
                self.offset = Animation::new(1.0).unwrap();
                self.offset
                    .retarget_at(0.0, SLIDE, Easing::SmoothStep, boundary)
                    .unwrap();
            }
            self.offset.sample_at(now);
        }
    }
}
use clock::{Clock, Snapshot};

const FRAME: Duration = Duration::from_millis(16);
const MOTION: Duration = Duration::from_millis(240);
const HOVER: u64 = 0x64_0901;
const DETAILS_SCROLL: u64 = 0x64_0905;

#[derive(Clone)]
pub(super) struct IslandState {
    theme: ExampleTheme,
    clock: Clock,
    island: DynamicIsland,
    expansion: Animation,
    available: f32,
    paused: bool,
    queued: Vec<Message>,
    frame: Option<Instant>,
    closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Message {
    ThemeChanged(ExampleTheme),
    Hover(bool),
    Pointer(PointerEvent),
    Expand,
    PauseResume,
    Collapse,
}

impl IslandState {
    fn at(now: Instant, snapshot: Snapshot) -> Self {
        Self {
            theme: ExampleTheme::Dark,
            clock: Clock::new(now, snapshot),
            island: DynamicIsland::interactive(),
            expansion: Animation::new(0.0).unwrap(),
            available: 404.0,
            paused: false,
            queued: vec![],
            frame: None,
            closed: false,
        }
    }

    fn apply(&mut self, message: Message, now: Instant) {
        let expanded = match message {
            Message::ThemeChanged(theme) => {
                self.theme = theme;
                return;
            }
            Message::Hover(true) | Message::Expand => true,
            Message::Collapse => false,
            Message::PauseResume => {
                self.paused = !self.paused;
                return;
            }
            Message::Hover(false) | Message::Pointer(_) => return,
        };
        if expanded == self.island.is_expanded() {
            return;
        }
        self.island
            .set_expanded(expanded)
            .expect("interactive showcase mode");
        self.expansion
            .retarget_at(f32::from(expanded), MOTION, Easing::SmoothStep, now)
            .unwrap();
    }

    fn wake_at(&mut self, now: Instant, snapshot: Option<Snapshot>) -> Vec<SurfaceCommand> {
        if self.closed {
            return vec![];
        }
        let mut changed = false;
        for message in std::mem::take(&mut self.queued) {
            self.apply(message, now);
            changed = true;
        }
        if let Some(snapshot) = snapshot {
            self.clock.refresh(now, snapshot);
            changed = true;
        }
        if self.frame.is_some_and(|deadline| now >= deadline) {
            self.expansion.sample_at(now);
            self.clock.sample(now);
            self.frame = None;
            changed = true;
        }
        if self.expansion.is_animating() || self.clock.active() {
            self.frame.get_or_insert(now + FRAME);
        }
        if changed {
            vec![SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY)]
        } else {
            vec![]
        }
    }

    fn dimensions(&self) -> (f32, f32) {
        let compact = self.available.min(240.0);
        let expanded = self.available.min(404.0);
        let progress = self.expansion.value();
        (
            compact + (expanded - compact) * progress,
            76.0 + 176.0 * progress,
        )
    }
}

pub(super) struct DynamicIslandDemo;

impl MultiWindowAppLogic for DynamicIslandDemo {
    type State = IslandState;
    type Message = Message;

    fn new(_: &mut FontSystem) -> IslandState {
        IslandState::at(Instant::now(), Snapshot::local())
    }

    fn initial_surfaces() -> Vec<SurfaceRequest> {
        vec![SurfaceRequest::new(
            SurfaceId::PRIMARY,
            WindowConfig::default()
                .with_title("Dynamic Island — widget showcase")
                .with_inner_size(800, 700)
                .expect("positive showcase dimensions"),
        )]
    }

    fn view<'a>(state: &'a mut IslandState, _: SurfaceId) -> Widget<'a, Message> {
        showcase(state)
    }

    fn theme_for(state: &IslandState) -> Theme {
        state.theme.resolve()
    }

    fn update(
        state: &mut IslandState,
        _: SurfaceId,
        message: Message,
        _: &mut Clipboard,
    ) -> Vec<SurfaceCommand> {
        // Hover leave and uncaptured presses are observations, not animations.
        if !matches!(message, Message::Hover(false) | Message::Pointer(_)) {
            state.queued.push(message);
        }
        vec![]
    }

    fn surface_metrics_changed(state: &mut IslandState, _: SurfaceId, metrics: SurfaceMetrics) {
        let width = metrics.logical_size.0 as f32;
        if width.is_finite() && width > 2.0 * spacing::XL {
            // page() owns the only horizontal inset; section() caps at 760px.
            state.available = (width - 2.0 * spacing::XL).min(760.0);
        }
    }

    fn surface_closed(state: &mut IslandState, _: SurfaceId) {
        state.closed = true;
    }

    fn next_wakeup(state: &IslandState, now: Instant) -> Option<Instant> {
        if state.closed {
            None
        } else if !state.queued.is_empty() {
            Some(now)
        } else {
            Some(state.frame.map_or(state.clock.deadline(), |frame| {
                frame.min(state.clock.deadline())
            }))
        }
    }

    fn wakeup(state: &mut IslandState, now: Instant) -> Vec<SurfaceCommand> {
        let snapshot = (!state.closed && now >= state.clock.deadline()).then(Snapshot::local);
        state.wake_at(now, snapshot)
    }

    fn shortcut(
        state: &IslandState,
        surface: SurfaceId,
        event: ShortcutEvent,
    ) -> ShortcutOutcome<Message> {
        if surface == SurfaceId::PRIMARY
            && state.island.is_expanded()
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

fn text<'a>(content: impl Into<String>) -> Widget<'a, Message> {
    Widget::rich_text(
        RichText::plain(content.into()),
        layout::responsive_width(760.0, Dimension::auto()),
    )
}

fn action<'a>(label: &'static str, message: Message) -> Widget<'a, Message> {
    Widget::Button {
        text: label,
        on_press: message,
        style: Style {
            min_size: Size::from_lengths(0.0, 44.0),
            ..layout::responsive_width(404.0, Dimension::auto())
        },
        color: None,
        variant: ButtonVariant::Primary,
    }
}

fn interactive<'a>(state: &IslandState, theme: &Theme) -> Widget<'a, Message> {
    let details = layout::section(vec![
        text(if state.paused {
            "Playback paused. Resume when you are ready."
        } else {
            "Now playing: Evening walk. Playback is running."
        }),
        action(
            if state.paused { "Resume" } else { "Pause" },
            Message::PauseResume,
        ),
        action("Collapse details", Message::Collapse),
    ]);
    let summary = if state.island.is_expanded() {
        "Playback · Details"
    } else {
        "Playback · Show details"
    };
    // Buttons use stable structural IDs: keep this composition identical across
    // theme, playback and motion samples (details are removed only on collapse).
    let capsule = state
        .island
        .interactive_view(summary, Message::Expand, details, theme)
        .expect("interactive view");
    let (width, height) = state.dimensions();
    let capsule = bounded(capsule, width, height, DETAILS_SCROLL);
    Widget::pointer_region(
        WidgetId::manual(HOVER).expect("manual showcase region ID"),
        capsule,
        PointerRegionConfig::new(Message::Pointer).with_hover(Message::Hover),
        Style {
            align_self: Some(AlignSelf::Center),
            ..layout::responsive_width(width, Dimension::auto())
        },
    )
}

/// Only the demo constrains capsule height. The document retains natural height
/// inside its viewport, so wrapped headers/details stay scrollable during motion.
fn bounded<'a>(
    mut capsule: Widget<'a, Message>,
    width: f32,
    height: f32,
    id: u64,
) -> Widget<'a, Message> {
    let Widget::Container { style, child, .. } = &mut capsule else {
        unreachable!()
    };
    style.size.height = Dimension::length(height);
    style.max_size.width = Dimension::length(width);
    style.align_self = Some(AlignSelf::Center);
    let document = std::mem::replace(child, Box::new(text("")));
    **child = Widget::ScrollView {
        id,
        style: Style {
            min_size: Size::from_lengths(0.0, 0.0),
            size: Size::percent(1.0_f32),
            ..Style::default()
        },
        child: document,
    };
    capsule
}

fn showcase<'a>(state: &'a IslandState) -> Widget<'a, Message> {
    let theme = state.theme.resolve();
    let mut passive = DynamicIsland::passive()
        .passive_view(state.clock.visual(), &theme)
        .expect("passive view");
    if let Widget::Container { style, child, .. } = &mut passive {
        style.max_size.width = Dimension::length(240.0);
        style.align_self = Some(AlignSelf::Center);
        if let Widget::Column { children, .. } = child.as_mut()
            && let Widget::RichText { content, .. } = &mut children[0]
        {
            *content = content
                .clone()
                .with_bounds_relative_visual_offset(0.0, state.clock.offset())
                .with_accessibility_label(state.clock.snapshot.label.clone());
        }
    }
    layout::page(
        "Dynamic Island",
        Widget::Column {
            children: vec![
                example_theme_selector(state.theme, Message::ThemeChanged),
                layout::section(vec![
                    Widget::heading(HeadingLevel::H2, "Passive · Local clock", Style::default()),
                    text(
                        "Hours, date and weekday appear one at a time. This display never takes keyboard focus.",
                    ),
                    passive,
                ]),
                layout::section(vec![
                    Widget::heading(HeadingLevel::H2, "Interactive · Playback", Style::default()),
                    text(
                        "Hover or activate the header to expand. Use the detail controls, Collapse details, or Escape.",
                    ),
                    interactive(state, &theme),
                ]),
            ],
            style: Style {
                gap: Size::length(spacing::XL),
                ..Style::default()
            },
        },
    )
}

pub fn run() {
    MultiWindowRunner::<DynamicIslandDemo>::run();
}

#[cfg(test)]
pub(super) fn controlled_state(now: Instant) -> IslandState {
    use chrono::TimeZone;
    IslandState::at(
        now,
        Snapshot::at(
            chrono::FixedOffset::east_opt(0)
                .unwrap()
                .with_ymd_and_hms(2026, 10, 29, 12, 34, 0)
                .unwrap(),
        ),
    )
}

#[cfg(test)]
pub(super) fn advance(state: &mut IslandState, now: Instant) {
    state.wake_at(now, None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use chrono::{FixedOffset, TimeZone};
    use cosmic_text::SwashCache;
    use rutter::layout::{RutterContext, build_taffy_tree, compute_layout};
    use rutter::render::{
        RichTextRenderer, draw_widgets,
        hit_test::{HitResult, hit_test},
        text::TextBufferCache,
    };
    use rutter::{
        WidgetIdSnapshot,
        engine::widget_state::{ScrollState, WidgetState},
    };
    use skia_safe::{Color, Point, Rect as Bounds, surfaces};
    use winit::dpi::PhysicalSize;

    mod raster_tests {
        use super::*;

        #[test]
        fn interactive_samples_paint_the_same_centered_body_and_reachable_header_at_every_scale() {
            for (width, height) in [(320, 260), (800, 260), (320, 700), (800, 700)] {
                for theme in [ExampleTheme::Dark, ExampleTheme::Light] {
                    let now = Instant::now();
                    let mut model = state(now);
                    model.theme = theme;
                    DynamicIslandDemo::surface_metrics_changed(
                        &mut model,
                        SurfaceId::PRIMARY,
                        metrics(width, height),
                    );
                    model.queued.push(Message::Expand);
                    model.wake_at(now, None);
                    for millis in [120, 240, 360, 480] {
                        let at = now + Duration::from_millis(millis);
                        if millis == 360 {
                            model.queued.push(Message::Collapse);
                            model.wake_at(now + MOTION, None);
                        }
                        model.wake_at(at, None);
                        let view = showcase(&model);
                        let (tree, root) = layout(&view, width, height);
                        let capsule = bounds(&tree, root, &[1, 0, 2, 2, 0]);
                        let text = bounds(&tree, root, &[1, 0, 2, 2, 0, 0, 0, 0, 0]);
                        let viewport = bounds(&tree, root, &[1]);
                        let document = bounds(&tree, root, &[1, 0]);
                        assert!((capsule.width() - model.dimensions().0).abs() <= 1.0);
                        assert!((capsule.height() - model.dimensions().1).abs() <= 1.0);
                        assert!((capsule.center_x() - width as f32 / 2.0).abs() <= 1.0);
                        let mut scroll = ScrollState {
                            offset_y: 0.0,
                            content_height: document.height(),
                            viewport_h: viewport.height(),
                        };
                        scroll.scroll_by((capsule.top - viewport.top).max(0.0));
                        let capsule = capsule.with_offset((0.0, -scroll.offset_y));
                        let text = text.with_offset((0.0, -scroll.offset_y));
                        let states =
                            HashMap::from([(layout::PAGE_SCROLL_ID, WidgetState::Scroll(scroll))]);
                        let focus = match hit_test(
                            &view,
                            &tree,
                            root,
                            Point::new(text.center_x(), text.center_y()),
                            Point::default(),
                            &states,
                        ) {
                            Some(HitResult::Message {
                                msg: Message::Expand,
                                focus_id: Some(id),
                            }) => id,
                            _ => panic!("painted header must remain actionable"),
                        };
                        for scale in [1.0_f32, 1.5, 2.0] {
                            let mut raster = surfaces::raster_n32_premul((
                                (width as f32 * scale) as i32,
                                (height as f32 * scale) as i32,
                            ))
                            .unwrap();
                            raster.canvas().clear(Color::TRANSPARENT);
                            raster.canvas().scale((scale, scale));
                            draw_widgets(
                                raster.canvas(),
                                &tree,
                                root,
                                &view,
                                &mut FontSystem::new(),
                                &mut SwashCache::new(),
                                Point::new(-1.0, -1.0),
                                Some(focus),
                                &HashMap::new(),
                                &states,
                                &mut HashMap::new(),
                                &mut TextBufferCache::default(),
                                true,
                                &theme.resolve(),
                                scale,
                            );
                            let pixels = raster.peek_pixels().unwrap();
                            assert_eq!(
                                pixels.get_color((
                                    (capsule.center_x() * scale) as i32,
                                    ((capsule.top + spacing::XS) * scale) as i32
                                )),
                                theme.resolve().surface
                            );
                            let mut ink = 0;
                            for y in (text.top * scale).ceil() as i32
                                ..(text.bottom.min(viewport.bottom) * scale).floor() as i32
                            {
                                for x in (text.left * scale).ceil() as i32
                                    ..(text.right * scale).floor() as i32
                                {
                                    if pixels.get_color((x, y)) != theme.resolve().surface {
                                        ink += 1;
                                    }
                                }
                            }
                            assert!(
                                ink > 10,
                                "wrapped header ink at {width}x{height}, {scale}x, {millis}ms"
                            );
                            assert_eq!(
                                pixels
                                    .get_color((
                                        ((capsule.left - spacing::XS) * scale) as i32,
                                        ((capsule.top + spacing::LG) * scale) as i32
                                    ))
                                    .a(),
                                0
                            );
                        }
                    }
                }
            }
        }
    }

    fn snapshot(day: u32, hour: u32, minute: u32, second: u32) -> Snapshot {
        Snapshot::at(
            FixedOffset::east_opt(7200)
                .unwrap()
                .with_ymd_and_hms(2026, 10, day, hour, minute, second)
                .unwrap(),
        )
    }

    fn state(now: Instant) -> IslandState {
        IslandState::at(now, snapshot(29, 23, 59, 58))
    }

    #[test]
    fn clock_cycles_hours_date_weekday_with_exit_then_entry_and_current_accessible_snapshot() {
        let now = Instant::now();
        let mut model = state(now);
        assert_eq!(model.clock.visual(), "23:59");
        for (seconds, part, expected) in
            [(3, 1, "30 October 2026"), (6, 2, "Friday"), (9, 0, "00:00")]
        {
            let at = now + Duration::from_secs(seconds);
            let old = model.clock.visual().to_owned();
            model.wake_at(at, Some(snapshot(30, 0, 0, seconds as u32 - 2)));
            assert_eq!(model.clock.part, part);
            assert_eq!(model.clock.visual(), old);
            assert_eq!(model.clock.offset(), 0.0);
            assert_eq!(
                model.clock.snapshot.label,
                "00:00 · 30 October 2026 · Friday"
            );
            model.wake_at(at + clock::SLIDE / 2, None);
            assert_eq!(model.clock.offset(), -0.5);
            assert_eq!(model.clock.visual(), old);
            model.wake_at(at + clock::SLIDE, None);
            assert_eq!(model.clock.visual(), expected);
            assert_eq!(model.clock.offset(), 1.0);
            model.wake_at(at + clock::SLIDE + clock::SLIDE / 2, None);
            assert_eq!(model.clock.offset(), 0.5);
            model.wake_at(at + clock::SLIDE * 2, None);
            assert_eq!(model.clock.offset(), 0.0);
            assert!(!model.clock.active());
            assert_eq!(model.frame, None);
            assert!(DynamicIslandDemo::next_wakeup(&model, at).unwrap() > at + clock::SLIDE * 2);
        }
    }

    #[test]
    fn minute_deadline_midnight_and_late_wakes_are_sparse_bounded_and_thread_free() {
        let now = Instant::now();
        let mut controlled = controlled_state(now);
        advance(&mut controlled, now);
        assert_eq!(controlled.clock.visual(), "12:34");
        let mut model = state(now);
        assert_eq!(
            DynamicIslandDemo::next_wakeup(&model, now),
            Some(now + Duration::from_secs(2))
        );
        assert!(model.wake_at(now + Duration::from_secs(1), None).is_empty());
        let midnight = now + Duration::from_secs(2);
        model.wake_at(midnight, Some(snapshot(30, 0, 0, 0)));
        assert_eq!(model.clock.part, 0);
        assert_eq!(model.clock.visual(), "23:59");
        model.wake_at(midnight + Duration::from_secs(1), None);
        assert_eq!(model.clock.visual(), "00:00");
        assert_eq!(model.frame, None);
        let late = now + Duration::from_secs(86_401);
        model.wake_at(late, Some(snapshot(31, 0, 0, 1)));
        assert!(model.clock.deadline() > late);
        model.wake_at(late + Duration::from_secs(1), None);
        assert!(!model.clock.active());
        assert_eq!(model.frame, None);
        assert!(
            model
                .wake_at(late + Duration::from_secs(1), None)
                .is_empty()
        );
        DynamicIslandDemo::surface_closed(&mut model, SurfaceId::PRIMARY);
        assert_eq!(DynamicIslandDemo::next_wakeup(&model, late), None);
    }

    fn layout(
        view: &Widget<'_, Message>,
        width: u32,
        height: u32,
    ) -> (TaffyTree<RutterContext>, NodeId) {
        let fonts = Rc::new(RefCell::new(FontSystem::new()));
        let mut tree = TaffyTree::new();
        let root = build_taffy_tree(&mut tree, view, fonts.clone(), &HashMap::new());
        compute_layout(
            &mut tree,
            root,
            PhysicalSize::new(width, height),
            fonts,
            &RichTextRenderer::default(),
        );
        (tree, root)
    }

    fn bounds(tree: &TaffyTree<RutterContext>, root: NodeId, path: &[usize]) -> Bounds {
        let mut node = root;
        let mut origin = Point::default();
        for &index in path {
            let layout = tree.layout(node).unwrap();
            origin.x += layout.location.x;
            origin.y += layout.location.y;
            node = tree.children(node).unwrap()[index];
        }
        let layout = tree.layout(node).unwrap();
        Bounds::from_xywh(
            origin.x + layout.location.x,
            origin.y + layout.location.y,
            layout.size.width,
            layout.size.height,
        )
    }

    fn metrics(width: u32, height: u32) -> SurfaceMetrics {
        SurfaceMetrics {
            logical_size: (width.into(), height.into()),
            physical_size: (width, height),
            scale_factor: 1.0,
            output_name: None,
            layer_shell_version: None,
        }
    }

    fn escape() -> ShortcutEvent {
        ShortcutEvent {
            key: ShortcutKey::Named(ShortcutNamedKey::Escape),
            control: false,
            alt: false,
            shift: false,
            super_key: false,
            repeat: false,
        }
    }

    #[test]
    fn interactive_motion_is_two_axis_centered_reversible_and_details_have_stable_hits() {
        let mut identities = HashMap::new();
        for width in [320, 800] {
            for height in [260, 700] {
                let now = Instant::now();
                let mut model = state(now);
                DynamicIslandDemo::surface_metrics_changed(
                    &mut model,
                    SurfaceId::PRIMARY,
                    metrics(width, height),
                );
                let compact = model.dimensions();
                model.queued.push(Message::Expand); // the ordinary header's keyboard/a11y action
                model.wake_at(now, None);
                assert!(model.island.is_expanded());
                model.wake_at(now + MOTION / 2, None);
                let midpoint = model.dimensions();
                assert!(midpoint.0 > compact.0 && midpoint.1 > compact.1);
                let view = showcase(&model);
                WidgetIdSnapshot::capture(&view).unwrap();
                let (tree, root) = layout(&view, width, height);
                // page root -> viewport -> document -> interactive section -> region
                let region_path = [1, 0, 2, 2];
                let region = bounds(&tree, root, &region_path);
                assert!((region.width() - midpoint.0).abs() <= 1.0);
                assert!((region.height() - midpoint.1).abs() <= 1.0);
                assert!((region.center_x() - width as f32 / 2.0).abs() <= 1.0);
                let passive = bounds(&tree, root, &[1, 0, 1, 2]);
                assert!(passive.bottom < region.top);
                let viewport = bounds(&tree, root, &[1]);
                let document = bounds(&tree, root, &[1, 0]);
                let inner = bounds(&tree, root, &[1, 0, 2, 2, 0, 0]);
                let inner_document = bounds(&tree, root, &[1, 0, 2, 2, 0, 0, 0]);
                for (path, message, role) in [
                    (vec![1, 0, 2, 2, 0, 0, 0, 0], Message::Expand, "header"),
                    (
                        vec![1, 0, 2, 2, 0, 0, 0, 1, 1],
                        Message::PauseResume,
                        "pause",
                    ),
                    (
                        vec![1, 0, 2, 2, 0, 0, 0, 1, 2],
                        Message::Collapse,
                        "collapse",
                    ),
                ] {
                    let action = bounds(&tree, root, &path);
                    let mut inside = ScrollState {
                        offset_y: 0.0,
                        content_height: inner_document.height(),
                        viewport_h: inner.height(),
                    };
                    inside.scroll_by((action.bottom - inner.bottom).max(0.0));
                    let mut page = ScrollState {
                        offset_y: 0.0,
                        content_height: document.height(),
                        viewport_h: viewport.height(),
                    };
                    page.scroll_by((inner.bottom - viewport.bottom).max(0.0));
                    let point = Point::new(
                        action.center_x(),
                        action.center_y() - inside.offset_y - page.offset_y,
                    );
                    assert!(point.y >= viewport.top && point.y < viewport.bottom);
                    let states = HashMap::from([
                        (layout::PAGE_SCROLL_ID, WidgetState::Scroll(page)),
                        (DETAILS_SCROLL, WidgetState::Scroll(inside)),
                    ]);
                    match hit_test(&view, &tree, root, point, Point::default(), &states) {
                        Some(HitResult::Message {
                            msg,
                            focus_id: Some(id),
                            ..
                        }) => {
                            assert_eq!(msg, message, "{width}x{height} {point:?} {action:?}");
                            assert_eq!(*identities.entry(role).or_insert(id), id);
                        }
                        Some(HitResult::PointerRegion(id)) => panic!(
                            "region {id} consumed {message:?} at {point:?}, action {action:?}, inner {inner:?}"
                        ),
                        Some(HitResult::ScrollFocus(id)) => panic!(
                            "scroll {id} consumed {message:?} at {point:?}, action {action:?}, inner {inner:?}"
                        ),
                        _ => panic!(
                            "missing {message:?} at {point:?}, action {action:?}, inner {inner:?}"
                        ),
                    }
                }
                drop(view);
                model.wake_at(now + MOTION, None);
                assert_eq!(
                    DynamicIslandDemo::shortcut(&model, SurfaceId::PRIMARY, escape()),
                    ShortcutOutcome::Message(Message::Collapse)
                );
                model.queued.push(Message::PauseResume);
                model.queued.push(Message::Collapse);
                model.wake_at(now + MOTION, None);
                assert!(model.paused && !model.island.is_expanded());
                model.wake_at(now + MOTION + MOTION / 2, None);
                assert_eq!(model.dimensions(), midpoint);
                model.wake_at(now + MOTION * 2, None);
                assert_eq!(model.dimensions(), compact);
                assert_eq!(model.frame, None);
            }
        }
    }

    #[test]
    fn passive_is_nonactionable_and_current_accessible_label_survives_outgoing_visual() {
        let now = Instant::now();
        let mut model = state(now);
        model.wake_at(now + clock::CYCLE, Some(snapshot(30, 0, 0, 1)));
        let view = showcase(&model);
        let sections = layout::document_children(&view);
        let Widget::Column { children, .. } = &sections[1] else {
            panic!()
        };
        let Widget::Container { child, .. } = &children[2] else {
            panic!()
        };
        let Widget::Column { children, .. } = child.as_ref() else {
            panic!()
        };
        let [Widget::RichText { content, .. }] = children.as_slice() else {
            panic!()
        };
        assert_eq!(content.plain_text(), "23:59");
        assert_eq!(
            content.accessibility_label(),
            "00:00 · 30 October 2026 · Friday"
        );
        let (tree, root) = layout(&view, 800, 700);
        let passive = bounds(&tree, root, &[1, 0, 1, 2]);
        assert!(matches!(
            hit_test(
                &view,
                &tree,
                root,
                Point::new(passive.center_x(), passive.center_y()),
                Point::default(),
                &HashMap::new()
            ),
            None | Some(HitResult::ScrollFocus(layout::PAGE_SCROLL_ID))
        ));
    }

    #[test]
    fn responsive_raster_paints_capsules_text_and_clips_slides_in_both_themes_and_scales() {
        for width in [320, 800] {
            for height in [260, 700] {
                for theme in [ExampleTheme::Dark, ExampleTheme::Light] {
                    let now = Instant::now();
                    let mut model = state(now);
                    model.theme = theme;
                    DynamicIslandDemo::surface_metrics_changed(
                        &mut model,
                        SurfaceId::PRIMARY,
                        metrics(width, height),
                    );
                    for phase in [0, 120, 240, 360, 480] {
                        if phase == 0 {
                            model.wake_at(now + clock::CYCLE, Some(snapshot(30, 0, 0, 1)));
                        }
                        model.wake_at(now + clock::CYCLE + Duration::from_millis(phase), None);
                        let view = showcase(&model);
                        let (tree, root) = layout(&view, width, height);
                        let capsule = bounds(&tree, root, &[1, 0, 1, 2]);
                        let text = bounds(&tree, root, &[1, 0, 1, 2, 0, 0]);
                        let viewport = bounds(&tree, root, &[1]);
                        let document = bounds(&tree, root, &[1, 0]);
                        let mut scroll = ScrollState {
                            offset_y: 0.0,
                            content_height: document.height(),
                            viewport_h: viewport.height(),
                        };
                        scroll.scroll_by((capsule.bottom + spacing::XS - viewport.bottom).max(0.0));
                        let capsule = capsule.with_offset((0.0, -scroll.offset_y));
                        let text = text.with_offset((0.0, -scroll.offset_y));
                        let states =
                            HashMap::from([(layout::PAGE_SCROLL_ID, WidgetState::Scroll(scroll))]);
                        for scale in [1.0_f32, 1.5, 2.0] {
                            let mut raster = surfaces::raster_n32_premul((
                                (width as f32 * scale) as i32,
                                (height as f32 * scale) as i32,
                            ))
                            .unwrap();
                            raster.canvas().clear(Color::TRANSPARENT);
                            raster.canvas().scale((scale, scale));
                            draw_widgets(
                                raster.canvas(),
                                &tree,
                                root,
                                &view,
                                &mut FontSystem::new(),
                                &mut SwashCache::new(),
                                Point::new(-1.0, -1.0),
                                None,
                                &HashMap::new(),
                                &states,
                                &mut HashMap::new(),
                                &mut TextBufferCache::default(),
                                true,
                                &theme.resolve(),
                                scale,
                            );
                            let pixels = raster.peek_pixels().unwrap();
                            assert!(
                                capsule.top >= viewport.top && capsule.bottom < height as f32 - 2.0
                            );
                            {
                                assert_eq!(
                                    pixels.get_color((
                                        (capsule.center_x() * scale) as i32,
                                        ((capsule.top + 4.0) * scale) as i32
                                    )),
                                    theme.resolve().surface
                                );
                                assert_eq!(
                                    pixels
                                        .get_color((
                                            (capsule.center_x() * scale) as i32,
                                            ((capsule.bottom + 2.0) * scale) as i32
                                        ))
                                        .a(),
                                    0
                                );
                                let mut ink = 0;
                                for y in (text.top * scale).ceil() as i32
                                    ..(text.bottom * scale).floor() as i32
                                {
                                    for x in (text.left * scale).ceil() as i32
                                        ..(text.right * scale).floor() as i32
                                    {
                                        if pixels.get_color((x, y)) != theme.resolve().surface {
                                            ink += 1;
                                        }
                                    }
                                }
                                if phase == 0 || phase == 480 {
                                    assert!(ink > 10, "resting text must paint");
                                }
                                if phase == 240 {
                                    assert_eq!(ink, 0, "entry starts outside its text clip");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod page_tests {
    use super::super::theme_selector::test_support::assert_theme_toggle;
    use super::*;

    #[test]
    fn dynamic_island_starts_dark_with_selector_in_an_ordinary_window() {
        let mut fonts = FontSystem::new();
        let mut state = DynamicIslandDemo::new(&mut fonts);
        assert_eq!(
            DynamicIslandDemo::theme_for(&state).surface,
            Theme::dark().surface
        );
        assert_theme_toggle(&DynamicIslandDemo::view(&mut state, SurfaceId::PRIMARY));
        let requests = DynamicIslandDemo::initial_surfaces();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].window.layer_shell().is_none());
    }

    #[test]
    fn pages_reflow_and_retain_reachable_documents() {
        let fonts = std::rc::Rc::new(std::cell::RefCell::new(FontSystem::new()));
        let mut state = DynamicIslandDemo::new(&mut fonts.borrow_mut());
        let view = DynamicIslandDemo::view(&mut state, SurfaceId::PRIMARY);
        super::super::layout::test_support::assert_page(&view, &fonts, "dynamic_island_demo");
    }
}
