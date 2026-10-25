// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Validated, unpremultiplied sRGB colors without renderer dependencies.
//!
//! HSV, HSL, and CMYK operate mathematically on encoded sRGB components, not
//! linear-light values. CMYK is a simple device-independent mathematical
//! conversion; it does not implement ICC profiles, ink limits, or printing color
//! management. IEEE `f32` conversions are approximate: compare round trips with
//! a tolerance (typically `1e-6` for unit components, larger for hue in degrees),
//! not exact equality. Only byte/HEX export rounds to eight-bit channels.

use std::fmt;

/// A validation failure at a color or editor boundary.
///
/// Messages identify the field and expected input without echoing draft text.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorError {
    /// A component was NaN or infinite.
    NonFinite { component: &'static str },
    /// A finite component was outside its inclusive range.
    OutOfRange {
        component: &'static str,
        min: f32,
        max: f32,
    },
    /// HEX syntax was invalid; alpha, when present, must be last.
    InvalidHex,
    /// A numeric draft was empty, incomplete, or not a number.
    InvalidNumber { component: &'static str },
    /// A draft exceeded the bounded numeric parser's input limit.
    TextTooLong { component: &'static str, max: usize },
    /// A format or channel index did not identify an available entry.
    InvalidIndex {
        kind: &'static str,
        index: usize,
        count: usize,
    },
}

impl fmt::Display for ColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { component } => write!(f, "{component} must be finite"),
            Self::OutOfRange {
                component,
                min,
                max,
            } => {
                write!(f, "{component} must be between {min} and {max} inclusive")
            }
            Self::InvalidHex => f.write_str(
                "HEX must contain 3, 4, 6, or 8 ASCII hexadecimal digits, optionally prefixed by #; alpha is last",
            ),
            Self::InvalidNumber { component } => {
                write!(f, "{component} must be a complete decimal number")
            }
            Self::TextTooLong { component, max } => {
                write!(f, "{component} must contain at most {max} bytes")
            }
            Self::InvalidIndex { kind, index, count } => {
                write!(f, "{kind} index {index} is invalid for {count} available entries")
            }
        }
    }
}

impl std::error::Error for ColorError {}

pub(super) fn validate_range(
    component: &'static str,
    value: f32,
    max: f32,
) -> Result<(), ColorError> {
    if !value.is_finite() {
        return Err(ColorError::NonFinite { component });
    }
    if !(0.0..=max).contains(&value) {
        return Err(ColorError::OutOfRange {
            component,
            min: 0.0,
            max,
        });
    }
    Ok(())
}

