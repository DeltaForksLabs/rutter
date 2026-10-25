// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Renderer-independent color-picker events, validated drafts, and retained HSV.

use super::color::{ColorError, PickerColor, validate_range};

const MAX_NUMERIC_BYTES: usize = 64;

/// Editable representations, in the order used by the format selector.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ColorFormat {
    /// Encoded sRGB channels in byte-scale units, plus percentage alpha.
    #[default]
    Rgb,
    /// One full HEX field; there are no numeric channels.
    Hex,
    /// Hue in degrees, percentage saturation/lightness/alpha.
    Hsl,
    /// Hue in degrees, percentage saturation/value/alpha.
    Hsv,
    /// Mathematical CMYK percentages followed by percentage alpha; no ICC profiles.
    Cmyk,
}

impl ColorFormat {
    /// All five representations in selector order.
    pub const ALL: [Self; 5] = [Self::Rgb, Self::Hex, Self::Hsl, Self::Hsv, Self::Cmyk];
    /// Display labels corresponding to [`Self::ALL`].
    pub const LABELS: [&'static str; 5] = ["RGB", "HEX", "HSL", "HSV", "CMYK"];

    /// Returns this representation's selector index.
    pub fn index(self) -> usize {
        match self {
            Self::Rgb => 0,
            Self::Hex => 1,
            Self::Hsl => 2,
            Self::Hsv => 3,
            Self::Cmyk => 4,
        }
    }

    /// Looks up a selector index, rejecting unavailable entries.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

/// Owned reducer input suitable for applications using function-pointer callbacks.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorPickerEvent {
    /// Unit saturation and value from the SV plane.
    SaturationValue { saturation: f32, value: f32 },
    /// Hue in inclusive degrees `0..=360`; 360 normalizes to zero.
    Hue(f32),
    /// Unit alpha in `0..=1`.
    Alpha(f32),
    /// Selector index in [`ColorFormat::ALL`].
    Format(usize),
    /// Numeric draft for an active channel; alpha is always the final channel.
    Channel { index: usize, text: String },
    /// Full RGB/RGBA HEX draft, independent of the selected representation.
    Hex(String),
    /// External validated color replacement.
    SetColor(PickerColor),
}

/// Working color, retained HSV, and editable text for a single picker.
///
/// Gestures never pass through byte/HEX quantization. Hue survives achromatic
/// colors, and HSV saturation survives black. Active HSL/CMYK components are
/// retained separately because RGB cannot recover them at degenerate colors.
/// Switching representations or replacing the color refreshes numeric drafts;
/// it does not reparse rounded display text. See the color module for `f32`
/// round-trip tolerance and the mathematical (non-ICC) CMYK model.
#[derive(Clone, Debug)]
pub struct ColorPickerState {
    color: PickerColor,
    format: ColorFormat,
    hsv: [f32; 3],
    // Native units: RGB/percent channels are normalized, hue is in degrees.
    components: [f32; 5],
    channels: [String; 5],
    channel_errors: [Option<String>; 5],
    hex: String,
    hex_error: Option<String>,
}

impl ColorPickerState {
    /// Starts in RGB format with coherent drafts for a validated color.
    pub fn new(color: PickerColor) -> Self {
        let mut state = Self {
            color,
            format: ColorFormat::default(),
            hsv: color.hsv(),
            components: [0.0; 5],
            channels: std::array::from_fn(|_| String::new()),
            channel_errors: std::array::from_fn(|_| None),
            hex: String::new(),
            hex_error: None,
        };
        state.sync_components();
        state.refresh_drafts();
        state
    }

    /// Returns the last valid color, including unquantized alpha.
    pub fn color(&self) -> PickerColor {
        self.color
    }

    /// Returns the active numeric/HEX representation.
    pub fn format(&self) -> ColorFormat {
        self.format
    }

    /// Returns retained hue in `0..360` degrees, even for gray or black.
    pub fn hue(&self) -> f32 {
        self.hsv[0]
    }

    /// Returns retained unit HSV saturation, even at black.
    pub fn saturation(&self) -> f32 {
        self.hsv[1]
    }

    /// Returns unit HSV value.
    pub fn value(&self) -> f32 {
        self.hsv[2]
    }

