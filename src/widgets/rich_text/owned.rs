// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::borrow::Cow;

use super::{RichText, RichTextSpan, RichTextSpanStyle, RichTextStyle};
use crate::text_controls::{TextControlNormalizer, TextControlPolicy};

impl<'a> RichText<'a> {
    /// Converts every span to owned storage. Example: `RichText::plain("Hi").into_owned()`.
    pub fn into_owned(self) -> RichText<'static> {
        let spans = self
            .spans
            .into_iter()
            .map(|span| RichTextSpan {
                text: Cow::Owned(span.text.into_owned()),
                style: span.style,
            })
            .collect();
        RichText {
            spans,
            default_style: self.default_style,
        }
    }

    pub(crate) fn to_owned_spec(&self) -> OwnedRichTextSpec {
        OwnedRichTextSpec {
            spans: self.normalized_owned_spans(),
            default_style: self.default_style,
        }
    }

    fn normalized_owned_spans(&self) -> Vec<OwnedRichTextSpan> {
        // A CRLF pair can cross a style boundary, so fragment-local cleanup would duplicate it.
        let mut normalizer = TextControlNormalizer::new(TextControlPolicy::PreserveLineBreaks);
        self.spans
            .iter()
            .map(|span| normalized_owned_span(span, &mut normalizer))
            .collect()
    }
}

fn normalized_owned_span(
    span: &RichTextSpan<'_>,
    normalizer: &mut TextControlNormalizer,
) -> OwnedRichTextSpan {
    let mut text = String::with_capacity(span.text().len());
    normalizer.append_text(span.text(), &mut text);
    OwnedRichTextSpan {
        text,
        style: *span.style(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OwnedRichTextSpec {
    pub(super) spans: Vec<OwnedRichTextSpan>,
    pub(super) default_style: RichTextStyle,
}

impl OwnedRichTextSpec {
    pub(crate) fn spans(&self) -> &[OwnedRichTextSpan] {
        &self.spans
    }

    pub(crate) const fn default_style(&self) -> &RichTextStyle {
        &self.default_style
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OwnedRichTextSpan {
    pub(super) text: String,
    pub(super) style: RichTextSpanStyle,
}

impl OwnedRichTextSpan {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) const fn style(&self) -> &RichTextSpanStyle {
        &self.style
    }
}
