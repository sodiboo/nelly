use std::sync::Mutex;

use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_seat;
use smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::Connection;
use smithay_client_toolkit::reexports::client::Dispatch;
use smithay_client_toolkit::reexports::client::Proxy;
use smithay_client_toolkit::reexports::client::QueueHandle;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::registry::RegistryHandler;

use crate::nelly::Nelly;
use crate::nelly::NellySurfaceData;

use self::keyboard::Keyboard;
use self::keyboard::KeyboardGlobalState;
use self::pointer::Pointer;
use self::pointer::PointerGlobalState;
use self::touch::Touch;
use self::touch::TouchGlobalState;
use self::util::SerialCounter;

mod keyboard;
mod pointer;
mod touch;
mod util;

static DEVICE_ID: SerialCounter = SerialCounter::new();

#[derive(Debug)]
struct DeviceData {
    id: i32,
    surface: Mutex<Option<WlSurface>>,
}

impl DeviceData {
    fn new() -> Self {
        DeviceData {
            id: DEVICE_ID.next_serial() as i32,
            surface: Mutex::new(None),
        }
    }

    fn surface(&self) -> WlSurface {
        self.surface
            .lock()
            .unwrap()
            .clone()
            .expect("Received event for a device with no surface")
    }

    fn nelly_surface(&self) -> NellySurfaceData {
        self.surface()
            .data::<NellySurfaceData>()
            .expect("WlSurface wasn't created by Nelly")
            .clone()
    }

    fn enter(&self, surface: &WlSurface) -> &DeviceData {
        let prev = self.surface.lock().unwrap().replace(surface.clone());
        assert_eq!(prev, None, "Device already entered a surface");
        self
    }

    fn leave(&self, surface: &WlSurface) -> &DeviceData {
        let prev = self.surface.lock().unwrap().take();
        assert_eq!(
            prev.as_ref(),
            Some(surface),
            "Device left a surface it wasn't on"
        );
        self
    }
}

#[derive(Debug)]
pub struct SeatState {
    seats: Vec<SeatInner>,
    keyboard_state: KeyboardGlobalState,
    pointer_state: PointerGlobalState,
    touch_state: TouchGlobalState,
}

impl SeatState {
    #[allow(dead_code)]
    pub fn seats(&self) -> Vec<Seat> {
        self.seats.iter().map(|inner| inner.seat.clone()).collect()
    }
}

#[derive(Debug)]
struct SeatInner {
    seat: Seat,
    name: u32,
}

#[derive(Debug, Clone)]
pub struct Seat {
    seat: WlSeat,
}

impl Seat {
    fn data(&self) -> &SeatData {
        self.seat.data().expect("WlSeat has no SeatData")
    }
}

/// Serves to own as many input devices as possible,
/// for the sole purpose of receiving the appropriate events.
#[derive(Debug, Default)]
struct SeatDevices {
    keyboard: Option<Keyboard>,
    pointer: Option<Pointer>,
    touch: Option<Touch>,
}

impl RegistryHandler<Nelly> for SeatState {
    fn new_global(
        nelly: &mut Nelly,
        _: &Connection,
        qh: &QueueHandle<Nelly>,
        name: u32,
        interface: &str,
        _: u32,
    ) {
        if interface == WlSeat::interface().name {
            let seat = nelly
                .registry()
                .bind_specific(qh, name, 1..=7, SeatData::default())
                .expect("failed to bind global");

            let seat = Seat { seat };

            nelly.seat_state.seats.push(SeatInner { seat, name });
        }
    }

    fn remove_global(
        backend: &mut Nelly,
        _: &Connection,
        _: &QueueHandle<Nelly>,
        name: u32,
        interface: &str,
    ) {
        if interface == WlSeat::interface().name {
            if let Some(seat) = backend
                .seat_state
                .seats
                .iter()
                .find_map(|inner| (inner.name == name).then_some(inner.seat.clone()))
            {
                seat.data().with_devices_mut(|devices| {
                    if let Some(keyboard) = devices.keyboard.take() {
                        drop(keyboard);
                    }
                    if let Some(pointer) = devices.pointer.take() {
                        drop(pointer);
                    }

                    if let Some(touch) = devices.touch.take() {
                        drop(touch);
                    }
                });

                backend.seat_state.seats.retain(|inner| inner.name != name);
            }
        }
    }
}

const SEAT_VERSION: u32 = 7;

impl SeatState {
    pub fn new(global_list: &GlobalList, qh: &QueueHandle<Nelly>) -> SeatState {
        let keyboard_state = KeyboardGlobalState::bind(global_list, qh);
        let pointer_state = PointerGlobalState::bind(global_list, qh);
        let touch_state = TouchGlobalState::bind(global_list, qh);
        // but by inlining it here, this function is actually a lot nicer lol.
        // smithay_client_toolkit::registry::bind_all is private
        global_list.contents().with_list(|globals| {
            assert!(SEAT_VERSION <= WlSeat::interface().version);
            SeatState {
                seats: globals
                    .iter()
                    .filter(|global| global.interface == WlSeat::interface().name)
                    .map(|global| {
                        let version = global.version.min(SEAT_VERSION);
                        let name = global.name;
                        let seat: WlSeat = global_list.registry().bind(
                            global.name,
                            version,
                            qh,
                            SeatData::default(),
                        );
                        let seat = Seat { seat };
                        SeatInner { seat, name }
                    })
                    .collect(),
                keyboard_state,
                pointer_state,
                touch_state,
            }
        })
    }
}

#[derive(Debug, Default)]
struct SeatData {
    devices: Mutex<SeatDevices>,
}

impl SeatData {
    fn with_devices_mut<T>(&self, f: impl FnOnce(&mut SeatDevices) -> T) -> T {
        f(&mut self.devices.lock().unwrap())
    }
}

impl Dispatch<WlSeat, SeatData> for Nelly {
    fn event(
        nelly: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        data: &SeatData,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Name { .. } => {
                // we don't care about the name lol
            }
            wl_seat::Event::Capabilities { capabilities } => {
                let capabilities = wl_seat::Capability::from_bits_truncate(capabilities.into());

                data.with_devices_mut(|devices| {
                    if capabilities.contains(wl_seat::Capability::Keyboard) {
                        devices.keyboard.get_or_insert_with(|| {
                            nelly.seat_state.keyboard_state.get_keyboard(seat, qh)
                        });
                    } else if let Some(keyboard) = devices.keyboard.take() {
                        drop(keyboard)
                    }

                    if capabilities.contains(wl_seat::Capability::Pointer) {
                        devices.pointer.get_or_insert_with(|| {
                            nelly.seat_state.pointer_state.get_pointer(seat, qh)
                        });
                    } else if let Some(pointer) = devices.pointer.take() {
                        drop(pointer);
                    }

                    if capabilities.contains(wl_seat::Capability::Touch) {
                        devices.touch.get_or_insert_with(|| {
                            nelly.seat_state.touch_state.get_touch(seat, qh)
                        });
                    } else if let Some(touch) = devices.touch.take() {
                        drop(touch);
                    }
                });
            }
            _ => unreachable!(),
        }
    }
}
