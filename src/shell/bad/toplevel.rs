use smithay_client_toolkit::reexports::{
    client::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    protocols::xdg::shell::client::{xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel},
};

use super::AnyNellySurface;

pub struct NellyToplevelSurface {
    wl_surface: WlSurface,
    xdg_surface: XdgSurface,
    xdg_toplevel: XdgToplevel,

    state: ToplevelState,
}
impl From<NellyToplevelSurface> for AnyNellySurface {
    fn from(surface: NellyToplevelSurface) -> Self {
        AnyNellySurface::Toplevel(surface)
    }
}
impl NellyToplevelSurface {
    pub fn attach(&self, wl_buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.wl_surface.attach(wl_buffer, x, y);
    }

    pub fn commit(&self) {
        self.wl_surface.commit();
    }
}

enum ToplevelState {
    PreInitialConfigure,
}
