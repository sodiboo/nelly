use std::{
    mem,
    os::fd::OwnedFd,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use fluster::ViewId;
use smithay_client_toolkit::{
    error::GlobalError,
    globals::{GlobalData, ProvidesBoundGlobal},
    output::{OutputData, OutputHandler, OutputState, ScaleWatcherHandle},
    reexports::{
        client::{
            backend::{protocol::Message, Backend, ObjectData, ObjectId},
            globals::{BindError, GlobalList},
            protocol::{
                wl_callback,
                wl_compositor::{self, WlCompositor},
                wl_output, wl_region,
                wl_surface::{self, WlSurface},
            },
            Connection, Dispatch, Proxy, QueueHandle, WEnum,
        },
        protocols::wp::{
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            },
            viewporter::{
                self,
                client::{
                    wp_viewport::WpViewport,
                    wp_viewporter::{self, WpViewporter},
                },
            },
        },
    },
};

use crate::atomic_f64::AtomicF64;

pub trait CompositorHandler: Sized {
    fn compositor_state(&self) -> &CompositorState;

    /// The surface has either been moved into or out of an output and the output has a different scale factor.
    fn scale_factor_changed(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &SurfaceData,
        new_factor: f64,
    );

    /// A frame callback has been completed.
    ///
    /// Frame callbacks are used to avoid updating surfaces that are not currently visible.  If a
    /// frame callback is requested prior to committing a surface, the client should avoid drawing
    /// to that surface until the callback completes.  See the
    /// [`WlSurface::frame`](wl_surface::WlSurface::frame) request for more details.
    ///
    /// This function will be called if you request a frame callback by passing the surface itself
    /// as the userdata (`surface.frame(&queue, &surface)`); you can also implement [`Dispatch`]
    /// for other values to more easily dispatch rendering for specific surface types.
    fn frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &SurfaceData,
        time: u32,
    );
}

pub trait SurfaceDataExt: Send + Sync {
    fn surface_data(&self) -> &SurfaceData;
}

impl SurfaceDataExt for SurfaceData {
    fn surface_data(&self) -> &SurfaceData {
        self
    }
}

#[derive(Clone, Debug)]
pub struct CompositorState {
    wl_compositor: WlCompositor,
    wp_viewporter: WpViewporter,
    fractional_scale_manager: WpFractionalScaleManagerV1,
}

impl CompositorState {
    pub const COMPOSITOR_VERSION: u32 = 6;

    pub const VIEWPORTER_VERSION: u32 = 1;

    pub const FRACTIONAL_SCALE_VERSION: u32 = 1;

    pub fn bind<State>(
        globals: &GlobalList,
        qh: &QueueHandle<State>,
    ) -> Result<CompositorState, BindError>
    where
        State: Dispatch<WlCompositor, (), State> + 'static,
        State: Dispatch<WpViewporter, (), State> + 'static,
        State: Dispatch<WpFractionalScaleManagerV1, (), State> + 'static,
    {
        let wl_compositor = globals.bind(qh, 1..=Self::COMPOSITOR_VERSION, ())?;
        let wp_viewporter = globals.bind(qh, 1..=Self::VIEWPORTER_VERSION, ())?;
        let fractional_scale_manager = globals.bind(qh, 1..=Self::FRACTIONAL_SCALE_VERSION, ())?;

        Ok(CompositorState {
            wl_compositor,
            wp_viewporter,
            fractional_scale_manager,
        })
    }

    pub fn wl_compositor(&self) -> &WlCompositor {
        &self.wl_compositor
    }

    pub fn create_surface<D>(&self, qh: &QueueHandle<D>, view_id: ViewId) -> Surface
    where
        D: 'static,
        D: Dispatch<WlSurface, SurfaceData>,
        D: Dispatch<WpViewport, SurfaceData>,
        D: Dispatch<WpFractionalScaleV1, SurfaceData>,
    {
        Surface::new(self, qh, view_id)
    }
}

