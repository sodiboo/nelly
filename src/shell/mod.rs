use std::sync::atomic::AtomicBool;

use volito::ViewId;
use smithay_client_toolkit::reexports::{
    client::{
        protocol::{
            wl_buffer::WlBuffer,
            wl_callback::WlCallback,
            wl_output,
            wl_region::{self, WlRegion},
            wl_surface::{self, WlSurface},
        },
        Dispatch, Proxy, QueueHandle,
    },
    protocols::wp::{
        fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1,
        viewporter::client::wp_viewport::WpViewport,
    },
};

use self::compositor::Surface;

pub mod compositor;
pub mod layer;

#[allow(clippy::pedantic)]
pub mod xdg;

/// An unsupported operation, often due to the version of the protocol.
#[derive(Debug, Default)]
pub struct Unsupported;

/// Functionality shared by all [`wl_surface::WlSurface`] backed shell role objects.
pub trait WaylandSurface: Sized {
    fn surface(&self) -> &Surface;

    fn view_id(&self) -> ViewId {
        self.surface().data().view_id()
    }

    fn wl_surface(&self) -> &WlSurface {
        self.surface().wl_surface()
    }
    fn viewport(&self) -> &WpViewport {
        self.surface().viewport()
    }
    fn fractional_scale(&self) -> &WpFractionalScaleV1 {
        self.surface().fractional_scale()
    }

    fn attach(&self, buffer: Option<&WlBuffer>, x: u32, y: u32) {
        // In version 5 and later, the x and y offset of `wl_surface::attach` must be zero and uses the
        // `offset` request instead.
        let (attach_x, attach_y) = if self.wl_surface().version() >= 5 {
            (0, 0)
        } else {
            (x, y)
        };

        self.wl_surface()
            .attach(buffer, attach_x as i32, attach_y as i32);

        if self.wl_surface().version() >= 5 {
            // Ignore the error since the version is garunteed to be at least 5 here.
            let _ = self.offset(x, y);
        }
    }

    fn scale_factor(&self) -> f64 {
        self.surface().data().scale_factor()
    }

    fn was_mapped(&self) -> &AtomicBool {
        self.surface().data().was_mapped()
    }

    fn previous_physical_size(&self) -> Option<volito::Size<u32>> {
        self.surface().data().previous_physical_size()
    }

    fn set_physical_size(&self, size: volito::Size<u32>, engine: &mut volito::Engine) {
        self.surface().data().set_physical_size(size, engine);
    }

    fn request_throttled_frame_callback<D>(&self, qh: &QueueHandle<D>)
    where
        D: Dispatch<WlCallback, WlSurface> + 'static,
    {
        if !self.surface().data().swap_waiting_for_frame(true) {
            self.request_frame_callback(qh);
        }
    }

    fn request_frame_callback<D>(&self, qh: &QueueHandle<D>)
    where
        D: Dispatch<WlCallback, WlSurface> + 'static,
    {
        self.wl_surface().frame(qh, self.wl_surface().clone());
    }

    fn damage_buffer(&self, x: i32, y: i32, width: i32, height: i32) {
        self.wl_surface().damage_buffer(x, y, width, height);
    }

    fn set_opaque_region(&self, region: Option<&WlRegion>) {
        self.wl_surface().set_opaque_region(region);
    }

    fn set_input_region(&self, region: Option<&WlRegion>) {
        self.wl_surface().set_input_region(region);
    }

    fn set_buffer_transform(&self, transform: wl_output::Transform) -> Result<(), Unsupported> {
        if self.wl_surface().version() < 2 {
            return Err(Unsupported);
        }

        self.wl_surface().set_buffer_transform(transform);
        Ok(())
    }

    fn set_buffer_scale(&self, scale: u32) -> Result<(), Unsupported> {
        if self.wl_surface().version() < 3 {
            return Err(Unsupported);
        }

        self.wl_surface().set_buffer_scale(scale as i32);
        Ok(())
    }

    fn offset(&self, x: u32, y: u32) -> Result<(), Unsupported> {
        if self.wl_surface().version() < 5 {
            return Err(Unsupported);
        }

        self.wl_surface().offset(x as i32, y as i32);
        Ok(())
    }

    /// Commits pending surface state.
    ///
    /// On commit, the pending double buffered state from the surface, including role dependent state is
    /// applied.
    ///
    /// # Initial commit
    ///
    /// In many protocol extensions, the concept of an initial commit is used. A initial commit provides the
    /// initial state of a surface to the compositor. For example with the [xdg shell](xdg),
    /// creating a window requires an initial commit.
    ///
    /// # Protocol Errors
    ///
    /// If the commit is the initial commit, no buffers must have been attached to the surface. This rule
    /// applies whether attaching the buffer was done using [`WaylandSurface::attach`] or under the hood in
    /// via window system integration in graphics APIs such as Vulkan (using `vkQueuePresentKHR`) and EGL
    /// (using `eglSwapBuffers`).
    fn commit(&self) {
        self.wl_surface().commit();
    }
}
