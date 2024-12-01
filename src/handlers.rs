use std::sync::Arc;

use smithay_client_toolkit::compositor::CompositorHandler;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::protocol::wl_output::{Transform, WlOutput};
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::protocol::{wl_buffer, wl_shm_pool};
use smithay_client_toolkit::reexports::client::{delegate_noop, Connection, Dispatch, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shell::xdg::window::WindowHandler;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::ShmHandler;
use tracing::debug;

use crate::pool::BufferBacking;

use super::{Nelly, NellySurfaceData, WaylandBackendEvent};

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
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {
        self.send_event(WaylandBackendEvent::Frame);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }
}
smithay_client_toolkit::delegate_compositor!(Nelly, surface: [ NellySurfaceData ]);

impl WindowHandler for Nelly {
    fn request_close(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &smithay_client_toolkit::shell::xdg::window::Window,
    ) {
        self.events.send(WaylandBackendEvent::Close).unwrap();
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &smithay_client_toolkit::shell::xdg::window::Window,
        configure: smithay_client_toolkit::shell::xdg::window::WindowConfigure,
        _: u32,
    ) {
        if let Some((width, height)) = Option::zip(configure.new_size.0, configure.new_size.1) {
            let width = u32::from(width);
            let height = u32::from(height);
            let new_size = { fluster::Size { width, height } };
            if self.previous_size != Some(new_size) {
                self.send_event(WaylandBackendEvent::Resize(window.clone(), new_size));
            }
        } else if self.previous_size.is_none() {
            self.send_event(WaylandBackendEvent::Resize(
                window.clone(),
                fluster::Size {
                    width: 800,
                    height: 600,
                },
            ));
        }
    }
}

smithay_client_toolkit::delegate_xdg_shell!(Nelly);
smithay_client_toolkit::delegate_xdg_window!(Nelly);
