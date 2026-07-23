// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt::{self, Display, Formatter};

use unicode_segmentation::UnicodeSegmentation;

const HARD_MAX_INPUT_BYTES: usize = 1024 * 1024;
const HARD_MAX_INPUT_GRAPHEMES: usize = 262_144;
const HARD_MAX_INPUT_LINES: usize = 16_384;
const HARD_MAX_UNDO_BYTES: usize = 8 * 1024 * 1024;
const HARD_MAX_UNDO_ENTRIES: usize = 100;

/// Identifies the input profile used to choose bounded resource limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    /// A single-line editable text field.
    TextInput,
    /// A short single-line search query field.
    SearchBar,
    /// A multi-line editable text area.
    TextArea,
}

impl InputKind {
    /// Returns the safe default limits for this kind of input.
    ///
    /// ```
    /// use rutter::input_limits::InputKind;
    ///
    /// assert_eq!(InputKind::SearchBar.limits().max_bytes, 8 * 1024);
    /// ```
    pub const fn limits(self) -> InputLimits {
        InputLimits::for_kind(self)
    }
}

/// Bounds the text and undo history retained by one input widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputLimits {
    /// Maximum UTF-8 bytes retained in the input text.
    pub max_bytes: usize,
    /// Maximum extended grapheme clusters retained in the input text.
    pub max_graphemes: usize,
    /// Maximum logical lines retained in the input text.
    pub max_lines: usize,
    /// Maximum cumulative UTF-8 bytes retained by undo snapshots.
    pub max_undo_bytes: usize,
    /// Maximum number of undo snapshots retained.
    pub max_undo_entries: usize,
}

impl InputLimits {
    /// Restricts application-provided limits to the framework's hard caps.
    ///
    /// ```
    /// use rutter::InputLimits;
    ///
    /// let limits = InputLimits { max_bytes: usize::MAX, ..InputLimits::default() };
    /// assert_eq!(limits.clamp_to_hard_caps().max_bytes, 1024 * 1024);
    /// ```
    pub const fn clamp_to_hard_caps(self) -> Self {
        Self {
            max_bytes: cap_limit(self.max_bytes, HARD_MAX_INPUT_BYTES),
            max_graphemes: cap_limit(self.max_graphemes, HARD_MAX_INPUT_GRAPHEMES),
            max_lines: cap_limit(self.max_lines, HARD_MAX_INPUT_LINES),
            max_undo_bytes: cap_limit(self.max_undo_bytes, HARD_MAX_UNDO_BYTES),
            max_undo_entries: cap_limit(self.max_undo_entries, HARD_MAX_UNDO_ENTRIES),
        }
    }

    /// Creates the safe profile for an input kind.
    ///
    /// ```
    /// use rutter::input_limits::{InputKind, InputLimits};
    ///
    /// let limits = InputLimits::for_kind(InputKind::TextInput);
    /// assert_eq!(limits.max_lines, 1);
    /// ```
    pub const fn for_kind(kind: InputKind) -> Self {
        match kind {
            InputKind::TextInput => Self {
                max_bytes: 64 * 1024,
                max_graphemes: 16_384,
                max_lines: 1,
                max_undo_bytes: 1024 * 1024,
                max_undo_entries: 100,
            },
            InputKind::SearchBar => Self {
                max_bytes: 8 * 1024,
                max_graphemes: 2_048,
                max_lines: 1,
                max_undo_bytes: 128 * 1024,
                max_undo_entries: 32,
            },
            InputKind::TextArea => Self {
                max_bytes: 1024 * 1024,
                max_graphemes: 262_144,
                max_lines: 16_384,
                max_undo_bytes: 8 * 1024 * 1024,
                max_undo_entries: 100,
            },
        }
    }
}

impl Default for InputLimits {
    fn default() -> Self {
        Self::for_kind(InputKind::TextArea)
    }
}

