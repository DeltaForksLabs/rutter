// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use taffy::prelude::*;

use super::controls::ColorControl;
use super::{ColorFormat, ColorPickerState, PickerColor};
use crate::{ButtonVariant, InputState, Widget, WidgetId, WidgetIdError, spacing};

// Original overlapping-page outline, embedded rather than fetched at runtime.
// Neutral slate keeps the icon visible on both light and dark surfaces.
const COPY_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#788496" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
<path d="M7 16H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v2"/>
<rect x="8" y="8" width="13" height="13" rx="2"/>
</svg>"##;

// Editor body/hit-target dimension. Labels and errors add their own annotation
// rows; this is not the old annotation-inclusive 68px minimum-height workaround.
const EDITOR_BODY_HEIGHT: f32 = 48.0;

/// Eleven consecutive manual IDs reserved for one color picker instance.
///
/// Keep these IDs stable across edits and representation changes. Other widgets
/// (including another picker) must not use this range. IDs are not derived from
/// the selected color, so editing never resets input focus to synchronize text.
#[derive(Clone, Copy, Debug)]
pub struct ColorPickerIds {
    ids: [WidgetId; 11],
}

impl ColorPickerIds {
    /// Reserves `first..=first + 10` in the manual-ID namespace.
    pub fn new(first: u64) -> Result<Self, WidgetIdError> {
        WidgetId::manual(first)?;
        // A validated manual ID is below 2^63, so adding ten cannot overflow u64.
        WidgetId::manual(first + 10)?;
        Ok(Self {
            ids: std::array::from_fn(|index| {
                WidgetId::manual(first + index as u64).expect("validated picker ID range")
            }),
        })
    }

    /// The saturation/value plane's stable ID.
    pub const fn plane(self) -> WidgetId {
        self.ids[0]
    }
    /// The hue strip's stable ID.
    pub const fn hue(self) -> WidgetId {
        self.ids[1]
    }
    /// The opacity strip's stable ID.
    pub const fn alpha(self) -> WidgetId {
        self.ids[2]
    }
    /// The color preview's stable ID.
    pub const fn preview(self) -> WidgetId {
        self.ids[3]
    }
    /// The representation selector's stable ID.
    pub const fn format(self) -> WidgetId {
        self.ids[4]
    }
    /// The HEX editor's stable ID, distinct from numeric editors.
    pub const fn hex(self) -> WidgetId {
        self.ids[5]
    }
    /// One of the five numeric editors, or `None` for an invalid index.
    pub fn channel(self, index: usize) -> Option<WidgetId> {
        self.ids[6..].get(index).copied()
    }
}

/// Typed callbacks for a controlled color picker.
///
/// Like TimePicker, each existing text/select control uses a function pointer.
/// Translate callbacks to [`super::ColorPickerEvent`] in application update and
/// call [`ColorPickerState::update`]. The channel callback index follows the
/// current representation, including percentage alpha as its last channel.
pub struct ColorPickerCallbacks<Msg> {
    pub(super) saturation_value: fn((f32, f32)) -> Msg,
    pub(super) hue: fn(f32) -> Msg,
    pub(super) alpha: fn(f32) -> Msg,
    pub(super) format: fn(usize) -> Msg,
    pub(super) hex: fn(String) -> Msg,
    pub(super) channels: [fn(String) -> Msg; 5],
    copy: Option<Msg>,
}

impl<Msg> ColorPickerCallbacks<Msg> {
    /// Creates callbacks for unit SV/alpha, hue degrees, format index, HEX and
    /// up to five channel drafts. See `cargo run -- color_picker` for a full app.
    pub const fn new(
        saturation_value: fn((f32, f32)) -> Msg,
        hue: fn(f32) -> Msg,
        alpha: fn(f32) -> Msg,
        format: fn(usize) -> Msg,
        hex: fn(String) -> Msg,
        channels: [fn(String) -> Msg; 5],
    ) -> Self {
        Self {
            saturation_value,
            hue,
            alpha,
            format,
            hex,
            channels,
            copy: None,
        }
    }

