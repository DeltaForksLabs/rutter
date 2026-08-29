// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Canonical text-control handling shared by layout, editing, and painting.

use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextControlPolicy {
    PreserveLineBreaks,
    FlattenLineBreaks,
}

impl TextControlPolicy {
    pub(crate) const fn from_allows_line_breaks(allows_line_breaks: bool) -> Self {
        if allows_line_breaks {
            Self::PreserveLineBreaks
        } else {
            Self::FlattenLineBreaks
        }
    }
}

/// Normalizes fragments while retaining CRLF state across rich-text spans.
pub(crate) struct TextControlNormalizer {
    policy: TextControlPolicy,
    previous_line_ending: Option<char>,
}

impl TextControlNormalizer {
    pub(crate) const fn new(policy: TextControlPolicy) -> Self {
        Self {
            policy,
            previous_line_ending: None,
        }
    }

    pub(crate) fn append_text(&mut self, text: &str, normalized: &mut String) {
        for character in text.chars() {
            self.append_character(character, normalized);
        }
    }

    pub(crate) fn append_character(&mut self, character: char, normalized: &mut String) {
        if is_line_separator(character) {
            self.append_line_separator(character, normalized);
            return;
        }
        self.previous_line_ending = None;
        append_non_line_control(character, normalized);
    }

    fn append_line_separator(&mut self, character: char, normalized: &mut String) {
        if self
            .previous_line_ending
            .is_some_and(|previous| is_crlf_pair(previous, character))
        {
            self.previous_line_ending = None;
            return;
        }
        normalized.push(self.line_break_replacement());
        self.previous_line_ending = (character == '\r').then_some(character);
    }

    fn line_break_replacement(&self) -> char {
        match self.policy {
            TextControlPolicy::PreserveLineBreaks => '\n',
            TextControlPolicy::FlattenLineBreaks => ' ',
        }
    }
}

pub(crate) fn normalize_text_controls<'text>(
    text: &'text str,
    policy: TextControlPolicy,
) -> Cow<'text, str> {
    if !text
        .chars()
        .any(|character| requires_normalization(character, policy))
    {
        return Cow::Borrowed(text);
    }
    let mut normalized = String::with_capacity(text.len());
    TextControlNormalizer::new(policy).append_text(text, &mut normalized);
    Cow::Owned(normalized)
}

fn append_non_line_control(character: char, normalized: &mut String) {
    if character == '\t' {
        normalized.push(' ');
    } else if !character.is_control() {
        normalized.push(character);
    }
}

fn requires_normalization(character: char, policy: TextControlPolicy) -> bool {
    match policy {
        TextControlPolicy::PreserveLineBreaks => {
            character != '\n' && (is_line_separator(character) || character.is_control())
        }
        TextControlPolicy::FlattenLineBreaks => {
            is_line_separator(character) || character.is_control()
        }
    }
}

fn is_line_separator(character: char) -> bool {
    matches!(
        character,
        '\r' | '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

fn is_crlf_pair(first: char, second: char) -> bool {
    matches!((first, second), ('\r', '\n'))
}

#[cfg(test)]
mod tests {
    use super::{TextControlNormalizer, TextControlPolicy, normalize_text_controls};

    #[test]
    fn multiline_policy_canonicalizes_line_endings_tabs_and_controls() {
        let raw = "a\r\nb\rc\u{0085}d\u{2028}e\u{2029}f\tg\u{0000}h";

        assert_eq!(
            normalize_text_controls(raw, TextControlPolicy::PreserveLineBreaks),
            "a\nb\nc\nd\ne\nf gh"
        );
    }

    #[test]
    fn single_line_policy_flattens_every_line_separator() {
        let raw = "a\r\nb\rc\nd\u{0085}e\u{2028}f\u{2029}g\th\u{0008}i";

        assert_eq!(
            normalize_text_controls(raw, TextControlPolicy::FlattenLineBreaks),
            "a b c d e f g hi"
        );
    }

    #[test]
    fn stateful_normalizer_collapses_crlf_split_between_fragments() {
        let mut normalized = String::new();
        let mut normalizer = TextControlNormalizer::new(TextControlPolicy::PreserveLineBreaks);

        normalizer.append_text("first\r", &mut normalized);
        normalizer.append_text("\nsecond\n\rthird", &mut normalized);

        assert_eq!(normalized, "first\nsecond\n\nthird");
    }

    #[test]
    fn unchanged_multiline_text_borrows_the_source() {
        let normalized =
            normalize_text_controls("first\nsecond", TextControlPolicy::PreserveLineBreaks);

        assert!(matches!(
            normalized,
            std::borrow::Cow::Borrowed("first\nsecond")
        ));
    }
}
