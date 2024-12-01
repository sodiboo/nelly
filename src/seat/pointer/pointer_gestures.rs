use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use fluster::{PointerDeviceKind, PointerEvent, PointerPhase, PointerSignalKind};
use smithay_client_toolkit::{
    error::GlobalError,
    reexports::{
        client::{
            globals::GlobalList, protocol::wl_pointer::WlPointer, Connection, Dispatch, Proxy,
            QueueHandle,
        },
        protocols::wp::pointer_gestures::zv1::client::{
            zwp_pointer_gesture_hold_v1::{self, ZwpPointerGestureHoldV1},
            zwp_pointer_gesture_pinch_v1::{self, ZwpPointerGesturePinchV1},
            zwp_pointer_gesture_swipe_v1::{self, ZwpPointerGestureSwipeV1},
            zwp_pointer_gestures_v1::ZwpPointerGesturesV1,
        },
    },
    registry::GlobalProxy,
};

use crate::nelly::Nelly;

use super::{DeviceData, PointerData};

#[derive(Debug)]
pub(super) struct PointerGesturesGlobalState {
    pointer_gestures: GlobalProxy<ZwpPointerGesturesV1>,
}
impl PointerGesturesGlobalState {
    /// Bind `zwp_pointer_gestures_manager_v1` global, if it exists
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<Nelly>) -> Self {
        Self {
            pointer_gestures: GlobalProxy::from(globals.bind(qh, 3..=3, ())),
        }
    }

    pub fn get_pointer_gestures(
        &self,
        pointer: &WlPointer,
        qh: &QueueHandle<Nelly>,
    ) -> Result<PointerGestures, GlobalError> {
        let manager = self.pointer_gestures.get()?;

        let gesture_state = Arc::new(GestureState::new(pointer));
        Ok(PointerGestures {
            swipe: manager.get_swipe_gesture(pointer, qh, gesture_state.clone()),
            pinch: manager.get_pinch_gesture(pointer, qh, gesture_state.clone()),
            hold: manager.get_hold_gesture(pointer, qh, gesture_state.clone()),
        })
    }
}

#[derive(Debug)]
pub(super) struct PointerGestures {
    swipe: ZwpPointerGestureSwipeV1,
    pinch: ZwpPointerGesturePinchV1,
    hold: ZwpPointerGestureHoldV1,
}
impl Drop for PointerGestures {
    fn drop(&mut self) {
        self.swipe.destroy();
        self.pinch.destroy();
        self.hold.destroy();
    }
}

impl Dispatch<ZwpPointerGesturesV1, ()> for Nelly {
    fn event(
        _: &mut Self,
        _: &ZwpPointerGesturesV1,
        _: <ZwpPointerGesturesV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // no events
    }
}

struct GestureState {
    device: DeviceData,
    pointer: WlPointer,

    cumulative_pos: Mutex<(f64, f64)>,
    cumulative_rot: Mutex<f64>,
}

impl GestureState {
    fn new(pointer: &WlPointer) -> Self {
        GestureState {
            device: DeviceData::new(),
            pointer: pointer.clone(),
            cumulative_pos: Default::default(),
            cumulative_rot: Default::default(),
        }
    }

    fn pointer_data(&self) -> &PointerData {
        self.pointer.data::<PointerData>().unwrap()
    }
}

