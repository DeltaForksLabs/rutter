// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Controlled sRGB color selection with RGB, HEX, HSL, HSV and mathematical CMYK.
//!
//! The picker composes native text/select controls and gradient custom leaves;
//! it can be used inline or hosted in an ordinary [`crate::Widget::popover`].
//! CMYK conversion is mathematical, not an ICC/print-profile simulation.
//! Colors use finite unpremultiplied encoded-sRGB `f32` components. Conversions
//! are approximate (typically allow `1e-6` for unit-component round trips and
//! more for hue degrees); only byte/HEX export rounds to eight bits.
//! Apply every event to [`ColorPickerState`] and publish the resulting view even
//! when its reducer returns `false` or an error: those can still update drafts,
//! representation, retained hue and validation feedback. See
//! `cargo run -- color_picker` for the interactive inline/popover example.

mod color;
mod controls;
mod paint;
mod state;
mod widgets;

pub use color::{ColorError, PickerColor};
pub use state::{ColorFormat, ColorPickerEvent, ColorPickerState};
pub use widgets::{ColorPickerCallbacks, ColorPickerIds};

#[cfg(test)]
mod widget_tests;