    /// Returns unit alpha.
    pub fn alpha(&self) -> f32 {
        self.color.rgba()[3]
    }

    /// Returns the HEX draft, which can be invalid or retain accepted spelling.
    pub fn hex(&self) -> &str {
        &self.hex
    }

    /// Returns validation feedback for the current HEX draft.
    pub fn hex_error(&self) -> Option<&str> {
        self.hex_error.as_deref()
    }

    /// Returns 4 for RGB/HSL/HSV, 5 for CMYK, and 0 for HEX.
    pub fn channel_count(&self) -> usize {
        match self.format {
            ColorFormat::Hex => 0,
            ColorFormat::Cmyk => 5,
            _ => 4,
        }
    }

    /// Returns a channel abbreviation, or an empty string for an invalid index.
    pub fn channel_label(&self, index: usize) -> &'static str {
        let labels: &[&str] = match self.format {
            ColorFormat::Rgb => &["R", "G", "B", "A"],
            ColorFormat::Hex => &[],
            ColorFormat::Hsl => &["H", "S", "L", "A"],
            ColorFormat::Hsv => &["H", "S", "V", "A"],
            ColorFormat::Cmyk => &["C", "M", "Y", "K", "A"],
        };
        labels.get(index).copied().unwrap_or("")
    }

    /// Returns a numeric draft, or an empty string for an invalid index.
    pub fn channel_text(&self, index: usize) -> &str {
        if index >= self.channel_count() {
            return "";
        }
        &self.channels[index]
    }

    /// Returns draft feedback, or `None` for an invalid index.
    pub fn channel_error(&self, index: usize) -> Option<&str> {
        if index >= self.channel_count() {
            return None;
        }
        self.channel_errors[index].as_deref()
    }

    /// Applies one input and reports whether the **color** changed.
    ///
    /// Format, retained hue/saturation, and draft changes alone return `false`.
    /// Invalid numeric/HEX drafts are retained with feedback and return `Err`
    /// without changing the last valid color. Invalid indices and gesture values
    /// are rejected without mutation. Numeric input is bounded to 64 bytes;
    /// RGB uses `0..=255`, hue `0..=360`, and other channels `0..=100` percent.
    /// Accepted text retains its spelling until another successful update.
    pub fn update(&mut self, event: ColorPickerEvent) -> Result<bool, ColorError> {
        let previous = self.color;
        match event {
            ColorPickerEvent::SaturationValue { saturation, value } => {
                validate_range("saturation", saturation, 1.0)?;
                validate_range("value", value, 1.0)?;
                self.apply_hsv([self.hue(), saturation, value])?;
            }
            ColorPickerEvent::Hue(hue) => {
                validate_range("hue", hue, 360.0)?;
                self.apply_hsv([hue % 360.0, self.saturation(), self.value()])?;
            }
            ColorPickerEvent::Alpha(alpha) => {
                validate_range("alpha", alpha, 1.0)?;
                self.color = self.with_alpha(alpha)?;
                if let Some(index) = self.channel_count().checked_sub(1) {
                    self.components[index] = alpha;
                }
                self.refresh_drafts();
            }
            ColorPickerEvent::Format(index) => {
                let format = ColorFormat::from_index(index).ok_or(ColorError::InvalidIndex {
                    kind: "format",
                    index,
                    count: ColorFormat::ALL.len(),
                })?;
                if self.format != format {
                    self.format = format;
                    self.sync_components();
                }
                self.refresh_drafts();
            }
            ColorPickerEvent::Channel { index, text } => self.edit_channel(index, text)?,
            ColorPickerEvent::Hex(text) => self.edit_hex(text)?,
            ColorPickerEvent::SetColor(color) => {
                self.replace_color(color);
            }
        }
        Ok(self.color != previous)
    }

    /// Replaces the color, refreshing drafts and retaining hue at gray/black.
    /// Unchanged RGB also preserves retained HSV and active native components.
    /// Returns whether the color changed, just like [`Self::update`].
    pub fn set_color(&mut self, color: PickerColor) -> bool {
        let changed = self.color != color;
        self.replace_color(color);
        changed
    }

    fn with_alpha(&self, alpha: f32) -> Result<PickerColor, ColorError> {
        let [r, g, b, _] = self.color.rgba();
        PickerColor::try_rgba(r, g, b, alpha)
    }

