// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Shared spacing references in logical pixels (not physical display pixels).
//!
//! Choose tokens by relationship, not by rounding every dimension to the scale.
//! Control sizes, typography, borders and radii are separate design decisions.
//! Applications may use a different density when justified by content or platform.

/// `space-xs`: icon/text separation and tightly related details (4 logical px).
pub const XS: f32 = 4.0;
/// `space-sm`: labels, supporting text and compact related actions (8 logical px).
pub const SM: f32 = 8.0;
/// `space-md`: related elements within a group (12 logical px).
pub const MD: f32 = 12.0;
/// `space-lg`: separate controls and card padding (16 logical px).
pub const LG: f32 = 16.0;
/// `space-xl`: headings, content blocks and page padding (24 logical px).
pub const XL: f32 = 24.0;
/// `space-2xl`: independent sections (32 logical px).
pub const TWO_XL: f32 = 32.0;
/// `space-3xl`: major content divisions (48 logical px).
pub const THREE_XL: f32 = 48.0;
