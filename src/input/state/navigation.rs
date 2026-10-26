// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use cosmic_text::{Action, Cursor, Edit, FontSystem, Motion};
use unicode_segmentation::UnicodeSegmentation;

use super::InputWidgetState;

impl InputWidgetState {
    /// Moves to a Unicode word's start, skipping separators and empty lines.
    /// Left/right follow paragraph direction, like the editor's plain arrows.
    pub(crate) fn move_word_start(&mut self, fonts: &mut FontSystem, right: bool) {
        self.normalize_cursor();
        let cursor = self.editor.cursor();
        let target = self.editor.with_buffer_mut(|buffer| {
            let rtl = buffer
                .line_shape(fonts, cursor.line)
                .is_some_and(|shape| shape.rtl);
            let forward = right != rtl;
            let mut starts = buffer.lines.iter().enumerate().flat_map(|(line, text)| {
                text.text()
                    .unicode_word_indices()
                    .map(move |(index, _)| Cursor::new(line, index))
            });
            let position = (cursor.line, cursor.index);
            if forward {
                starts
                    .find(|start| (start.line, start.index) > position)
                    .unwrap_or_else(|| {
                        let line = buffer.lines.len().saturating_sub(1);
                        Cursor::new(line, buffer.lines[line].text().len())
                    })
            } else {
                starts
                    .take_while(|start| (start.line, start.index) < position)
                    .last()
                    .unwrap_or(Cursor::new(0, 0))
            }
        });
        // A horizontal move must reset the editor's preferred vertical column.
        self.editor.action(fonts, Action::Motion(Motion::Home));
        self.editor.set_cursor(target);
    }

    /// Pages by complete wrapped visual rows in the editable body viewport.
    /// Keep the unbounded document buffer and the editor's preferred column.
    pub(crate) fn move_page(&mut self, fonts: &mut FontSystem, height: f32, down: bool) {
        self.normalize_cursor();
        let line_height = self
            .editor
            .with_buffer(|buffer| buffer.metrics().line_height);
        let rows = (height / line_height).floor().max(1.0) as usize;
        let before = self.editor.cursor();
        // Cosmic-text cannot always resolve a trailing wrapped-line end to a
        // layout cursor. Do not let a further page wrap back to the first row.
        if self.cursor_at_document_boundary(down) {
            return;
        }
        let motion = if down { Motion::Down } else { Motion::Up };
        for _ in 0..rows {
            if self.cursor_at_document_boundary(down) {
                break;
            }
            let previous = self.editor.cursor();
            self.editor.action(fonts, Action::Motion(motion));
            if self.editor.cursor() == previous {
                break;
            }
        }
        if self.editor.cursor() == before {
            self.editor.action(
                fonts,
                Action::Motion(if down {
                    Motion::BufferEnd
                } else {
                    Motion::BufferStart
                }),
            );
        }
        let distance = rows as f32 * line_height;
        self.scroll_y = (self.scroll_y + if down { distance } else { -distance }).max(0.0);
    }