    fn retain_hsv_from_color(&mut self) {
        let [hue, saturation, value] = self.color.hsv();
        if saturation > 0.0 {
            self.hsv[0] = hue;
        }
        if value > 0.0 {
            self.hsv[1] = saturation;
        }
        self.hsv[2] = value;
    }

    fn replace_color(&mut self, color: PickerColor) {
        let rgb_changed = self.color.rgba()[..3] != color.rgba()[..3];
        self.color = color;
        if rgb_changed {
            self.retain_hsv_from_color();
            self.sync_components();
        } else if let Some(index) = self.channel_count().checked_sub(1) {
            // RGB cannot recover active HSL/CMYK intent at degenerate colors.
            self.components[index] = self.alpha();
        }
        self.refresh_drafts();
    }

    fn apply_hsv(&mut self, hsv: [f32; 3]) -> Result<(), ColorError> {
        let color = PickerColor::try_hsv(hsv[0], hsv[1], hsv[2], self.alpha())?;
        let rgb_changed = color != self.color;
        self.color = color;
        self.hsv = hsv;
        if rgb_changed {
            self.sync_components();
        } else {
            // A hue/SV gesture at black must not discard active HSL/CMYK intent.
            match self.format {
                ColorFormat::Hsv => self.components[..3].copy_from_slice(&hsv),
                ColorFormat::Hsl => self.components[0] = hsv[0],
                _ => {}
            }
        }
        self.refresh_drafts();
        Ok(())
    }

    fn sync_components(&mut self) {
        let [r, g, b, alpha] = self.color.rgba();
        self.components = match self.format {
            ColorFormat::Rgb => [r, g, b, alpha, 0.0],
            ColorFormat::Hex => [0.0; 5],
            ColorFormat::Hsv => [self.hue(), self.saturation(), self.value(), alpha, 0.0],
            ColorFormat::Hsl => {
                let [_, saturation, lightness] = self.color.hsl();
                [self.hue(), saturation, lightness, alpha, 0.0]
            }
            ColorFormat::Cmyk => {
                let [c, m, y, k] = self.color.cmyk();
                [c, m, y, k, alpha]
            }
        };
    }

    fn channel_scale(&self, index: usize) -> f32 {
        match self.format {
            ColorFormat::Rgb if index < 3 => 255.0,
            ColorFormat::Hsl | ColorFormat::Hsv if index == 0 => 1.0,
            _ => 100.0,
        }
    }

    fn refresh_drafts(&mut self) {
        for index in 0..5 {
            self.channels[index] = if index < self.channel_count() {
                (self.components[index] * self.channel_scale(index)).to_string()
            } else {
                String::new()
            };
            self.channel_errors[index] = None;
        }
        self.hex = self.color.to_hex();
        self.hex_error = None;
    }

    fn edit_channel(&mut self, index: usize, text: String) -> Result<(), ColorError> {
        if index >= self.channel_count() {
            return Err(ColorError::InvalidIndex {
                kind: "channel",
                index,
                count: self.channel_count(),
            });
        }
        let label = self.channel_label(index);
        let max = if index == 0 && matches!(self.format, ColorFormat::Hsl | ColorFormat::Hsv) {
            360.0
        } else {
            self.channel_scale(index)
        };
        let parsed = parse_channel(&text, label, max);
        self.channels[index] = text;
        let value = match parsed {
            Ok(value) => value / self.channel_scale(index),
            Err(error) => {
                self.channel_errors[index] = Some(error.to_string());
                return Err(error);
            }
        };
        let mut components = self.components;
        components[index] = if max == 360.0 { value % 360.0 } else { value };
        let color = self.color_from_components(components, index)?;
        self.color = color;
        self.components = components;
        if index != self.channel_count() - 1 {
            self.retain_hsv_from_color();
            match self.format {
                ColorFormat::Hsv => self.hsv.copy_from_slice(&components[..3]),
                ColorFormat::Hsl => self.hsv[0] = components[0],
                _ => {}
            }
        }
        let accepted = std::mem::take(&mut self.channels[index]);
        self.refresh_drafts();
        self.channels[index] = accepted;
        Ok(())
    }

