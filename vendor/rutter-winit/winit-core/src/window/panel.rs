//! Immutable, backend-neutral desktop panel requests.
use super::{
    LayerAnchors, LayerExclusiveZone, LayerKeyboardInteractivity, LayerMargins, LayerOutput,
    LayerShellConfig, LayerShellConfigError, WindowAttributes, WindowType,
};

/// Exactly one output border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PanelEdge {
    Top,
    Right,
    Bottom,
    Left,
}

/// Output policy; enumeration order is never used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PanelOutput {
    /// Wayland: compositor choice. X11: the unique output containing the pointer
    /// at creation, pinned for the lifetime of the panel; ambiguity is an error.
    #[default]
    Default,
    /// Exact connection-local output name (XRandR on X11).
    Name(String),
}

/// Panels never activate on creation or remapping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PanelFocus {
    /// Do not accept native keyboard focus.
    #[default]
    NonActivating,
    /// Accept user-driven focus (Wayland requires layer-shell v4).
    Normal,
}

/// Logical distances from output borders; negative margins are rejected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelMargins {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

/// Validated panel settings. Reconfigure by closing and reopening the surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelConfig {
    layer: LayerShellConfig,
    edge: PanelEdge,
    length: u32,
    thickness: u32,
    margins: PanelMargins,
    output: PanelOutput,
    reservation: Option<u32>,
    focus: PanelFocus,
}

/// Invalid panel identity, geometry, or conflicting role attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelConfigError(pub &'static str);
impl std::fmt::Display for PanelConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for PanelConfigError {}
impl From<LayerShellConfigError> for PanelConfigError {
    fn from(error: LayerShellConfigError) -> Self {
        Self(error.0)
    }
}

impl PanelConfig {
    /// Length zero stretches along the border; positive lengths are centered.
    /// Thickness is positive. Defaults: no reservation, no focus, zero margins.
    pub fn new(
        namespace: impl Into<String>,
        edge: PanelEdge,
        length: u32,
        thickness: u32,
    ) -> Result<Self, PanelConfigError> {
        if thickness == 0 {
            return Err(PanelConfigError("panel thickness must be positive"));
        }
        let (mut anchors, horizontal) = match edge {
            PanelEdge::Top => (LayerAnchors::TOP, true),
            PanelEdge::Bottom => (LayerAnchors::BOTTOM, true),
            PanelEdge::Left => (LayerAnchors::LEFT, false),
            PanelEdge::Right => (LayerAnchors::RIGHT, false),
        };
        if length == 0 {
            anchors |= if horizontal {
                LayerAnchors::LEFT | LayerAnchors::RIGHT
            } else {
                LayerAnchors::TOP | LayerAnchors::BOTTOM
            };
        }
        let (width, height) = if horizontal { (length, thickness) } else { (thickness, length) };
        let layer = LayerShellConfig::new(namespace, anchors, width, height)?;
        Ok(Self {
            layer,
            edge,
            length,
            thickness,
            margins: PanelMargins::default(),
            output: PanelOutput::default(),
            reservation: None,
            focus: PanelFocus::default(),
        })
    }
    /// Select an exact output or the documented backend default policy.
    pub fn with_output(mut self, output: PanelOutput) -> Result<Self, PanelConfigError> {
        self.layer = self.layer.with_output(match &output {
            PanelOutput::Default => LayerOutput::Compositor,
            PanelOutput::Name(name) => LayerOutput::Name(name.clone()),
        })?;
        self.output = output;
        Ok(self)
    }
    /// Reserve a positive logical distance, plus the anchored margin, or none.
    pub fn with_reservation(mut self, reservation: Option<u32>) -> Result<Self, PanelConfigError> {
        self.layer = self.layer.with_exclusive_zone(match reservation {
            Some(zone) => LayerExclusiveZone::Reserve(zone),
            None => LayerExclusiveZone::None,
        })?;
        self.reservation = reservation;
        self.validate_margin_zone()?;
        Ok(self)
    }
    /// Set nonnegative logical margins fitting signed protocol coordinates.
    pub fn with_margins(mut self, margins: PanelMargins) -> Result<Self, PanelConfigError> {
        let perpendicular = match self.edge {
            PanelEdge::Top | PanelEdge::Bottom => margins.left != 0 || margins.right != 0,
            PanelEdge::Left | PanelEdge::Right => margins.top != 0 || margins.bottom != 0,
        };
        if self.length > 0 && perpendicular {
            return Err(PanelConfigError("compact panels cannot have perpendicular margins"));
        }
        if [margins.top, margins.right, margins.bottom, margins.left]
            .into_iter()
            .any(|m| m > i32::MAX as u32)
        {
            return Err(PanelConfigError("panel margins exceed signed logical coordinates"));
        }
        self.layer = self.layer.with_margins(LayerMargins {
            top: margins.top as i32,
            right: margins.right as i32,
            bottom: margins.bottom as i32,
            left: margins.left as i32,
        });
        self.margins = margins;
        self.validate_margin_zone()?;
        Ok(self)
    }
    /// Choose native keyboard policy without startup activation.
    pub fn with_focus(mut self, focus: PanelFocus) -> Self {
        self.layer = self.layer.with_keyboard_interactivity(match focus {
            PanelFocus::NonActivating => LayerKeyboardInteractivity::None,
            PanelFocus::Normal => LayerKeyboardInteractivity::OnDemand,
        });
        self.focus = focus;
        self
    }
    pub fn namespace(&self) -> &str {
        self.layer.namespace()
    }
    pub fn edge(&self) -> PanelEdge {
        self.edge
    }
    pub fn length(&self) -> u32 {
        self.length
    }
    pub fn thickness(&self) -> u32 {
        self.thickness
    }
    pub fn margins(&self) -> PanelMargins {
        self.margins
    }
    pub fn output(&self) -> &PanelOutput {
        &self.output
    }
    pub fn reservation(&self) -> Option<u32> {
        self.reservation
    }
    pub fn focus(&self) -> PanelFocus {
        self.focus
    }
    /// Wayland translation adds the anchored margin to the base reservation.
    /// Protocol-version validation is performed at creation.
    pub fn layer_shell_config(&self) -> LayerShellConfig {
        let zone = match self.reservation {
            Some(zone) => LayerExclusiveZone::Reserve(zone + self.anchored_margin()),
            None => LayerExclusiveZone::None,
        };
        // Private fields only change through validated setters: a positive base
        // plus its anchored margin fits i32, and the anchors identify one edge.
        self.layer.clone().with_exclusive_zone(zone).expect("validated neutral panel reservation")
    }
    fn anchored_margin(&self) -> u32 {
        match self.edge {
            PanelEdge::Top => self.margins.top,
            PanelEdge::Right => self.margins.right,
            PanelEdge::Bottom => self.margins.bottom,
            PanelEdge::Left => self.margins.left,
        }
    }
    fn validate_margin_zone(&self) -> Result<(), PanelConfigError> {
        if self.reservation.is_some_and(|zone| {
            u64::from(zone) + u64::from(self.anchored_margin()) > i32::MAX as u64
        }) {
            return Err(PanelConfigError(
                "panel reservation plus edge margin exceeds signed coordinates",
            ));
        }
        Ok(())
    }
}