    fn cursor_at_document_boundary(&self, end: bool) -> bool {
        let cursor = self.editor.cursor();
        self.editor.with_buffer(|buffer| {
            if end {
                cursor.line + 1 == buffer.lines.len()
                    && cursor.index == buffer.lines[cursor.line].text().len()
            } else {
                cursor.line == 0 && cursor.index == 0
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_starts_skip_punctuation_spaces_and_empty_lines_in_both_directions() {
        let mut fonts = FontSystem::new();
        let mut state = InputWidgetState::new(&mut fonts);
        let text = "one,  two!\n\n  cafe\u{301} 世界";
        state.set_text(&mut fonts, text);
        state.editor.set_cursor(Cursor::new(0, 0));
        let starts = [0, 6, 14, 21, 24, 27];
        for expected in starts.into_iter().skip(1) {
            state.move_word_start(&mut fonts, true);
            assert_eq!(state.cursor_byte_index(), expected);
            assert!(text.is_char_boundary(expected));
        }
        state.move_word_start(&mut fonts, true);
        assert_eq!(state.cursor_byte_index(), text.len());
        for expected in starts.into_iter().rev().skip(1) {
            state.move_word_start(&mut fonts, false);
            assert_eq!(state.cursor_byte_index(), expected);
        }
        state.editor.set_cursor(Cursor::new(0, 2));
        state.move_word_start(&mut fonts, false);
        assert_eq!(state.cursor_byte_index(), 0);
    }

    #[test]
    fn rtl_word_arrows_follow_paragraph_direction_and_keep_combining_marks() {
        let mut fonts = FontSystem::new();
        let mut state = InputWidgetState::new(&mut fonts);
        state.set_text(&mut fonts, "שָׁלוֹם עולם");
        state.editor.set_cursor(Cursor::new(0, 0));
        state.move_word_start(&mut fonts, false);
        assert_eq!(state.cursor_byte_index(), "שָׁלוֹם ".len());
        state.move_word_start(&mut fonts, true);
        assert_eq!(state.cursor_byte_index(), 0);
    }

    #[test]
    fn empty_and_separator_only_documents_have_bounded_navigation() {
        let mut fonts = FontSystem::new();
        let mut state = InputWidgetState::new(&mut fonts);
        for text in ["", "a", "  ...\n\n  "] {
            state.set_text(&mut fonts, text);
            state.move_word_start(&mut fonts, false);
            assert_eq!(state.cursor_byte_index(), 0);
            state.move_word_start(&mut fonts, true);
            assert_eq!(state.cursor_byte_index(), text.len());
            state.sync_layout(&mut fonts, 1.0, 14.0, true);
            state.move_page(&mut fonts, 0.0, false);
            state.move_page(&mut fonts, 0.0, true);
            assert!(state.cursor_byte_index() <= text.len());
        }
    }

    #[test]
    fn page_uses_visual_rows_and_retains_preferred_column_across_short_lines() {
        let mut fonts = FontSystem::new();
        let mut state = InputWidgetState::new(&mut fonts);
        state.set_text(&mut fonts, "abcdef\nx\nabcdef\nabcdef\nabcdef\nabcdef");
        state.sync_layout(&mut fonts, 800.0, 14.0, true);
        state.editor.set_cursor(Cursor::new(0, 4));
        state.move_page(&mut fonts, 14.0 * 1.3, true);
        assert_eq!(state.editor.cursor().line, 1);
        state.move_page(&mut fonts, 14.0 * 1.3 * 2.0, true);
        assert_eq!(
            (state.editor.cursor().line, state.editor.cursor().index),
            (3, 4)
        );
        state.move_page(&mut fonts, 14.0 * 1.3 * 2.0, false);
        state.move_page(&mut fonts, 14.0 * 1.3, false);
        assert_eq!(
            (state.editor.cursor().line, state.editor.cursor().index),
            (0, 4)
        );
    }

    #[test]
    fn wrapped_page_matches_repeated_visual_row_motion_without_bounding_buffer() {
        let mut fonts = FontSystem::new();
        let mut state = InputWidgetState::new(&mut fonts);
        state.set_text(&mut fonts, &"word word word word ".repeat(20));
        state.sync_layout(&mut fonts, 100.0, 14.0, true);
        state.editor.set_cursor(Cursor::new(0, 1));
        let mut expected = state.editor.clone();
        for _ in 0..3 {
            expected.action(&mut fonts, Action::Motion(Motion::Down));
        }
        state.move_page(&mut fonts, 14.0 * 1.3 * 3.1, true);
        assert_eq!(state.editor.cursor(), expected.cursor());
        assert_eq!(state.editor.cursor().line, 0);
        assert!(state.editor.cursor().index > 1);
        state
            .editor
            .with_buffer(|buffer| assert_eq!(buffer.size().1, None));
        state.update_scroll(&mut fonts, 100.0, 14.0 * 1.3 * 3.1, 14.0, false, true);
        assert!(state.scroll_y > 0.0);
        for _ in 0..30 {
            let before = state.cursor_byte_index();
            state.move_page(&mut fonts, 14.0 * 1.3 * 3.1, true);
            assert!(
                state.cursor_byte_index() >= before,
                "page down moved backward: {before} -> {}",
                state.cursor_byte_index()
            );
        }
        assert_eq!(state.cursor_byte_index(), state.text().len());
        for _ in 0..30 {
            state.move_page(&mut fonts, 14.0 * 1.3 * 3.1, false);
        }
        assert_eq!(state.cursor_byte_index(), 0);
    }

    #[test]
    fn rtl_pages_in_narrow_wide_and_short_viewports_keep_caret_visible() {
        let mut fonts = FontSystem::new();
        for width in [320.0, 800.0] {
            for height in [20.0, 80.0] {
                let mut state = InputWidgetState::new(&mut fonts);
                let text = "שָׁלוֹם עולם מילים ארוכות נוספות ".repeat(40);
                state.set_text(&mut fonts, &text);
                state.sync_layout(&mut fonts, width, 14.0, true);
                state.editor.set_cursor(Cursor::new(0, 0));
                for _ in 0..8 {
                    state.move_page(&mut fonts, height, true);
                    state.update_scroll(&mut fonts, width, height, 14.0, false, true);
                    let (_, top) = state.editor.cursor_position().expect("shaped caret");
                    assert!(top as f32 + 1.0 >= state.scroll_y);
                    assert!(top as f32 + 14.0 * 1.3 <= state.scroll_y + height + 1.0);
                    assert!(text.is_char_boundary(state.cursor_byte_index()));
                }
                assert!(state.scroll_y > 0.0);
            }
        }
    }
}
