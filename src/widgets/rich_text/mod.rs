// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::borrow::Cow;
use std::fmt;

mod owned;

use owned::OwnedRichTextSpan;
#[doc(hidden)]
pub use owned::OwnedRichTextSpec;

/// Reports a value outside the supported rich-text model bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RichTextError {
    /// The requested logical-pixel size is not supported.
    InvalidSize { value: f32 },
    /// The requested typographic weight is not supported.
    InvalidWeight { value: u16 },
}

impl fmt::Display for RichTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { value } => write!(
                formatter,
                "invalid rich text size {value:?}, expected a finite value in 0 < value <= 4096"
            ),
            Self::InvalidWeight { value } => write!(
                formatter,
                "invalid rich text weight {value}, expected an integer in 1..=1000"
            ),
        }
    }
}

impl std::error::Error for RichTextError {}

/// A validated rich-text size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RichTextSize(f32);

impl RichTextSize {
    /// The standard inherited text size.
    pub const DEFAULT: Self = Self(16.0);
    /// The largest accepted logical-pixel size.
    pub const MAX: f32 = 4096.0;

    /// Validates a finite, positive size. Example: `RichTextSize::new(18.0).unwrap()`.
    pub fn new(value: f32) -> Result<Self, RichTextError> {
        if !value.is_finite() || value <= 0.0 || value > Self::MAX {
            return Err(RichTextError::InvalidSize { value });
        }
        Ok(Self(value))
    }
    /// Returns the validated size. Example: `assert_eq!(RichTextSize::DEFAULT.get(), 16.0);`
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// A validated typographic weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RichTextWeight(u16);

impl RichTextWeight {
    /// The standard text weight.
    pub const NORMAL: Self = Self(400);
    /// A medium emphasis weight.
    pub const MEDIUM: Self = Self(500);
    /// A semi-bold emphasis weight.
    pub const SEMI_BOLD: Self = Self(600);
    /// A bold emphasis weight.
    pub const BOLD: Self = Self(700);

    /// Validates a weight from 1 through 1000. Example: `RichTextWeight::new(350).unwrap()`.
    pub fn new(value: u16) -> Result<Self, RichTextError> {
        if !(1..=1000).contains(&value) {
            return Err(RichTextError::InvalidWeight { value });
        }
        Ok(Self(value))
    }
    /// Returns the validated weight. Example: `assert_eq!(RichTextWeight::BOLD.get(), 700);`
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Selects upright or italic glyph forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RichTextSlant {
    /// Uses upright glyph forms.
    #[default]
    Upright,
    /// Uses italic glyph forms.
    Italic,
}

/// A project-owned red, green, blue, and alpha color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RichTextColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl RichTextColor {
    /// Creates an opaque RGB color. Example: `RichTextColor::rgb(1, 2, 3)`.
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba(red, green, blue, u8::MAX)
    }
    /// Creates an RGBA color. Example: `RichTextColor::rgba(1, 2, 3, 4)`.
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
    /// Returns red. Example: `assert_eq!(RichTextColor::rgb(9, 8, 7).red(), 9);`
    pub const fn red(self) -> u8 {
        self.red
    }
    /// Returns green. Example: `assert_eq!(RichTextColor::rgb(9, 8, 7).green(), 8);`
    pub const fn green(self) -> u8 {
        self.green
    }

    /// Returns blue. Example: `assert_eq!(RichTextColor::rgb(9, 8, 7).blue(), 7);`
    pub const fn blue(self) -> u8 {
        self.blue
    }

    /// Returns alpha. Example: `assert_eq!(RichTextColor::rgba(1, 2, 3, 4).alpha(), 4);`
    pub const fn alpha(self) -> u8 {
        self.alpha
    }
}

/// Complete defaults inherited by spans without matching overrides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RichTextStyle {
    size: RichTextSize,
    color: Option<RichTextColor>,
    weight: RichTextWeight,
    slant: RichTextSlant,
    underline: bool,
}

impl Default for RichTextStyle {
    fn default() -> Self {
        Self {
            size: RichTextSize::DEFAULT,
            color: None,
            weight: RichTextWeight::NORMAL,
            slant: RichTextSlant::Upright,
            underline: false,
        }
    }
}

impl RichTextStyle {
    /// Sets inherited size. Example: `RichTextStyle::default().with_size(RichTextSize::DEFAULT)`.
    #[must_use]
    pub const fn with_size(mut self, size: RichTextSize) -> Self {
        self.size = size;
        self
    }

