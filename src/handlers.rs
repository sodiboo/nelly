use std::sync::atomic::Ordering;
use std::sync::Arc;

use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::protocol::wl_output::{Transform, WlOutput};
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shm::ShmHandler;
use tracing::debug;

use crate::pool::{BufferBacking, SinglePool};
use crate::shell::compositor::{CompositorHandler, CompositorState, SurfaceData};
use crate::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use crate::shell::WaylandSurface;
use crate::{delegate_compositor, delegate_xdg_shell, delegate_xdg_window};

use super::{Nelly, NellyEvent};

impl ProvidesRegistryState for Nelly {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, super::seat::SeatState];
}
smithay_client_toolkit::delegate_registry!(Nelly);

impl ShmHandler for Nelly {
    fn shm_state(&mut self) -> &mut smithay_client_toolkit::shm::Shm {
        &mut self.shm
    }
}
smithay_client_toolkit::delegate_shm!(Nelly);

impl OutputHandler for Nelly {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
}
smithay_client_toolkit::delegate_output!(Nelly);

impl CompositorHandler for Nelly {
    fn compositor_state(&self) -> &CompositorState {
        &self.compositor_state
    }

    fn scale_factor_changed(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &SurfaceData,
        new_factor: f64,
    ) {
        let view_id = surface.view_id();
        debug!("Scale factor {view_id:?} changed to {}", new_factor);
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &SurfaceData, _: u32) {
        surface.swap_waiting_for_frame(false);
        self.send_event(NellyEvent::Frame);
    }
}
crate::delegate_compositor!(Nelly);

impl WindowHandler for Nelly {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        self.events.send(NellyEvent::Close(window.clone())).unwrap();
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        if let Some((width, height)) = Option::zip(configure.new_size.0, configure.new_size.1) {
            let width = u32::from(width);
            let height = u32::from(height);
            let new_size = { fluster::Size { width, height } };
            if window.previous_size() != Some(new_size) {
                self.send_event(NellyEvent::Resize(window.clone(), new_size));
            }

            if window
                .with_previous_size(|size| size.replace(new_size))
                .is_none()
            {
                // let pool = SinglePool::new(
                //     new_size.width as i32,
                //     new_size.height as i32,
                //     new_size.width as i32 * 4,
                //     wl_shm::Format::Argb8888,
                //     &self.qh,
                //     self.shm.wl_shm(),
                // )
                // .unwrap();

                // debug!("Configuring window with size {:?}", new_size); // 1634x1361

                // // Yeah, just attach an empty buffer. It's fine.

                // window.attach(Some(pool.buffer()), 0, 0);
                // window.commit();
            }
        } else if window.previous_size().is_none() {
            self.send_event(NellyEvent::Resize(
                window.clone(),
                fluster::Size {
                    width: 800,
                    height: 600,
                },
            ));
        }
    }
}
crate::delegate_xdg_shell!(Nelly);
crate::delegate_xdg_window!(Nelly);