impl WindowAttributes {
    /// Select a panel role. Conflicting role/geometry requests fail at creation.
    pub fn with_panel(mut self, config: PanelConfig) -> Self {
        self.panel = Some(config);
        self.window_type = WindowType::Panel;
        self
    }
    /// Validate panel role conflicts before allocating native resources.
    pub fn validate_panel(&self) -> Result<(), PanelConfigError> {
        if self.panel.is_none() && self.window_type != WindowType::Panel {
            return Ok(());
        }
        if self.panel.is_none()
            || self.window_type != WindowType::Panel
            || self.layer_shell.is_some()
            || self.parent_window().is_some()
        {
            return Err(PanelConfigError(
                "panel requires its own settings and cannot have another role or parent",
            ));
        }
        if self.position.is_some()
            || self.positioner.is_some()
            || self.surface_size.is_some()
            || self.min_surface_size.is_some()
            || self.max_surface_size.is_some()
            || self.surface_resize_increments.is_some()
            || self.fullscreen.is_some()
            || self.maximized
            || self.window_level != super::WindowLevel::Normal
        {
            return Err(PanelConfigError(
                "panel geometry and stacking must be specified only by PanelConfig",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge_margins(edge: PanelEdge, anchored: u32) -> PanelMargins {
        match edge {
            PanelEdge::Top => PanelMargins { top: anchored, bottom: 3, ..Default::default() },
            PanelEdge::Right => PanelMargins { right: anchored, left: 3, ..Default::default() },
            PanelEdge::Bottom => PanelMargins { bottom: anchored, top: 3, ..Default::default() },
            PanelEdge::Left => PanelMargins { left: anchored, right: 3, ..Default::default() },
        }
    }

    #[test]
    fn translated_reservation_includes_only_anchored_margin_in_either_setter_order() {
        for edge in [PanelEdge::Top, PanelEdge::Right, PanelEdge::Bottom, PanelEdge::Left] {
            for length in [0, 200] {
                let panel = PanelConfig::new("reservation", edge, length, 32).unwrap();
                let margins = edge_margins(edge, 8);
                let margin_first = panel.clone().with_margins(margins).unwrap();
                assert_eq!(
                    margin_first.layer_shell_config().exclusive_zone(),
                    LayerExclusiveZone::None
                );
                let margin_first = margin_first.with_reservation(Some(32)).unwrap();
                let zone_first =
                    panel.with_reservation(Some(32)).unwrap().with_margins(margins).unwrap();
                assert_eq!(margin_first, zone_first);
                for config in [margin_first, zone_first] {
                    assert_eq!(config.reservation(), Some(32));
                    assert_eq!(
                        config.layer_shell_config().exclusive_zone(),
                        LayerExclusiveZone::Reserve(40)
                    );
                    assert!(config.layer_shell_config().validate().is_ok());
                    assert_eq!(
                        config
                            .with_reservation(None)
                            .unwrap()
                            .layer_shell_config()
                            .exclusive_zone(),
                        LayerExclusiveZone::None
                    );
                }
            }
        }
        // Explicit layer-shell callers supply the total protocol distance themselves.
        let explicit = LayerShellConfig::new("explicit", LayerAnchors::TOP, 200, 32)
            .unwrap()
            .with_margins(LayerMargins { top: 8, ..Default::default() })
            .with_exclusive_zone(LayerExclusiveZone::Reserve(32))
            .unwrap();
        assert_eq!(explicit.exclusive_zone(), LayerExclusiveZone::Reserve(32));
    }

    #[test]
    fn reservation_plus_margin_bounds_hold_in_either_setter_order() {
        let max = i32::MAX as u32;
        for edge in [PanelEdge::Top, PanelEdge::Right, PanelEdge::Bottom, PanelEdge::Left] {
            let panel = PanelConfig::new("bounds", edge, 0, 32).unwrap();
            let margins = edge_margins(edge, 8);
            let margin_first = panel.clone().with_margins(margins).unwrap();
            for base in [max - 8, max - 7, u32::MAX, 0] {
                let first = margin_first.clone().with_reservation(Some(base));
                let second = panel
                    .clone()
                    .with_reservation(Some(base))
                    .and_then(|p| p.with_margins(margins));
                if base == max - 8 {
                    for config in [first.unwrap(), second.unwrap()] {
                        assert_eq!(config.reservation(), Some(base));
                        assert_eq!(
                            config.layer_shell_config().exclusive_zone(),
                            LayerExclusiveZone::Reserve(max)
                        );
                    }
                } else {
                    assert!(first.is_err());
                    assert!(second.is_err());
                }
            }
            let no_reservation = panel.with_margins(edge_margins(edge, max)).unwrap();
            assert_eq!(
                no_reservation.layer_shell_config().exclusive_zone(),
                LayerExclusiveZone::None
            );
        }
    }
    #[test]
    fn validated_defaults_translation_and_conflicts() {
        for edge in [PanelEdge::Top, PanelEdge::Right, PanelEdge::Bottom, PanelEdge::Left] {
            for length in [0, 400] {
                let config = PanelConfig::new("panel", edge, length, 32).unwrap();
                assert_eq!(config.focus(), PanelFocus::NonActivating);
                assert_eq!(config.reservation(), None);
                assert!(config.layer_shell_config().validate().is_ok());
                let attrs = WindowAttributes::default().with_panel(config.clone());
                assert_eq!(attrs.clone().panel, Some(config));
                assert!(attrs.validate_panel().is_ok());
                assert!(
                    attrs.with_surface_size(dpi::PhysicalSize::new(1, 1)).validate_panel().is_err()
                );
            }
        }
        assert!(PanelConfig::new("", PanelEdge::Top, 0, 32).is_err());
        assert!(PanelConfig::new("panel", PanelEdge::Top, 0, 0).is_err());
        let panel = PanelConfig::new("panel", PanelEdge::Top, 0, 32).unwrap();
        assert!(panel.clone().with_output(PanelOutput::Name("bad\0name".into())).is_err());
        assert!(panel.clone().with_reservation(Some(0)).is_err());
        assert!(
            panel
                .clone()
                .with_margins(PanelMargins { top: u32::MAX, ..Default::default() })
                .is_err()
        );
        assert!(
            panel
                .with_focus(PanelFocus::Normal)
                .layer_shell_config()
                .validate_protocol_version(3)
                .is_err()
        );
        assert!(WindowAttributes::default().validate_panel().is_ok());
    }

    #[test]
    fn compact_perpendicular_margins_are_rejected_for_every_edge() {
        for edge in [PanelEdge::Top, PanelEdge::Bottom, PanelEdge::Left, PanelEdge::Right] {
            let horizontal = matches!(edge, PanelEdge::Top | PanelEdge::Bottom);
            let perpendicular = if horizontal {
                PanelMargins { left: 8, right: 4, ..Default::default() }
            } else {
                PanelMargins { top: 8, bottom: 4, ..Default::default() }
            };
            assert!(
                PanelConfig::new("compact", edge, 200, 32)
                    .unwrap()
                    .with_margins(perpendicular)
                    .is_err()
            );
            assert!(
                PanelConfig::new("stretch", edge, 0, 32)
                    .unwrap()
                    .with_margins(perpendicular)
                    .is_ok()
            );
            let anchored = if horizontal {
                PanelMargins { top: 8, bottom: 4, ..Default::default() }
            } else {
                PanelMargins { left: 8, right: 4, ..Default::default() }
            };
            assert!(
                PanelConfig::new("compact", edge, 200, 32).unwrap().with_margins(anchored).is_ok()
            );
        }
    }
}
