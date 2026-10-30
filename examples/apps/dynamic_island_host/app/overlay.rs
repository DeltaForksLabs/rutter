//! Native policy stays local to this example; IPC never requests keyboard focus.
use rutter::{SurfaceInputRegion, SurfaceMetrics, skia_safe::Rect as PaintRect};

use super::*;

const DOCUMENT: u64 = 9_000_001;

pub(super) mod focus;

#[derive(Clone, Default)]
pub(super) struct Overlay {
    pub metrics: Option<SurfaceMetrics>,
    pub pending: bool,
    pub closed: bool,
    published: Option<SurfaceInputRegion>,
    pub focus: focus::Focus,
}

impl Overlay {
    pub fn invalidate(&mut self) {
        if !self.closed {
            self.pending = true;
            self.published = None;
        }
    }
}

pub(super) fn bounds(state: &State) -> Option<PaintRect> {
    let overlay = state.overlay.as_ref()?;
    if overlay.closed {
        return None;
    }
    let (width, height) = overlay.metrics.as_ref()?.logical_size;
    if !width.is_finite()
        || !height.is_finite()
        || width > 16384.0
        || height > 16384.0
        || width <= 48.0
        || height <= 48.0
    {
        return None;
    }
    // These are viewport/control dimensions, not spacing tokens. An 8px host
    // inset preserves transparent margins; the capsule owns its 16px padding.
    let (cap_width, cap_height) = if state.island.is_expanded() {
        (404.0, 284.0)
    } else {
        (240.0, 76.0)
    };
    let capsule_width = (width as f32 - 2.0 * spacing::SM).min(cap_width);
    let capsule_height = (height as f32 - 2.0 * spacing::SM).min(cap_height);
    Some(PaintRect::from_xywh(
        ((width as f32 - capsule_width) / 2.0).floor(),
        spacing::SM,
        capsule_width,
        capsule_height,
    ))
}

pub(super) fn publish(state: &mut State) -> Vec<SurfaceCommand> {
    let Some(overlay) = &state.overlay else {
        return vec![];
    };
    if overlay.closed {
        return vec![];
    }
    let region = bounds(state).map_or(SurfaceInputRegion::Empty, |bounds| {
        state
            .island
            .input_region(bounds, DynamicIsland::RADIUS)
            .expect("bounded capsule geometry")
    });
    let overlay = state.overlay.as_mut().expect("overlay checked");
    overlay.pending = false;
    if overlay.published.as_ref() == Some(&region) {
        return vec![];
    }
    overlay.published = Some(region.clone());
    vec![
        SurfaceCommand::SetInputRegion {
            surface: SurfaceId::PRIMARY,
            region,
        },
        SurfaceCommand::RequestRedraw(SurfaceId::PRIMARY),
    ]
}

pub(super) fn close(state: &mut State) -> Vec<SurfaceCommand> {
    let Some(overlay) = &mut state.overlay else {
        return vec![];
    };
    if overlay.closed {
        return vec![];
    }
    overlay.closed = true;
    overlay.pending = false;
    overlay.published = Some(SurfaceInputRegion::Empty);
    let mut commands = overlay.focus.release();
    commands.extend([
        SurfaceCommand::SetInputRegion {
            surface: SurfaceId::PRIMARY,
            region: SurfaceInputRegion::Empty,
        },
        SurfaceCommand::Close(SurfaceId::PRIMARY),
    ]);
    commands
}

pub(super) fn next_wakeup(state: &State, now: std::time::Instant) -> Option<std::time::Instant> {
    let overlay = state.overlay.as_ref().filter(|overlay| !overlay.closed)?;
    overlay
        .pending
        .then_some(now)
        .into_iter()
        .chain(overlay.focus.deadline)
        .min()
}

pub(super) fn wakeup(state: &mut State, now: std::time::Instant) -> Vec<SurfaceCommand> {
    let mut commands = focus::timeout(state, now);
    commands.extend(publish(state));
    commands
}

pub(super) fn shortcut(
    state: &State,
    surface: SurfaceId,
    event: rutter::ShortcutEvent,
) -> rutter::ShortcutOutcome<Message> {
    if surface == SurfaceId::PRIMARY
        && state
            .overlay
            .as_ref()
            .is_some_and(|overlay| !overlay.closed)
        && state.island.is_expanded()
        && event.key == rutter::ShortcutKey::Named(rutter::ShortcutNamedKey::Escape)
        && !event.control
        && !event.alt
        && !event.super_key
    {
        rutter::ShortcutOutcome::Message(Message::Collapse)
    } else {
        rutter::ShortcutOutcome::Ignored
    }
}

fn details<'a>(state: &'a State, theme: &Theme) -> Widget<'a, Message> {
    let mut children = vec![button("Switch light / dark theme", Message::ToggleTheme)];
    if let Some(error) = &state.ingress_error {
        children.push(text(error, theme));
    }
    for (id, label) in &state.selector_labels {
        children.push(Widget::button_content(
            label,
            text(label, theme),
            Message::Select(*id),
            Style {
                min_size: Size::from_lengths(0.0, 44.0),
                ..fluid()
            },
            None,
            ButtonVariant::Text,
        ));
    }
    if let Some(activity) = state
        .registry
        .selected()
        .and_then(|id| state.registry.get(id))
    {
        children.push(view::details(activity, Message::Action, theme));
        if let Some(error) = state.errors.get(&activity.id()) {
            children.push(text(error, theme));
        }
    } else {
        children.push(text(
            "No activities. Connect a producer to this host's activity.sock.",
            theme,
        ));
    }
    children.push(button("Close host", Message::Close));
    Widget::Column {
        children,
        style: Style {
            gap: Size::length(spacing::MD),
            ..fluid()
        },
    }
}

pub(super) fn view<'a>(state: &'a State, theme: &Theme) -> Widget<'a, Message> {
    let Some(bounds) = bounds(state) else {
        return Widget::Column {
            children: vec![],
            style: Style::default(),
        };
    };
    let summary = if state.selected_summary.is_empty() {
        if state.island.is_expanded() {
            "Activity host · Collapse details"
        } else {
            "Activity host · Expand details"
        }
    } else {
        &state.selected_summary
    };
    let mut capsule = state
        .island
        .interactive_view(
            summary,
            Message::ToggleDetails,
            details(state, theme),
            theme,
        )
        .expect("interactive host");
    let Widget::Container { style, child, .. } = &mut capsule else {
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
    // Scroll the complete natural document, including the header. A long summary
    // must not permanently consume the allocation reserved for producer controls.
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
    Widget::Container {
        child: Box::new(capsule),
        style: Style {
            size: Size::percent(1.0_f32),
            ..Style::default()
        },
        color: None,
        radius: 0.0,
    }
}

#[cfg(test)]
mod tests;
