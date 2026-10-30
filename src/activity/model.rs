use std::time::Duration;

use super::{ActivityAction, ActivityError, InvalidField, MAX_SUPPORTING_BYTES, MAX_TITLE_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Download,
    Upload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaState {
    Playing,
    Paused,
    Stopped,
}

/// An immutable transfer snapshot with integer units chosen by its producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    title: String,
    supporting: String,
    direction: TransferDirection,
    completed: u64,
    total: Option<u64>,
    state: TransferState,
}

impl Transfer {
    /// Known totals may be zero. Completed snapshots with a known total must
    /// exactly reach it; unknown totals permit any completed-unit count.
    pub fn new(
        title: impl Into<String>,
        direction: TransferDirection,
        completed: u64,
        total: Option<u64>,
        state: TransferState,
    ) -> Result<Self, ActivityError> {
        if let Some(total) = total {
            if completed > total {
                return Err(ActivityError::InvalidField(
                    InvalidField::ProgressExceedsTotal { completed, total },
                ));
            }
            if state == TransferState::Completed && completed != total {
                return Err(ActivityError::InvalidField(
                    InvalidField::IncompleteTransfer { completed, total },
                ));
            }
        }
        Ok(Self {
            title: text(title, "title", MAX_TITLE_BYTES)?,
            supporting: String::new(),
            direction,
            completed,
            total,
            state,
        })
    }

    /// Add optional supporting text, validating before accepting the snapshot.
    pub fn with_supporting(mut self, supporting: impl Into<String>) -> Result<Self, ActivityError> {
        self.supporting = text(supporting, "supporting", MAX_SUPPORTING_BYTES)?;
        Ok(self)
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn supporting(&self) -> &str {
        &self.supporting
    }
    pub fn direction(&self) -> TransferDirection {
        self.direction
    }
    pub fn completed(&self) -> u64 {
        self.completed
    }
    pub fn total(&self) -> Option<u64> {
        self.total
    }
    pub fn state(&self) -> TransferState {
        self.state
    }

    /// Finite percentage in 0..=100, or None for indeterminate progress.
    /// Zero/zero is 100 only when completed, otherwise 0. No integer product
    /// is used, so even u64::MAX units cannot overflow.
    pub fn percentage(&self) -> Option<f64> {
        self.total.map(|total| {
            if total == 0 {
                if self.state == TransferState::Completed {
                    100.0
                } else {
                    0.0
                }
            } else {
                (self.completed as f64 / total as f64) * 100.0
            }
        })
    }

    pub fn supported_actions(&self) -> &'static [ActivityAction] {
        use ActivityAction::*;
        match self.state {
            TransferState::Running => &[Pause, Cancel],
            TransferState::Paused => &[Resume, Cancel],
            TransferState::Completed => &[],
        }
    }
}

/// An immutable playback snapshot. Duration is optional for live/unknown media.
/// Position never advances implicitly; stopped/paused positions are preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    title: String,
    artist: String,
    position: Duration,
    duration: Option<Duration>,
    state: MediaState,
}

impl Media {
    /// Position may equal a known duration (including zero), but never exceed it.
    pub fn new(
        title: impl Into<String>,
        artist: impl Into<String>,
        position: Duration,
        duration: Option<Duration>,
        state: MediaState,
    ) -> Result<Self, ActivityError> {
        if let Some(duration) = duration
            && position > duration
        {
            return Err(ActivityError::InvalidField(
                InvalidField::PositionExceedsDuration { position, duration },
            ));
        }
        Ok(Self {
            title: text(title, "title", MAX_TITLE_BYTES)?,
            artist: text(artist, "artist", MAX_SUPPORTING_BYTES)?,
            position,
            duration,
            state,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn artist(&self) -> &str {
        &self.artist
    }
    pub fn position(&self) -> Duration {
        self.position
    }
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }
    pub fn state(&self) -> MediaState {
        self.state
    }

    /// Previous/Next are semantic requests in every playback state. Cancel is
    /// transfer-only; Resume also means start playback from the stopped state.
    pub fn supported_actions(&self) -> &'static [ActivityAction] {
        use ActivityAction::*;
        match self.state {
            MediaState::Playing => &[Pause, Previous, Next],
            MediaState::Paused | MediaState::Stopped => &[Resume, Previous, Next],
        }
    }
}

/// Full validated content; kinds may be changed by registry replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityContent {
    Transfer(Transfer),
    Media(Media),
}

impl From<Transfer> for ActivityContent {
    fn from(value: Transfer) -> Self {
        Self::Transfer(value)
    }
}

impl From<Media> for ActivityContent {
    fn from(value: Media) -> Self {
        Self::Media(value)
    }
}

/// Borrowed presentation facts, with no localized formatting or renderer types.
#[derive(Debug, Clone, PartialEq)]
pub enum ActivitySummary<'a> {
    Transfer {
        title: &'a str,
        supporting: &'a str,
        direction: TransferDirection,
        completed: u64,
        total: Option<u64>,
        percentage: Option<f64>,
        state: TransferState,
    },
    Media {
        title: &'a str,
        artist: &'a str,
        position: Duration,
        duration: Option<Duration>,
        state: MediaState,
    },
}

impl ActivityContent {
    pub fn supported_actions(&self) -> &'static [ActivityAction] {
        match self {
            Self::Transfer(transfer) => transfer.supported_actions(),
            Self::Media(media) => media.supported_actions(),
        }
    }

    pub fn summary(&self) -> ActivitySummary<'_> {
        match self {
            Self::Transfer(t) => ActivitySummary::Transfer {
                title: t.title(),
                supporting: t.supporting(),
                direction: t.direction(),
                completed: t.completed(),
                total: t.total(),
                percentage: t.percentage(),
                state: t.state(),
            },
            Self::Media(m) => ActivitySummary::Media {
                title: m.title(),
                artist: m.artist(),
                position: m.position(),
                duration: m.duration(),
                state: m.state(),
            },
        }
    }
}

fn text(
    value: impl Into<String>,
    field: &'static str,
    max: usize,
) -> Result<String, ActivityError> {
    let value = value.into();
    if value.len() > max {
        return Err(ActivityError::InvalidField(InvalidField::TextTooLong {
            field,
            bytes: value.len(),
            max,
        }));
    }
    // A short String may carry an arbitrarily large reserved allocation. Keep
    // retained snapshots bounded by validated text, not caller-owned capacity.
    Ok(value.into_boxed_str().into_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validated_text_does_not_retain_unbounded_reserved_capacity() {
        let oversized_allocation = || {
            let mut value = String::with_capacity(1024 * 1024);
            value.push_str("short");
            value
        };
        let transfer = Transfer::new(
            oversized_allocation(),
            TransferDirection::Download,
            0,
            None,
            TransferState::Running,
        )
        .unwrap()
        .with_supporting(oversized_allocation())
        .unwrap();
        assert_eq!(transfer.title.capacity(), transfer.title.len());
        assert_eq!(transfer.supporting.capacity(), transfer.supporting.len());
        let media = Media::new(
            oversized_allocation(),
            oversized_allocation(),
            Duration::ZERO,
            None,
            MediaState::Playing,
        )
        .unwrap();
        assert_eq!(media.title.capacity(), media.title.len());
        assert_eq!(media.artist.capacity(), media.artist.len());
    }
}