/// Reports a rejected input operation without exposing unbounded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputLimitError {
    /// The candidate text exceeds its UTF-8 byte budget.
    BytesExceeded { actual: usize, max: usize },
    /// The candidate text exceeds its extended grapheme budget.
    GraphemesExceeded { actual: usize, max: usize },
    /// The candidate text exceeds its logical line budget.
    LinesExceeded { actual: usize, max: usize },
    /// One undo snapshot exceeds the configured retained-byte budget.
    UndoBudgetExceeded { actual: usize, max: usize },
    /// A byte range is outside text bounds or splits a UTF-8 code point.
    InvalidUtf8Range {
        start: usize,
        end: usize,
        text_bytes: usize,
    },
    /// Reserving a bounded candidate buffer failed.
    AllocationFailed {
        requested_bytes: usize,
        operation: &'static str,
    },
}

impl Display for InputLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BytesExceeded { actual, max } => {
                write!(
                    formatter,
                    "input has {actual} bytes; expected at most {max} bytes"
                )
            }
            Self::GraphemesExceeded { actual, max } => write!(
                formatter,
                "input has {actual} graphemes; expected at most {max} graphemes"
            ),
            Self::LinesExceeded { actual, max } => {
                write!(
                    formatter,
                    "input has {actual} lines; expected at most {max} lines"
                )
            }
            Self::UndoBudgetExceeded { actual, max } => write!(
                formatter,
                "undo snapshot retains {actual} bytes; expected at most {max} bytes"
            ),
            Self::InvalidUtf8Range {
                start,
                end,
                text_bytes,
            } => write!(
                formatter,
                "invalid UTF-8 range {start}..{end} for {text_bytes} bytes; expected in-bounds byte offsets at UTF-8 character boundaries"
            ),
            Self::AllocationFailed {
                requested_bytes,
                operation,
            } => write!(
                formatter,
                "could not reserve {requested_bytes} bytes for {operation}; expected an allocation for a valid UTF-8 text candidate"
            ),
        }
    }
}