/// Finite, normalized, unpremultiplied encoded sRGB and alpha components.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PickerColor {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl PickerColor {
    /// Constructs a color from byte channels, with alpha last.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// Constructs an opaque color from byte channels.
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba8(r, g, b, 255)
    }

    /// Returns normalized `[red, green, blue, alpha]` components.
    pub fn rgba(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Rounds normalized components to the nearest byte, with alpha last.
    pub fn to_rgba8(self) -> [u8; 4] {
        self.rgba()
            .map(|component| (component * 255.0).round() as u8)
    }

    /// Validates four finite components in the inclusive range `0..=1`.
    pub fn try_rgba(r: f32, g: f32, b: f32, a: f32) -> Result<Self, ColorError> {
        for (name, value) in [("red", r), ("green", g), ("blue", b), ("alpha", a)] {
            validate_range(name, value, 1.0)?;
        }
        Ok(Self { r, g, b, a })
    }

    /// Accepts hue in `0..=360` degrees and saturation, value, alpha in `0..=1`.
    /// Hue 360 is equivalent to zero; all inputs must be finite.
    pub fn try_hsv(h_deg: f32, s_unit: f32, v_unit: f32, a_unit: f32) -> Result<Self, ColorError> {
        validate_cylindrical(h_deg, s_unit, v_unit, "value", a_unit)?;
        let chroma = v_unit * s_unit;
        Ok(Self::from_chroma(h_deg, chroma, v_unit - chroma, a_unit))
    }

    /// Returns `[hue_degrees, saturation, value]` with hue in `0..360`.
    /// Achromatic colors return hue zero; black also returns saturation zero.
    pub fn hsv(self) -> [f32; 3] {
        let (maximum, _, delta) = self.extrema();
        let saturation = if maximum == 0.0 { 0.0 } else { delta / maximum };
        [self.chromatic_hue(delta, maximum), saturation, maximum]
    }

    /// Accepts hue in `0..=360` degrees and saturation, lightness, alpha in `0..=1`.
    /// Hue 360 is equivalent to zero; all inputs must be finite.
    pub fn try_hsl(h_deg: f32, s_unit: f32, l_unit: f32, a: f32) -> Result<Self, ColorError> {
        validate_cylindrical(h_deg, s_unit, l_unit, "lightness", a)?;
        let chroma = 2.0 * l_unit.min(1.0 - l_unit) * s_unit;
        Ok(Self::from_chroma(h_deg, chroma, l_unit - chroma / 2.0, a))
    }

    /// Returns `[hue_degrees, saturation, lightness]`; gray has hue/saturation zero.
    pub fn hsl(self) -> [f32; 3] {
        let (maximum, minimum, delta) = self.extrema();
        let lightness = (maximum + minimum) / 2.0;
        // Avoid losing the denominator near white to rounded `2 - (max + min)`.
        let denominator = if lightness <= 0.5 {
            maximum + minimum
        } else {
            (1.0 - maximum) + (1.0 - minimum)
        };
        let saturation = if delta == 0.0 {
            0.0
        } else {
            delta / denominator
        };
        [self.chromatic_hue(delta, maximum), saturation, lightness]
    }

    /// Converts mathematical CMYK and alpha, each finite and in `0..=1`.
    /// This is not an ICC/profile-aware printing conversion.
    pub fn try_cmyk(c: f32, m: f32, y: f32, k: f32, a: f32) -> Result<Self, ColorError> {
        for (name, value) in [
            ("cyan", c),
            ("magenta", m),
            ("yellow", y),
            ("black", k),
            ("alpha", a),
        ] {
            validate_range(name, value, 1.0)?;
        }
        Self::try_rgba(
            (1.0 - c) * (1.0 - k),
            (1.0 - m) * (1.0 - k),
            (1.0 - y) * (1.0 - k),
            a,
        )
    }

    /// Returns canonical mathematical `[cyan, magenta, yellow, black]` in `0..=1`.
    /// Pure black is represented as `[0, 0, 0, 1]`.
    pub fn cmyk(self) -> [f32; 4] {
        let maximum = self.r.max(self.g).max(self.b);
        if maximum == 0.0 {
            return [0.0, 0.0, 0.0, 1.0];
        }
        [
            (maximum - self.r) / maximum,
            (maximum - self.g) / maximum,
            (maximum - self.b) / maximum,
            1.0 - maximum,
        ]
    }

    /// Parses ASCII RGB/RGBA short or long HEX, with an optional leading `#`.
    /// Alpha is last; absent alpha defaults to opaque. Whitespace is rejected.
    pub fn from_hex(text: &str) -> Result<Self, ColorError> {
        let digits = text.strip_prefix('#').unwrap_or(text).as_bytes();
        if !matches!(digits.len(), 3 | 4 | 6 | 8) || !digits.is_ascii() {
            return Err(ColorError::InvalidHex);
        }
        let short = digits.len() <= 4;
        let count = if short {
            digits.len()
        } else {
            digits.len() / 2
        };
        let mut rgba = [255; 4];
        for (index, channel) in rgba.iter_mut().enumerate().take(count) {
            *channel = if short {
                hex_digit(digits[index])? * 17
            } else {
                hex_digit(digits[index * 2])? * 16 + hex_digit(digits[index * 2 + 1])?
            };
        }
        Ok(Self::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]))
    }

    /// Exports uppercase `#RRGGBBAA`, rounding only at this boundary.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_rgba8();
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    /// Exports uppercase `#RRGGBB`, omitting alpha.
    pub fn to_hex_rgb(self) -> String {
        let [r, g, b, _] = self.to_rgba8();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    fn extrema(self) -> (f32, f32, f32) {
        let maximum = self.r.max(self.g).max(self.b);
        let minimum = self.r.min(self.g).min(self.b);
        (maximum, minimum, maximum - minimum)
    }

    fn chromatic_hue(self, delta: f32, maximum: f32) -> f32 {
        if delta == 0.0 {
            return 0.0;
        }
        let sector = if maximum == self.r {
            (self.g - self.b) / delta
        } else if maximum == self.g {
            (self.b - self.r) / delta + 2.0
        } else {
            (self.r - self.g) / delta + 4.0
        };
        (sector * 60.0).rem_euclid(360.0) % 360.0
    }

    fn from_chroma(hue: f32, chroma: f32, minimum: f32, a: f32) -> Self {
        let sector = (hue % 360.0) / 60.0;
        let secondary = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
        let (r, g, b) = match sector as u8 {
            0 => (chroma, secondary, 0.0),
            1 => (secondary, chroma, 0.0),
            2 => (0.0, chroma, secondary),
            3 => (0.0, secondary, chroma),
            4 => (secondary, 0.0, chroma),
            _ => (chroma, 0.0, secondary),
        };
        // Valid input can still accumulate a final rounding error at endpoints.
        Self {
            r: (r + minimum).clamp(0.0, 1.0),
            g: (g + minimum).clamp(0.0, 1.0),
            b: (b + minimum).clamp(0.0, 1.0),
            a,
        }
    }
}

