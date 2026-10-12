//! Layer role construction and configure routing on Winit's existing queue.
use sctk::shell::WaylandSurface;
use sctk::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer as SctkLayer, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure, SurfaceKind,
};
use wayland_client::{Connection, Proxy, QueueHandle};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::window::{
    Layer, LayerExclusiveZone, LayerKeyboardInteractivity, LayerOutput, LayerShellConfig,
};

use crate::state::{WindowCompositorUpdate, WinitState};
use crate::window::state::WindowType;

pub(crate) fn create_layer(
    state: &WinitState,
    qh: &QueueHandle<WinitState>,
    surface: wayland_client::protocol::wl_surface::WlSurface,
    config: &LayerShellConfig,
) -> Result<WindowType, RequestError> {
    // Own the unassigned surface even on validation/support/output failure.
    let surface = sctk::compositor::Surface::from(surface);
    let unsupported = |message| RequestError::NotSupported(NotSupportedError::new(message));
    config.validate().map_err(|_| unsupported("invalid layer-shell configuration"))?;
    let shell = state.layer_shell.as_ref().ok_or_else(|| {
        unsupported(
            "zwlr_layer_shell_v1 is unavailable; native panel creation requires layer-shell",
        )
    })?;
    let output = match config.output() {
        LayerOutput::Compositor => None,
        LayerOutput::Name(name) => {
            let mut matching = state.output_state.outputs().filter(|output| {
                state.output_state.info(output).and_then(|info| info.name).as_ref() == Some(name)
            });
            let output = matching.next().ok_or_else(|| {
                unsupported("requested layer output name is unavailable on this Wayland connection")
            })?;
            if matching.next().is_some() {
                return Err(unsupported("requested layer output name is ambiguous"));
            }
            Some(output)
        },
        _ => return Err(unsupported("unsupported layer output selector")),
    };
    let layer = match config.layer() {
        Layer::Background => SctkLayer::Background,
        Layer::Bottom => SctkLayer::Bottom,
        Layer::Top => SctkLayer::Top,
        Layer::Overlay => SctkLayer::Overlay,
        _ => return Err(unsupported("unsupported layer")),
    };
    let layer =
        shell.create_layer_surface(qh, surface, layer, Some(config.namespace()), output.as_ref());
    let SurfaceKind::Wlr(role) = layer.kind() else {
        return Err(unsupported("unknown layer-shell role"));
    };
    if config.validate_protocol_version(role.version()).is_err() {
        return Err(unsupported("on-demand layer keyboard focus requires layer-shell version 4"));
    }
    let (width, height) = config.logical_size();
    layer.set_size(width, height);
    layer.set_anchor(Anchor::from_bits_truncate(config.anchors().bits()));
    let margins = config.margins();
    layer.set_margin(margins.top, margins.right, margins.bottom, margins.left);
    layer.set_exclusive_zone(match config.exclusive_zone() {
        LayerExclusiveZone::None => 0,
        LayerExclusiveZone::Ignore => -1,
        LayerExclusiveZone::Reserve(zone) => zone as i32,
        _ => return Err(unsupported("unsupported layer exclusive-zone policy")),
    });
    layer.set_keyboard_interactivity(match config.keyboard_interactivity() {
        LayerKeyboardInteractivity::None => KeyboardInteractivity::None,
        LayerKeyboardInteractivity::OnDemand => KeyboardInteractivity::OnDemand,
        LayerKeyboardInteractivity::Exclusive => KeyboardInteractivity::Exclusive,
        _ => return Err(unsupported("unsupported layer keyboard interactivity")),
    });
    Ok(WindowType::Layer { layer, configured: false, output })
}

impl LayerShellHandler for WinitState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.queue_terminal_layer(crate::make_wid(layer.wl_surface()));
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        // SCTK's dispatch sends ack_configure before entering this callback.
        let id = crate::make_wid(layer.wl_surface());
        if self.window_requests.get_mut().get(&id).is_none_or(|requests| requests.is_terminal()) {
            return;
        }
        if configure.new_size.0 > i32::MAX as u32 || configure.new_size.1 > i32::MAX as u32 {
            tracing::warn!(
                width = configure.new_size.0,
                height = configure.new_size.1,
                "layer configure exceeds signed logical viewport coordinates; closing surface"
            );
            self.queue_terminal_layer(id);
            return;
        }
        let Some(window) = self.windows.get_mut().get(&id) else {
            return;
        };
        window.lock().unwrap().configure_layer(configure.new_size);
        if let Some(update) =
            self.window_compositor_updates.iter_mut().find(|update| update.window_id == id)
        {
            update.resized = true;
        } else {
            self.window_compositor_updates.push(WindowCompositorUpdate {
                window_id: id,
                resized: true,
                scale_changed: false,
                close_window: false,
            });
        }
        self.window_requests
            .get_mut()
            .get(&id)
            .unwrap()
            .redraw_requested
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.dispatched_events = true;
    }
}
