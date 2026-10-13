//! Managed EWMH panel geometry and checked native properties.
use dpi::{PhysicalPosition, PhysicalSize};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::window::{PanelConfig, PanelEdge, PanelOutput};
use x11rb::protocol::randr::{self, Connection, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ConnectionExt as _};

use crate::atoms::AtomName::{
    _NET_SUPPORTED, _NET_SUPPORTING_WM_CHECK, _NET_WM_STRUT_PARTIAL, _NET_WM_WINDOW_TYPE,
    _NET_WM_WINDOW_TYPE_DOCK,
};
use crate::monitor::MonitorHandle;
use crate::xdisplay::XConnection;

fn unsupported(message: &'static str) -> RequestError {
    NotSupportedError::new(message).into()
}
fn native(error: impl std::error::Error + Send + Sync + 'static) -> RequestError {
    os_error!(error).into()
}

#[derive(Debug)]
pub(crate) struct PanelState {
    pub(crate) config: PanelConfig,
    pub(crate) output_name: String,
    pub(crate) requested_visible: std::sync::atomic::AtomicBool,
    pub(crate) retry: std::sync::atomic::AtomicBool,
    pub(crate) notified_scale: std::sync::Mutex<f64>,
    // Publish one validated scale/size pair, without racing redirected WM configures.
    pub(crate) committed_geometry: std::sync::Mutex<(f64, PhysicalSize<u32>)>,
}

#[derive(Debug)]
pub(crate) enum UpdateError {
    /// A complete, stable snapshot confirmed that the request is no longer possible.
    Terminal(RequestError),
    /// A native read/write failed or the topology changed while it was read.
    Retry(RequestError),
}
impl From<RequestError> for UpdateError {
    fn from(error: RequestError) -> Self {
        Self::Retry(error)
    }
}
impl From<winit_core::error::OsError> for UpdateError {
    fn from(error: winit_core::error::OsError) -> Self {
        Self::Retry(error.into())
    }
}
impl UpdateError {
    pub(crate) fn into_request(self) -> RequestError {
        match self {
            Self::Terminal(error) | Self::Retry(error) => error,
        }
    }
}

pub(crate) struct OutputSnapshot {
    pub(crate) monitor: MonitorHandle,
    pub(crate) root_size: (u32, u32),
}

pub(crate) struct CreationGuard<'a> {
    xconn: &'a XConnection,
    window: u32,
    pub(crate) armed: bool,
}
impl<'a> CreationGuard<'a> {
    pub(crate) fn new(xconn: &'a XConnection, window: u32, armed: bool) -> Self {
        Self { xconn, window, armed }
    }
}
impl Drop for CreationGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let result = self
                .xconn
                .xcb_connection()
                .destroy_window(self.window)
                .map_err(native)
                .and_then(|cookie| cookie.check().map_err(native));
            if let Err(error) = result {
                tracing::warn!(%error, "Failed to destroy incomplete X11 panel");
            }
        }
    }
}

/// Resolve real XRandR output names, including cloned outputs sharing a CRTC.
pub(crate) fn select_output(
    xconn: &XConnection,
    root: u32,
    policy: &PanelOutput,
) -> Result<OutputSnapshot, UpdateError> {
    // Xlib owns this connection's event queue. Its RandR wire converters must
    // be registered or Xlib discards events selected through x11rb. Retain the
    // library while Xlib holds its converter function pointers.
    static RANDR: std::sync::OnceLock<Result<x11_dl::xrandr::Xrandr, x11_dl::error::OpenError>> =
        std::sync::OnceLock::new();
    let randr = RANDR.get_or_init(x11_dl::xrandr::Xrandr::open).as_ref().map_err(|_| {
        unsupported("X11 panels require the XRandR client library for output events")
    })?;
    let (mut major, mut minor) = (1, 3);
    // SAFETY: the retained XConnection owns a live Display and writable integers.
    if unsafe { (randr.XRRQueryVersion)(xconn.display, &mut major, &mut minor) } == 0 {
        return Err(unsupported("failed to initialize XRandR output events").into());
    }
    // Never join fresh output identities to the shared monitor cache. A clone
    // can disappear or move to another CRTC without invalidating that cache.
    retry_snapshot(|| read_snapshot(xconn, root, policy))
}

