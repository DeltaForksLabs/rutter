use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sctk::compositor::{CompositorState, Region};
use sctk::reexports::client::QueueHandle;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;
use sctk::shell::wlr_layer::{KeyboardInteractivity, LayerSurface, SurfaceKind};
use tracing::warn;
use wayland_client::Proxy;
use winit_core::error::RequestError;
use winit_core::event::WindowEvent;
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::{
    LayerKeyboardInteractivity, SurfaceInputRegion, UserAttentionType, WindowId,
};

use super::super::event_loop::sink::EventSink;
use super::super::output::MonitorHandle;
use super::super::state::WinitState;
use super::super::types::xdg_activation::XdgActivationTokenData;

#[derive(Debug)]
pub struct Handles {
    /// Handle to the main queue to perform requests.
    pub(crate) queue_handle: QueueHandle<WinitState>,

    /// Window requests to the event loop.
    pub(crate) window_requests: Arc<WindowRequests>,

    /// Observed monitors.
    pub(crate) monitors: Arc<Mutex<Vec<MonitorHandle>>>,

    /// Source to wake-up the event-loop for window requests.
    pub(crate) event_loop_awakener: calloop::ping::Ping,

    /// The event sink to deliver synthetic events.
    pub(crate) window_events_sink: Arc<Mutex<EventSink>>,

    /// Xdg activation to request user attention.
    pub(crate) xdg_activation: Option<XdgActivationV1>,

    /// The state of the requested attention from the `xdg_activation`.
    pub(crate) attention_requested: Arc<AtomicBool>,

    /// Compositor to handle WlRegion stuff.
    pub(crate) compositor: Arc<CompositorState>,
}

impl Handles {
    pub(crate) fn set_layer_keyboard_interactivity(
        &self,
        layer: Option<&LayerSurface>,
        policy: LayerKeyboardInteractivity,
    ) -> Result<(), RequestError> {
        let version = layer.and_then(|layer| match layer.kind() {
            SurfaceKind::Wlr(role) => Some(role.version()),
            _ => None,
        });
        queue_layer_keyboard_interactivity(
            self.window_requests.is_terminal(),
            version,
            policy,
            |policy| layer.expect("validated live layer role").set_keyboard_interactivity(policy),
            || self.request_redraw(),
        )
    }

    pub(crate) fn request_redraw(&self) {
        if self.window_requests.is_terminal() {
            return;
        }
        // NOTE: try to not wake up the loop when the event was already scheduled and not yet
        // processed by the loop, because if at this point the value was `true` it could only
        // mean that the loop still haven't dispatched the value to the client and will do
        // eventually, resetting it to `false`.
        if self
            .window_requests
            .redraw_requested
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            self.event_loop_awakener.ping();
        }
    }

    pub(crate) fn push_window_event(&self, event: WindowEvent, id: WindowId) {
        if self.window_requests.is_terminal() {
            return;
        }
        self.window_events_sink.lock().unwrap().push_window_event(event, id);
        self.event_loop_awakener.ping();
    }

    pub(crate) fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(
            self.monitors
                .lock()
                .unwrap()
                .clone()
                .into_iter()
                .map(|inner| CoreMonitorHandle(Arc::new(inner))),
        )
    }

    pub(crate) fn request_user_attention(
        &self,
        surface: &WlSurface,
        request_type: Option<UserAttentionType>,
    ) {
        if self.window_requests.is_terminal() {
            return;
        }
        let xdg_activation = match self.xdg_activation.as_ref() {
            Some(xdg_activation) => xdg_activation,
            None => {
                warn!("`request_user_attention` isn't supported");
                return;
            },
        };

        // Urgency is only removed by the compositor and there's no need to raise urgency when it
        // was already raised.
        if request_type.is_none() || self.attention_requested.load(Ordering::Relaxed) {
            return;
        }

        self.attention_requested.store(true, Ordering::Relaxed);
        let data = XdgActivationTokenData::Attention((
            surface.clone(),
            Arc::downgrade(&self.attention_requested),
        ));
        let xdg_activation_token = xdg_activation.get_activation_token(&self.queue_handle, data);
        xdg_activation_token.set_surface(surface);
        xdg_activation_token.commit();
    }

    pub(crate) fn set_cursor_hittest(
        &self,
        surface: &WlSurface,
        hittest: bool,
    ) -> Result<(), RequestError> {
        if self.window_requests.is_terminal() {
            return Err(RequestError::Ignored);
        }
        if hittest {
            surface.set_input_region(None);
            Ok(())
        } else {
            let region = Region::new(&*self.compositor).map_err(|err| os_error!(err))?;
            region.add(0, 0, 0, 0);
            surface.set_input_region(Some(region.wl_region()));
            Ok(())
        }
    }

    pub(crate) fn set_surface_input_region(
        &self,
        surface: &WlSurface,
        policy: &SurfaceInputRegion,
    ) -> Result<(), RequestError> {
        if self.window_requests.is_terminal() {
            return Err(RequestError::Ignored);
        }
        match policy {
            SurfaceInputRegion::Full => surface.set_input_region(None),
            SurfaceInputRegion::Empty | SurfaceInputRegion::Rectangles(_) => {
                let region = Region::new(&*self.compositor).map_err(|err| os_error!(err))?;
                if let SurfaceInputRegion::Rectangles(rectangles) = policy {
                    for rect in rectangles.as_slice() {
                        let (x, y, width, height) = rect.logical_rect();
                        region.add(x, y, width, height);
                    }
                }
                surface.set_input_region(Some(region.wl_region()));
            },
            _ => {
                return Err(winit_core::error::NotSupportedError::new(
                    "unknown surface input region policy",
                )
                .into());
            },
        }
        // Input regions are double-buffered. Do not commit outside presentation:
        // schedule a frame so the renderer intentionally commits pending state.
        self.request_redraw();
        Ok(())
    }
}