    /// Sets inherited color. Example: `RichTextStyle::default().with_color(RichTextColor::rgb(1, 2, 3))`.
    #[must_use]
    pub const fn with_color(mut self, color: RichTextColor) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets inherited weight. Example: `RichTextStyle::default().with_weight(RichTextWeight::BOLD)`.
    #[must_use]
    pub const fn with_weight(mut self, weight: RichTextWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Sets inherited slant. Example: `RichTextStyle::default().with_slant(RichTextSlant::Italic)`.
    #[must_use]
    pub const fn with_slant(mut self, slant: RichTextSlant) -> Self {
        self.slant = slant;
        self
    }

    /// Sets inherited underline. Example: `RichTextStyle::default().with_underline(true)`.
    #[must_use]
    pub const fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    /// Returns inherited size. Example: `RichTextStyle::default().size()`.
    pub const fn size(&self) -> RichTextSize {
        self.size
    }

    /// Returns inherited color. Example: `assert_eq!(RichTextStyle::default().color(), None);`
    pub const fn color(&self) -> Option<RichTextColor> {
        self.color
    }

    /// Returns inherited weight. Example: `RichTextStyle::default().weight()`.
    pub const fn weight(&self) -> RichTextWeight {
        self.weight
    }

    /// Returns inherited slant. Example: `RichTextStyle::default().slant()`.
    pub const fn slant(&self) -> RichTextSlant {
        self.slant
    }

    /// Returns inherited underline. Example: `RichTextStyle::default().underline()`.
    pub const fn underline(&self) -> bool {
        self.underline
    }
}

/// Optional per-span overrides applied over [`RichTextStyle`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RichTextSpanStyle {
    size: Option<RichTextSize>,
    color: Option<Option<RichTextColor>>,
    weight: Option<RichTextWeight>,
    slant: Option<RichTextSlant>,
    underline: Option<bool>,
}

impl RichTextSpanStyle {
    /// Overrides size. Example: `RichTextSpanStyle::default().with_size(RichTextSize::DEFAULT)`.
    #[must_use]
    pub const fn with_size(mut self, size: RichTextSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Overrides color. Example: `RichTextSpanStyle::default().with_color(RichTextColor::rgb(1, 2, 3))`.
    #[must_use]
    pub const fn with_color(mut self, color: RichTextColor) -> Self {
        self.color = Some(Some(color));
        self
    }

    /// Overrides an inherited color with the runtime theme color. Example: `RichTextSpanStyle::default().with_theme_color()`.
    #[must_use]
    pub const fn with_theme_color(mut self) -> Self {
        self.color = Some(None);
        self
    }

    /// Overrides weight. Example: `RichTextSpanStyle::default().with_weight(RichTextWeight::BOLD)`.
    #[must_use]
    pub const fn with_weight(mut self, weight: RichTextWeight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Overrides slant. Example: `RichTextSpanStyle::default().with_slant(RichTextSlant::Italic)`.
    #[must_use]
    pub const fn with_slant(mut self, slant: RichTextSlant) -> Self {
        self.slant = Some(slant);
        self
    }

    /// Overrides underline. Example: `RichTextSpanStyle::default().with_underline(false)`.
    #[must_use]
    pub const fn with_underline(mut self, underline: bool) -> Self {
        self.underline = Some(underline);
        self
    }

    /// Returns size override. Example: `RichTextSpanStyle::default().size()`.
    pub const fn size(&self) -> Option<RichTextSize> {
        self.size
    }

    /// Returns color override. Example: `RichTextSpanStyle::default().color()`.
    pub const fn color(&self) -> Option<RichTextColor> {
        self.color.flatten()
    }

    /// Reports an explicit theme-color reset. Example: `RichTextSpanStyle::default().with_theme_color().uses_theme_color()`.
    pub const fn uses_theme_color(&self) -> bool {
        matches!(self.color, Some(None))
    }

    pub(crate) const fn color_override(&self) -> Option<Option<RichTextColor>> {
        self.color
    }

    /// Returns weight override. Example: `RichTextSpanStyle::default().weight()`.
    pub const fn weight(&self) -> Option<RichTextWeight> {
        self.weight
    }

    /// Returns slant override. Example: `RichTextSpanStyle::default().slant()`.
    pub const fn slant(&self) -> Option<RichTextSlant> {
        self.slant
    }

    /// Returns underline override. Example: `RichTextSpanStyle::default().underline()`.
    pub const fn underline(&self) -> Option<bool> {
        self.underline
    }
}

/// One borrowed or owned text fragment and its style overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct RichTextSpan<'a> {
    text: Cow<'a, str>,
    style: RichTextSpanStyle,
}

impl<'a> RichTextSpan<'a> {
    /// Creates a span from borrowed or owned text. Example: `RichTextSpan::new("Hello")`.
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            text: text.into(),
            style: RichTextSpanStyle::default(),
        }
    }

    /// Creates a borrowed span. Example: `RichTextSpan::borrowed("Hello")`.
    pub fn borrowed(text: &'a str) -> Self {
        Self::new(Cow::Borrowed(text))
    }

    /// Creates an owned span. Example: `RichTextSpan::owned(String::from("Hello"))`.
    pub fn owned(text: impl Into<String>) -> Self {
        Self::new(Cow::Owned(text.into()))
    }

    /// Replaces all overrides. Example: `RichTextSpan::new("Hi").with_style(Default::default())`.
    #[must_use]
    pub fn with_style(mut self, style: RichTextSpanStyle) -> Self {
        self.style = style;
        self
    }

    /// Overrides size. Example: `RichTextSpan::new("Hi").with_size(RichTextSize::DEFAULT)`.
    #[must_use]
    pub fn with_size(mut self, size: RichTextSize) -> Self {
        self.style = self.style.with_size(size);
        self
    }

    /// Overrides color. Example: `RichTextSpan::new("Hi").with_color(RichTextColor::rgb(1, 2, 3))`.
    #[must_use]
    pub fn with_color(mut self, color: RichTextColor) -> Self {
        self.style = self.style.with_color(color);
        self
    }

    /// Resets inherited color to the runtime theme. Example: `RichTextSpan::new("Hi").with_theme_color()`.
    #[must_use]
    pub fn with_theme_color(mut self) -> Self {
        self.style = self.style.with_theme_color();
        self
    }

    /// Overrides weight. Example: `RichTextSpan::new("Hi").with_weight(RichTextWeight::MEDIUM)`.
    #[must_use]
    pub fn with_weight(mut self, weight: RichTextWeight) -> Self {
        self.style = self.style.with_weight(weight);
        self
    }

    /// Applies bold. Example: `RichTextSpan::new("Hi").bold()`.
    #[must_use]
    pub fn bold(self) -> Self {
        self.with_weight(RichTextWeight::BOLD)
    }

    /// Applies italic. Example: `RichTextSpan::new("Hi").italic()`.
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.style = self.style.with_slant(RichTextSlant::Italic);
        self
    }

    /// Resets inherited italic slant. Example: `RichTextSpan::new("Hi").upright()`.
    #[must_use]
    pub fn upright(mut self) -> Self {
        self.style = self.style.with_slant(RichTextSlant::Upright);
        self
    }

    /// Enables underline. Example: `RichTextSpan::new("Hi").underline()`.
    #[must_use]
    pub fn underline(mut self) -> Self {
        self.style = self.style.with_underline(true);
        self
    }

    /// Resets inherited underline. Example: `RichTextSpan::new("Hi").without_underline()`.
    #[must_use]
    pub fn without_underline(mut self) -> Self {
        self.style = self.style.with_underline(false);
        self
    }

    /// Returns text. Example: `assert_eq!(RichTextSpan::new("Hi").text(), "Hi");`
    pub fn text(&self) -> &str {
        self.text.as_ref()
    }

    /// Returns overrides. Example: `RichTextSpan::new("Hi").style()`.
    pub const fn style(&self) -> &RichTextSpanStyle {
        &self.style
    }
}

