use std::{sync::Mutex, time::Duration};

use volito::{
    Engine, PointerButtons, PointerDeviceKind, PointerEvent, PointerPhase, PointerSignalKind,
};
use smithay_client_toolkit::reexports::client::{
    globals::GlobalList,
    protocol::{
        wl_pointer::{self, WlPointer},
        wl_seat::WlSeat,
    },
    Connection, Dispatch, Proxy, QueueHandle,
};
use tracing::warn;

use crate::nelly::Nelly;

use self::{
    pointer_gestures::PointerGesturesGlobalState, relative_pointer::RelativePointerGlobalState,
};

use super::{
    util::{Axis, AxisFrame, AxisRelativeDirection, AxisSource, ButtonState},
    DeviceData,
};

mod pointer_gestures;
mod relative_pointer;

#[derive(Debug)]
pub(super) struct PointerGlobalState {
    relative_pointer: RelativePointerGlobalState,
    pointer_gestures: PointerGesturesGlobalState,
}
impl PointerGlobalState {
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<Nelly>) -> Self {
        Self {
            relative_pointer: RelativePointerGlobalState::bind(globals, qh),
            pointer_gestures: PointerGesturesGlobalState::bind(globals, qh),
        }
    }

    pub fn get_pointer(&self, seat: &WlSeat, qh: &QueueHandle<Nelly>) -> Pointer {
        let wl_pointer = seat.get_pointer(qh, PointerData::new());
        let relative_pointer = self
            .relative_pointer
            .get_relative_pointer(&wl_pointer, qh)
            .ok();
        let pointer_gestures = self
            .pointer_gestures
            .get_pointer_gestures(&wl_pointer, qh)
            .ok();

        Pointer {
            wl_pointer,
            relative_pointer,
            pointer_gestures,
        }
    }
}

#[derive(Debug)]
pub(super) struct Pointer {
    wl_pointer: WlPointer,

    relative_pointer: Option<self::relative_pointer::RelativePointer>,
    pointer_gestures: Option<self::pointer_gestures::PointerGestures>,
}
impl Drop for Pointer {
    fn drop(&mut self) {
        if let Some(relative_pointer) = self.relative_pointer.take() {
            drop(relative_pointer);
        }

        if let Some(pointer_gestures) = self.pointer_gestures.take() {
            drop(pointer_gestures);
        }
        self.wl_pointer.release();
    }
}

pub(super) struct PointerData {
    axis_frame: Mutex<AxisFrame>,
    state: Mutex<PointerState>,
    device: DeviceData,
}

struct PointerState {
    buttons: PointerButtons,
    x: f64,
    y: f64,

    events: Vec<PointerEvent>,
}

impl PointerData {
    pub fn new() -> Self {
        Self {
            device: DeviceData::new(),
            state: Mutex::new(PointerState {
                buttons: PointerButtons::default(),
                x: 0.0,
                y: 0.0,
                events: Vec::new(),
            }),
            axis_frame: Mutex::new(AxisFrame::default()),
        }
    }

    fn with_axis_frame_mut<T>(&self, f: impl FnOnce(&mut AxisFrame) -> T) -> T {
        f(&mut self.axis_frame.lock().unwrap())
    }

    fn frame(&self) -> Vec<PointerEvent> {
        let mut state = self.state.lock().unwrap();
        let axis_frame = self.with_axis_frame_mut(std::mem::take);

        if axis_frame != AxisFrame::default() {
            let event = PointerEvent {
                view_id: self.device.surface_data().view_id(),
                device: self.device.id,
                timestamp: Duration::from_millis(u64::from(axis_frame.time)),

                phase: if state.buttons.is_empty() {
                    PointerPhase::Hover
                } else {
                    PointerPhase::Move
                },
                x: state.x,
                y: state.y,

                device_kind: match axis_frame.source {
                    AxisSource::Finger | AxisSource::Continuous => PointerDeviceKind::Trackpad,
                    AxisSource::Wheel | AxisSource::WheelTilt => PointerDeviceKind::Mouse,
                },
                buttons: state.buttons,

                signal_kind: PointerSignalKind::Scroll,
                scroll_delta_x: f64::from(axis_frame.horizontal.v120) / 120.0,
                scroll_delta_y: f64::from(axis_frame.vertical.v120) / 120.0,

                pan_x: 0.0,
                pan_y: 0.0,
                scale: 1.0,
                rotation: 0.0,
            };
            state.events.push(event);
        }

        std::mem::take(&mut state.events)
    }
}

