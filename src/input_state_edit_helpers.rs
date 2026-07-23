// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use cosmic_text::{Buffer, Cursor, Edit, Editor};

use crate::input_limits::{InputLimitError, TextMetrics, normalized_text_byte_len, text_metrics};

pub(crate) fn buffer_matches_text(editor: &Editor<'_>, expected: &str) -> bool {
    editor.with_buffer(|buffer| {
        let mut offset: usize = 0;
        for (index, line) in buffer.lines.iter().enumerate() {
            let next_offset = offset.saturating_add(line.text().len());
            if expected.get(offset..next_offset) != Some(line.text()) {
                return false;
            }
            offset = next_offset;
            if index + 1 < buffer.lines.len() && expected.as_bytes().get(offset) != Some(&b'\n') {
                return false;
            }
            offset = offset.saturating_add(usize::from(index + 1 < buffer.lines.len()));
        }
        offset == expected.len()
    })
}

pub(crate) fn buffer_metrics(editor: &Editor<'_>) -> TextMetrics {
    editor.with_buffer(|buffer| {
        let mut metrics = TextMetrics::default();
        for (index, line) in buffer.lines.iter().enumerate() {
            let line_metrics = text_metrics(line.text());
            metrics.bytes = metrics.bytes.saturating_add(line_metrics.bytes);
            metrics.graphemes = metrics.graphemes.saturating_add(line_metrics.graphemes);
            metrics.scalars = metrics.scalars.saturating_add(line_metrics.scalars);
            if index > 0 {
                metrics.bytes = metrics.bytes.saturating_add(1);
                metrics.graphemes = metrics.graphemes.saturating_add(1);
                metrics.scalars = metrics.scalars.saturating_add(1);
            }
        }
        metrics.lines = buffer.lines.len().max(1);
        metrics
    })
}

pub(crate) fn cursor_flattened_offset(
    editor: &Editor<'_>,
    cursor: Cursor,
) -> Result<usize, InputLimitError> {
    editor.with_buffer(|buffer| {
        let text_bytes = buffer_text_byte_len(buffer);
        let Some(line) = buffer.lines.get(cursor.line) else {
            return Err(invalid_range(cursor.index, cursor.index, text_bytes));
        };
        if cursor.index > line.text().len() || !line.text().is_char_boundary(cursor.index) {
            return Err(invalid_range(cursor.index, cursor.index, text_bytes));
        }
        let before_cursor = buffer
            .lines
            .iter()
            .take(cursor.line)
            .fold(0usize, |total, line| {
                total.saturating_add(line.text().len()).saturating_add(1)
            });
        before_cursor
            .checked_add(cursor.index)
            .ok_or_else(|| allocation_failure("cursor byte offset"))
    })
}

pub(crate) fn set_cursor_at_flattened_offset(
    editor: &mut Editor<'_>,
    offset: usize,
) -> Result<(), InputLimitError> {
    let cursor = editor.with_buffer(|buffer| {
        let text_bytes = buffer_text_byte_len(buffer);
        let mut remaining = offset;
        for (line_index, line) in buffer.lines.iter().enumerate() {
            if remaining <= line.text().len() {
                if line.text().is_char_boundary(remaining) {
                    return Ok(Cursor::new(line_index, remaining));
                }
                return Err(invalid_range(offset, offset, text_bytes));
            }
            if line_index + 1 == buffer.lines.len() {
                break;
            }
            let line_with_ending = line
                .text()
                .len()
                .checked_add(1)
                .ok_or_else(|| allocation_failure("cursor byte offset"))?;
            remaining = remaining
                .checked_sub(line_with_ending)
                .ok_or_else(|| allocation_failure("cursor byte offset"))?;
        }
        Err(invalid_range(offset, offset, text_bytes))
    })?;
    editor.set_cursor(cursor);
    Ok(())
}

pub(crate) fn replacement_cursor_offset(
    start: usize,
    replacement: &str,
) -> Result<usize, InputLimitError> {
    start
        .checked_add(normalized_text_byte_len(replacement))
        .ok_or_else(|| allocation_failure("replacement cursor offset"))
}

fn buffer_text_byte_len(buffer: &Buffer) -> usize {
    buffer
        .lines
        .iter()
        .enumerate()
        .fold(0, |total, (index, line)| {
            total
                .saturating_add(line.text().len())
                .saturating_add(usize::from(index > 0))
        })
}

fn invalid_range(start: usize, end: usize, text_bytes: usize) -> InputLimitError {
    InputLimitError::InvalidUtf8Range {
        start,
        end,
        text_bytes,
    }
}

fn allocation_failure(operation: &'static str) -> InputLimitError {
    InputLimitError::AllocationFailed {
        requested_bytes: usize::MAX,
        operation,
    }
}