// Keep validation ahead of every protocol request and schedule presentation only
// after queuing the double-buffered layer state, as for input regions.
fn queue_layer_keyboard_interactivity(
    terminal: bool,
    version: Option<u32>,
    policy: LayerKeyboardInteractivity,
    queue: impl FnOnce(KeyboardInteractivity),
    redraw: impl FnOnce(),
) -> Result<(), RequestError> {
    use winit_core::error::NotSupportedError;
    if terminal {
        return Err(RequestError::Ignored);
    }
    let Some(version) = version.filter(|version| *version > 0) else {
        return Err(NotSupportedError::new("expected a live wlr-layer-shell surface").into());
    };
    let policy = match policy {
        LayerKeyboardInteractivity::None => KeyboardInteractivity::None,
        LayerKeyboardInteractivity::Exclusive => KeyboardInteractivity::Exclusive,
        LayerKeyboardInteractivity::OnDemand if version >= 4 => KeyboardInteractivity::OnDemand,
        LayerKeyboardInteractivity::OnDemand => {
            return Err(NotSupportedError::new(
                "on-demand layer keyboard focus requires layer-shell version 4",
            )
            .into());
        },
        _ => return Err(NotSupportedError::new("unknown layer keyboard interactivity").into()),
    };
    queue(policy);
    redraw();
    Ok(())
}

#[cfg(test)]
mod keyboard_tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn keyboard_policy_validation_precedes_native_queue_and_redraw() {
        for (policy, version, terminal, succeeds) in [
            (LayerKeyboardInteractivity::None, Some(1), false, true),
            (LayerKeyboardInteractivity::Exclusive, Some(1), false, true),
            (LayerKeyboardInteractivity::OnDemand, Some(4), false, true),
            (LayerKeyboardInteractivity::OnDemand, Some(3), false, false),
            (LayerKeyboardInteractivity::Exclusive, None, false, false),
            (LayerKeyboardInteractivity::None, Some(0), false, false),
            (LayerKeyboardInteractivity::Exclusive, Some(4), true, false),
        ] {
            let operations = RefCell::new(Vec::new());
            let result = queue_layer_keyboard_interactivity(
                terminal,
                version,
                policy,
                |_| operations.borrow_mut().push("set_keyboard_interactivity"),
                || operations.borrow_mut().push("redraw"),
            );
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(
                *operations.borrow(),
                if succeeds { vec!["set_keyboard_interactivity", "redraw"] } else { vec![] }
            );
            if terminal {
                assert!(matches!(result, Err(RequestError::Ignored)));
            } else if !succeeds {
                assert!(matches!(result, Err(RequestError::NotSupported(_))));
            }
        }
    }
}

/// The request from the window to the event loop.
#[derive(Debug)]
pub struct WindowRequests {
    /// Sticky native-lifetime invalidation, independent of application close requests.
    pub terminal: AtomicBool,
    /// The window was closed.
    pub closed: AtomicBool,

    /// Redraw Requested.
    pub redraw_requested: AtomicBool,
}

impl WindowRequests {
    pub fn is_terminal(&self) -> bool {
        self.terminal.load(Ordering::Acquire)
    }
    pub fn mark_terminal(&self) {
        self.terminal.store(true, Ordering::Release);
        self.redraw_requested.store(false, Ordering::Relaxed);
    }
    pub fn take_closed(&self) -> bool {
        self.closed.swap(false, Ordering::Relaxed)
    }

    pub fn take_redraw_requested(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::Relaxed)
    }
}