    /// Adds an accessible SVG copy button to the right of the color editors.
    ///
    /// Handle `on_copy` in application update by writing
    /// `state.color().to_hex()` to the injected clipboard and reporting failures.
    /// This always copies the last valid color as `#RRGGBBAA`, not a potentially
    /// incomplete draft or the selected representation's channel spelling.
    /// The widget itself performs no clipboard I/O; existing constructors and
    /// the eleven manual IDs are unchanged. Copy is absent unless configured.
    ///
    /// ```
    /// use rutter::{ColorPickerCallbacks, ColorPickerIds, ColorPickerState, PickerColor, Widget};
    /// enum Msg { Edit, CopyColor }
    /// let callbacks = ColorPickerCallbacks::new(
    ///     |_| Msg::Edit, |_| Msg::Edit, |_| Msg::Edit, |_| Msg::Edit,
    ///     |_| Msg::Edit, [|_| Msg::Edit; 5],
    /// ).with_copy_button(Msg::CopyColor);
    /// let state = ColorPickerState::new(PickerColor::from_rgb8(103, 80, 164));
    /// let picker = Widget::color_picker(&state, ColorPickerIds::new(100)?, callbacks, taffy::Style::default());
    /// # Ok::<(), rutter::WidgetIdError>(())
    /// ```
    pub fn with_copy_button(mut self, on_copy: Msg) -> Self {
        self.copy = Some(on_copy);
        self
    }
}

impl<'a, Msg: 'a> Widget<'a, Msg> {
    /// Creates an inline controlled Color Picker with SV plane, hue/alpha strips,
    /// color preview, format selection and editable numeric/HEX fields.
    ///
    /// Store the picker state in the application and apply callbacks through its
    /// reducer. Invalid drafts remain visible without changing the selected color.
    /// The outer style normally specifies a width of about 360 logical pixels;
    /// the height is intrinsic and numeric fields wrap at narrower widths.
    ///
    /// Keyboard: plane arrows adjust S/V, Page Up/Down adjust V by ten percent,
    /// Home/End select minimum/maximum S. Strips use arrows, Home/End and Page
    /// Up/Down. Channel fields provide the text-editable alternative to dragging.
    /// The whole result supports `.enabled(false)` and ordinary popover hosting.
    /// Use [`ColorPickerCallbacks::with_copy_button`] to add clipboard interaction.
    ///
    /// ```
    /// use rutter::{ColorPickerCallbacks, ColorPickerIds, ColorPickerState, PickerColor, Widget};
    /// let state = ColorPickerState::new(PickerColor::from_rgb8(103, 80, 164));
    /// let callbacks = ColorPickerCallbacks::new(|_| (), |_| (), |_| (), |_| (), |_| (), [|_| (); 5]);
    /// let picker = Widget::color_picker(&state, ColorPickerIds::new(100)?, callbacks, taffy::Style::default());
    /// # Ok::<(), rutter::WidgetIdError>(())
    /// ```
    pub fn color_picker(
        state: &'a ColorPickerState,
        ids: ColorPickerIds,
        callbacks: ColorPickerCallbacks<Msg>,
        mut style: Style,
    ) -> Self {
        style.flex_direction = FlexDirection::Column;
        style.gap = Size::length(spacing::MD);
        let plane = ColorControl::Plane {
            hue: state.hue(),
            saturation: state.saturation(),
            value: state.value(),
            on_change: callbacks.saturation_value,
        };
        let preview = preview_and_strips(state, ids, &callbacks);
        let editors = channel_editors(state, ids, &callbacks);
        let editors = match callbacks.copy {
            Some(on_copy) => editors_with_copy(editors, on_copy),
            None => editors,
        };
        Self::Column {
            children: vec![
                Self::custom(ids.plane(), plane, control_style(190.0)),
                preview,
                Self::select(
                    &ColorFormat::LABELS,
                    state.format().index(),
                    callbacks.format,
                    control_style(40.0),
                    "Color format",
                    "Choose a format",
                )
                .with_id(ids.format().get()),
                editors,
            ],
            style,
        }
    }

    /// Draws a color over the theme surface, using a checkerboard only at zero alpha.
    /// This is a visual-only custom leaf with a descriptive accessibility node.
    pub fn color_swatch(color: PickerColor, id: WidgetId, style: Style) -> Self {
        Self::custom(id, ColorControl::Preview { color }, style)
    }
}