fn retry_snapshot(
    mut read: impl FnMut() -> Result<Option<OutputSnapshot>, UpdateError>,
) -> Result<OutputSnapshot, UpdateError> {
    for _ in 0..3 {
        if let Some(snapshot) = read()? {
            return Ok(snapshot);
        }
    }
    Err(UpdateError::Retry(unsupported(
        "XRandR panel topology changed during three snapshot attempts",
    )))
}

fn read_snapshot(
    xconn: &XConnection,
    root: u32,
    policy: &PanelOutput,
) -> Result<Option<OutputSnapshot>, UpdateError> {
    let conn = xconn.xcb_connection();
    let resources =
        conn.randr_get_screen_resources_current(root).map_err(native)?.reply().map_err(native)?;
    let geometry = conn.get_geometry(root).map_err(native)?.reply().map_err(native)?;
    let pointer = match policy {
        PanelOutput::Default => {
            Some(conn.query_pointer(root).map_err(native)?.reply().map_err(native)?)
        },
        PanelOutput::Name(_) => None,
        _ => return Err(UpdateError::Terminal(unsupported("unsupported panel output policy"))),
    };
    let mut outputs = Vec::new();
    let mut valid = true;
    for &output in &resources.outputs {
        let info = conn
            .randr_get_output_info(output, resources.config_timestamp)
            .map_err(native)?
            .reply()
            .map_err(native)?;
        valid &= info.status == randr::SetConfig::SUCCESS;
        if info.connection != Connection::CONNECTED || info.crtc == 0 {
            continue;
        }
        let crtc = conn
            .randr_get_crtc_info(info.crtc, resources.config_timestamp)
            .map_err(native)?
            .reply()
            .map_err(native)?;
        valid &= crtc.status == randr::SetConfig::SUCCESS
            && resources.crtcs.contains(&info.crtc)
            && crtc.outputs.contains(&output)
            && crtc.width != 0
            && crtc.height != 0;
        outputs.push((info, crtc));
    }
    let after =
        conn.randr_get_screen_resources_current(root).map_err(native)?.reply().map_err(native)?;
    let root_after = conn.get_geometry(root).map_err(native)?.reply().map_err(native)?;
    if !snapshot_stable(
        &resources,
        &after,
        (geometry.width, geometry.height),
        (root_after.width, root_after.height),
        valid,
    ) {
        return Ok(None);
    }
    let modes = crate::monitor::ScreenResources::from_get_screen_resources_current_reply(resources);
    let mut matches = Vec::new();
    for (info, crtc) in outputs {
        let name = std::str::from_utf8(&info.name).map_err(native)?;
        let selected = output_selected(
            name,
            (crtc.x, crtc.y),
            (crtc.width, crtc.height),
            policy,
            pointer.as_ref().filter(|p| p.same_screen).map(|p| (p.root_x, p.root_y)),
        );
        if selected {
            let monitor =
                MonitorHandle::from_panel_snapshot(xconn, &modes, info.crtc, &crtc, &info)
                    .ok_or_else(|| {
                        UpdateError::Retry(unsupported(
                            "failed to decode panel output name or scale",
                        ))
                    })?;
            matches.push(monitor);
        }
    }
    if matches.len() != 1 {
        return Err(UpdateError::Terminal(unsupported(
            "panel output selection requires exactly one connected XRandR output; specify an \
             unambiguous name",
        )));
    }
    Ok(Some(OutputSnapshot {
        monitor: matches.remove(0),
        root_size: (u32::from(geometry.width), u32::from(geometry.height)),
    }))
}

fn snapshot_stable(
    before: &randr::GetScreenResourcesCurrentReply,
    after: &randr::GetScreenResourcesCurrentReply,
    root_before: (u16, u16),
    root_after: (u16, u16),
    replies_valid: bool,
) -> bool {
    replies_valid
        && before.timestamp == after.timestamp
        && before.config_timestamp == after.config_timestamp
        && before.outputs == after.outputs
        && before.crtcs == after.crtcs
        && root_before == root_after
}

fn contains_output(origin: (i16, i16), size: (u16, u16), pointer: (i16, i16)) -> bool {
    let (x, y) = (i32::from(pointer.0), i32::from(pointer.1));
    let (ox, oy) = (i32::from(origin.0), i32::from(origin.1));
    x >= ox && y >= oy && x < ox + i32::from(size.0) && y < oy + i32::from(size.1)
}

fn output_selected(
    name: &str,
    origin: (i16, i16),
    size: (u16, u16),
    policy: &PanelOutput,
    pointer: Option<(i16, i16)>,
) -> bool {
    match policy {
        PanelOutput::Name(requested) => name == requested,
        PanelOutput::Default => {
            pointer.is_some_and(|pointer| contains_output(origin, size, pointer))
        },
        _ => false,
    }
}

