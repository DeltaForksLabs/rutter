//! Seat handling.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use foldhash::HashMap;
use sctk::data_device_manager::data_device::DataDevice;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_touch::WlTouch;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;
use sctk::seat::pointer::{PointerData, ThemeSpec, ThemedPointer};
use sctk::seat::{Capability as SeatCapability, SeatHandler, SeatState};
use tracing::warn;
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_hold_v1::ZwpPointerGestureHoldV1;
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use winit_core::event::WindowEvent;
use winit_core::keyboard::ModifiersState;
use winit_core::window::WindowId;

use crate::state::WinitState;

mod keyboard;
mod pointer;
mod text_input;
mod touch;

use keyboard::{KeyboardData, KeyboardState};
pub use pointer::pointer_gesture::{PointerGestureData, PointerGesturesState};
pub use pointer::relative_pointer::RelativePointerState;
pub use pointer::{PointerConstraintsState, WinitPointerData, WinitPointerDataExt};
use text_input::TextInputData;
pub use text_input::{ClientState as TextInputClientState, TextInputState};
use touch::TouchPoint;

pub(crate) use crate::seat::text_input::ZwpTextInputV3Ext;

/// A physical press, shared with its queued event. Invalidating the seat's copy
/// also revokes events already drained into the event loop's delivery buffer.
#[derive(Debug, Clone)]
pub(crate) struct PopupInput<S = WlSeat> {
    pub(crate) seat: S,
    parent: WindowId,
    serial: u32,
    valid: Arc<AtomicBool>,
}

impl<S> PopupInput<S> {
    pub(crate) fn consume(
        &self,
        parent: WindowId,
        seat_is_live: impl FnOnce(&S) -> bool,
    ) -> Option<u32> {
        if self.parent != parent || !seat_is_live(&self.seat) {
            return None;
        }
        self.valid.swap(false, Ordering::Relaxed).then_some(self.serial)
    }

