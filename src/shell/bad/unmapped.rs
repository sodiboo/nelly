use smithay_client_toolkit::reexports::client::protocol::{
    wl_buffer::WlBuffer, wl_surface::WlSurface,
};

use super::AnyNellySurface;

pub struct NellyUnmappedSurface {
    wl_surface: WlSurface,
}
impl From<NellyUnmappedSurface> for AnyNellySurface {
    fn from(surface: NellyUnmappedSurface) -> Self {
        AnyNellySurface::Unmapped(surface)
    }
}
impl NellyUnmappedSurface {
    pub fn attach(&self, wl_buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.wl_surface.attach(wl_buffer, x, y);
    }

    pub fn commit(&self) {
        self.wl_surface.commit();
    }
}
