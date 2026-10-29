// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

//! Controlled, composed capsules. No native surface policy is published by a view.

mod geometry;
pub mod motion;
pub use motion::{DynamicIslandMotion, DynamicIslandSlide};

use std::fmt;

use taffy::prelude::*;

use crate::{ButtonVariant, RichText, SurfaceInputRegionError, Theme, Widget, spacing};

/// An island's immutable interaction policy, selected at construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicIslandMode {
    /// Display-only summary; always collapsed and never a keyboard stop.
    Passive,
    /// An ordinary header button toggles application-controlled details.
    Interactive,
}

/// A rejected mode operation or invalid logical input-region geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicIslandError {
    /// Passive instances reject every expansion operation, including collapse.
    PassiveExpansion,
    /// The selected view does not match the instance's immutable mode.
    ModeMismatch,
    /// Expected finite, positive bounds, nonnegative radius, and i32 pixel edges.
    InvalidGeometry,
    /// One-pixel corner bands would exceed the native 256-rectangle budget.
    GeometryTooComplex,
    /// Native rectangle validation failed.
    InputRegion(SurfaceInputRegionError),
}

impl fmt::Display for DynamicIslandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PassiveExpansion => "passive Dynamic Island rejects expansion changes",
            Self::ModeMismatch => "Dynamic Island view does not match its immutable mode",
            Self::InvalidGeometry => "invalid Dynamic Island geometry: expected finite positive logical bounds, nonnegative radius and i32 pixel edges",
            Self::GeometryTooComplex => "Dynamic Island corner bands exceed the 256-rectangle input-region budget",
            Self::InputRegion(error) => return write!(f, "construct Dynamic Island input region: {error}"),
        })
    }
}

impl std::error::Error for DynamicIslandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InputRegion(error) => Some(error),
            _ => None,
        }
    }
}

/// Owned application state with a private, lifetime-immutable mode.
///
/// Rebuilding a summary never changes expansion. Clones retain the mode and
/// current expansion, then evolve independently. There is no mode setter;
/// replacing this value with another instance is an application decision, not
/// something the widget-ID validator prevents.
///
/// Details are ordinary widgets, inserted only while expanded. Their structural
/// paths (and automatic IDs) are stable on reopening if the application supplies
/// the same structure. Use explicit IDs where supported for dynamic details.
/// Keep editor models in the application: hidden controls leave focus and
/// accessibility and may have their engine-owned transient state retired.
/// Focus is not restored on reopen.
/// Supply usable, fluid details and host tall documents in a scroll view.
/// These semantic APIs are immediate. Opt into [`DynamicIslandMotion`] in the
/// host for animated bounds, retaining immediate removal of collapsed details.
///
/// ```
/// use rutter::{DynamicIsland, DynamicIslandError, Theme, Widget};
/// use taffy::prelude::*;
/// #[derive(Clone)]
/// enum Msg { Toggle, Action }
/// let mut island = DynamicIsland::interactive();
/// island.set_expanded(true)?; // in application update, or use toggle()
/// let details = Widget::Button {
///     text: "Pause", on_press: Msg::Action, style: Style::default(),
///     color: None, variant: rutter::ButtonVariant::Primary,
/// };
/// let view = island.interactive_view(
///     "Download — collapse details", Msg::Toggle, details, &Theme::dark(),
/// )?;
/// let passive: Widget<'_, Msg> = DynamicIsland::passive()
///     .passive_view("Downloading…", &Theme::light())?;
/// # Ok::<(), DynamicIslandError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicIsland {
    mode: DynamicIslandMode,
    expanded: bool,
}

impl DynamicIsland {
    /// Painted capsule radius in logical pixels; use with the exact laid-out
    /// outer container bounds when constructing a host input region.
    pub const RADIUS: f32 = 24.0;
    /// Bounded fluid width; a parent may allocate less (including 320px).
    pub const MAX_WIDTH: f32 = 800.0;

    /// Creates a permanently collapsed, display-only instance.
    pub const fn passive() -> Self {
        Self {
            mode: DynamicIslandMode::Passive,
            expanded: false,
        }
    }

    /// Creates an initially collapsed, interactive instance.
    pub const fn interactive() -> Self {
        Self {
            mode: DynamicIslandMode::Interactive,
            expanded: false,
        }
    }

    /// Returns the construction-time policy.
    pub const fn mode(&self) -> DynamicIslandMode {
        self.mode
    }

