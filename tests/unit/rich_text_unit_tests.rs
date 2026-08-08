use std::borrow::Cow;

use super::*;

#[test]
fn size_accepts_boundaries_and_rejects_values_outside_them() {
    assert_eq!(
        RichTextSize::new(f32::MIN_POSITIVE).unwrap().get(),
        f32::MIN_POSITIVE
    );
    assert_eq!(RichTextSize::new(RichTextSize::MAX).unwrap().get(), 4096.0);

    for value in [
        0.0,
        -1.0,
        4096.1,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        let error = RichTextSize::new(value).unwrap_err();
        let RichTextError::InvalidSize { value: offending } = error else {
            panic!("expected invalid size, got {error:?}");
        };
        if value.is_nan() {
            assert!(offending.is_nan());
        } else {
            assert_eq!(offending, value);
        }
        assert!(error.to_string().contains(&format!("{value:?}")));
        assert!(
            error
                .to_string()
                .contains("finite value in 0 < value <= 4096")
        );
    }
}

#[test]
fn weight_accepts_boundaries_and_rejects_values_outside_them() {
    assert_eq!(RichTextWeight::new(1).unwrap().get(), 1);
    assert_eq!(RichTextWeight::new(1000).unwrap().get(), 1000);

    for value in [0, 1001, u16::MAX] {
        let error = RichTextWeight::new(value).unwrap_err();
        assert_eq!(error, RichTextError::InvalidWeight { value });
        assert!(error.to_string().contains(&value.to_string()));
        assert!(error.to_string().contains("integer in 1..=1000"));
    }
}

#[test]
fn named_size_and_weight_values_match_typographic_defaults() {
    assert_eq!(RichTextSize::DEFAULT.get(), 16.0);
    assert_eq!(RichTextWeight::NORMAL.get(), 400);
    assert_eq!(RichTextWeight::MEDIUM.get(), 500);
    assert_eq!(RichTextWeight::SEMI_BOLD.get(), 600);
    assert_eq!(RichTextWeight::BOLD.get(), 700);
}

#[test]
fn colors_preserve_all_channels_and_rgb_is_opaque() {
    let opaque = RichTextColor::rgb(12, 34, 56);
    let translucent = RichTextColor::rgba(90, 80, 70, 60);

    assert_eq!((opaque.red(), opaque.green(), opaque.blue()), (12, 34, 56));
    assert_eq!(opaque.alpha(), 255);
    assert_eq!(
        (
            translucent.red(),
            translucent.green(),
            translucent.blue(),
            translucent.alpha(),
        ),
        (90, 80, 70, 60)
    );
}

#[test]
fn complete_style_defaults_allow_runtime_color_fallback() {
    let style = RichTextStyle::default();

    assert_eq!(style.size(), RichTextSize::DEFAULT);
    assert_eq!(style.color(), None);
    assert_eq!(style.weight(), RichTextWeight::NORMAL);
    assert_eq!(style.slant(), RichTextSlant::Upright);
    assert!(!style.underline());
    assert_eq!(RichTextSlant::default(), RichTextSlant::Upright);
}

#[test]
fn complete_style_builders_replace_every_inherited_value() {
    let size = RichTextSize::new(24.0).unwrap();
    let color = RichTextColor::rgba(1, 2, 3, 4);
    let weight = RichTextWeight::new(850).unwrap();
    let style = RichTextStyle::default()
        .with_size(size)
        .with_color(color)
        .with_weight(weight)
        .with_slant(RichTextSlant::Italic)
        .with_underline(true);

    assert_eq!(style.size(), size);
    assert_eq!(style.color(), Some(color));
    assert_eq!(style.weight(), weight);
    assert_eq!(style.slant(), RichTextSlant::Italic);
    assert!(style.underline());
}

#[test]
fn span_style_keeps_each_inherited_value_optional() {
    let inherited = RichTextSpanStyle::default();
    assert_eq!(inherited.size(), None);
    assert_eq!(inherited.color(), None);
    assert_eq!(inherited.weight(), None);
    assert_eq!(inherited.slant(), None);
    assert_eq!(inherited.underline(), None);

    let size = RichTextSize::new(20.0).unwrap();
    let color = RichTextColor::rgb(5, 6, 7);
    let overrides = inherited
        .with_size(size)
        .with_color(color)
        .with_weight(RichTextWeight::MEDIUM)
        .with_slant(RichTextSlant::Italic)
        .with_underline(false);

    assert_eq!(overrides.size(), Some(size));
    assert_eq!(overrides.color(), Some(color));
    assert_eq!(overrides.weight(), Some(RichTextWeight::MEDIUM));
    assert_eq!(overrides.slant(), Some(RichTextSlant::Italic));
    assert_eq!(overrides.underline(), Some(false));
}