    pub(crate) fn invalidate(&self) {
        self.valid.store(false, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopupInputKind {
    Pointer(u32),
    Keyboard(u32),
}

/// Only the latest physical press on a seat can authorize a required grab.
/// Serial numbers are opaque (including zero); receipt order defines freshness.
#[derive(Debug)]
pub(crate) struct PopupInputState<S = WlSeat> {
    current: RefCell<Option<(PopupInputKind, PopupInput<S>)>>,
}

impl<S> Default for PopupInputState<S> {
    fn default() -> Self {
        Self { current: RefCell::new(None) }
    }
}

impl<S: Clone> PopupInputState<S> {
    pub(crate) fn press(
        &self,
        kind: PopupInputKind,
        seat: S,
        parent: WindowId,
        serial: u32,
    ) -> PopupInput<S> {
        self.invalidate();
        let input = PopupInput { seat, parent, serial, valid: Arc::new(AtomicBool::new(true)) };
        *self.current.borrow_mut() = Some((kind, input.clone()));
        input
    }
}

impl<S> PopupInputState<S> {
    pub(crate) fn release(&self, kind: PopupInputKind) {
        self.invalidate_if(|current| current == kind);
    }

    pub(crate) fn pointer_left(&self) {
        self.invalidate_if(|kind| matches!(kind, PopupInputKind::Pointer(_)));
    }

    pub(crate) fn keyboard_left(&self) {
        self.invalidate_if(|kind| matches!(kind, PopupInputKind::Keyboard(_)));
    }

    fn invalidate_if(&self, predicate: impl FnOnce(PopupInputKind) -> bool) {
        if self.current.borrow().as_ref().is_some_and(|(kind, _)| predicate(*kind)) {
            self.invalidate();
        }
    }

    pub(crate) fn invalidate(&self) {
        if let Some((_, input)) = self.current.borrow_mut().take() {
            input.invalidate();
        }
    }
}

impl<S> Drop for PopupInputState<S> {
    fn drop(&mut self) {
        self.invalidate();
    }
}

#[derive(Debug, Default)]
pub struct WinitSeatState {
    /// The pointer bound on the seat.
    pointer: Option<Arc<ThemedPointer<WinitPointerData>>>,

    /// The touch bound on the seat.
    touch: Option<WlTouch>,

    /// The mapping from touched points to the surfaces they're present.
    touch_map: HashMap<i32, TouchPoint>,

    /// Id of the first touch event.
    first_touch_id: Option<i32>,

    /// The text input bound on the seat.
    text_input: Option<Arc<ZwpTextInputV3>>,

    /// The tablet input bound on the seat.
    tablet: Option<Arc<ZwpTabletSeatV2>>,

    /// The relative pointer bound on the seat.
    relative_pointer: Option<ZwpRelativePointerV1>,

    /// The pinch pointer gesture bound on the seat.
    pointer_gesture_pinch: Option<ZwpPointerGesturePinchV1>,

    /// The hold pointer gesture bound on the seat.
    pointer_gesture_hold: Option<ZwpPointerGestureHoldV1>,

    /// The drag-and-drop state
    data_device: Option<DataDevice>,

    /// The keyboard bound on the seat.
    keyboard_state: Option<KeyboardState>,

    /// The current modifiers state on the seat.
    modifiers: ModifiersState,

    /// Whether we have pending modifiers.
    modifiers_pending: bool,

    /// Live press authorization shared with application-dispatch metadata.
    pub(crate) popup_input: PopupInputState,
}

impl WinitSeatState {
    pub fn new() -> Self {
        Default::default()
    }

    pub(crate) fn data_device(&self) -> Option<&DataDevice> {
        self.data_device.as_ref()
    }

    pub(crate) fn pointer_data(&self) -> Option<&PointerData<WinitPointerData>> {
        self.pointer.as_ref().and_then(|pointer| pointer.pointer().data())
    }
}

impl SeatHandler for WinitState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_capability(
        &mut self,
        _: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
        capability: SeatCapability,
    ) {
        let seat_state = match self.seats.get_mut(&seat.id()) {
            Some(seat_state) => seat_state,
            None => {
                warn!("Received wl_seat::new_capability for unknown seat");
                return;
            },
        };

        match capability {
            SeatCapability::Touch if seat_state.touch.is_none() => {
                seat_state.touch = self.seat_state.get_touch(queue_handle, &seat).ok();
            },
            SeatCapability::Keyboard if seat_state.keyboard_state.is_none() => {
                let keyboard = seat.get_keyboard(queue_handle, KeyboardData::new(seat.clone()));
                seat_state.keyboard_state =
                    Some(KeyboardState::new(keyboard, self.loop_handle.clone()));
            },
            SeatCapability::Pointer if seat_state.pointer.is_none() => {
                let surface = self.compositor_state.create_surface(queue_handle);
                let viewport = self
                    .viewporter_state
                    .as_ref()
                    .map(|state| state.get_viewport(&surface, queue_handle));
                let surface_id = surface.id();
                let pointer_data = WinitPointerData::new(viewport);
                let themed_pointer = self
                    .seat_state
                    .get_pointer_with_theme_and_data(
                        queue_handle,
                        &seat,
                        self.shm.wl_shm(),
                        surface,
                        ThemeSpec::System,
                        pointer_data,
                    )
                    .expect("failed to create pointer with present capability.");

                seat_state.data_device = self
                    .data_device_manager_state
                    .as_ref()
                    .map(|device| device.get_data_device(queue_handle, &seat));

                seat_state.relative_pointer = self.relative_pointer.as_ref().map(|manager| {
                    manager.get_relative_pointer(
                        themed_pointer.pointer(),
                        queue_handle,
                        sctk::globals::GlobalData,
                    )
                });

                seat_state.pointer_gesture_pinch = self.pointer_gestures.as_ref().map(|manager| {
                    manager.get_pinch_gesture(
                        themed_pointer.pointer(),
                        queue_handle,
                        PointerGestureData::default(),
                    )
                });

                seat_state.pointer_gesture_hold = self.pointer_gestures.as_ref().map(|manager| {
                    manager.get_hold_gesture(
                        themed_pointer.pointer(),
                        queue_handle,
                        PointerGestureData::default(),
                    )
                });

                let themed_pointer = Arc::new(themed_pointer);

                // Register cursor surface.
                self.pointer_surfaces.insert(surface_id, themed_pointer.clone());

                seat_state.pointer = Some(themed_pointer);
            },
            _ => (),
        }

        if let Some(text_input_state) =
            seat_state.text_input.is_none().then_some(self.text_input_state.as_ref()).flatten()
        {
            seat_state.text_input = Some(Arc::new(text_input_state.get_text_input(
                &seat,
                queue_handle,
                TextInputData::default(),
            )));
        }

        if let Some(tablet_state) =
            seat_state.tablet.is_none().then_some(self.tablet_state.as_ref()).flatten()
        {
            seat_state.tablet = Some(Arc::new(tablet_state.get_tablet_seat(&seat, queue_handle)));
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
        capability: SeatCapability,
    ) {
        let seat_state = match self.seats.get_mut(&seat.id()) {
            Some(seat_state) => seat_state,
            None => {
                warn!("Received wl_seat::remove_capability for unknown seat");
                return;
            },
        };

        if let Some(text_input) = seat_state.text_input.take() {
            text_input.destroy();
        }

        // NOTE: figure out when this should actually be destroyed.
        if let Some(tablet) = seat_state.tablet.take() {
            tablet.destroy();
        }

        match capability {
            SeatCapability::Touch => {
                if let Some(touch) = seat_state.touch.take() {
                    if touch.version() >= 3 {
                        touch.release();
                    }
                }
            },
            SeatCapability::Pointer => {
                seat_state.popup_input.pointer_left();
                if let Some(relative_pointer) = seat_state.relative_pointer.take() {
                    relative_pointer.destroy();
                }

                if let Some(pointer_gesture_pinch) = seat_state.pointer_gesture_pinch.take() {
                    pointer_gesture_pinch.destroy();
                }

                if let Some(pointer_gesture_hold) = seat_state.pointer_gesture_hold.take() {
                    pointer_gesture_hold.destroy();
                }

                seat_state.data_device = None;

                if let Some(pointer) = seat_state.pointer.take() {
                    let pointer_data = pointer.pointer().winit_data();

                    // Remove the cursor from the mapping.
                    let surface_id = pointer.surface().id();
                    let _ = self.pointer_surfaces.remove(&surface_id);

                    // Remove the inner locks/confines before dropping the pointer.
                    pointer_data.data().unlock_pointer();
                    pointer_data.data().unconfine_pointer();

                    if pointer.pointer().version() >= 3 {
                        pointer.pointer().release();
                    }
                }
            },
            SeatCapability::Keyboard => {
                seat_state.popup_input.keyboard_left();
                seat_state.keyboard_state = None;
                self.on_keyboard_destroy(&seat.id());
            },
            _ => (),
        }
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
    ) {
        self.seats.insert(seat.id(), WinitSeatState::new());
    }

    fn remove_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
    ) {
        let _ = self.seats.remove(&seat.id());
        self.on_keyboard_destroy(&seat.id());
    }
}

impl WinitState {
    fn on_keyboard_destroy(&mut self, seat: &ObjectId) {
        for (window_id, window) in self.windows.get_mut() {
            let mut window = window.lock().unwrap();
            let had_focus = window.has_focus();
            window.remove_seat_focus(seat);
            if had_focus != window.has_focus() {
                self.events_sink.push_window_event(WindowEvent::Focused(false), *window_id);
            }
        }
    }
}

#[cfg(test)]
mod popup_input_tests {
    use super::*;

    #[test]
    fn newer_press_revokes_queued_input_even_across_serial_wrap() {
        let parent = WindowId::from_raw(1);
        let other_parent = WindowId::from_raw(2);
        let state = PopupInputState::default();
        let old = state.press(PopupInputKind::Pointer(1), 7_u8, parent, u32::MAX);
        let new = state.press(PopupInputKind::Keyboard(2), 7_u8, other_parent, 0);
        assert_eq!(old.consume(parent, |_| true), None);
        assert_eq!(new.consume(parent, |_| true), None);
        assert_eq!(new.consume(other_parent, |seat| *seat == 7), Some(0));
        assert_eq!(new.consume(other_parent, |_| true), None);
    }

    #[test]
    fn release_before_application_delivery_revokes_only_the_matching_press() {
        let parent = WindowId::from_raw(1);
        for kind in [PopupInputKind::Pointer(1), PopupInputKind::Keyboard(2)] {
            let state = PopupInputState::default();
            let input = state.press(kind, 7_u8, parent, 42);
            state.release(PopupInputKind::Pointer(9));
            state.release(kind);
            assert_eq!(input.consume(parent, |_| true), None);
        }
    }

    #[test]
    fn focus_loss_capability_removal_and_seat_removal_revoke_queued_input() {
        let parent = WindowId::from_raw(1);
        let state = PopupInputState::default();
        let pointer = state.press(PopupInputKind::Pointer(1), 7_u8, parent, 42);
        state.pointer_left();
        assert_eq!(pointer.consume(parent, |_| true), None);
        let keyboard = state.press(PopupInputKind::Keyboard(2), 7_u8, parent, 43);
        state.keyboard_left();
        assert_eq!(keyboard.consume(parent, |_| true), None);
        let removed = state.press(PopupInputKind::Pointer(1), 7_u8, parent, 44);
        drop(state);
        assert_eq!(removed.consume(parent, |_| true), None);
    }
}