impl AsRef<Self> for CompositorState {
    fn as_ref(&self) -> &Self {
        self
    }
}

#[derive(Debug, Clone)]
pub struct SurfaceData {
    inner: Arc<SurfaceDataInner>,
}

/// Data associated with a [`WlSurface`](wl_surface::WlSurface).
#[derive(Debug)]
struct SurfaceDataInner {
    view_id: ViewId,

    /// The scale factor of the surface.
    scale_factor: AtomicF64,

    /// Parent surface used when creating subsurfaces.
    ///
    /// For top-level surfaces this is always `None`.
    parent_surface: Option<WlSurface>,

    was_mapped: AtomicBool,

    previous_size: Mutex<Option<fluster::Size<u32>>>,

    waiting_for_frame: AtomicBool,
}

impl SurfaceData {
    pub fn for_view(view_id: ViewId) -> Self {
        Self::new(view_id, None, 1.0)
    }

    /// Create a new surface that initially reports the given scale factor and parent.
    pub fn new(view_id: ViewId, parent_surface: Option<WlSurface>, scale_factor: f64) -> Self {
        Self {
            inner: Arc::new(SurfaceDataInner {
                view_id,
                scale_factor: AtomicF64::new(scale_factor),
                parent_surface,
                was_mapped: AtomicBool::new(view_id == ViewId::IMPLICIT),
                previous_size: Mutex::new(None),
                waiting_for_frame: AtomicBool::new(false),
            }),
        }
    }

    /// The view ID associated with this surface.
    pub fn view_id(&self) -> ViewId {
        self.inner.view_id
    }

    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor.load()
    }

    pub fn was_mapped(&self) -> &AtomicBool {
        &self.inner.was_mapped
    }

    pub fn with_previous_size<T>(&self, f: impl FnOnce(&mut Option<fluster::Size<u32>>) -> T) -> T {
        f(&mut self.inner.previous_size.lock().unwrap())
    }

    /// The parent surface used for this surface.
    ///
    /// The surface is `Some` for primarily for subsurfaces,
    /// since they must have a parent surface.
    pub fn parent_surface(&self) -> Option<&WlSurface> {
        self.inner.parent_surface.as_ref()
    }

    pub fn is_waiting_for_frame(&self) -> bool {
        self.inner.waiting_for_frame.load(Ordering::Relaxed)
    }

    pub fn swap_waiting_for_frame(&self, waiting: bool) -> bool {
        self.inner
            .waiting_for_frame
            .swap(waiting, Ordering::Relaxed)
    }
}

/// An owned [`WlSurface`](wl_surface::WlSurface).
///
/// This destroys the surface on drop.
#[derive(Debug)]
pub struct Surface {
    wl_surface: WlSurface,
    viewport: WpViewport,
    fractional_scale: WpFractionalScaleV1,
    data: SurfaceData,
}

impl Surface {
    pub fn new<D>(state: &impl AsRef<CompositorState>, qh: &QueueHandle<D>, view_id: ViewId) -> Self
    where
        D: 'static,
        D: Dispatch<WlSurface, SurfaceData>,
        D: Dispatch<WpViewport, SurfaceData>,
        D: Dispatch<WpFractionalScaleV1, SurfaceData>,
    {
        Self::with_data(state, qh, SurfaceData::for_view(view_id))
    }

    pub fn with_data<D>(
        state: &impl AsRef<CompositorState>,
        qh: &QueueHandle<D>,
        data: SurfaceData,
    ) -> Self
    where
        D: 'static,
        D: Dispatch<WlSurface, SurfaceData>,
        D: Dispatch<WpViewport, SurfaceData>,
        D: Dispatch<WpFractionalScaleV1, SurfaceData>,
    {
        let wl_surface = state
            .as_ref()
            .wl_compositor
            .create_surface(qh, data.clone());
        let viewport = state
            .as_ref()
            .wp_viewporter
            .get_viewport(&wl_surface, qh, data.clone());
        let fractional_scale = state
            .as_ref()
            .fractional_scale_manager
            .get_fractional_scale(&wl_surface, qh, data.clone());

        Surface {
            wl_surface,
            viewport,
            fractional_scale,
            data,
        }
    }

