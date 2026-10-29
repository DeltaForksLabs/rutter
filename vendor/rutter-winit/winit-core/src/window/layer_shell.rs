//! Connection-independent, validated wlr-layer-shell requests.
use std::fmt;

bitflags::bitflags! {
    /// Output edges, in surface-local logical coordinates.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LayerAnchors: u32 {
        const TOP = 1;
        const BOTTOM = 2;
        const LEFT = 4;
        const RIGHT = 8;
    }
}

/// Output selection never uses enumeration order as a primary output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayerOutput {
    /// Pass a null output and let the compositor choose.
    #[default]
    Compositor,
    /// Match the compositor's wl_output/xdg-output name on this connection.
    Name(String),
}

/// Native stacking layer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Layer {
    /// Behind desktop windows and bottom-layer surfaces.
    Background,
    /// Below ordinary desktop windows.
    Bottom,
    /// Above ordinary desktop windows; the default panel layer.
    #[default]
    Top,
    /// Above top-layer surfaces.
    Overlay,
}

/// Keyboard policy. The default does not steal application focus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayerKeyboardInteractivity {
    /// No native keyboard focus; normal applications retain focus.
    #[default]
    None,
    /// Normal user-driven focus; requires protocol version 4.
    OnDemand,
    /// Explicitly request exclusive keyboard focus; intended for modal shell UI.
    Exclusive,
}

/// Work-area policy, in logical pixels. Margins are included in a reservation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayerExclusiveZone {
    /// Respect other reservations, without reserving space.
    #[default]
    None,
    /// Ignore other reservations; equivalent to protocol -1.
    Ignore,
    /// Reserve a positive logical distance from a single anchored edge.
    Reserve(u32),
}

/// Signed logical margins (negative values allow intentional overlap).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayerMargins {
    /// Signed distance from the top anchor.
    pub top: i32,
    /// Signed distance from the right anchor.
    pub right: i32,
    /// Signed distance from the bottom anchor.
    pub bottom: i32,
    /// Signed distance from the left anchor.
    pub left: i32,
}

/// Initial role settings, selected before the first surface commit. Keyboard
/// interactivity can subsequently change through the fallible Window setter;
/// role identity and geometry remain fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerShellConfig {
    namespace: String,
    anchors: LayerAnchors,
    size: (u32, u32),
    output: LayerOutput,
    layer: Layer,
    margins: LayerMargins,
    exclusive_zone: LayerExclusiveZone,
    keyboard: LayerKeyboardInteractivity,
}

/// A request would violate protocol geometry or identity constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerShellConfigError(pub &'static str);
impl fmt::Display for LayerShellConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for LayerShellConfigError {}