#[test]
fn span_can_reset_an_inherited_color_to_the_runtime_theme() {
    let span = RichTextSpan::new("theme").with_theme_color();

    assert_eq!(span.style().color(), None);
    assert!(span.style().uses_theme_color());
}

#[test]
fn spans_retain_borrowed_and_owned_text() {
    let borrowed_source = String::from("borrowed");
    let borrowed = RichTextSpan::borrowed(&borrowed_source);
    let owned = RichTextSpan::owned(String::from("owned"));
    let generic_borrowed = RichTextSpan::new(Cow::Borrowed("generic"));

    assert!(matches!(borrowed.text, Cow::Borrowed(_)));
    assert!(matches!(owned.text, Cow::Owned(_)));
    assert_eq!(borrowed.text(), "borrowed");
    assert_eq!(owned.text(), "owned");
    assert_eq!(generic_borrowed.text(), "generic");
}

#[test]
fn span_builders_apply_emphasis_and_explicit_resets() {
    let emphasized = RichTextSpan::new("styled")
        .with_size(RichTextSize::new(22.0).unwrap())
        .with_color(RichTextColor::rgb(10, 20, 30))
        .with_weight(RichTextWeight::MEDIUM)
        .bold()
        .italic()
        .underline();

    assert_eq!(emphasized.style().size().unwrap().get(), 22.0);
    assert_eq!(
        emphasized.style().color(),
        Some(RichTextColor::rgb(10, 20, 30))
    );
    assert_eq!(emphasized.style().weight(), Some(RichTextWeight::BOLD));
    assert_eq!(emphasized.style().slant(), Some(RichTextSlant::Italic));
    assert_eq!(emphasized.style().underline(), Some(true));

    let reset = emphasized.upright().without_underline();
    assert_eq!(reset.style().slant(), Some(RichTextSlant::Upright));
    assert_eq!(reset.style().underline(), Some(false));
}

#[test]
fn with_style_replaces_existing_span_overrides() {
    let replacement = RichTextSpanStyle::default().with_underline(true);
    let span = RichTextSpan::new("replacement")
        .bold()
        .with_style(replacement);

    assert_eq!(span.style(), &replacement);
    assert_eq!(span.style().weight(), None);
}

#[test]
fn plain_text_concatenates_exact_content_without_normalization() {
    let mut text = RichText::from_spans([
        RichTextSpan::new(""),
        RichTextSpan::new("first\n"),
        RichTextSpan::new("مرحبا"),
    ]);
    text.push_span(RichTextSpan::owned(String::from("👩🏽‍💻")));
    text.push_span(RichTextSpan::new("\nlast"));

    assert_eq!(text.plain_text(), "first\nمرحبا👩🏽‍💻\nlast");
    assert_eq!(text.spans().len(), 5);
}

#[test]
fn empty_semantics_use_concatenated_content() {
    let no_spans = RichText::from_spans(Vec::<RichTextSpan<'static>>::new());

    assert!(no_spans.is_empty());
    assert!(RichText::plain("").is_empty());
    assert!(RichText::from_spans([RichTextSpan::new(""), RichTextSpan::new("")]).is_empty());
    assert!(!RichText::plain(" ").is_empty());
    assert!(!RichText::plain("\n").is_empty());
}

#[test]
fn rich_text_preserves_custom_default_style() {
    let default_style = RichTextStyle::default()
        .with_weight(RichTextWeight::SEMI_BOLD)
        .with_underline(true);
    let text =
        RichText::from_span(RichTextSpan::new("inherited")).with_default_style(default_style);

    assert_eq!(text.default_style(), &default_style);
    assert_eq!(text.spans()[0].style(), &RichTextSpanStyle::default());
}

#[test]
fn owned_spec_detaches_all_content_from_borrowed_input() {
    let mut source = String::from("borrowed");
    let default_style = RichTextStyle::default().with_slant(RichTextSlant::Italic);
    let owned = {
        let text = RichText::from_spans([
            RichTextSpan::borrowed(&source).bold(),
            RichTextSpan::new(" tail").underline(),
        ])
        .with_default_style(default_style);
        text.to_owned_spec()
    };

    source.clear();
    source.push_str("changed");

    assert_eq!(owned.spans()[0].text(), "borrowed");
    assert_eq!(
        owned.spans()[0].style().weight(),
        Some(RichTextWeight::BOLD)
    );
    assert_eq!(owned.spans()[1].text(), " tail");
    assert_eq!(owned.spans()[1].style().underline(), Some(true));
    assert_eq!(owned.default_style(), &default_style);
}

#[test]
fn public_owned_conversion_detaches_borrowed_spans() {
    let source = String::from("borrowed");
    let rich = RichText::from_span(RichTextSpan::borrowed(&source));
    let owned = rich.into_owned();
    drop(source);

    assert_eq!(owned.plain_text(), "borrowed");
}
