use smithay_client_toolkit::reexports::{
    client::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
};

use super::AnyNellySurface;

pub struct NellyLayerSurface {
    wl_surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
}
impl From<NellyLayerSurface> for AnyNellySurface {
    fn from(surface: NellyLayerSurface) -> Self {
        AnyNellySurface::Layer(surface)
    }
}
impl NellyLayerSurface {
    pub fn attach(&self, wl_buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.wl_surface.attach(wl_buffer, x, y);
    }

    pub fn commit(&self) {
        self.wl_surface.commit();
    }
}
