// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::borrow::Cow;

use super::{RichText, RichTextSpan, RichTextSpanStyle, RichTextStyle};

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
