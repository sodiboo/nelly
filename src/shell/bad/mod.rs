use std::sync::Mutex;

use fluster::ViewId;
use smithay_client_toolkit::reexports::{
    client::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    protocols::{
        ext::session_lock::v1::client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        wp::{
            fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1,
            viewporter::client::wp_viewport::WpViewport,
        },
        xdg::shell::client::{
            xdg_popup::XdgPopup, xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel,
        },
    },
    protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
};

mod fundamental;
mod layer;
mod lock;
mod popup;
mod toplevel;
mod unmapped;

// use fundamental::FundamentalNellySurface;
pub use layer::NellyLayerSurface;
pub use lock::NellyLockSurface;
pub use popup::NellyPopupSurface;
pub use toplevel::NellyToplevelSurface;
pub use unmapped::NellyUnmappedSurface;

pub trait NellySurfacePhase {
    type ExtraState;
}

pub struct NellySurface<Phase: NellySurfacePhase> {
    fundamental: FundamentalSurfaceData,
    phase: Phase,
    state: Mutex<NellySurfaceState<Phase>>,
}

impl<Phase: NellySurfacePhase> NellySurface<Phase> {
    fn fundamental(&self) -> &FundamentalSurfaceData {
        &self.fundamental
    }
}

struct FundamentalSurfaceData {
    view_id: ViewId,
    wl_surface: WlSurface,
    viewport: WpViewport,
    fractional_scale: WpFractionalScaleV1,
}

impl FundamentalSurfaceData {
    pub fn attach(&self, buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.wl_surface.attach(buffer, x, y);
    }

    pub fn commit(&self) {
        self.wl_surface.commit();
    }
}

impl Drop for FundamentalSurfaceData {
    fn drop(&mut self) {
        self.fractional_scale.destroy();
        self.viewport.destroy();
        self.wl_surface.destroy();
    }
}

struct NellySurfaceState<Phase: NellySurfacePhase> {
    state: Phase::ExtraState,
}

pub enum AnyNellySurface {
    Unmapped(NellySurface<Unmapped>),
    Toplevel(NellySurface<Toplevel>),
    Popup(NellySurface<Popup>),
    Layer(NellySurface<Layer>),
    Lock(NellySurface<Lock>),
}

impl AnyNellySurface {
    pub fn fundamental(&self) -> &FundamentalSurfaceData {
        match self {
            AnyNellySurface::Unmapped(surface) => surface.fundamental(),
            AnyNellySurface::Toplevel(surface) => surface.fundamental(),
            AnyNellySurface::Popup(surface) => surface.fundamental(),
            AnyNellySurface::Layer(surface) => surface.fundamental(),
            AnyNellySurface::Lock(surface) => surface.fundamental(),
        }
    }

    pub fn attach(&self, wl_buffer: Option<&WlBuffer>, x: i32, y: i32) {
        self.fundamental().attach(wl_buffer, x, y);
    }

    pub fn commit(&self) {
        self.fundamental().commit();
    }
}

struct Unmapped;

impl NellySurfacePhase for Unmapped {
    type ExtraState = ();
}

struct Toplevel {
    xdg_surface: XdgSurface,
    xdg_toplevel: XdgToplevel,
}

impl Drop for Toplevel {
    fn drop(&mut self) {
        self.xdg_toplevel.destroy();
        self.xdg_surface.destroy();
    }
}

impl NellySurfacePhase for Toplevel {
    type ExtraState = ();
}

struct Popup {
    xdg_surface: XdgSurface,
    xdg_popup: XdgPopup,
}

impl NellySurfacePhase for Popup {
    type ExtraState = ();
}

struct Layer {
    layer_surface: ZwlrLayerSurfaceV1,
}

impl NellySurfacePhase for Layer {
    type ExtraState = ();
}

struct Lock {
    session_lock_surface: ExtSessionLockSurfaceV1,
}

impl NellySurfacePhase for Lock {
    type ExtraState = ();
}
