use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Mutex,
    time::Duration,
};

use fluster::{
    Engine, PointerButtons, PointerDeviceKind, PointerEvent, PointerPhase, PointerSignalKind,
};
use smithay_client_toolkit::reexports::client::{
    globals::GlobalList,
    protocol::{
        wl_seat::WlSeat,
        wl_touch::{self, WlTouch},
    },
    Connection, Dispatch, Proxy, QueueHandle,
};
use tracing::error;

use crate::nelly::Nelly;

use super::DeviceData;

#[derive(Debug)]
pub(super) struct TouchGlobalState {
    _private: (), // ensure only this file can construct this struct
}
impl TouchGlobalState {
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<Nelly>) -> Self {
        // there are no optional globals to bind. just Seat.
        _ = (globals, qh);
        Self { _private: () }
    }

    pub fn get_touch(&self, seat: &WlSeat, qh: &QueueHandle<Nelly>) -> Touch {
        _ = self;

        let wl_touch = seat.get_touch(qh, TouchData::new());

        Touch { wl_touch }
    }
}

#[derive(Debug)]
pub(super) struct Touch {
    wl_touch: WlTouch,
}
impl Drop for Touch {
    fn drop(&mut self) {
        self.wl_touch.release();
    }
}

pub(super) struct TouchData {
    state: Mutex<TouchState>,
}

#[derive(Default)]
struct TouchState {
    slots: HashMap<i32, TouchSlot>,
    events: Vec<PointerEvent>,
}

struct TouchSlot {
    x: f64,
    y: f64,
    device: DeviceData,
}

impl TouchSlot {
    fn new(x: f64, y: f64) -> Self {
        TouchSlot {
            x,
            y,
            device: DeviceData::new(),
        }
    }
}

impl TouchData {
    pub fn new() -> Self {
        TouchData {
            state: Mutex::new(TouchState::default()),
        }
    }
}

impl Dispatch<WlTouch, TouchData> for Nelly {
    fn event(
        nelly: &mut Self,
        _: &WlTouch,
        event: <WlTouch as Proxy>::Event,
        data: &TouchData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut state = data.state.lock().unwrap();
        let state = &mut *state;
        match event {
            wl_touch::Event::Down {
                serial: _,
                time,
                surface,
                id,
                x,
                y,
            } => {
                let slot = match state.slots.entry(id) {
                    Entry::Occupied(_) => {
                        error!("Touch ID {id} already exists in the slot map");
                        return;
                    }
                    Entry::Vacant(entry) => entry.insert(TouchSlot::new(x, y)),
                };

                slot.device.enter(&surface);

                (slot.x, slot.y) = (
                    x * slot.device.surface_data().scale_factor(),
                    y * slot.device.surface_data().scale_factor(),
                );

                state.events.push(PointerEvent {
                    view_id: slot.device.surface_data().view_id(),
                    device: slot.device.id,
                    timestamp: Duration::from_millis(u64::from(time)),

                    phase: PointerPhase::Down,
                    x: slot.x,
                    y: slot.y,

                    device_kind: PointerDeviceKind::Touch,
                    buttons: PointerButtons::TouchContact,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                });
            }
            wl_touch::Event::Up {
                serial: _,
                time,
                id,
            } => {
                let Some(slot) = state.slots.remove(&id) else {
                    error!("Touch ID {id} doesn't exist in the slot map");
                    return;
                };

                state.events.push(PointerEvent {
                    view_id: slot.device.surface_data().view_id(),
                    device: slot.device.id,
                    timestamp: Duration::from_millis(u64::from(time)),

                    phase: PointerPhase::Up,
                    x: slot.x,
                    y: slot.y,

                    device_kind: PointerDeviceKind::Touch,
                    buttons: PointerButtons::empty(),

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                });
            }
            wl_touch::Event::Motion { time, id, x, y } => {
                let Some(slot) = state.slots.get_mut(&id) else {
                    error!("Touch ID {id} doesn't exist in the slot map");
                    return;
                };

                (slot.x, slot.y) = (
                    x * slot.device.surface_data().scale_factor(),
                    y * slot.device.surface_data().scale_factor(),
                );

                state.events.push(PointerEvent {
                    view_id: slot.device.surface_data().view_id(),
                    device: slot.device.id,
                    timestamp: Duration::from_millis(u64::from(time)),

                    phase: PointerPhase::Move,
                    x: slot.x,
                    y: slot.y,

                    device_kind: PointerDeviceKind::Touch,
                    buttons: PointerButtons::TouchContact,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                });
            }
            wl_touch::Event::Frame => {
                let events = std::mem::take(&mut state.events);
                nelly.engine().send_pointer_event(&events).unwrap();
            }
            wl_touch::Event::Cancel => {
                state
                    .events
                    .extend(state.slots.drain().map(|(_, slot)| PointerEvent {
                        view_id: slot.device.surface_data().view_id(),
                        device: slot.device.id,
                        timestamp: Engine::get_current_time(),

                        phase: PointerPhase::Move,
                        x: slot.x,
                        y: slot.y,

                        device_kind: PointerDeviceKind::Touch,
                        buttons: PointerButtons::TouchContact,

                        signal_kind: PointerSignalKind::None,
                        scroll_delta_x: 0.0,
                        scroll_delta_y: 0.0,

                        pan_x: 0.0,
                        pan_y: 0.0,
                        scale: 1.0,
                        rotation: 0.0,
                    }));
            }
            #[allow(unused_variables)]
            wl_touch::Event::Shape { id, major, minor } => {
                // Flutter embedder API doesn't expose this.
            }
            #[allow(unused_variables)]
            wl_touch::Event::Orientation { id, orientation } => {
                // Flutter embedder API doesn't expose this.
            }
            _ => unreachable!(),
        }
    }
}