impl std::error::Error for InputLimitError {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TextMetrics {
    pub(crate) bytes: usize,
    pub(crate) graphemes: usize,
    pub(crate) lines: usize,
    pub(crate) scalars: usize,
}

impl TextMetrics {
    pub(crate) fn insertion_upper_bound(self, inserted: Self) -> Result<Self, InputLimitError> {
        Ok(Self {
            bytes: checked_sum(self.bytes, inserted.bytes, "input byte metrics")?,
            graphemes: checked_sum(self.graphemes, inserted.graphemes, "input grapheme metrics")?,
            lines: checked_sum(
                self.lines,
                inserted.lines.saturating_sub(1),
                "input line metrics",
            )?,
            scalars: checked_sum(self.scalars, inserted.scalars, "input scalar metrics")?,
        })
    }
}

/// Measures text without allocating a normalized copy.
pub(crate) fn text_metrics(text: &str) -> TextMetrics {
    TextMetrics {
        bytes: text.len(),
        graphemes: text.graphemes(true).count(),
        lines: logical_line_count(text),
        scalars: text.chars().count(),
    }
}

/// Validates source text before it reaches the editor buffer.
pub(crate) fn validate_text(
    text: &str,
    limits: InputLimits,
) -> Result<TextMetrics, InputLimitError> {
    if text.len() > limits.max_bytes {
        return Err(InputLimitError::BytesExceeded {
            actual: text.len(),
            max: limits.max_bytes,
        });
    }
    let metrics = text_metrics(text);
    validate_metrics(metrics, limits)?;
    Ok(metrics)
}

pub(crate) fn validate_clipboard_source(
    text: &str,
    limits: InputLimits,
) -> Result<(), InputLimitError> {
    if text.len() > limits.max_bytes {
        return Err(InputLimitError::BytesExceeded {
            actual: text.len(),
            max: limits.max_bytes,
        });
    }
    let graphemes = text.graphemes(true).count();
    if graphemes > limits.max_graphemes {
        return Err(InputLimitError::GraphemesExceeded {
            actual: graphemes,
            max: limits.max_graphemes,
        });
    }
    Ok(())
}

/// Validates a fully assembled replacement candidate.
pub(crate) fn validate_candidate(
    candidate: &str,
    limits: InputLimits,
) -> Result<TextMetrics, InputLimitError> {
    validate_text(candidate, limits)
}

pub(crate) fn validate_metrics(
    metrics: TextMetrics,
    limits: InputLimits,
) -> Result<(), InputLimitError> {
    validate_bytes_and_lines(metrics, limits)?;
    if metrics.graphemes > limits.max_graphemes {
        return Err(InputLimitError::GraphemesExceeded {
            actual: metrics.graphemes,
            max: limits.max_graphemes,
        });
    }
    Ok(())
}

pub(crate) fn validate_bytes_and_lines(
    metrics: TextMetrics,
    limits: InputLimits,
) -> Result<(), InputLimitError> {
    if metrics.bytes > limits.max_bytes {
        return Err(InputLimitError::BytesExceeded {
            actual: metrics.bytes,
            max: limits.max_bytes,
        });
    }
    if metrics.lines > limits.max_lines {
        return Err(InputLimitError::LinesExceeded {
            actual: metrics.lines,
            max: limits.max_lines,
        });
    }
    Ok(())
}

pub(crate) fn validate_inserted_bytes(
    existing_bytes: usize,
    inserted_bytes: usize,
    limits: InputLimits,
) -> Result<(), InputLimitError> {
    let actual = checked_sum(existing_bytes, inserted_bytes, "input byte metrics")?;
    if actual <= limits.max_bytes {
        return Ok(());
    }
    Err(InputLimitError::BytesExceeded {
        actual,
        max: limits.max_bytes,
    })
}

pub(crate) fn validate_utf8_range(
    text: &str,
    start: usize,
    end: usize,
) -> Result<(), InputLimitError> {
    if start <= end
        && end <= text.len()
        && text.is_char_boundary(start)
        && text.is_char_boundary(end)
    {
        return Ok(());
    }
    Err(InputLimitError::InvalidUtf8Range {
        start,
        end,
        text_bytes: text.len(),
    })
}

pub(crate) fn replacement_candidate(
    text: &str,
    start: usize,
    end: usize,
    replacement: &str,
    max_bytes: usize,
) -> Result<String, InputLimitError> {
    validate_utf8_range(text, start, end)?;
    let retained_bytes = text.len() - (end - start);
    let requested_bytes = checked_sum(retained_bytes, replacement.len(), "input candidate")?;
    if requested_bytes > max_bytes {
        return Err(InputLimitError::BytesExceeded {
            actual: requested_bytes,
            max: max_bytes,
        });
    }
    let mut candidate = reserve_text(requested_bytes, "input candidate")?;
    candidate.push_str(&text[..start]);
    candidate.push_str(replacement);
    candidate.push_str(&text[end..]);
    Ok(candidate)
}

pub(crate) fn copy_text_with_reserve(
    text: &str,
    operation: &'static str,
) -> Result<String, InputLimitError> {
    let mut copy = reserve_text(text.len(), operation)?;
    copy.push_str(text);
    Ok(copy)
}

pub(crate) fn normalized_text_byte_len(text: &str) -> usize {
    let mut index = 0;
    let mut normalized_len = 0;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        normalized_len += 1;
        index += 1;
        if matches!(byte, b'\r' | b'\n')
            && bytes
                .get(index)
                .is_some_and(|next| *next != byte && matches!(*next, b'\r' | b'\n'))
        {
            index += 1;
        }
    }
    normalized_len
}

fn logical_line_count(text: &str) -> usize {
    let mut index = 0;
    let mut lines: usize = 1;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if !matches!(byte, b'\r' | b'\n') {
            continue;
        }
        lines = lines.saturating_add(1);
        if bytes
            .get(index)
            .is_some_and(|next| *next != byte && matches!(*next, b'\r' | b'\n'))
        {
            index += 1;
        }
    }
    lines
}

fn checked_sum(
    left: usize,
    right: usize,
    operation: &'static str,
) -> Result<usize, InputLimitError> {
    left.checked_add(right)
        .ok_or(InputLimitError::AllocationFailed {
            requested_bytes: usize::MAX,
            operation,
        })
}

const fn cap_limit(value: usize, maximum: usize) -> usize {
    if value > maximum { maximum } else { value }
}

fn reserve_text(
    requested_bytes: usize,
    operation: &'static str,
) -> Result<String, InputLimitError> {
    let mut text = String::new();
    text.try_reserve(requested_bytes)
        .map_err(|_| InputLimitError::AllocationFailed {
            requested_bytes,
            operation,
        })?;
    Ok(text)
}
