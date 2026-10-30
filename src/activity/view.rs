//! Reusable, plain-text presentation of approved activity snapshots.
//!
//! [`details`] works with an in-process registry or an IPC-backed host. It emits
//! typed requests only: reducers must call [`super::ActivityRegistry::route_action`]
//! against their latest snapshot before delivering a request to the producer.
//! Passive islands should use [`summary`] as text only, without action controls.
//! Transfer counts are producer-defined units, never assumed to be bytes.

use std::time::Duration;

use taffy::prelude::*;

use super::{Activity, ActivityAction, ActivityContent, ActivityId, TransferDirection};
use crate::{ButtonVariant, HeadingLevel, Theme, Widget, spacing};

/// English, plain-text summary suitable for selectors and passive islands.
/// Includes state and progress without interpreting producer text as markup.
pub fn summary(activity: &Activity) -> String {
    match activity.content() {
        ActivityContent::Transfer(t) => {
            let direction = match t.direction() {
                TransferDirection::Download => "Download",
                TransferDirection::Upload => "Upload",
            };
            let progress = match t.total() {
                Some(total) => format!(
                    "{} / {total} units ({:.0}%)",
                    t.completed(),
                    t.percentage().unwrap()
                ),
                None => format!("{} units · total unknown", t.completed()),
            };
            format!("{direction}: {} · {:?} · {progress}", t.title(), t.state())
        }
        ActivityContent::Media(m) => {
            let duration = m
                .duration()
                .map(timestamp)
                .unwrap_or_else(|| "live / unknown duration".into());
            format!(
                "{} · {} · {:?} · {} / {duration}",
                m.title(),
                m.artist(),
                m.state(),
                timestamp(m.position())
            )
        }
    }
}

/// Natural-height details with wrapped controls and a typed action callback.
///
/// The parent owns padding and scrolling; this view owns 12px sibling gaps.
/// Known transfer totals use determinate progress, unknown running totals use
/// indeterminate progress (paused/completed snapshots do not animate). Media
/// shows the producer's position and duration, without advancing either.
/// Buttons currently have structural identity in Rutter, not manual keys;
/// callbacks always carry the snapshot's activity ID, including after switching.
/// Progress uses automatic identity too, avoiding collisions with caller IDs.
///
/// In-process integration uses the same registry and callback contract:
/// ```
/// use rutter::{Theme, activity::{ActivityAction, ActivityId, ActivityRegistry,
///     Transfer, TransferDirection, TransferState, view}};
/// enum Message { Action(ActivityId, ActivityAction) }
/// let mut registry = ActivityRegistry::new();
/// let owner = registry.register_owner()?;
/// let id = registry.start(owner, Transfer::new("Download", TransferDirection::Download,
///     25, Some(100), TransferState::Running)?)?;
/// let widget = view::details(registry.get(id).unwrap(), Message::Action, &Theme::light());
/// // In the reducer, validate again before notifying your producer:
/// let Message::Action(id, action) = Message::Action(id, ActivityAction::Pause);
/// let event = registry.route_action(id, action)?;
/// # Ok::<(), rutter::activity::ActivityError>(())
/// ```
pub fn details<'a, Msg>(
    activity: &Activity,
    on_action: fn(ActivityId, ActivityAction) -> Msg,
    theme: &Theme,
) -> Widget<'a, Msg> {
    let (title, supporting) = match activity.content() {
        ActivityContent::Transfer(t) => (t.title(), t.supporting()),
        ActivityContent::Media(m) => (m.title(), m.artist()),
    };
    let mut children = vec![Widget::Heading {
        content: title.into(),
        level: HeadingLevel::H2,
        style: fluid(),
        color: Some(theme.on_surface),
    }];
    if !supporting.is_empty() {
        children.push(text(supporting, theme));
    }
    children.push(text(&summary(activity), theme));
    if let ActivityContent::Transfer(t) = activity.content() {
        children.push(Widget::ProgressBar {
            id: crate::widget::AUTO_ID,
            value: (t.percentage().unwrap_or(0.0) / 100.0) as f32,
            indeterminate: t.total().is_none() && t.state() == super::TransferState::Running,
            style: Style {
                size: Size {
                    width: Dimension::percent(1.0),
                    height: Dimension::length(12.0),
                },
                ..fluid()
            },
        });
    }
    if !activity.supported_actions().is_empty() {
        children.push(Widget::Row {
            children: activity
                .supported_actions()
                .iter()
                .map(|&action| Widget::Button {
                    text: action_label(action),
                    on_press: on_action(activity.id(), action),
                    style: Style {
                        min_size: Size::from_lengths(88.0, 40.0),
                        flex_shrink: 0.0,
                        ..Style::default()
                    },
                    color: None,
                    variant: ButtonVariant::Ghost,
                })
                .collect(),
            style: Style {
                flex_wrap: FlexWrap::Wrap,
                gap: Size::length(spacing::SM),
                ..fluid()
            },
        });
    }
    Widget::Column {
        children,
        style: Style {
            gap: Size::length(spacing::MD),
            ..fluid()
        },
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

fn text<'a, Msg>(content: &str, theme: &Theme) -> Widget<'a, Msg> {
    Widget::Text {
        content: content.into(),
        style: fluid(),
        color: Some(theme.on_surface),
        size: theme.font_body,
    }
}

fn timestamp(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn action_label(action: ActivityAction) -> &'static str {
    match action {
        ActivityAction::Pause => "Pause",
        ActivityAction::Resume => "Resume",
        ActivityAction::Cancel => "Cancel",
        ActivityAction::Previous => "Previous",
        ActivityAction::Next => "Next",
    }
}

#[cfg(test)]
mod tests;
