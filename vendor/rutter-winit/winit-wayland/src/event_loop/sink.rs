//! An event loop's sink to deliver events from the Wayland event callbacks.

use std::vec::Drain;

use wayland_client::protocol::wl_seat::WlSeat;
use winit_core::event::{DeviceEvent, WindowEvent};
use winit_core::window::WindowId;

use super::Event;
use crate::seat::PopupInput;

/// An event loop's sink to deliver events from the Wayland event callbacks
/// to the winit's user.
#[derive(Default, Debug)]
pub struct EventSink {
    pub(crate) window_events: Vec<Event>,
    /// Legacy best-effort grab selection follows receipt order, never serial magnitude.
    pub(crate) latest_input: Option<(WlSeat, u32)>,
}

impl EventSink {
    pub(crate) fn retire_windows(&mut self, retired: &[WindowId]) {
        self.window_events.retain(|event| !matches!(event, Event::WindowEvent { window_id, .. } if retired.contains(window_id)));
    }
    pub fn new() -> Self {
        Default::default()
    }

    /// Return `true` if there're pending events.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.window_events.is_empty()
    }

    /// Add new device event to a queue.
    #[inline]
    pub fn push_device_event(&mut self, event: DeviceEvent) {
        self.window_events.push(Event::DeviceEvent { event });
    }

    /// Add new window event to a queue.
    #[inline]
    pub fn push_window_event(&mut self, event: WindowEvent, window_id: WindowId) {
        self.window_events.push(Event::WindowEvent { event, window_id, popup_input: None });
    }

    pub(crate) fn record_input_serial(&mut self, seat: WlSeat, serial: u32) {
        self.latest_input = Some((seat, serial));
    }

    pub(crate) fn push_popup_input_event(
        &mut self,
        event: WindowEvent,
        window_id: WindowId,
        input: PopupInput,
    ) {
        self.window_events.push(Event::WindowEvent {
            event,
            window_id,
            popup_input: Some(Box::new(input)),
        });
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.window_events.append(&mut other.window_events);
    }

    #[inline]
    pub(crate) fn drain(&mut self) -> Drain<'_, Event> {
        self.window_events.drain(..)
    }
}
