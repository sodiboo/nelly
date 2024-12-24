use smithay_client_toolkit::reexports::{
    client::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
};

use super::AnyNellySurface;

pub struct NellyLockSurface {
    wl_surface: WlSurface,
    session_lock_surface: ExtSessionLockSurfaceV1,
}
impl From<NellyLockSurface> for AnyNellySurface {
    fn from(surface: NellyLockSurface) -> Self {
        AnyNellySurface::Lock(surface)
    }
}
impl NellyLockSurface {
    pub fn attach(&self, wl_buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.wl_surface.attach(wl_buffer, x, y);
    }

    pub fn commit(&self) {
        self.wl_surface.commit();
    }
}