    fn color_from_components(&self, c: [f32; 5], edited: usize) -> Result<PickerColor, ColorError> {
        if edited == self.channel_count() - 1 {
            return self.with_alpha(c[edited]);
        }
        match self.format {
            ColorFormat::Rgb => PickerColor::try_rgba(c[0], c[1], c[2], c[3]),
            ColorFormat::Hsl => PickerColor::try_hsl(c[0], c[1], c[2], c[3]),
            ColorFormat::Hsv => PickerColor::try_hsv(c[0], c[1], c[2], c[3]),
            ColorFormat::Cmyk => PickerColor::try_cmyk(c[0], c[1], c[2], c[3], c[4]),
            ColorFormat::Hex => unreachable!("HEX has no numeric channels"),
        }
    }

    fn edit_hex(&mut self, text: String) -> Result<(), ColorError> {
        let parsed = PickerColor::from_hex(&text);
        self.hex = text;
        let mut color = match parsed {
            Ok(color) => color,
            Err(error) => {
                self.hex_error = Some(error.to_string());
                return Err(error);
            }
        };
        let digits = self.hex.strip_prefix('#').unwrap_or(&self.hex).len();
        if matches!(digits, 3 | 6) {
            let [r, g, b, _] = color.rgba();
            color = PickerColor::try_rgba(r, g, b, self.alpha())?;
        }
        let accepted = std::mem::take(&mut self.hex);
        self.replace_color(color);
        self.hex = accepted;
        Ok(())
    }
}