fn editors_with_copy<Msg>(mut editors: Widget<'_, Msg>, on_copy: Msg) -> Widget<'_, Msg> {
    // HEX previously requested 100% width. In a row it must share the available
    // width with the fixed button, while numeric channels can continue wrapping.
    let style = match &mut editors {
        Widget::Row { style, .. } | Widget::TextInput { style, .. } => style,
        _ => unreachable!("color editors are numeric rows or a HEX input"),
    };
    style.size.width = Dimension::length(0.0);
    style.min_size.width = Dimension::length(0.0);
    style.flex_grow = 1.0;
    Widget::Row {
        children: vec![editors, copy_button(on_copy)],
        style: Style {
            align_items: Some(AlignItems::FlexStart),
            flex_shrink: 0.0,
            gap: Size::length(spacing::SM),
            ..Style::default()
        },
    }
}

fn copy_button<'a, Msg>(on_copy: Msg) -> Widget<'a, Msg> {
    Widget::button_content(
        "Copy color as HEX",
        Widget::Image {
            data: COPY_ICON_SVG,
            style: Style {
                size: Size::length(24.0_f32),
                ..Style::default()
            },
            radius: 0.0,
        },
        on_copy,
        Style {
            size: Size::length(44.0_f32),
            flex_shrink: 0.0,
            margin: Rect {
                // Optical/body alignment: this is the field's fixed annotation
                // row, not inter-control spacing (independent of Theme.spacing).
                top: LengthPercentageAuto::length(20.0),
                ..Rect::zero()
            },
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            ..Style::default()
        },
        None,
        ButtonVariant::Ghost,
    )
}

fn preview_and_strips<'a, Msg: 'a>(
    state: &ColorPickerState,
    ids: ColorPickerIds,
    callbacks: &ColorPickerCallbacks<Msg>,
) -> Widget<'a, Msg> {
    Widget::Row {
        children: vec![
            Widget::color_swatch(
                state.color(),
                ids.preview(),
                Style {
                    size: Size::from_lengths(58.0, 58.0),
                    flex_shrink: 0.0,
                    ..Style::default()
                },
            ),
            Widget::Column {
                children: vec![
                    Widget::custom(
                        ids.hue(),
                        ColorControl::Hue {
                            hue: state.hue(),
                            on_change: callbacks.hue,
                        },
                        control_style(24.0),
                    ),
                    Widget::custom(
                        ids.alpha(),
                        ColorControl::Alpha {
                            color: state.color(),
                            on_change: callbacks.alpha,
                        },
                        control_style(24.0),
                    ),
                ],
                style: Style {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_size: Size::from_lengths(0.0, 0.0),
                    gap: Size::length(spacing::SM),
                    ..Style::default()
                },
            },
        ],
        style: Style {
            gap: Size::length(spacing::MD),
            ..Style::default()
        },
    }
}

fn channel_editors<'a, Msg>(
    state: &'a ColorPickerState,
    ids: ColorPickerIds,
    callbacks: &ColorPickerCallbacks<Msg>,
) -> Widget<'a, Msg> {
    if state.format() == ColorFormat::Hex {
        return draft_input(
            state.hex(),
            callbacks.hex,
            "HEX (RRGGBB or RRGGBBAA)",
            state.hex_error(),
            ids.hex(),
            control_style(EDITOR_BODY_HEIGHT),
        );
    }
    Widget::Row {
        children: (0..state.channel_count())
            .map(|index| {
                let label = channel_label(state.format(), index);
                draft_input(
                    state.channel_text(index),
                    callbacks.channels[index],
                    label,
                    state.channel_error(index),
                    ids.channel(index).expect("five picker channels"),
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::length(66.0),
                        min_size: Size::from_lengths(60.0, EDITOR_BODY_HEIGHT),
                        ..control_style(EDITOR_BODY_HEIGHT)
                    },
                )
            })
            .collect(),
        style: Style {
            flex_wrap: FlexWrap::Wrap,
            gap: Size::length(spacing::SM),
            ..Style::default()
        },
    }
}

fn channel_label(format: ColorFormat, index: usize) -> &'static str {
    match format {
        ColorFormat::Rgb => ["Red (0-255)", "Green (0-255)", "Blue (0-255)", "Alpha (%)"][index],
        ColorFormat::Hsl => ["Hue (°)", "Saturation (%)", "Lightness (%)", "Alpha (%)"][index],
        ColorFormat::Hsv => ["Hue (°)", "Saturation (%)", "Value (%)", "Alpha (%)"][index],
        ColorFormat::Cmyk => [
            "Cyan (%)",
            "Magenta (%)",
            "Yellow (%)",
            "Black (%)",
            "Alpha (%)",
        ][index],
        ColorFormat::Hex => "HEX",
    }
}

fn draft_input<'a, Msg>(
    value: &'a str,
    on_change: fn(String) -> Msg,
    label: &'a str,
    error: Option<&str>,
    id: WidgetId,
    style: Style,
) -> Widget<'a, Msg> {
    Widget::controlled_text_input(
        value,
        on_change,
        None,
        style,
        label,
        "",
        if error.is_some() {
            InputState::Error
        } else {
            InputState::Idle
        },
        error.map(str::to_owned),
        false,
    )
    .with_id(id.get())
}

fn control_style(height: f32) -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::length(height),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}
