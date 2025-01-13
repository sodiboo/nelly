use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shm::ShmHandler;
use tracing::debug;

use crate::platform_message::PlatformEvent;
use crate::shell::compositor::{CompositorHandler, CompositorState, SurfaceData};
use crate::shell::layer::LayerShellHandler;
use crate::shell::xdg::window::{WindowConfigure, WindowHandler, XdgToplevelSurface};
use crate::shell::WaylandSurface;

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

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &SurfaceData, _: u32) {
        surface.swap_waiting_for_frame(false);
        self.send_event(NellyEvent::Frame);
    }
}
crate::delegate_compositor!(Nelly);

impl WindowHandler for Nelly {
    fn request_close(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &XdgToplevelSurface,
    ) {
        let view_id = window.view_id();
        crate::platform_message::xdg_toplevel::Close { view_id }
            .send(self, |response, nelly| {
                // no need to do anything with the response. but we still hanve a callback here :3
                let () = response.unwrap();
                _ = nelly;
            })
            .unwrap();
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &XdgToplevelSurface,
        configure: WindowConfigure,
        _: u32,
    ) {
        let new_size_logical = {
            let default_dim = window.previous_physical_size().unwrap_or(volito::Size {
                width: 800,
                height: 600,
            });
            let (width, height) = configure.new_size;

            volito::Size {
                width: width.map_or(default_dim.width, u32::from),
                height: height.map_or(default_dim.height, u32::from),
            }
        };

        let view_id = window.view_id();
        let pixel_ratio = window.surface().data().scale_factor();
        debug!(
            "Resizing window {view_id:?} to {}",
            format_args!("{}x{}", new_size_logical.width, new_size_logical.height)
        );

        let new_size_physical = {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "i promise you it's fine"
            )]
            volito::Size {
                width: (f64::from(new_size_logical.width) * pixel_ratio).round() as u32,
                height: (f64::from(new_size_logical.height) * pixel_ratio).round() as u32,
            }
        };

        window.set_physical_size(new_size_physical, self.engine());
    }
}
crate::delegate_xdg_shell!(Nelly);
crate::delegate_xdg_window!(Nelly);

impl LayerShellHandler for Nelly {
    fn closed(
        &mut self,
        _: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &crate::shell::layer::WlrLayerSurface,
    ) {
        tracing::error!("Layer surface was closed");
    }

    fn configure(
        &mut self,
        _: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &crate::shell::layer::WlrLayerSurface,
        _configure: crate::shell::layer::LayerSurfaceConfigure,
        _serial: u32,
    ) {
        tracing::debug!("Layer surface was configured");
    }
}

crate::delegate_layer!(Nelly);