/// Ordered rich-text spans and the complete style they inherit.
#[derive(Debug, Clone, PartialEq)]
pub struct RichText<'a> {
    spans: Vec<RichTextSpan<'a>>,
    default_style: RichTextStyle,
}

impl<'a> RichText<'a> {
    /// Creates one unstyled span. Example: `RichText::plain("Hello")`.
    pub fn plain(text: impl Into<Cow<'a, str>>) -> Self {
        Self::from_span(RichTextSpan::new(text))
    }

    /// Creates one supplied span. Example: `RichText::from_span(RichTextSpan::new("Hello"))`.
    pub fn from_span(span: RichTextSpan<'a>) -> Self {
        Self::from_spans([span])
    }

    /// Creates ordered spans. Example: `RichText::from_spans([RichTextSpan::new("Hello")])`.
    pub fn from_spans(spans: impl IntoIterator<Item = RichTextSpan<'a>>) -> Self {
        Self {
            spans: spans.into_iter().collect(),
            default_style: RichTextStyle::default(),
        }
    }

    /// Appends without separators. Example: `text.push_span(RichTextSpan::new("B"))`.
    pub fn push_span(&mut self, span: RichTextSpan<'a>) {
        self.spans.push(span);
    }

    /// Replaces inherited style. Example: `RichText::plain("Hi").with_default_style(Default::default())`.
    #[must_use]
    pub fn with_default_style(mut self, default_style: RichTextStyle) -> Self {
        self.default_style = default_style;
        self
    }

    /// Returns ordered spans. Example: `RichText::plain("Hi").spans()`.
    pub fn spans(&self) -> &[RichTextSpan<'a>] {
        &self.spans
    }

    /// Returns inherited style. Example: `RichText::plain("Hi").default_style()`.
    pub const fn default_style(&self) -> &RichTextStyle {
        &self.default_style
    }

    /// Concatenates exactly. Example: `assert_eq!(RichText::plain("Hi").plain_text(), "Hi");`
    pub fn plain_text(&self) -> String {
        self.spans.iter().map(RichTextSpan::text).collect()
    }

    /// Checks concatenated content. Example: `assert!(RichText::plain("").is_empty());`
    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|span| span.text().is_empty())
    }

    pub(crate) fn to_owned_spec(&self) -> OwnedRichTextSpec {
        let spans = self
            .spans
            .iter()
            .map(|span| OwnedRichTextSpan {
                text: span.text().to_owned(),
                style: *span.style(),
            })
            .collect();
        OwnedRichTextSpec {
            spans,
            default_style: self.default_style,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/rich_text_unit_tests.rs"]
mod tests;