fn parse_channel(text: &str, component: &'static str, max: f32) -> Result<f32, ColorError> {
    if text.len() > MAX_NUMERIC_BYTES {
        return Err(ColorError::TextTooLong {
            component,
            max: MAX_NUMERIC_BYTES,
        });
    }
    let value = text
        .trim()
        .parse::<f32>()
        .map_err(|_| ColorError::InvalidNumber { component })?;
    validate_range(component, value, max)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(state: &mut ColorPickerState, format: ColorFormat) {
        assert!(
            !state
                .update(ColorPickerEvent::Format(format.index()))
                .unwrap()
        );
    }

    fn edit(state: &mut ColorPickerState, index: usize, text: &str) -> Result<bool, ColorError> {
        state.update(ColorPickerEvent::Channel {
            index,
            text: text.to_owned(),
        })
    }

    fn close(actual: f32, expected: f32) {
        let tolerance = 1e-6 * expected.abs().max(1.0);
        assert!(
            (actual - expected).abs() < tolerance,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn retained_hue_and_sv_saturation_survive_black_and_restore_color() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(255, 0, 0));
        state.update(ColorPickerEvent::Hue(213.25)).unwrap();
        state
            .update(ColorPickerEvent::SaturationValue {
                saturation: 0.72345,
                value: 0.0,
            })
            .unwrap();
        assert_eq!(state.color(), PickerColor::from_rgb8(0, 0, 0));
        close(state.hue(), 213.25);
        close(state.saturation(), 0.72345);
        assert!(!state.update(ColorPickerEvent::Hue(271.5)).unwrap());
        assert!(
            !state
                .update(ColorPickerEvent::SaturationValue {
                    saturation: 0.81234,
                    value: 0.0
                })
                .unwrap()
        );
        format(&mut state, ColorFormat::Hsv);
        close(state.channel_text(1).parse().unwrap(), 81.234);
        state
            .update(ColorPickerEvent::SaturationValue {
                saturation: state.saturation(),
                value: 0.654321,
            })
            .unwrap();
        let expected = PickerColor::try_hsv(271.5, 0.81234, 0.654321, 1.0).unwrap();
        assert_eq!(state.color(), expected);
        assert_ne!(state.color(), PickerColor::from_rgba8(0, 0, 0, 255));
    }

    #[test]
    fn gestures_preserve_working_precision_and_alpha_does_not_reconvert_rgb() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(0, 0, 0));
        state.update(ColorPickerEvent::Hue(123.456)).unwrap();
        state
            .update(ColorPickerEvent::SaturationValue {
                saturation: 0.654321,
                value: 0.456789,
            })
            .unwrap();
        let expected = PickerColor::try_hsv(123.456, 0.654321, 0.456789, 1.0).unwrap();
        assert_eq!(state.color(), expected);
        for representation in ColorFormat::ALL {
            format(&mut state, representation);
            state.update(ColorPickerEvent::Alpha(0.123456)).unwrap();
            assert_eq!(&state.color().rgba()[..3], &expected.rgba()[..3]);
            let hsv = [state.hue(), state.saturation(), state.value()];
            if state.channel_count() > 0 {
                let alpha_index = state.channel_count() - 1;
                edit(&mut state, alpha_index, "45.6789").unwrap();
                assert_eq!(&state.color().rgba()[..3], &expected.rgba()[..3]);
                assert_eq!([state.hue(), state.saturation(), state.value()], hsv);
            }
        }
        close(state.alpha(), 0.456789);
    }

    #[test]
    fn hsl_saturation_at_black_and_white_is_retained_for_subsequent_edits() {
        for endpoint in ["0", "100"] {
            let mut state = ColorPickerState::new(PickerColor::from_rgb8(0, 0, 0));
            format(&mut state, ColorFormat::Hsl);
            edit(&mut state, 2, endpoint).unwrap();
            assert!(!edit(&mut state, 0, "240").unwrap());
            assert!(!edit(&mut state, 1, "075.00").unwrap());
            assert_eq!(state.channel_text(1), "075.00");
            state.update(ColorPickerEvent::Alpha(0.5)).unwrap();
            assert_eq!(state.channel_text(1), "75");
            edit(&mut state, 2, "50").unwrap();
            assert_eq!(
                state.color(),
                PickerColor::try_hsl(240.0, 0.75, 0.5, 0.5).unwrap()
            );
        }
    }

    #[test]
    fn cmyk_nonunique_components_survive_black_and_alpha_edits() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(0, 0, 0));
        format(&mut state, ColorFormat::Cmyk);
        assert!(!edit(&mut state, 0, "25").unwrap());
        assert!(!edit(&mut state, 1, "50").unwrap());
        assert!(!edit(&mut state, 2, "75").unwrap());
        state.update(ColorPickerEvent::Hue(120.0)).unwrap();
        edit(&mut state, 4, "40").unwrap();
        assert_eq!(state.channel_text(0), "25");
        edit(&mut state, 3, "50").unwrap();
        assert_eq!(
            state.color(),
            PickerColor::try_cmyk(0.25, 0.5, 0.75, 0.5, 0.4).unwrap()
        );
        // Noncanonical components must also survive when K is not an endpoint.
        edit(&mut state, 4, "80").unwrap();
        assert_eq!(state.channel_text(0), "25");
        assert_eq!(state.channel_text(3), "50");
    }

    #[test]
    fn same_rgb_replacements_preserve_retained_hsv_and_native_component_intent() {
        let black = PickerColor::from_rgb8(0, 0, 0);
        let translucent_black = PickerColor::from_rgba8(0, 0, 0, 128);
        for representation in [ColorFormat::Hsl, ColorFormat::Cmyk] {
            for (event, alpha) in [
                (None, 1.0), // Direct set_color with the same color.
                (Some(ColorPickerEvent::SetColor(black)), 1.0),
                (
                    Some(ColorPickerEvent::SetColor(translucent_black)),
                    128.0 / 255.0,
                ),
                (
                    Some(ColorPickerEvent::Hex("#00000080".into())),
                    128.0 / 255.0,
                ),
            ] {
                let mut state =
                    ColorPickerState::new(PickerColor::try_hsv(217.0, 0.7, 0.8, 1.0).unwrap());
                state
                    .update(ColorPickerEvent::SaturationValue {
                        saturation: 0.7,
                        value: 0.0,
                    })
                    .unwrap();
                format(&mut state, representation);
                let drafts: &[&str] = match representation {
                    ColorFormat::Hsl => &["240", "075.00", "0"],
                    ColorFormat::Cmyk => &["025.00", "50", "75", "100"],
                    _ => unreachable!(),
                };
                for (index, text) in drafts.iter().enumerate() {
                    assert!(!edit(&mut state, index, text).unwrap());
                }
                let hsv = [state.hue(), state.saturation(), state.value()];
                assert!(edit(&mut state, 0, "invalid").is_err());
                assert!(state.update(ColorPickerEvent::Hex("#12".into())).is_err());
                let changed = match event {
                    Some(event) => state.update(event).unwrap(),
                    None => state.set_color(state.color()),
                };
                assert_eq!(changed, alpha != 1.0);
                assert_eq!(state.color().rgba(), [0.0, 0.0, 0.0, alpha]);
                assert_eq!([state.hue(), state.saturation(), state.value()], hsv);
                for (index, text) in drafts.iter().enumerate() {
                    close(
                        state.channel_text(index).parse().unwrap(),
                        text.parse().unwrap(),
                    );
                    assert!(state.channel_error(index).is_none());
                }
                assert_eq!(
                    state.channel_text(0),
                    drafts[0].parse::<f32>().unwrap().to_string()
                );
                close(
                    state.channel_text(drafts.len()).parse().unwrap(),
                    alpha * 100.0,
                );
                assert!(state.channel_error(drafts.len()).is_none());
                assert_eq!(state.hex(), state.color().to_hex());
                assert!(state.hex_error().is_none());
                let expected = match representation {
                    ColorFormat::Hsl => {
                        edit(&mut state, 2, "50").unwrap();
                        PickerColor::try_hsl(240.0, 0.75, 0.5, alpha).unwrap()
                    }
                    ColorFormat::Cmyk => {
                        edit(&mut state, 3, "50").unwrap();
                        PickerColor::try_cmyk(0.25, 0.5, 0.75, 0.5, alpha).unwrap()
                    }
                    _ => unreachable!(),
                };
                assert_eq!(state.color(), expected);
                assert!(state.set_color(PickerColor::from_rgb8(0, 255, 0)));
                let canonical: &[f32] = match representation {
                    ColorFormat::Hsl => &[120.0, 100.0, 50.0, 100.0],
                    ColorFormat::Cmyk => &[100.0, 0.0, 100.0, 0.0, 100.0],
                    _ => unreachable!(),
                };
                for (index, expected) in canonical.iter().enumerate() {
                    close(state.channel_text(index).parse().unwrap(), *expected);
                }
            }
        }
    }

    #[test]
    fn invalid_drafts_keep_last_color_and_valid_edit_clears_all_stale_errors() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(12, 34, 56));
        let original = state.color();
        assert!(edit(&mut state, 0, "-").is_err());
        assert!(edit(&mut state, 1, "").is_err());
        assert!(state.update(ColorPickerEvent::Hex("#12".into())).is_err());
        assert_eq!(state.color(), original);
        assert_eq!(state.channel_text(0), "-");
        assert_eq!(state.channel_text(1), "");
        assert!(state.channel_error(0).is_some());
        assert_eq!(state.hex(), "#12");
        assert!(state.hex_error().is_some());
        assert!(edit(&mut state, 0, "012.500").unwrap());
        assert_eq!(state.channel_text(0), "012.500");
        assert_eq!(state.channel_text(1), "34");
        assert!(state.channel_error(0).is_none());
        assert!(state.channel_error(1).is_none());
        assert!(state.hex_error().is_none());
        close(state.color().rgba()[0], 12.5 / 255.0);
        close(state.color().rgba()[1], 34.0 / 255.0);
    }

    #[test]
    fn valid_unchanged_drafts_report_false_and_still_keep_spelling() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(255, 0, 0));
        assert!(!edit(&mut state, 0, " 255.00 ").unwrap());
        assert_eq!(state.channel_text(0), " 255.00 ");
        assert!(!state.update(ColorPickerEvent::Hex("f00".into())).unwrap());
        assert_eq!(state.hex(), "f00");
        format(&mut state, ColorFormat::Hsv);
        assert!(!edit(&mut state, 0, "360").unwrap());
        assert_eq!(state.hue(), 0.0);
        assert_eq!(state.channel_text(0), "360");
        assert!(!state.update(ColorPickerEvent::Hue(360.0)).unwrap());
        assert_eq!(state.channel_text(0), "0");
    }

    #[test]
    fn numeric_hsv_edits_retain_black_components_and_restore_them() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(0, 0, 0));
        format(&mut state, ColorFormat::Hsv);
        assert!(!edit(&mut state, 0, "210.125").unwrap());
        assert!(!edit(&mut state, 1, "067.875").unwrap());
        assert_eq!(state.channel_text(1), "067.875");
        assert_eq!(state.hue(), 210.125);
        assert_eq!(state.saturation(), 0.67875);
        edit(&mut state, 3, "33.25").unwrap();
        assert!(edit(&mut state, 2, "42.125").unwrap());
        let expected = PickerColor::try_hsv(210.125, 0.67875, 0.42125, 0.3325).unwrap();
        assert_eq!(state.color(), expected);
    }

    #[test]
    fn editing_one_rgb_channel_does_not_round_other_fractional_channels() {
        let original = PickerColor::try_rgba(0.1234567, 0.2345678, 0.3456789, 0.4567891).unwrap();
        let mut state = ColorPickerState::new(original);
        assert!(edit(&mut state, 0, "12.75").unwrap());
        assert_eq!(
            state.color().rgba(),
            [0.05, 0.2345678, 0.3456789, 0.4567891]
        );
        assert_eq!(state.channel_text(0), "12.75");
    }

    #[test]
    fn hex_rgb_preserves_exact_alpha_and_rgba_replaces_alpha_last() {
        let mut state =
            ColorPickerState::new(PickerColor::try_rgba(0.0, 0.0, 0.0, 0.123456).unwrap());
        for text in ["#abc", "123456", "ABC", "#aBcDeF"] {
            state.update(ColorPickerEvent::Hex(text.into())).unwrap();
            assert_eq!(state.alpha(), 0.123456);
            assert_eq!(state.hex(), text);
        }
        state.update(ColorPickerEvent::Hex("#0f08".into())).unwrap();
        assert_eq!(state.color().to_rgba8(), [0, 255, 0, 136]);
        state
            .update(ColorPickerEvent::Hex("12345678".into()))
            .unwrap();
        assert_eq!(state.color().to_rgba8(), [18, 52, 86, 120]);
        assert_eq!(state.channel_text(0), "18");
    }

    #[test]
    fn formats_refresh_coherent_drafts_without_altering_color() {
        let original = PickerColor::try_rgba(0.12345, 0.23456, 0.34567, 0.45678).unwrap();
        let mut state = ColorPickerState::new(original);
        for representation in ColorFormat::ALL {
            assert!(edit(&mut state, 0, "invalid").is_err());
            format(&mut state, representation);
            assert_eq!(state.format(), representation);
            assert_eq!(state.color(), original);
            let count = match representation {
                ColorFormat::Hex => 0,
                ColorFormat::Cmyk => 5,
                _ => 4,
            };
            assert_eq!(state.channel_count(), count);
            for index in 0..count {
                assert!(!state.channel_label(index).is_empty());
                assert!(state.channel_text(index).parse::<f32>().is_ok());
                assert!(state.channel_error(index).is_none());
            }
            if count > 0 {
                assert_eq!(state.channel_label(count - 1), "A");
                close(
                    state.channel_text(count - 1).parse().unwrap(),
                    original.rgba()[3] * 100.0,
                );
            }
            assert_eq!(state.hex(), original.to_hex());
        }
        assert_eq!(ColorFormat::from_index(usize::MAX), None);
    }

    #[test]
    fn channel_units_and_labels_match_each_selected_representation() {
        let mut state = ColorPickerState::new(PickerColor::from_rgba8(255, 0, 0, 128));
        let alpha = 128.0 / 255.0 * 100.0;
        let cases: [(ColorFormat, &[&str], &[f32]); 5] = [
            (
                ColorFormat::Rgb,
                &["R", "G", "B", "A"],
                &[255.0, 0.0, 0.0, alpha],
            ),
            (ColorFormat::Hex, &[], &[]),
            (
                ColorFormat::Hsl,
                &["H", "S", "L", "A"],
                &[0.0, 100.0, 50.0, alpha],
            ),
            (
                ColorFormat::Hsv,
                &["H", "S", "V", "A"],
                &[0.0, 100.0, 100.0, alpha],
            ),
            (
                ColorFormat::Cmyk,
                &["C", "M", "Y", "K", "A"],
                &[0.0, 100.0, 100.0, 0.0, alpha],
            ),
        ];
        for (representation, labels, values) in cases {
            format(&mut state, representation);
            assert_eq!(state.channel_count(), labels.len());
            for (index, (label, expected)) in labels.iter().zip(values).enumerate() {
                assert_eq!(state.channel_label(index), *label);
                close(state.channel_text(index).parse().unwrap(), *expected);
            }
            assert_eq!(state.hex(), "#FF000080");
        }
    }

    #[test]
    fn external_replacement_retains_achromatic_hue_and_refreshes_drafts() {
        let mut state = ColorPickerState::new(PickerColor::try_hsv(217.0, 0.7, 0.8, 1.0).unwrap());
        format(&mut state, ColorFormat::Hsv);
        edit(&mut state, 1, "oops").unwrap_err();
        state
            .update(ColorPickerEvent::SetColor(PickerColor::from_rgb8(0, 0, 0)))
            .unwrap();
        close(state.hue(), 217.0);
        close(state.saturation(), 0.7);
        assert!(state.channel_error(1).is_none());
        assert!(state.set_color(PickerColor::from_rgb8(128, 128, 128)));
        close(state.hue(), 217.0);
        assert_eq!(state.saturation(), 0.0);
        assert!(!state.set_color(state.color()));
        format(&mut state, ColorFormat::Hsl);
        close(state.channel_text(0).parse().unwrap(), 217.0);
        state.set_color(PickerColor::from_rgb8(0, 255, 0));
        close(state.hue(), 120.0);
    }

    #[test]
    fn invalid_indices_and_gestures_do_not_mutate_any_state() {
        let mut state = ColorPickerState::new(PickerColor::from_rgb8(10, 20, 30));
        edit(&mut state, 0, "-").unwrap_err();
        let original = format!("{state:?}");
        let events = [
            ColorPickerEvent::Format(5),
            ColorPickerEvent::Format(usize::MAX),
            ColorPickerEvent::Channel {
                index: 4,
                text: "50".into(),
            },
            ColorPickerEvent::Channel {
                index: usize::MAX,
                text: "50".into(),
            },
            ColorPickerEvent::Hue(-1.0),
            ColorPickerEvent::Hue(360.1),
            ColorPickerEvent::Hue(f32::NAN),
            ColorPickerEvent::Alpha(f32::INFINITY),
            ColorPickerEvent::Alpha(-0.1),
            ColorPickerEvent::Alpha(1.1),
            ColorPickerEvent::SaturationValue {
                saturation: f32::NAN,
                value: 0.5,
            },
            ColorPickerEvent::SaturationValue {
                saturation: 0.5,
                value: 1.1,
            },
        ];
        for event in events {
            assert!(state.update(event).is_err());
            assert_eq!(format!("{state:?}"), original);
        }
        assert_eq!(state.channel_label(usize::MAX), "");
        assert_eq!(state.channel_text(usize::MAX), "");
        assert_eq!(state.channel_error(usize::MAX), None);
        format(&mut state, ColorFormat::Hex);
        assert!(edit(&mut state, 0, "50").is_err());
    }

    #[test]
    fn numeric_parser_is_bounded_and_enforces_every_representation_range() {
        let original = PickerColor::from_rgb8(12, 34, 56);
        let mut state = ColorPickerState::new(original);
        for text in [
            "NaN", "inf", "-inf", "1e999", "-1", "256", ".", "1e", "１２", "1 2",
        ] {
            assert!(edit(&mut state, 0, text).is_err(), "{text}");
            assert_eq!(state.channel_text(0), text);
            assert!(state.channel_error(0).is_some());
            assert_eq!(state.color(), original);
        }
        assert!(matches!(
            edit(&mut state, 0, &"1".repeat(65)),
            Err(ColorError::TextTooLong { .. })
        ));
        assert!(!edit(&mut state, 0, &format!("{}12", " ".repeat(62))).unwrap());
        for representation in ColorFormat::ALL {
            format(&mut state, representation);
            for index in 0..state.channel_count() {
                let too_large = match representation {
                    ColorFormat::Rgb if index < 3 => "255.01",
                    ColorFormat::Hsl | ColorFormat::Hsv if index == 0 => "360.01",
                    _ => "100.01",
                };
                for text in [too_large, "-0.01", "NaN"] {
                    assert!(edit(&mut state, index, text).is_err());
                    assert_eq!(state.color(), original);
                }
            }
        }
    }
}
