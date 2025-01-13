use std::{
    os::fd::OwnedFd,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use volito::ViewId;
use smithay_client_toolkit::{
    error::GlobalError,
    globals::ProvidesBoundGlobal,
    output::OutputHandler,
    reexports::{
        client::{
            backend::{protocol::Message, Backend, ObjectData, ObjectId},
            globals::{BindError, GlobalList},
            protocol::{
                wl_callback,
                wl_compositor::{self, WlCompositor},
                wl_region,
                wl_surface::{self, WlSurface},
            },
            Connection, Dispatch, Proxy, QueueHandle,
        },
        protocols::wp::{
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            },
            viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
        },
    },
};
use tracing::{error, info};

use crate::atomic_f64::AtomicF64;

pub trait CompositorHandler: Sized {
    fn compositor_state(&self) -> &CompositorState;

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

    previous_size: Mutex<Option<volito::Size<u32>>>,

    logical_size_constraints: Mutex<Option<LogicalSizeConstraints>>,

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
                logical_size_constraints: Mutex::new(None),
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

    pub fn previous_physical_size(&self) -> Option<volito::Size<u32>> {
        *self.inner.previous_size.lock().unwrap()
    }

    // this function both sets `was_mapped()` to `true`, and sets `previous_size()` to `Some()` for the first time.
    // so why isn't `was_mapped()` just defined as `previous_size().is_some()`?
    // it's because the implicit view is always mapped, but it needs to be configured to get an initial size.
    // so `was_mapped()` is set to true *before* the initial configuration, if and only if `view_id == ViewId::IMPLICIT`.
    pub fn set_physical_size(&self, size: volito::Size<u32>, engine: &mut volito::Engine) {
        let size = self
            .physical_size_constraints()
            .map_or(size, |constraints| {
                constraints.constrain_physical_size(size)
            });

        let view_id = self.view_id();
        if self.previous_physical_size() == Some(size) {
            return;
        }
        let pixel_ratio = self.scale_factor();
        let view_metrics = volito::WindowMetricsEvent {
            view_id,
            width: size.width as usize,
            height: size.height as usize,
            pixel_ratio,
            left: 0,
            top: 0,
            physical_view_inset_top: 0.0,
            physical_view_inset_right: 0.0,
            physical_view_inset_bottom: 0.0,
            physical_view_inset_left: 0.0,
            display_id: 0,
        };

        if self.was_mapped().swap(true, Ordering::Relaxed) {
            engine.send_window_metrics_event(view_metrics).unwrap();
        } else {
            let inner = self.inner.clone();
            engine
                .add_view(view_id, view_metrics, move |success| {
                    if success {
                        info!("Added view {view_id:?}");
                    } else {
                        // oopsie, didn't get to map it anyway
                        inner.was_mapped.store(false, Ordering::Relaxed);
                        error!("Failed to add view {view_id:?}");
                    }
                })
                .unwrap()
        }

        *self.inner.previous_size.lock().unwrap() = Some(size);
    }

    pub fn physical_size_constraints(&self) -> Option<PhysicalSizeConstraints> {
        self.inner
            .logical_size_constraints
            .lock()
            .unwrap()
            .map(|logical| logical.to_physical(self.scale_factor()))
    }

    pub fn set_logical_size_constraints(
        &self,
        constraints: LogicalSizeConstraints,
        engine: &mut volito::Engine,
    ) {
        let prev_constraints = self
            .inner
            .logical_size_constraints
            .lock()
            .unwrap()
            .replace(constraints);

        if prev_constraints == Some(constraints) {
            error!("set_logical_size_constraints called with the same constraints as before");
            return;
        }

        if let Some(previous_size) = self.previous_physical_size() {
            // reapply constraints
            self.set_physical_size(previous_size, engine);
        }
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

#[derive(Debug, Clone, Copy)]
pub struct LogicalSizeConstraints {
    pub min_width: f64,
    pub min_height: f64,
    pub max_width: f64,
    pub max_height: f64,
}

impl Eq for LogicalSizeConstraints {}
impl PartialEq for LogicalSizeConstraints {
    fn eq(&self, other: &Self) -> bool {
        self.min_width.to_bits() == other.min_width.to_bits()
            && self.min_height.to_bits() == other.min_height.to_bits()
            && self.max_width.to_bits() == other.max_width.to_bits()
            && self.max_height.to_bits() == other.max_height.to_bits()
    }
}

impl LogicalSizeConstraints {
    fn to_physical(self, pixel_ratio: f64) -> PhysicalSizeConstraints {
        fn normalize(rounded: f64) -> Option<u32> {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "bro it's checked"
            )]
            (rounded.is_finite() && rounded >= 0.0 && rounded <= f64::from(u32::MAX))
                .then_some(rounded as u32)
        }

        fn min(orig: f64) -> u32 {
            normalize(orig.ceil()).unwrap_or(0)
        }

        fn max(orig: f64) -> u32 {
            normalize(orig.floor()).unwrap_or(u32::MAX)
        }

        PhysicalSizeConstraints {
            min_width: min(self.min_width * pixel_ratio),
            min_height: min(self.min_height * pixel_ratio),
            max_width: max(self.max_width * pixel_ratio),
            max_height: max(self.max_height * pixel_ratio),
        }
    }

    pub fn to_wayland_constraints(self) -> Option<(i32, i32, i32, i32)> {
        fn normalize(rounded: f64) -> Option<i32> {
            #[expect(clippy::cast_possible_truncation, reason = "bro it's checked")]
            (rounded.is_finite() && rounded >= 0.0 && rounded <= f64::from(i32::MAX))
                .then_some(rounded as i32)
        }

        fn min(orig: f64) -> i32 {
            normalize(orig.ceil()).unwrap_or(0)
        }

        fn max(orig: f64) -> i32 {
            normalize(orig.floor()).unwrap_or(0)
        }

        let min_width = min(self.min_width);
        let min_height = min(self.min_height);
        let max_width = max(self.max_width);
        let max_height = max(self.max_height);

        if min_width > max_width || min_height > max_height {
            None // will cause us to be disconnected, so don't submit anything
        } else {
            Some((min_width, min_height, max_width, max_height))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSizeConstraints {
    pub min_width: u32,
    pub min_height: u32,
    pub max_width: u32,
    pub max_height: u32,
}

impl PhysicalSizeConstraints {
    fn constrain_physical_size(self, size: volito::Size<u32>) -> volito::Size<u32> {
        volito::Size {
            width: u32::clamp(size.width, self.min_width, self.max_width),
            height: u32::clamp(size.height, self.min_height, self.max_height),
        }
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
        _: &mut D,
        _: &WlSurface,
        event: wl_surface::Event,
        _: &SurfaceData,
        _: &Connection,
        _: &QueueHandle<D>,
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
        _: &mut D,
        _: &WpViewport,
        _: <WpViewport as Proxy>::Event,
        _: &SurfaceData,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        unreachable!("wp_viewport has no events")
    }
}

impl<D> Dispatch<WpFractionalScaleV1, SurfaceData, D> for CompositorState
where
    D: Dispatch<WpFractionalScaleV1, SurfaceData> + CompositorHandler + OutputHandler + 'static,
{
    fn event(
        _: &mut D,
        _: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as Proxy>::Event,
        data: &SurfaceData,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        match event {
            wp_fractional_scale_v1::Event::PreferredScale { scale } => {
                let scale = f64::from(scale) / 120.0;

                data.inner.scale_factor.store(scale);
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
