// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Input-state normalization that preserves sensitive temporary handling.

use std::borrow::Cow;

use zeroize::Zeroize;

use crate::text_controls::{TextControlPolicy, normalize_text_controls};

pub(crate) fn normalized_input_text<'text>(text: &'text str) -> Cow<'text, str> {
    normalize_text_controls(text, TextControlPolicy::PreserveLineBreaks)
}

pub(crate) fn zeroize_normalized_input_text(sensitive: bool, normalized: &mut Cow<'_, str>) {
    if sensitive && let Cow::Owned(text) = normalized {
        text.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{normalized_input_text, zeroize_normalized_input_text};

    #[test]
    fn input_state_normalization_preserves_line_breaks_for_limit_validation() {
        let raw = "first\r\nsecond\tthird\u{0000}";

        assert_eq!(normalized_input_text(raw), "first\nsecond third");
    }

    #[test]
    fn sensitive_normalized_text_is_zeroized_after_use() {
        let mut normalized = Cow::Owned(String::from("secret"));

        zeroize_normalized_input_text(true, &mut normalized);

        assert!(normalized.is_empty());
    }
}