    pub fn data(&self) -> &SurfaceData {
        &self.data
    }

    pub fn wl_surface(&self) -> &WlSurface {
        &self.wl_surface
    }

    pub fn viewport(&self) -> &WpViewport {
        &self.viewport
    }

    pub fn fractional_scale(&self) -> &WpFractionalScaleV1 {
        &self.fractional_scale
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.fractional_scale.destroy();
        self.viewport.destroy();
        self.wl_surface.destroy();
    }
}

#[macro_export]
macro_rules! delegate_compositor {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        $crate::delegate_compositor!(@{ $(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty }; surface: []);
        $crate::delegate_compositor!(@{ $(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty }; surface-only: $crate::shell::compositor::SurfaceData);
    };
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty, surface: [$($surface: ty),*$(,)?]) => {
        $crate::delegate_compositor!(@{ $(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty }; surface: [ $($surface),* ]);
    };
    (@{$($ty:tt)*}; surface: []) => {
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::client::protocol::wl_compositor::WlCompositor: ()
            ] => $crate::shell::compositor::CompositorState
        );
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewporter::WpViewporter: ()
            ] => $crate::shell::compositor::CompositorState
        );
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1: ()
            ] => $crate::shell::compositor::CompositorState
        );
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::client::protocol::wl_callback::WlCallback: ::smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface
            ] => $crate::shell::compositor::CompositorState
        );
    };
    (@{$($ty:tt)*}; surface-only: $surface:ty) => {
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface: $surface
            ] => $crate::shell::compositor::CompositorState
        );
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport: $surface
            ] => $crate::shell::compositor::CompositorState
        );
        ::smithay_client_toolkit::reexports::client::delegate_dispatch!($($ty)*:
            [
                ::smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1: $surface
            ] => $crate::shell::compositor::CompositorState
        );
    };
    (@$ty:tt; surface: [ $($surface:ty),+ ]) => {
        $crate::delegate_compositor!(@$ty; surface: []);
        $(
            $crate::delegate_compositor!(@$ty; surface-only: $surface);
        )*
    };
}

impl<D> Dispatch<WlSurface, SurfaceData, D> for CompositorState
where
    D: Dispatch<WlSurface, SurfaceData> + CompositorHandler + OutputHandler + 'static,
{
    fn event(
        state: &mut D,
        surface: &WlSurface,
        event: wl_surface::Event,
        data: &SurfaceData,
        conn: &Connection,
        qh: &QueueHandle<D>,
    ) {
        match event {
            wl_surface::Event::Enter { .. }
            | wl_surface::Event::Leave { .. }
            | wl_surface::Event::PreferredBufferScale { .. }
            | wl_surface::Event::PreferredBufferTransform { .. } => {
                // i don't care about any of these lol
            }
            _ => unreachable!(),
        }
    }
}

impl<D> Dispatch<WpViewport, SurfaceData, D> for CompositorState
where
    D: Dispatch<WpViewport, SurfaceData> + CompositorHandler + OutputHandler + 'static,
{
    fn event(
        state: &mut D,
        proxy: &WpViewport,
        event: <WpViewport as Proxy>::Event,
        data: &SurfaceData,
        conn: &Connection,
        qhandle: &QueueHandle<D>,
    ) {
        unreachable!("wp_viewport has no events")
    }
}

impl<D> Dispatch<WpFractionalScaleV1, SurfaceData, D> for CompositorState
where
    D: Dispatch<WpFractionalScaleV1, SurfaceData> + CompositorHandler + OutputHandler + 'static,
{
    fn event(
        state: &mut D,
        proxy: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as Proxy>::Event,
        data: &SurfaceData,
        conn: &Connection,
        qhandle: &QueueHandle<D>,
    ) {
        match event {
            wp_fractional_scale_v1::Event::PreferredScale { scale } => {
                let scale = f64::from(scale) / 120.0;

                data.inner.scale_factor.store(scale);

                state.scale_factor_changed(conn, qhandle, data, scale);
            }
            _ => todo!(),
        }
    }
}

