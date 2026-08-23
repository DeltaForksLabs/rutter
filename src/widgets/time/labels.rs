// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

/// Localized labels displayed by a composed time picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimePickerLabels<'a> {
    pub hour: &'a str,
    pub minute: &'a str,
    pub second: &'a str,
    pub time_zone: &'a str,
}

impl TimePickerLabels<'static> {
    /// Built-in English labels.
    pub const ENGLISH: Self = Self {
        hour: "Hour",
        minute: "Minute",
        second: "Second",
        time_zone: "Time zone",
    };

    /// Built-in Brazilian Portuguese labels.
    pub const PORTUGUESE: Self = Self {
        hour: "Hora",
        minute: "Minuto",
        second: "Segundo",
        time_zone: "Fuso horário",
    };
}

impl Default for TimePickerLabels<'static> {
    fn default() -> Self {
        Self::ENGLISH
    }
}

#[cfg(test)]
mod tests {
    use super::TimePickerLabels;

    #[test]
    fn built_in_time_picker_labels_are_explicit() {
        assert_eq!(TimePickerLabels::PORTUGUESE.time_zone, "Fuso horário");
        assert_eq!(TimePickerLabels::default().hour, "Hour");
    }
}