impl LayerShellConfig {
    /// Validates settings requiring a newer negotiated protocol version.
    pub fn validate_protocol_version(&self, version: u32) -> Result<(), LayerShellConfigError> {
        if version == 0 {
            return Err(LayerShellConfigError("layer-shell protocol is unavailable"));
        }
        if self.keyboard == LayerKeyboardInteractivity::OnDemand && version < 4 {
            return Err(LayerShellConfigError(
                "on-demand layer keyboard focus requires layer-shell version 4",
            ));
        }
        Ok(())
    }
    /// Zero width/height delegates that axis to the compositor and requires both
    /// opposing anchors. Nonzero lengths preserve compact/islands layouts.
    pub fn new(
        namespace: impl Into<String>,
        anchors: LayerAnchors,
        width: u32,
        height: u32,
    ) -> Result<Self, LayerShellConfigError> {
        let config = Self {
            namespace: namespace.into(),
            anchors,
            size: (width, height),
            output: LayerOutput::default(),
            layer: Layer::default(),
            margins: LayerMargins::default(),
            exclusive_zone: LayerExclusiveZone::default(),
            keyboard: LayerKeyboardInteractivity::default(),
        };
        config.validate()?;
        Ok(config)
    }
    /// Selects compositor policy or an exact, case-sensitive compositor output name.
    pub fn with_output(mut self, output: LayerOutput) -> Result<Self, LayerShellConfigError> {
        self.output = output;
        self.validate()?;
        Ok(self)
    }
    /// Validates a reservation edge and logical zone, or explicitly disables reservation.
    pub fn with_exclusive_zone(
        mut self,
        zone: LayerExclusiveZone,
    ) -> Result<Self, LayerShellConfigError> {
        self.exclusive_zone = zone;
        self.validate()?;
        Ok(self)
    }
    /// Selects stacking; defaults to Top.
    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.layer = layer;
        self
    }
    /// Sets signed logical margins; defaults to zero on every side.
    pub fn with_margins(mut self, margins: LayerMargins) -> Self {
        self.margins = margins;
        self
    }
    /// Selects keyboard policy; defaults to None. OnDemand requires protocol version 4.
    pub fn with_keyboard_interactivity(mut self, keyboard: LayerKeyboardInteractivity) -> Self {
        self.keyboard = keyboard;
        self
    }
    /// Returns the validated application namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Returns requested output edges.
    pub fn anchors(&self) -> LayerAnchors {
        self.anchors
    }
    /// Returns requested logical width/height, including compositor-sized zero axes.
    pub fn logical_size(&self) -> (u32, u32) {
        self.size
    }
    /// Returns the explicit output selection policy.
    pub fn output(&self) -> &LayerOutput {
        &self.output
    }
    /// Returns requested stacking.
    pub fn layer(&self) -> Layer {
        self.layer
    }
    /// Returns signed logical margins.
    pub fn margins(&self) -> LayerMargins {
        self.margins
    }
    /// Returns requested work-area policy.
    pub fn exclusive_zone(&self) -> LayerExclusiveZone {
        self.exclusive_zone
    }
    /// Returns requested keyboard policy.
    pub fn keyboard_interactivity(&self) -> LayerKeyboardInteractivity {
        self.keyboard
    }
    /// Checks protocol geometry, signed coordinate ranges and bounded NUL-free identities.
    pub fn validate(&self) -> Result<(), LayerShellConfigError> {
        let fail = |s| Err(LayerShellConfigError(s));
        if self.namespace.is_empty() || self.namespace.len() > 1024 || self.namespace.contains('\0')
        {
            return fail("layer namespace must contain 1..=1024 bytes and no NUL");
        }
        if self.anchors.bits() & !LayerAnchors::all().bits() != 0 {
            return fail("layer anchors contain unknown edge flags");
        }
        if let LayerOutput::Name(name) = &self.output {
            if name.is_empty() || name.len() > 1024 || name.contains('\0') {
                return fail("output name must contain 1..=1024 bytes and no NUL");
            }
        }
        if self.size.0 > i32::MAX as u32 || self.size.1 > i32::MAX as u32 {
            return fail("layer dimensions exceed the signed logical coordinate range");
        }
        if self.size.0 == 0 && !self.anchors.contains(LayerAnchors::LEFT | LayerAnchors::RIGHT) {
            return fail("zero layer width requires left and right anchors");
        }
        if self.size.1 == 0 && !self.anchors.contains(LayerAnchors::TOP | LayerAnchors::BOTTOM) {
            return fail("zero layer height requires top and bottom anchors");
        }
        if let LayerExclusiveZone::Reserve(zone) = self.exclusive_zone {
            if zone == 0 || zone > i32::MAX as u32 {
                return fail("reserved zone must be positive and fit a signed logical coordinate");
            }
            let bits = self.anchors.bits();
            if !matches!(bits, 1 | 2 | 4 | 8 | 13 | 14 | 7 | 11) {
                return fail(
                    "reservation requires one edge or that edge plus its two perpendicular edges",
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_negotiation_never_silently_downgrades_on_demand() {
        let config = LayerShellConfig::new("panel", LayerAnchors::TOP, 500, 30).unwrap();
        assert!(config.validate_protocol_version(0).is_err());
        for version in 1..=4 {
            assert!(config.validate_protocol_version(version).is_ok());
        }
        let config = config.with_keyboard_interactivity(LayerKeyboardInteractivity::OnDemand);
        for version in 1..=3 {
            assert!(config.validate_protocol_version(version).is_err());
        }
        assert!(config.validate_protocol_version(4).is_ok());
    }
    #[test]
    fn validates_every_anchor_combination_and_reservation_edge() {
        for bits in 0..16 {
            let anchors = LayerAnchors::from_bits(bits).unwrap();
            let config = LayerShellConfig::new("panel", anchors, 500, 30).unwrap();
            assert_eq!(
                config.with_exclusive_zone(LayerExclusiveZone::Reserve(30)).is_ok(),
                matches!(bits, 1 | 2 | 4 | 8 | 13 | 14 | 7 | 11)
            );
            assert_eq!(
                LayerShellConfig::new("panel", anchors, 0, 30).is_ok(),
                anchors.contains(LayerAnchors::LEFT | LayerAnchors::RIGHT)
            );
            assert_eq!(
                LayerShellConfig::new("panel", anchors, 500, 0).is_ok(),
                anchors.contains(LayerAnchors::TOP | LayerAnchors::BOTTOM)
            );
        }
        assert!(
            LayerShellConfig::new("panel", LayerAnchors::from_bits_retain(16), 500, 30).is_err()
        );
    }

    #[test]
    fn validates_identity_coordinate_limits_and_nonreserving_modes() {
        for namespace in ["".to_string(), "bad\0name".to_string(), "x".repeat(1025)] {
            assert!(LayerShellConfig::new(namespace, LayerAnchors::TOP, 500, 30).is_err());
        }
        assert!(LayerShellConfig::new("panel", LayerAnchors::TOP, u32::MAX, 30).is_err());
        let config = LayerShellConfig::new("panel", LayerAnchors::TOP, 500, 30).unwrap();
        assert_eq!(config.output(), &LayerOutput::Compositor);
        assert_eq!(config.exclusive_zone(), LayerExclusiveZone::None);
        assert!(config.clone().with_output(LayerOutput::Name("bad\0output".into())).is_err());
        assert!(config.clone().with_exclusive_zone(LayerExclusiveZone::Reserve(0)).is_err());
        assert_eq!(
            config.with_exclusive_zone(LayerExclusiveZone::Ignore).unwrap().exclusive_zone(),
            LayerExclusiveZone::Ignore
        );
    }
    #[test]
    fn validates_stretched_compact_and_reserved_geometry() {
        assert!(LayerShellConfig::new("panel", LayerAnchors::TOP, 0, 30).is_err());
        assert!(LayerShellConfig::new("panel", LayerAnchors::LEFT, 30, 0).is_err());
        let compact = LayerShellConfig::new("panel", LayerAnchors::TOP, 500, 30).unwrap();
        assert_eq!(compact.keyboard_interactivity(), LayerKeyboardInteractivity::None);
        assert_eq!(compact.logical_size(), (500, 30));
        assert!(compact.clone().with_exclusive_zone(LayerExclusiveZone::Reserve(30)).is_ok());
        assert!(compact.clone().with_output(LayerOutput::Name(String::new())).is_err());
        assert!(compact.with_exclusive_zone(LayerExclusiveZone::Reserve(u32::MAX)).is_err());
        assert!(LayerShellConfig::new("", LayerAnchors::TOP, 500, 30).is_err());
        assert!(
            LayerShellConfig::new("panel", LayerAnchors::TOP | LayerAnchors::LEFT, 500, 30)
                .unwrap()
                .with_exclusive_zone(LayerExclusiveZone::Reserve(30))
                .is_err()
        );
        assert!(
            LayerShellConfig::new(
                "panel",
                LayerAnchors::TOP | LayerAnchors::LEFT | LayerAnchors::RIGHT,
                0,
                30
            )
            .is_ok()
        );
    }
}