/// EWMH support is queried fresh for this root, never through the global WM cache.
pub(crate) fn require_ewmh(
    xconn: &XConnection,
    root: u32,
    reserve: bool,
) -> Result<(), RequestError> {
    let atoms = xconn.atoms();
    let check: Vec<u32> = xconn
        .get_property(root, atoms[_NET_SUPPORTING_WM_CHECK], xproto::AtomEnum::WINDOW.into())
        .map_err(native)?;
    if check.len() != 1 || check[0] == 0 || check[0] == root {
        return Err(unsupported("panel requires a valid EWMH window manager"));
    }
    let own: Vec<u32> = xconn
        .get_property(check[0], atoms[_NET_SUPPORTING_WM_CHECK], xproto::AtomEnum::WINDOW.into())
        .map_err(native)?;
    if own != check {
        return Err(unsupported("EWMH supporting WM window does not identify itself"));
    }
    let tree =
        xconn.xcb_connection().query_tree(check[0]).map_err(native)?.reply().map_err(native)?;
    if tree.root != root || tree.parent != root {
        return Err(unsupported(
            "EWMH supporting WM window must be an immediate child of the requested root",
        ));
    }
    let supported: Vec<u32> = xconn
        .get_property(root, atoms[_NET_SUPPORTED], xproto::AtomEnum::ATOM.into())
        .map_err(native)?;
    for required in [_NET_WM_WINDOW_TYPE, _NET_WM_WINDOW_TYPE_DOCK]
        .into_iter()
        .chain(reserve.then_some(_NET_WM_STRUT_PARTIAL))
    {
        if !supported.contains(&atoms[required]) {
            return Err(unsupported(
                "window manager does not advertise required EWMH panel properties",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PanelLayout {
    pub(crate) position: PhysicalPosition<i32>,
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) strut: Option<[u32; 12]>,
}

/// Compute against full output geometry, never _NET_WORKAREA (which includes us).
pub(crate) fn layout(
    config: &PanelConfig,
    origin: (i32, i32),
    output: (u32, u32),
    root: (u32, u32),
    scale: f64,
) -> Result<PanelLayout, RequestError> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(unsupported("invalid panel output scale"));
    }
    let physical = |value: u32| -> Result<i64, RequestError> {
        let rounded = (f64::from(value) * scale).round();
        if rounded > f64::from(i32::MAX) {
            return Err(unsupported("panel physical geometry exceeds coordinate range"));
        }
        Ok(rounded as i64)
    };
    let (ox, oy) = (i64::from(origin.0), i64::from(origin.1));
    let (ow, oh) = (i64::from(output.0), i64::from(output.1));
    let (rw, rh) = (i64::from(root.0), i64::from(root.1));
    if ox < 0 || oy < 0 || ow == 0 || oh == 0 || ox + ow > rw || oy + oh > rh {
        return Err(unsupported("panel output lies outside the X11 root"));
    }
    let m = config.margins();
    let (top, right, bottom, left) =
        (physical(m.top)?, physical(m.right)?, physical(m.bottom)?, physical(m.left)?);
    let thickness = physical(config.thickness())?;
    let horizontal = matches!(config.edge(), PanelEdge::Top | PanelEdge::Bottom);
    let available = if horizontal { ow - left - right } else { oh - top - bottom };
    let length = if config.length() == 0 { available } else { physical(config.length())? };
    if length <= 0
        || thickness <= 0
        || length > available
        || thickness > if horizontal { oh - top - bottom } else { ow - left - right }
    {
        return Err(unsupported(
            "panel length, thickness or margins do not fit the selected output",
        ));
    }
    let (width, height) = if horizontal { (length, thickness) } else { (thickness, length) };
    let x = match config.edge() {
        PanelEdge::Left => ox + left,
        PanelEdge::Right => ox + ow - right - width,
        _ => ox + left + (available - length) / 2,
    };
    let y = match config.edge() {
        PanelEdge::Top => oy + top,
        PanelEdge::Bottom => oy + oh - bottom - height,
        _ => oy + top + (available - length) / 2,
    };
    if x > i16::MAX as i64
        || y > i16::MAX as i64
        || width > u16::MAX as i64
        || height > u16::MAX as i64
    {
        return Err(unsupported("panel geometry exceeds X11 wire coordinates"));
    }
    let strut = if let Some(zone) = config.reservation() {
        let (index, margin, outer, span) = match config.edge() {
            PanelEdge::Left => (0, left, ox == 0, rw),
            PanelEdge::Right => (1, right, ox + ow == rw, rw),
            PanelEdge::Top => (2, top, oy == 0, rh),
            PanelEdge::Bottom => (3, bottom, oy + oh == rh, rh),
            _ => return Err(unsupported("unsupported panel border")),
        };
        if !outer {
            return Err(unsupported(
                "reserved X11 panel edge must coincide with an outer root border",
            ));
        }
        let distance = physical(zone)? + margin;
        if distance <= 0 || distance > span {
            return Err(unsupported("panel reservation exceeds root bounds"));
        }
        let mut strut = [0; 12];
        strut[index] = distance as u32;
        let (start, end) = if horizontal { (x, x + width - 1) } else { (y, y + height - 1) };
        strut[4 + index * 2] = start as u32;
        strut[5 + index * 2] = end as u32;
        Some(strut)
    } else {
        None
    };
    Ok(PanelLayout {
        position: PhysicalPosition::new(x as i32, y as i32),
        size: PhysicalSize::new(width as u32, height as u32),
        strut,
    })
}

pub(crate) fn set_reservation(
    xconn: &XConnection,
    window: u32,
    strut: Option<[u32; 12]>,
) -> Result<(), RequestError> {
    let atom = xconn.atoms()[_NET_WM_STRUT_PARTIAL];
    if let Some(strut) = strut {
        xconn
            .change_property(
                window,
                atom,
                xproto::AtomEnum::CARDINAL.into(),
                xproto::PropMode::REPLACE,
                &strut,
            )
            .map_err(native)?
            .check()
            .map_err(native)?;
    } else {
        xconn
            .xcb_connection()
            .delete_property(window, atom)
            .map_err(native)?
            .check()
            .map_err(native)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use winit_core::window::PanelMargins;

    use super::*;
    #[test]
    fn reservation_rounds_base_and_anchored_margin_independently() {
        let config = PanelConfig::new("rounding", PanelEdge::Top, 0, 1)
            .unwrap()
            .with_reservation(Some(1))
            .unwrap()
            .with_margins(PanelMargins { top: 1, ..Default::default() })
            .unwrap();
        let layout = layout(&config, (0, 0), (100, 100), (100, 100), 1.5).unwrap();
        assert_eq!(layout.position.y, 2);
        assert_eq!(layout.size.height, 2);
        assert_eq!(layout.strut.unwrap()[2], 4);
        assert_eq!(
            config.layer_shell_config().exclusive_zone(),
            winit_core::window::LayerExclusiveZone::Reserve(2)
        );
    }
    #[test]
    fn changing_topology_retries_then_accepts_the_reassigned_output() {
        let mut attempts = 0;
        let snapshot = retry_snapshot(|| {
            attempts += 1;
            if attempts == 1 {
                return Ok(None);
            }
            let mut monitor = MonitorHandle::dummy();
            monitor.name = "clone-B".into();
            monitor.id = 7;
            monitor.position = (800, 0);
            Ok(Some(OutputSnapshot { monitor, root_size: (1600, 600) }))
        })
        .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(snapshot.monitor.id, 7);
        assert_eq!(snapshot.monitor.name, "clone-B");
        assert_eq!(snapshot.monitor.position, (800, 0));
        assert_eq!(snapshot.root_size, (1600, 600));
        attempts = 0;
        let result = retry_snapshot(|| {
            attempts += 1;
            Ok(None)
        });
        assert_eq!(attempts, 3);
        assert!(
            matches!(result, Err(UpdateError::Retry(_))),
            "unstable reads must not confirm terminal loss"
        );
    }
    #[test]
    fn adjacent_outputs_use_half_open_pointer_bounds() {
        for pointer in [(799, 599), (800, 0), (800, 599), (1599, 599)] {
            let matches = [(0, 0), (800, 0)]
                .into_iter()
                .filter(|origin| contains_output(*origin, (800, 600), pointer))
                .count();
            assert_eq!(matches, 1);
        }
        assert!(!contains_output((0, 0), (800, 600), (0, 600)));
        assert!(!contains_output((800, 0), (800, 600), (1600, 0)));
    }

    #[test]
    fn named_clone_identity_survives_crtc_reassignment_but_not_removal() {
        let selected = PanelOutput::Name("clone-B".into());
        let choose = |outputs: &[(&str, (i16, i16))]| {
            outputs
                .iter()
                .filter(|(name, origin)| {
                    output_selected(name, *origin, (800, 600), &selected, None)
                })
                .map(|(_, origin)| *origin)
                .collect::<Vec<_>>()
        };
        assert_eq!(choose(&[("clone-A", (0, 0)), ("clone-B", (0, 0))]), [(0, 0)]);
        assert_eq!(choose(&[("clone-A", (0, 0)), ("clone-B", (800, 0))]), [(800, 0)]);
        assert!(choose(&[("clone-A", (0, 0))]).is_empty());
        assert_eq!(choose(&[("clone-B", (0, 0)), ("clone-B", (800, 0))]).len(), 2);
    }

    #[test]
    fn topology_generation_status_and_root_size_must_agree() {
        let before = randr::GetScreenResourcesCurrentReply {
            timestamp: 40,
            config_timestamp: 20,
            outputs: vec![1, 2],
            crtcs: vec![3],
            ..Default::default()
        };
        assert!(snapshot_stable(&before, &before, (800, 600), (800, 600), true));
        let mut after = before.clone();
        after.timestamp += 1;
        assert!(!snapshot_stable(&before, &after, (800, 600), (800, 600), true));
        after = before.clone();
        after.config_timestamp += 1;
        assert!(!snapshot_stable(&before, &after, (800, 600), (800, 600), true));
        after = before.clone();
        after.outputs.pop();
        assert!(!snapshot_stable(&before, &after, (800, 600), (800, 600), true));
        assert!(!snapshot_stable(&before, &before, (800, 600), (640, 480), true));
        assert!(!snapshot_stable(&before, &before, (800, 600), (800, 600), false));
    }
    #[test]
    fn all_edges_compact_stretch_scales_and_inclusive_struts() {
        for scale in [1.0, 1.5, 2.0] {
            for edge in [PanelEdge::Left, PanelEdge::Right, PanelEdge::Top, PanelEdge::Bottom] {
                for length in [0, 200] {
                    let panel = PanelConfig::new("panel", edge, length, 30)
                        .unwrap()
                        .with_reservation(Some(30))
                        .unwrap();
                    let result = layout(&panel, (0, 0), (1000, 800), (1000, 800), scale).unwrap();
                    let strut = result.strut.unwrap();
                    let index = match edge {
                        PanelEdge::Left => 0,
                        PanelEdge::Right => 1,
                        PanelEdge::Top => 2,
                        PanelEdge::Bottom => 3,
                        _ => unreachable!(),
                    };
                    assert_eq!(strut[index], (30.0 * scale) as u32);
                    let horizontal = index >= 2;
                    let start =
                        if horizontal { result.position.x } else { result.position.y } as u32;
                    let size = if horizontal { result.size.width } else { result.size.height };
                    assert_eq!(strut[4 + index * 2], start);
                    assert_eq!(strut[5 + index * 2], start + size - 1);
                    assert_eq!(
                        size,
                        if length == 0 {
                            if horizontal { 1000 } else { 800 }
                        } else {
                            (length as f64 * scale) as u32
                        }
                    );
                }
            }
        }
    }
    #[test]
    fn root_origins_internal_edges_margins_and_invalid_bounds() {
        let panel = PanelConfig::new("panel", PanelEdge::Left, 200, 30).unwrap();
        assert!(layout(&panel, (1000, 0), (1000, 800), (2000, 800), 1.0).is_ok());
        let reserved = panel.clone().with_reservation(Some(30)).unwrap();
        assert!(layout(&reserved, (1000, 0), (1000, 800), (2000, 800), 1.0).is_err());
        assert!(layout(&panel, (-1, 0), (1000, 800), (2000, 800), 1.0).is_err());
        for scale in [0.0, f64::NAN, f64::INFINITY, 100.0] {
            assert!(layout(&panel, (0, 0), (1000, 800), (1000, 800), scale).is_err());
        }
        let margin = reserved.with_margins(PanelMargins { left: 8, ..Default::default() }).unwrap();
        let result = layout(&margin, (0, 0), (1000, 800), (1000, 800), 1.5).unwrap();
        assert_eq!(result.position.x, 12);
        assert_eq!(result.strut.unwrap()[0], 57);
        assert!(layout(&margin, (0, 0), (10, 10), (10, 10), 1.0).is_err());
    }
}
