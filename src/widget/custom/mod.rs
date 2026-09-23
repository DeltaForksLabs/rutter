// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

mod api;
mod tree;

pub use api::{
    CUSTOM_WIDGET_API_VERSION, CustomAccessibility, CustomAccessibilityAction,
    CustomAccessibilityActions, CustomAccessibilityNode, CustomAccessibilityRole,
    CustomAccessibilityState, CustomEventOutcome, CustomInteraction, CustomLayout,
    CustomPaintContext, CustomPoint, CustomPointerEvent, CustomSize, CustomWidgetState,
    CustomWidgetStateError, CustomWidgetV1, MAX_CUSTOM_WIDGET_STATE_BYTES,
};
pub(crate) use tree::{
    collect_custom_widget_ids, collect_custom_widget_ids_at_path, custom_accessibility_is_valid,
    find_custom_widget,
};
