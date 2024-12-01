use smithay_client_toolkit::reexports::client::{
    globals::GlobalList,
    protocol::{
        wl_keyboard::{self, WlKeyboard},
        wl_seat::WlSeat,
    },
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
};

use crate::nelly::Nelly;

use super::{util::KeyState, DeviceData};

#[derive(Debug)]
pub(super) struct KeyboardGlobalState {
    _private: (), // ensure only this file can construct this struct
}
impl KeyboardGlobalState {
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<Nelly>) -> Self {
        // there are no optional globals to bind. just Seat.
        _ = (globals, qh);
        Self { _private: () }
    }

    pub fn get_keyboard(&self, seat: &WlSeat, qh: &QueueHandle<Nelly>) -> Keyboard {
        let wl_keyboard = seat.get_keyboard(qh, KeyboardData::new());

        Keyboard { wl_keyboard }
    }
}

#[derive(Debug)]
pub(super) struct Keyboard {
    wl_keyboard: WlKeyboard,
}
impl Drop for Keyboard {
    fn drop(&mut self) {
        self.wl_keyboard.release();
    }
}

pub(super) struct KeyboardData {
    device: DeviceData,
}

impl KeyboardData {
    pub fn new() -> Self {
        KeyboardData {
            device: DeviceData::new(),
        }
    }
}

#[allow(unused_variables)] //
impl Dispatch<WlKeyboard, KeyboardData> for Nelly {
    fn event(
        nelly: &mut Self,
        keyboard: &WlKeyboard,
        event: <WlKeyboard as Proxy>::Event,
        data: &KeyboardData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let keyboard = keyboard.clone();
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                assert_eq!(format, WEnum::Value(wl_keyboard::KeymapFormat::XkbV1));
                // backend.send_input_event(
                //     surface,
                //     InputEvent::Special(WaylandInputSpecialEvent::KeyboardKeymap {
                //         keyboard,
                //         fd,
                //         size,
                //     }),
                // );
            }
            wl_keyboard::Event::Enter {
                serial,
                surface,
                keys,
            } => {
                data.device.enter(&surface);

                // nelly.send_input_event(
                //     surface,
                //     InputEvent::Special(WaylandInputSpecialEvent::KeyboardEnter {
                //         keyboard,
                //         serial,
                //         keys: keys
                //             // Keysyms are encoded as an array of u32
                //             .chunks_exact(4)
                //             .flat_map(TryInto::<[u8; 4]>::try_into)
                //             .map(u32::from_le_bytes)
                //             // We must add 8 to the keycode for any functions we pass the raw
                //             // keycode into per wl_keyboard protocol
                //             .map(|raw| Keycode::new(raw + 8))
                //             .collect(),
                //     }),
                // );
            }
            wl_keyboard::Event::Leave { serial, surface } => {
                data.device.leave(&surface);

                // nelly.send_input_event(
                //     surface,
                //     InputEvent::Special(WaylandInputSpecialEvent::KeyboardLeave {
                //         keyboard,
                //         serial,
                //     }),
                // );
            }
            wl_keyboard::Event::Key {
                serial,
                time,
                key,
                state,
            } => {
                let state = match state.into_result().unwrap() {
                    wl_keyboard::KeyState::Pressed => KeyState::Pressed,
                    wl_keyboard::KeyState::Released => KeyState::Released,
                    _ => unreachable!(),
                };
                let surface = data.device.surface();

                // nelly.send_input_event(
                //     surface,
                //     InputEvent::Keyboard {
                //         event: WaylandKeyboardEvent {
                //             keyboard,
                //             serial,
                //             time,
                //             key: Keycode::new(key + 8),
                //             state,
                //         },
                //     },
                // );
            }
            wl_keyboard::Event::Modifiers {
                serial,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                // nelly.send_input_event(
                //     surface,
                //     InputEvent::Special(WaylandInputSpecialEvent::KeyboardModifiers {
                //         keyboard,
                //         serial,
                //         depressed: mods_depressed,
                //         latched: mods_latched,
                //         locked: mods_locked,
                //         group,
                //     }),
                // );
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                // nelly.send_input_event(
                //     surface,
                //     InputEvent::Special(WaylandInputSpecialEvent::KeyboardRepeatInfo {
                //         keyboard,
                //         rate,
                //         delay,
                //     }),
                // );
            }
            _ => unreachable!(),
        }
    }
}
