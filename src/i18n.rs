// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

pub use fluent::{FluentArgs, FluentValue, fluent_args};
use fluent::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutDirection {
    #[default]
    Ltr,
    Rtl,
}

impl From<LayoutDirection> for taffy::style::Direction {
    fn from(direction: LayoutDirection) -> Self {
        match direction {
            LayoutDirection::Ltr => Self::Ltr,
            LayoutDirection::Rtl => Self::Rtl,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    id: LanguageIdentifier,
    direction: LayoutDirection,
}

impl Default for Locale {
    fn default() -> Self {
        Self::parse("en-US").expect("default locale must be valid")
    }
}

impl Locale {
    /// Parses a BCP-47 locale tag and infers layout direction.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{LayoutDirection, Locale};
    ///
    /// let locale = Locale::parse("ar").unwrap();
    /// assert_eq!(locale.direction(), LayoutDirection::Rtl);
    /// ```
    pub fn parse(value: &str) -> Result<Self, I18nError> {
        let id = value.parse::<LanguageIdentifier>().map_err(|_| {
            I18nError::invalid_locale(value, "BCP-47 language tag such as en-US or ar")
        })?;
        Ok(Self::from_language_identifier(id))
    }

    /// Builds a locale from a parsed Unicode language identifier.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{LayoutDirection, Locale};
    /// use unic_langid::LanguageIdentifier;
    ///
    /// let id: LanguageIdentifier = "he".parse().unwrap();
    /// assert_eq!(Locale::from_language_identifier(id).direction(), LayoutDirection::Rtl);
    /// ```
    pub fn from_language_identifier(id: LanguageIdentifier) -> Self {
        let direction = infer_direction(&id);
        Self { id, direction }
    }

    /// Returns the parsed language identifier used by Fluent.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::Locale;
    ///
    /// assert_eq!(Locale::parse("en-US").unwrap().language_id().to_string(), "en-US");
    /// ```
    pub fn language_id(&self) -> &LanguageIdentifier {
        &self.id
    }

    /// Returns the layout direction inferred from language or script.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{LayoutDirection, Locale};
    ///
    /// assert_eq!(Locale::parse("fa").unwrap().direction(), LayoutDirection::Rtl);
    /// ```
    pub fn direction(&self) -> LayoutDirection {
        self.direction
    }
}

pub struct FluentCatalog {
    locale: Locale,
    bundle: FluentBundle<FluentResource>,
}

impl FluentCatalog {
    /// Creates an empty Fluent catalog for a locale.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, Locale};
    ///
    /// let catalog = FluentCatalog::new(Locale::parse("en-US").unwrap());
    /// assert_eq!(catalog.locale().language_id().to_string(), "en-US");
    /// ```
    pub fn new(locale: Locale) -> Self {
        let bundle = FluentBundle::new(vec![locale.language_id().clone()]);
        Self { locale, bundle }
    }

    /// Creates a catalog and loads one FTL resource into it.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, Locale};
    ///
    /// let catalog = FluentCatalog::from_ftl(Locale::parse("en-US").unwrap(), "hello = Hello").unwrap();
    /// assert_eq!(catalog.text("hello").unwrap(), "Hello");
    /// ```
    pub fn from_ftl(locale: Locale, source: &str) -> Result<Self, I18nError> {
        let mut catalog = Self::new(locale);
        catalog.add_ftl("inline", source)?;
        Ok(catalog)
    }

    /// Adds one FTL resource to the catalog.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, Locale};
    ///
    /// let mut catalog = FluentCatalog::new(Locale::parse("en-US").unwrap());
    /// catalog.add_ftl("app.ftl", "save = Save").unwrap();
    /// assert_eq!(catalog.text("save").unwrap(), "Save");
    /// ```
    pub fn add_ftl(&mut self, source_name: &str, source: &str) -> Result<(), I18nError> {
        let resource = parse_ftl_resource(source_name, source)?;
        self.bundle.add_resource(resource).map_err(|errors| {
            I18nError::invalid_resource(source_name, "FTL resource with unique message IDs", errors)
        })
    }

    /// Formats a message without variables.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, Locale};
    ///
    /// let catalog = FluentCatalog::from_ftl(Locale::parse("en-US").unwrap(), "title = Settings").unwrap();
    /// assert_eq!(catalog.text("title").unwrap(), "Settings");
    /// ```
    pub fn text(&self, id: &str) -> Result<String, I18nError> {
        self.format(id, None)
    }

    /// Formats a message with optional Fluent arguments.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, Locale, fluent_args};
    ///
    /// let catalog = FluentCatalog::from_ftl(Locale::parse("en-US").unwrap(), "hello = Hello, { $name }").unwrap();
    /// let args = fluent_args!["name" => "Ada"];
    /// assert_eq!(catalog.format("hello", Some(&args)).unwrap(), "Hello, \u{2068}Ada\u{2069}");
    /// ```
    pub fn format(&self, id: &str, args: Option<&FluentArgs>) -> Result<String, I18nError> {
        let message = self
            .bundle
            .get_message(id)
            .ok_or_else(|| I18nError::missing_message(id))?;
        let value = message
            .value()
            .ok_or_else(|| I18nError::missing_message_value(id))?;
        let mut errors = Vec::new();
        let text = self.bundle.format_pattern(value, args, &mut errors);
        if errors.is_empty() {
            Ok(text.into_owned())
        } else {
            Err(I18nError::format_error(id, errors))
        }
    }

    /// Returns the catalog locale.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, Locale};
    ///
    /// let catalog = FluentCatalog::new(Locale::parse("en-US").unwrap());
    /// assert_eq!(catalog.locale().language_id().to_string(), "en-US");
    /// ```
    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    /// Returns the catalog layout direction.
    ///
    /// # Example
    /// ```
    /// use rutter::i18n::{FluentCatalog, LayoutDirection, Locale};
    ///
    /// let catalog = FluentCatalog::new(Locale::parse("ur").unwrap());
    /// assert_eq!(catalog.direction(), LayoutDirection::Rtl);
    /// ```
    pub fn direction(&self) -> LayoutDirection {
        self.locale.direction()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nError {
    InvalidLocale {
        value: String,
        expected: String,
    },
    InvalidResource {
        value: String,
        expected: String,
        details: Vec<String>,
    },
    MissingMessage {
        value: String,
        expected: String,
    },
    MissingMessageValue {
        value: String,
        expected: String,
    },
    FormatError {
        value: String,
        expected: String,
        details: Vec<String>,
    },
}

impl I18nError {
    fn invalid_locale(value: &str, expected: &str) -> Self {
        Self::InvalidLocale {
            value: value.to_string(),
            expected: expected.to_string(),
        }
    }

    fn invalid_resource(value: &str, expected: &str, errors: Vec<fluent::FluentError>) -> Self {
        Self::InvalidResource {
            value: value.to_string(),
            expected: expected.to_string(),
            details: details(errors),
        }
    }

    fn missing_message(value: &str) -> Self {
        Self::MissingMessage {
            value: value.to_string(),
            expected: "existing Fluent message ID".to_string(),
        }
    }

    fn missing_message_value(value: &str) -> Self {
        Self::MissingMessageValue {
            value: value.to_string(),
            expected: "message with a value pattern".to_string(),
        }
    }

    fn format_error(value: &str, errors: Vec<fluent::FluentError>) -> Self {
        Self::FormatError {
            value: value.to_string(),
            expected: "message that formats without Fluent resolver errors".to_string(),
            details: details(errors),
        }
    }
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLocale { value, expected } => {
                write_expected(f, "invalid locale", value, expected, &[])
            }
            Self::InvalidResource {
                value,
                expected,
                details,
            } => write_expected(f, "invalid Fluent resource", value, expected, details),
            Self::MissingMessage { value, expected } => {
                write_expected(f, "missing Fluent message", value, expected, &[])
            }
            Self::MissingMessageValue { value, expected } => {
                write_expected(f, "missing Fluent message value", value, expected, &[])
            }
            Self::FormatError {
                value,
                expected,
                details,
            } => write_expected(f, "Fluent format error", value, expected, details),
        }
    }
}

impl std::error::Error for I18nError {}

fn parse_ftl_resource(source_name: &str, source: &str) -> Result<FluentResource, I18nError> {
    FluentResource::try_new(source.to_string()).map_err(|(_, errors)| I18nError::InvalidResource {
        value: source_name.to_string(),
        expected: "syntactically valid Fluent FTL source".to_string(),
        details: errors
            .into_iter()
            .map(|error| format!("{error:?}"))
            .collect(),
    })
}

fn infer_direction(id: &LanguageIdentifier) -> LayoutDirection {
    if id
        .script
        .as_ref()
        .is_some_and(|script| is_rtl_script(script.as_str()))
    {
        return LayoutDirection::Rtl;
    }
    if is_rtl_language(id.language.as_str()) {
        LayoutDirection::Rtl
    } else {
        LayoutDirection::Ltr
    }
}

fn is_rtl_script(script: &str) -> bool {
    matches!(
        script,
        "Arab" | "Hebr" | "Syrc" | "Thaa" | "Nkoo" | "Adlm" | "Rohg"
    )
}

fn is_rtl_language(language: &str) -> bool {
    matches!(
        language,
        "ar" | "arc" | "dv" | "fa" | "he" | "ku" | "nqo" | "ps" | "sd" | "syr" | "ug" | "ur" | "yi"
    )
}

fn details(errors: Vec<fluent::FluentError>) -> Vec<String> {
    errors
        .into_iter()
        .map(|error| format!("{error:?}"))
        .collect()
}

fn write_expected(
    f: &mut fmt::Formatter<'_>,
    label: &str,
    value: &str,
    expected: &str,
    details: &[String],
) -> fmt::Result {
    write!(f, "{label}: offending value `{value}`, expected {expected}")?;
    if !details.is_empty() {
        write!(f, "; details: {}", details.join("; "))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_parse_infers_rtl_languages() {
        assert_eq!(
            Locale::parse("ar").unwrap().direction(),
            LayoutDirection::Rtl
        );
        assert_eq!(
            Locale::parse("en-US").unwrap().direction(),
            LayoutDirection::Ltr
        );
    }

    #[test]
    fn locale_parse_infers_rtl_scripts() {
        assert_eq!(
            Locale::parse("az-Arab").unwrap().direction(),
            LayoutDirection::Rtl
        );
    }

    #[test]
    fn locale_parse_reports_offending_value() {
        let err = Locale::parse("not a locale").unwrap_err().to_string();
        assert!(err.contains("not a locale"));
        assert!(err.contains("expected BCP-47"));
    }

    #[test]
    fn fluent_catalog_formats_plain_message() {
        let catalog =
            FluentCatalog::from_ftl(Locale::parse("en-US").unwrap(), "save = Save").unwrap();
        assert_eq!(catalog.text("save").unwrap(), "Save");
    }

    #[test]
    fn fluent_catalog_formats_message_with_args() {
        let catalog =
            FluentCatalog::from_ftl(Locale::parse("en-US").unwrap(), "hello = Hello, { $name }")
                .unwrap();
        let args = fluent_args!["name" => "Ada"];
        assert_eq!(
            catalog.format("hello", Some(&args)).unwrap(),
            "Hello, \u{2068}Ada\u{2069}"
        );
    }

    #[test]
    fn fluent_catalog_reports_missing_message_id() {
        let catalog =
            FluentCatalog::from_ftl(Locale::parse("en-US").unwrap(), "save = Save").unwrap();
        let err = catalog.text("missing").unwrap_err().to_string();
        assert!(err.contains("missing"));
        assert!(err.contains("existing Fluent message ID"));
    }
}