    /// Returns the application-controlled expansion state.
    pub const fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Sets expansion; even `false` is explicitly rejected for passive instances.
    pub fn set_expanded(&mut self, expanded: bool) -> Result<(), DynamicIslandError> {
        if self.mode == DynamicIslandMode::Passive {
            return Err(DynamicIslandError::PassiveExpansion);
        }
        self.expanded = expanded;
        Ok(())
    }

    /// Toggles expansion in application update; rejects passive instances.
    pub fn toggle(&mut self) -> Result<(), DynamicIslandError> {
        self.set_expanded(!self.expanded)
    }

    /// Renders only wrapped, fully accessible text. Accepts no actionable child.
    pub fn passive_view<'a, Msg>(
        &self,
        summary: &'a str,
        theme: &Theme,
    ) -> Result<Widget<'a, Msg>, DynamicIslandError> {
        self.passive_view_with_offset(summary, (0.0, 0.0), theme)
    }

    /// Renders a summary with a clipped, paint-only logical text offset.
    pub fn passive_view_with_offset<'a, Msg>(
        &self,
        summary: &'a str,
        offset: (f32, f32),
        theme: &Theme,
    ) -> Result<Widget<'a, Msg>, DynamicIslandError> {
        if self.mode != DynamicIslandMode::Passive {
            return Err(DynamicIslandError::ModeMismatch);
        }
        Ok(capsule(vec![summary_text(summary, offset)], theme))
    }

    /// Renders an ordinary header button and, when expanded, arbitrary details.
    ///
    /// `summary` is both wrapped visual text and the full accessible button
    /// label. Localize it to include the current action, e.g. “Download — expand
    /// details” / “Download — collapse details”. Handle `on_toggle` in app update
    /// by calling [`Self::toggle`], then rebuild. Only the header toggles: child
    /// actions retain their own messages, hits, focus and accessibility nodes.
    /// The returned composition can be disabled using `Widget::enabled(false)`.
    pub fn interactive_view<'a, Msg>(
        &self,
        summary: &'a str,
        on_toggle: Msg,
        details: Widget<'a, Msg>,
        theme: &Theme,
    ) -> Result<Widget<'a, Msg>, DynamicIslandError> {
        self.interactive_view_with_offset(summary, on_toggle, details, (0.0, 0.0), theme)
    }

    /// Renders the interactive header with only its text visually translated.
    pub fn interactive_view_with_offset<'a, Msg>(
        &self,
        summary: &'a str,
        on_toggle: Msg,
        details: Widget<'a, Msg>,
        offset: (f32, f32),
        theme: &Theme,
    ) -> Result<Widget<'a, Msg>, DynamicIslandError> {
        if self.mode != DynamicIslandMode::Interactive {
            return Err(DynamicIslandError::ModeMismatch);
        }
        let header = Widget::button_content(
            summary,
            summary_text(summary, offset),
            on_toggle,
            Style {
                min_size: Size {
                    width: Dimension::length(0.0),
                    height: Dimension::length(44.0),
                },
                align_items: Some(AlignItems::Center),
                ..fluid_style()
            },
            None,
            ButtonVariant::Text,
        );
        let mut children = vec![header];
        if self.expanded {
            children.push(details);
        }
        Ok(capsule(children, theme))
    }
}

fn fluid_style() -> Style {
    Style {
        size: Size {
            width: Dimension::percent(1.0),
            height: Dimension::auto(),
        },
        min_size: Size {
            width: Dimension::length(0.0),
            height: Dimension::auto(),
        },
        flex_shrink: 0.0,
        ..Style::default()
    }
}

fn summary_text<'a, Msg>(summary: &'a str, offset: (f32, f32)) -> Widget<'a, Msg> {
    Widget::rich_text(
        RichText::plain(summary).with_visual_offset(offset.0, offset.1),
        fluid_style(),
    )
}

fn capsule<'a, Msg>(children: Vec<Widget<'a, Msg>>, theme: &Theme) -> Widget<'a, Msg> {
    Widget::Container {
        child: Box::new(Widget::Column {
            children,
            style: Style {
                flex_direction: FlexDirection::Column,
                gap: Size::length(spacing::MD),
                ..fluid_style()
            },
        }),
        style: Style {
            max_size: Size {
                width: Dimension::length(DynamicIsland::MAX_WIDTH),
                height: Dimension::auto(),
            },
            padding: Rect::length(spacing::LG),
            ..fluid_style()
        },
        color: Some(theme.surface),
        radius: DynamicIsland::RADIUS,
    }
}

#[cfg(test)]
mod tests;
