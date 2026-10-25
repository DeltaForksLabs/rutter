// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Field annotations reserve space without adding widget/path/ID-bearing nodes.

use skia_safe::Rect;
use taffy::prelude::{Dimension, Style};

use crate::widget::Widget;

// Fixed annotation rows keep layout independent of the active theme. Text is
// ellipsized and clipped to these rows, including with custom theme font sizes.
pub(crate) const ANNOTATION_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy, Default)]
pub(crate) struct FieldInsets {
    pub top: f32,
    pub bottom: f32,
}

impl FieldInsets {
    pub fn new(label: &str, error: Option<&str>) -> Self {
        Self {
            top: if label.is_empty() {
                0.0
            } else {
                ANNOTATION_HEIGHT
            },
            bottom: if error.is_some_and(|text| !text.is_empty()) {
                ANNOTATION_HEIGHT
            } else {
                0.0
            },
        }
    }

    pub fn for_widget<Msg>(widget: &Widget<'_, Msg>) -> Self {
        match widget {
            Widget::Disabled { child } => Self::for_widget(child),
            Widget::TextInput {
                label, error_msg, ..
            }
            | Widget::TextArea {
                label, error_msg, ..
            }
            | Widget::ComboBox {
                label, error_msg, ..
            } => Self::new(label, error_msg.as_deref()),
            Widget::Select { label, .. } => Self::new(label, None),
            _ => Self::default(),
        }
    }

    pub fn body(self, size: (f32, f32)) -> Rect {
        let top = self.top.min(size.1.max(0.0));
        Rect::from_xywh(
            0.0,
            top,
            size.0.max(0.0),
            (size.1 - top - self.bottom).max(0.0),
        )
    }

    pub fn intrinsic_height(self, body_height: f32) -> f32 {
        body_height + self.top + self.bottom
    }

    pub fn reserve(self, style: &Style) -> Style {
        let mut style = style.clone();
        let extra = self.top + self.bottom;
        // Length heights historically size the control body. Keep that body
        // intact and add annotation space; percentages retain parent sizing.
        style.size.height = add_annotation_height(style.size.height, extra);
        style.min_size.height = add_annotation_height(style.min_size.height, extra);
        style.max_size.height = add_annotation_height(style.max_size.height, extra);
        // Auto must stay auto: in particular, a row can stretch the field to its
        // cross-axis size. The leaf measure supplies its preferred height and
        // lets Taffy apply caller-provided percentage and min/max constraints.
        style
    }
}

fn add_annotation_height(height: Dimension, extra: f32) -> Dimension {
    height
        .into_option()
        .map(|height| Dimension::length(height + extra))
        .unwrap_or(height)
}
