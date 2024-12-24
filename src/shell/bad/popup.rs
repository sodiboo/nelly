use smithay_client_toolkit::reexports::{
    client::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    protocols::xdg::shell::client::{xdg_popup::XdgPopup, xdg_surface::XdgSurface},
};

use super::AnyNellySurface;

pub struct NellyPopupSurface {
    wl_surface: WlSurface,
    xdg_surface: XdgSurface,
    xdg_popup: XdgPopup,
}
impl From<NellyPopupSurface> for AnyNellySurface {
    fn from(surface: NellyPopupSurface) -> Self {
        AnyNellySurface::Popup(surface)
    }
}
impl NellyPopupSurface {
    pub fn attach(&self, wl_buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.wl_surface.attach(wl_buffer, x, y);
    }

    pub fn commit(&self) {
        self.wl_surface.commit();
    }
}