/// A trivial wrapper around a [`WlRegion`][wl_region::WlRegion].
///
/// This destroys the region on drop.
#[derive(Debug)]
pub struct Region(wl_region::WlRegion);

impl Region {
    pub fn new(
        compositor: &impl ProvidesBoundGlobal<WlCompositor, { CompositorState::COMPOSITOR_VERSION }>,
    ) -> Result<Region, GlobalError> {
        compositor
            .bound_global()
            .map(|c| {
                c.send_constructor(
                    wl_compositor::Request::CreateRegion {},
                    Arc::new(RegionData),
                )
                .unwrap_or_else(|_| Proxy::inert(c.backend().clone()))
            })
            .map(Region)
    }

    pub fn add(&self, x: i32, y: i32, width: i32, height: i32) {
        self.0.add(x, y, width, height);
    }

    pub fn subtract(&self, x: i32, y: i32, width: i32, height: i32) {
        self.0.subtract(x, y, width, height);
    }

    pub fn wl_region(&self) -> &wl_region::WlRegion {
        &self.0
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        self.0.destroy();
    }
}

struct RegionData;

impl ObjectData for RegionData {
    fn event(
        self: Arc<Self>,
        _: &Backend,
        _: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<(dyn ObjectData + 'static)>> {
        unreachable!("wl_region has no events");
    }
    fn destroyed(&self, _: ObjectId) {}
}

impl<D> Dispatch<WlCompositor, (), D> for CompositorState
where
    D: Dispatch<WlCompositor, ()> + CompositorHandler,
{
    fn event(
        _: &mut D,
        _: &WlCompositor,
        _: wl_compositor::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        unreachable!("wl_compositor has no events")
    }
}

impl ProvidesBoundGlobal<WlCompositor, { CompositorState::COMPOSITOR_VERSION }>
    for CompositorState
{
    fn bound_global(&self) -> Result<WlCompositor, GlobalError> {
        Ok(self.wl_compositor.clone())
    }
}

impl<D> Dispatch<WpViewporter, (), D> for CompositorState
where
    D: Dispatch<WpViewporter, ()> + CompositorHandler,
{
    fn event(
        _: &mut D,
        _: &WpViewporter,
        _: <WpViewporter as Proxy>::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        unreachable!("wp_viewporter has no events")
    }
}

impl ProvidesBoundGlobal<WpViewporter, { CompositorState::VIEWPORTER_VERSION }>
    for CompositorState
{
    fn bound_global(&self) -> Result<WpViewporter, GlobalError> {
        Ok(self.wp_viewporter.clone())
    }
}

impl<D> Dispatch<WpFractionalScaleManagerV1, (), D> for CompositorState
where
    D: Dispatch<WpFractionalScaleManagerV1, ()> + CompositorHandler,
{
    fn event(
        _: &mut D,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as Proxy>::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        unreachable!("wp_fractional_scale_manager_v1 has no events")
    }
}

impl ProvidesBoundGlobal<WpFractionalScaleManagerV1, { CompositorState::FRACTIONAL_SCALE_VERSION }>
    for CompositorState
{
    fn bound_global(&self) -> Result<WpFractionalScaleManagerV1, GlobalError> {
        Ok(self.fractional_scale_manager.clone())
    }
}

impl<D> Dispatch<wl_callback::WlCallback, WlSurface, D> for CompositorState
where
    D: Dispatch<wl_callback::WlCallback, WlSurface> + CompositorHandler,
{
    fn event(
        state: &mut D,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        surface: &WlSurface,
        conn: &Connection,
        qh: &QueueHandle<D>,
    ) {
        match event {
            wl_callback::Event::Done { callback_data } => {
                state.frame(conn, qh, surface.data().unwrap(), callback_data);
            }

            _ => unreachable!(),
        }
    }
}