impl Dispatch<ZwpPointerGestureSwipeV1, Arc<GestureState>> for Nelly {
    fn event(
        _: &mut Self,
        _: &ZwpPointerGestureSwipeV1,
        event: <ZwpPointerGestureSwipeV1 as Proxy>::Event,
        data: &Arc<GestureState>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut state = data.pointer_data().state.lock().unwrap();
        let mut cumulative_pos = data.cumulative_pos.lock().unwrap();

        let state = &mut *state;
        let (cx, cy) = &mut *cumulative_pos;
        match event {
            zwp_pointer_gesture_swipe_v1::Event::Begin {
                serial: _,
                time,
                surface,
                fingers: _,
            } => {
                data.device.enter(&surface);

                (*cx, *cy) = (0.0, 0.0);

                state.events.push(PointerEvent {
                    view_id: data.device.nelly_surface().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(time as u64),

                    phase: PointerPhase::PanZoomStart,
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Trackpad,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: *cx,
                    pan_y: *cy,
                    scale: 1.0,
                    rotation: 0.0,
                })
            }
            zwp_pointer_gesture_swipe_v1::Event::Update { time, dx, dy } => {
                *cx += dx;
                *cy += dy;

                state.events.push(PointerEvent {
                    view_id: data.device.nelly_surface().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(time as u64),

                    phase: PointerPhase::PanZoomUpdate,
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Trackpad,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: *cx,
                    pan_y: *cy,
                    scale: 1.0,
                    rotation: 0.0,
                })
            }
            zwp_pointer_gesture_swipe_v1::Event::End {
                serial: _,
                time,
                cancelled,
            } => {
                data.device.leave(&data.device.surface());

                state.events.push(PointerEvent {
                    view_id: data.device.nelly_surface().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(time as u64),

                    phase: if cancelled != 0 {
                        PointerPhase::Cancel
                    } else {
                        PointerPhase::PanZoomEnd
                    },
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Trackpad,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: *cx,
                    pan_y: *cy,
                    scale: 1.0,
                    rotation: 0.0,
                })
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwpPointerGesturePinchV1, Arc<GestureState>> for Nelly {
    fn event(
        _: &mut Self,
        _: &ZwpPointerGesturePinchV1,
        event: <ZwpPointerGesturePinchV1 as Proxy>::Event,
        data: &Arc<GestureState>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut state = data.pointer_data().state.lock().unwrap();
        let mut cumulative_pos = data.cumulative_pos.lock().unwrap();
        let mut cumulative_rot = data.cumulative_rot.lock().unwrap();

        let state = &mut *state;
        let (cx, cy) = &mut *cumulative_pos;
        let cr = &mut *cumulative_rot;
        match event {
            zwp_pointer_gesture_pinch_v1::Event::Begin {
                serial: _,
                time,
                surface,
                fingers: _,
            } => {
                data.device.enter(&surface);

                (*cx, *cy) = (0.0, 0.0);
                *cr = 0.0;

                state.events.push(PointerEvent {
                    view_id: data.device.nelly_surface().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(time as u64),

                    phase: PointerPhase::PanZoomStart,
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Trackpad,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: *cx,
                    pan_y: *cy,
                    scale: 1.0,
                    rotation: *cr,
                })
            }
            zwp_pointer_gesture_pinch_v1::Event::Update {
                time,
                dx,
                dy,
                scale,
                rotation,
            } => {
                *cx += dx;
                *cy += dy;
                *cr += rotation; // this is also a delta in Wayland

                state.events.push(PointerEvent {
                    view_id: data.device.nelly_surface().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(time as u64),

                    phase: PointerPhase::PanZoomUpdate,
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Trackpad,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: *cx,
                    pan_y: *cy,
                    scale,
                    rotation: cr.to_radians(),
                })
            }
            zwp_pointer_gesture_pinch_v1::Event::End {
                serial: _,
                time,
                cancelled,
            } => {
                data.device.leave(&data.device.surface());

                state.events.push(PointerEvent {
                    view_id: data.device.nelly_surface().view_id(),
                    device: data.device.id,
                    timestamp: Duration::from_millis(time as u64),

                    phase: if cancelled != 0 {
                        PointerPhase::Cancel
                    } else {
                        PointerPhase::PanZoomEnd
                    },
                    x: state.x,
                    y: state.y,

                    device_kind: PointerDeviceKind::Trackpad,
                    buttons: state.buttons,

                    signal_kind: PointerSignalKind::None,
                    scroll_delta_x: 0.0,
                    scroll_delta_y: 0.0,

                    pan_x: *cx,
                    pan_y: *cy,
                    scale: 1.0,
                    rotation: cr.to_radians(),
                })
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwpPointerGestureHoldV1, Arc<GestureState>> for Nelly {
    fn event(
        _: &mut Self,
        _: &ZwpPointerGestureHoldV1,
        event: <ZwpPointerGestureHoldV1 as Proxy>::Event,
        data: &Arc<GestureState>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // yea the hold gesture can't really be mapped to anything in Flutter
        match event {
            zwp_pointer_gesture_hold_v1::Event::Begin {
                serial: _,
                time,
                surface,
                fingers: _,
            } => {
                data.device.enter(&surface);

                _ = time;
            }
            zwp_pointer_gesture_hold_v1::Event::End {
                serial: _,
                time,
                cancelled,
            } => {
                data.device.leave(&data.device.surface());

                _ = (time, cancelled);
            }
            _ => unreachable!(),
        }
    }
}
