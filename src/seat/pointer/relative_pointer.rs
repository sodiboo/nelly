use smithay_client_toolkit::{
    error::GlobalError,
    reexports::{
        client::{
            globals::GlobalList, protocol::wl_pointer::WlPointer, Connection, Dispatch, Proxy,
            QueueHandle,
        },
        protocols::wp::relative_pointer::zv1::client::{
            zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
            zwp_relative_pointer_v1::{self, ZwpRelativePointerV1},
        },
    },
    registry::GlobalProxy,
};

use crate::nelly::Nelly;

use super::PointerData;

#[derive(Debug)]
pub(super) struct RelativePointerGlobalState {
    relative_pointer_manager: GlobalProxy<ZwpRelativePointerManagerV1>,
}
impl RelativePointerGlobalState {
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<Nelly>) -> Self {
        let relative_pointer_manager = GlobalProxy::from(globals.bind(qh, 1..=1, ()));
        Self {
            relative_pointer_manager,
        }
    }

    pub fn get_relative_pointer(
        &self,
        pointer: &WlPointer,
        qh: &QueueHandle<Nelly>,
    ) -> Result<RelativePointer, GlobalError> {
        Ok(RelativePointer {
            relative_pointer: self.relative_pointer_manager.get()?.get_relative_pointer(
                pointer,
                qh,
                pointer.clone(),
            ),
        })
    }
}

#[derive(Debug)]
pub(super) struct RelativePointer {
    relative_pointer: ZwpRelativePointerV1,
}
impl Drop for RelativePointer {
    fn drop(&mut self) {
        self.relative_pointer.destroy();
    }
}

impl Dispatch<ZwpRelativePointerManagerV1, ()> for Nelly {
    fn event(
        _: &mut Self,
        _: &ZwpRelativePointerManagerV1,
        _: <ZwpRelativePointerManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // no events
    }
}

impl Dispatch<ZwpRelativePointerV1, WlPointer> for Nelly {
    fn event(
        _: &mut Self,
        _: &ZwpRelativePointerV1,
        event: <ZwpRelativePointerV1 as Proxy>::Event,
        pointer: &WlPointer,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let data = pointer.data::<PointerData>().unwrap();
        match event {
            #[allow(unused_variables)]
            zwp_relative_pointer_v1::Event::RelativeMotion {
                utime_hi,
                utime_lo,
                dx,
                dy,
                dx_unaccel,
                dy_unaccel,
            } => {
                let surface = data.device.nelly_surface();
                let state = data.state.lock().unwrap();

                // there's actually no way to give Flutter relative motion events
                #[cfg(any())]
                data.state.lock().unwrap().events.push(PointerEvent {
                    view_id: data.device.nelly_surface(),
                    device: data.device.id,
                    timestamp: Duration::from_micros(((utime_hi as u64) << 32) | (utime_lo as u64)),

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
                })
            }
            _ => todo!(),
        }
    }
}