impl Dispatch<WlPointer, PointerData> for Nelly {
    fn event(
        backend: &mut Self,
        proxy: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        data: &PointerData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial: _,
                surface,
                surface_x,
                surface_y,
            } => {
                data.device.enter(&surface);

                let mut state = data.state.lock().unwrap();
                (state.x, state.y) = (
                    surface_x * data.device.surface_data().scale_factor(),
                    surface_y * data.device.surface_data().scale_factor(),
                );

                let event = PointerEvent {
                    view_id: data.device.surface_data().view_id(),
                    device: data.device.id,
                    timestamp: Engine::get_current_time(),

                    phase: PointerPhase::Add,
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Mouse,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                };
                state.events.push(event);
            }
            wl_pointer::Event::Leave { serial: _, surface } => {
                let nelly_surface = data.device.surface_data();
                data.device.leave(&surface);

                let mut state = data.state.lock().unwrap();
                (state.x, state.y) = (0.0, 0.0);
                (state.buttons) = PointerButtons::default();

                let event = PointerEvent {
                    view_id: nelly_surface.view_id(),
                    device: data.device.id,
                    timestamp: Engine::get_current_time(),

                    phase: PointerPhase::Remove,
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Mouse,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                };
                state.events.push(event);
            }
            wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => {
                let mut state = data.state.lock().unwrap();
                (state.x, state.y) = (
                    surface_x * data.device.surface_data().scale_factor(),
                    surface_y * data.device.surface_data().scale_factor(),
                );

                let event = PointerEvent {
                    view_id: data.device.surface_data().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(u64::from(time)),

                    phase: if state.buttons.is_empty() {
                        PointerPhase::Hover
                    } else {
                        PointerPhase::Move
                    },
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Mouse,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                };
                state.events.push(event);
            }
            wl_pointer::Event::Button {
                serial: _,
                time,
                button,
                state,
            } => {
                use input_linux::Key;

                let button_state = match state.into_result().unwrap() {
                    wl_pointer::ButtonState::Pressed => ButtonState::Pressed,
                    wl_pointer::ButtonState::Released => ButtonState::Released,
                    _ => unreachable!(),
                };

                #[allow(clippy::cast_possible_truncation)] // >u16 is disallowed by protocol for now
                let key = Key::from_code(button as u16)
                    .expect("Button codes should be within the range of kernel KEY_COUNT");

                let flutter_button = match key {
                    Key::ButtonLeft => PointerButtons::MousePrimary,
                    Key::ButtonRight => PointerButtons::MouseSecondary,
                    Key::ButtonMiddle => PointerButtons::MouseMiddle,
                    Key::ButtonBack => PointerButtons::MouseBack,
                    Key::ButtonForward => PointerButtons::MouseForward,
                    _ => {
                        warn!("Mouse press event for unsupported button: {key:?}");
                        return;
                    }
                };

                let mut state = data.state.lock().unwrap();

                let was_empty = state.buttons.is_empty();
                match button_state {
                    ButtonState::Pressed => state.buttons.press(flutter_button),
                    ButtonState::Released => state.buttons.release(flutter_button),
                }
                let is_empty = state.buttons.is_empty();

                let event = PointerEvent {
                    view_id: data.device.surface_data().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(u64::from(time)),

                    phase: match (was_empty, is_empty) {
                        (false, false) => PointerPhase::Move,
                        (false, true) => PointerPhase::Up,
                        (true, false) => PointerPhase::Down,
                        (true, true) => PointerPhase::Hover, // (unreachable))
                    },
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Mouse,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: 0.0,
                    pan_y: 0.0,
                    scale: 1.0,
                    rotation: 0.0,
                };
                state.events.push(event);
            }
            wl_pointer::Event::Axis { time, axis, value } => {
                let axis = match axis.into_result().unwrap() {
                    wl_pointer::Axis::VerticalScroll => Axis::Vertical,
                    wl_pointer::Axis::HorizontalScroll => Axis::Horizontal,
                    _ => unreachable!(),
                };

                data.with_axis_frame_mut(|axis_frame| {
                    axis_frame.time(time);
                    axis_frame[axis].absolute += value;
                });
            }
            wl_pointer::Event::Frame => {
                let events = data.frame();
                backend.engine().send_pointer_event(&events).unwrap();
            }
            wl_pointer::Event::AxisSource { axis_source } => {
                let source = match axis_source.into_result().unwrap() {
                    wl_pointer::AxisSource::Wheel => AxisSource::Wheel,
                    wl_pointer::AxisSource::Finger => AxisSource::Finger,
                    wl_pointer::AxisSource::Continuous => AxisSource::Continuous,
                    wl_pointer::AxisSource::WheelTilt => AxisSource::WheelTilt,
                    _ => unreachable!(),
                };

                data.with_axis_frame_mut(|axis_frame| axis_frame.source = source);
            }
            wl_pointer::Event::AxisStop { time, axis } => {
                let axis = match axis.into_result().unwrap() {
                    wl_pointer::Axis::VerticalScroll => Axis::Vertical,
                    wl_pointer::Axis::HorizontalScroll => Axis::Horizontal,
                    _ => unreachable!(),
                };

                // We don't actually have an InputEvent interpretation of AxisStop.
                // So just set the time and ignore the stop, lol.
                data.with_axis_frame_mut(|axis_frame| axis_frame.time(time));
                let _ = axis;
            }
            wl_pointer::Event::AxisDiscrete { axis, discrete } => {
                let axis = match axis.into_result().unwrap() {
                    wl_pointer::Axis::VerticalScroll => Axis::Vertical,
                    wl_pointer::Axis::HorizontalScroll => Axis::Horizontal,
                    _ => unreachable!(),
                };

                data.with_axis_frame_mut(|axis_frame| axis_frame[axis].v120 += discrete * 120);
            }
            wl_pointer::Event::AxisValue120 { axis, value120 } => {
                let axis = match axis.into_result().unwrap() {
                    wl_pointer::Axis::VerticalScroll => Axis::Vertical,
                    wl_pointer::Axis::HorizontalScroll => Axis::Horizontal,
                    _ => unreachable!(),
                };

                data.with_axis_frame_mut(|axis_frame| axis_frame[axis].v120 += value120);
            }
            wl_pointer::Event::AxisRelativeDirection { axis, direction } => {
                let axis = match axis.into_result().unwrap() {
                    wl_pointer::Axis::VerticalScroll => Axis::Vertical,
                    wl_pointer::Axis::HorizontalScroll => Axis::Horizontal,
                    _ => unreachable!(),
                };
                let direction = match direction.into_result().unwrap() {
                    wl_pointer::AxisRelativeDirection::Identical => {
                        AxisRelativeDirection::Identical
                    }
                    wl_pointer::AxisRelativeDirection::Inverted => AxisRelativeDirection::Inverted,
                    _ => unreachable!(),
                };

                data.with_axis_frame_mut(|axis_frame| {
                    axis_frame[axis].relative_direction = direction;
                });
            }
            _ => unreachable!(),
        }

        // the `wl_pointer::frame` event was added in version 5
        if proxy.version() < 5 {
            let events = data.frame();
            backend.engine().send_pointer_event(&events).unwrap();
        }
    }
}