fn validate_cylindrical(
    h: f32,
    s: f32,
    third: f32,
    name: &'static str,
    a: f32,
) -> Result<(), ColorError> {
    validate_range("hue", h, 360.0)?;
    validate_range("saturation", s, 1.0)?;
    validate_range(name, third, 1.0)?;
    validate_range("alpha", a, 1.0)
}

fn hex_digit(byte: u8) -> Result<u8, ColorError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ColorError::InvalidHex),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close<const N: usize>(actual: [f32; N], expected: [f32; N]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= 1e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn primaries_secondaries_and_endpoints_have_expected_conversions() {
        let samples = [
            ([255, 0, 0], 0.0, [0.0, 1.0, 1.0, 0.0]),
            ([255, 255, 0], 60.0, [0.0, 0.0, 1.0, 0.0]),
            ([0, 255, 0], 120.0, [1.0, 0.0, 1.0, 0.0]),
            ([0, 255, 255], 180.0, [1.0, 0.0, 0.0, 0.0]),
            ([0, 0, 255], 240.0, [1.0, 1.0, 0.0, 0.0]),
            ([255, 0, 255], 300.0, [0.0, 1.0, 0.0, 0.0]),
        ];
        for ([r, g, b], hue, cmyk) in samples {
            let color = PickerColor::from_rgb8(r, g, b);
            close(color.hsv(), [hue, 1.0, 1.0]);
            close(color.hsl(), [hue, 1.0, 0.5]);
            close(color.cmyk(), cmyk);
        }
        for level in [0, 1, 64, 128, 254, 255] {
            let color = PickerColor::from_rgb8(level, level, level);
            let unit = f32::from(level) / 255.0;
            close(color.hsv(), [0.0, 0.0, unit]);
            close(color.hsl(), [0.0, 0.0, unit]);
            close(color.cmyk(), [0.0, 0.0, 0.0, 1.0 - unit]);
        }
    }

    #[test]
    fn intermediate_conversions_match_independent_known_rgb_values() {
        close(
            PickerColor::try_hsv(210.0, 0.6, 0.7, 1.0).unwrap().rgba(),
            [0.28, 0.49, 0.7, 1.0],
        );
        close(
            PickerColor::try_hsl(210.0, 0.6, 0.4, 1.0).unwrap().rgba(),
            [0.16, 0.4, 0.64, 1.0],
        );
        close(
            PickerColor::try_cmyk(0.2, 0.4, 0.6, 0.3, 1.0)
                .unwrap()
                .rgba(),
            [0.56, 0.42, 0.28, 1.0],
        );
    }

    #[test]
    fn hue_sector_boundaries_and_adjacent_values_have_expected_rgb() {
        for (hue, rgb) in [
            (0.0, [1.0, 0.0, 0.0]),
            (0.06, [1.0, 0.001, 0.0]),
            (59.94, [1.0, 0.999, 0.0]),
            (60.0, [1.0, 1.0, 0.0]),
            (60.06, [0.999, 1.0, 0.0]),
            (120.0, [0.0, 1.0, 0.0]),
            (180.0, [0.0, 1.0, 1.0]),
            (240.0, [0.0, 0.0, 1.0]),
            (300.0, [1.0, 0.0, 1.0]),
            (359.94, [1.0, 0.0, 0.001]),
            (360.0, [1.0, 0.0, 0.0]),
        ] {
            let [r, g, b] = rgb;
            close(
                PickerColor::try_hsv(hue, 1.0, 1.0, 1.0).unwrap().rgba(),
                [r, g, b, 1.0],
            );
            close(
                PickerColor::try_hsl(hue, 1.0, 0.5, 1.0).unwrap().rgba(),
                [r, g, b, 1.0],
            );
            let color = PickerColor::try_rgba(r, g, b, 1.0).unwrap();
            for actual in [color.hsv()[0], color.hsl()[0]] {
                assert!((actual - hue % 360.0).abs() <= 1e-4, "{actual} != {hue}");
            }
        }
    }

    #[test]
    fn deterministic_rgb_grid_round_trips_all_models_without_byte_quantization() {
        for r in [0.0, 0.0001, 0.17, 0.5, 0.9999, 1.0] {
            for g in [0.0, 0.03, 0.6, 1.0] {
                for b in [0.0, 0.2, 0.91, 1.0] {
                    let color = PickerColor::try_rgba(r, g, b, 0.314159).unwrap();
                    let [h, s, v] = color.hsv();
                    close(
                        PickerColor::try_hsv(h, s, v, color.a).unwrap().rgba(),
                        color.rgba(),
                    );
                    let [h, s, l] = color.hsl();
                    close(
                        PickerColor::try_hsl(h, s, l, color.a).unwrap().rgba(),
                        color.rgba(),
                    );
                    let [c, m, y, k] = color.cmyk();
                    close(
                        PickerColor::try_cmyk(c, m, y, k, color.a).unwrap().rgba(),
                        color.rgba(),
                    );
                }
            }
        }
    }

    #[test]
    fn near_white_hsl_preserves_hue_and_saturation_with_bounded_rounding() {
        let tolerance = f32::EPSILON / 2.0;
        let near_white = PickerColor::try_rgba(1.0, 1.0, 1.0 - tolerance, 1.0).unwrap();
        let [hue, saturation, lightness] = near_white.hsl();
        assert_eq!(hue, 60.0);
        assert_eq!(saturation, 1.0);
        assert!((1.0 - tolerance..=1.0).contains(&lightness));
        let reconstructed = PickerColor::try_hsl(hue, saturation, lightness, 1.0).unwrap();
        for (actual, expected) in reconstructed.rgba().into_iter().zip(near_white.rgba()) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn byte_and_hex_round_trips_cover_every_byte_value() {
        for byte in 0..=255 {
            let color = PickerColor::from_rgba8(byte, 255 - byte, byte, 255 - byte);
            assert_eq!(color.to_rgba8(), [byte, 255 - byte, byte, 255 - byte]);
            assert_eq!(PickerColor::from_hex(&color.to_hex()).unwrap(), color);
        }
        assert_eq!(
            PickerColor::from_hex("#1aF8").unwrap().to_rgba8(),
            [17, 170, 255, 136]
        );
        assert_eq!(
            PickerColor::from_hex("aBc").unwrap().to_rgba8(),
            [170, 187, 204, 255]
        );
        assert_eq!(
            PickerColor::from_hex("12345678").unwrap().to_rgba8(),
            [18, 52, 86, 120]
        );
        assert_eq!(
            PickerColor::from_hex("#123456").unwrap().to_hex(),
            "#123456FF"
        );
        assert_eq!(
            PickerColor::from_hex("1234").unwrap().to_hex_rgb(),
            "#112233"
        );
    }

    #[test]
    fn malformed_hex_is_rejected_without_unicode_slicing() {
        for text in [
            "",
            "#",
            "12",
            "12345",
            "1234567",
            "123456789",
            "##fff",
            "#ggg",
            " ff",
            "fff ",
            "１２３",
            "é12",
            "#💙",
        ] {
            assert_eq!(
                PickerColor::from_hex(text),
                Err(ColorError::InvalidHex),
                "{text}"
            );
        }
    }

    #[test]
    fn all_constructor_boundaries_reject_nonfinite_and_out_of_range_components() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
            for index in 0..4 {
                let mut rgba = [0.5; 4];
                rgba[index] = invalid;
                assert!(PickerColor::try_rgba(rgba[0], rgba[1], rgba[2], rgba[3]).is_err());
            }
            for index in 1..4 {
                let mut cylindrical = [120.0, 0.5, 0.5, 0.5];
                cylindrical[index] = invalid;
                let [h, s, third, a] = cylindrical;
                assert!(PickerColor::try_hsv(h, s, third, a).is_err());
                assert!(PickerColor::try_hsl(h, s, third, a).is_err());
            }
            for index in 0..5 {
                let mut cmyk = [0.5; 5];
                cmyk[index] = invalid;
                assert!(
                    PickerColor::try_cmyk(cmyk[0], cmyk[1], cmyk[2], cmyk[3], cmyk[4]).is_err()
                );
            }
        }
        for hue in [-1.0, 360.01, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(PickerColor::try_hsv(hue, 1.0, 1.0, 1.0).is_err());
            assert!(PickerColor::try_hsl(hue, 1.0, 0.5, 1.0).is_err());
        }
        assert_eq!(
            PickerColor::try_hsv(360.0, 1.0, 1.0, 1.0).unwrap(),
            PickerColor::from_rgb8(255, 0, 0)
        );
        assert_eq!(
            PickerColor::try_hsl(360.0, 1.0, 0.5, 1.0).unwrap(),
            PickerColor::from_rgb8(255, 0, 0)
        );
        let error = PickerColor::try_rgba(f32::NAN, 0.0, 0.0, 1.0).unwrap_err();
        assert!(error.to_string().contains("red must be finite"));
    }
}
