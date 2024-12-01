use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use fluster::{Engine, ViewId, WindowMetricsEvent};
use smithay_client_toolkit::compositor::{CompositorState, Surface, SurfaceData, SurfaceDataExt};
use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::reexports::calloop::{self, LoopHandle};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_output;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, Proxy};
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::shell::xdg::window::{Window, WindowDecorations};
use smithay_client_toolkit::shell::xdg::{XdgShell, XdgSurface};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::Shm;
use tracing::debug;

use crate::config::Config;
use crate::embedder;

use self::seat::SeatState;

#[path = "handlers.rs"]
mod handlers;
#[path = "seat/mod.rs"]
mod seat;

#[allow(dead_code)]
pub struct Nelly {
    events: Sender<WaylandBackendEvent>,

    engine: Option<Engine>,

    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm: Shm,
    xdg_state: XdgShell,

    previous_size: Option<fluster::Size<u32>>,
}

pub enum WaylandBackendEvent {
    Frame,
    Close,
    Resize(Window, fluster::Size<u32>),
}

impl Nelly {
    pub fn new(
        config: Rc<RefCell<Config>>,
        loop_handle: LoopHandle<Nelly>,
    ) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;

        let (globals, mut queue) = registry_queue_init::<Nelly>(&connection).unwrap();
        debug!("init wayland");

        let qh = queue.handle();

        let (events, channel) = calloop::channel::channel();

        loop_handle
            .insert_source(channel, |event, _, nelly| {
                let calloop::channel::Event::Msg(event) = event else {
                    return;
                };
                match event {
                    WaylandBackendEvent::Frame => {
                        nelly.engine().schedule_frame().unwrap();
                    }
                    WaylandBackendEvent::Close => {
                        debug!("Closing window");
                    }
                    WaylandBackendEvent::Resize(window, size) => {
                        debug!(
                            "Resizing window to {}",
                            format_args!("{}x{}", size.width, size.height)
                        );
                        let view_id = window
                            .wl_surface()
                            .data::<NellySurfaceData>()
                            .unwrap()
                            .view_id();

                        nelly
                            .engine()
                            .send_window_metrics_event(WindowMetricsEvent {
                                view_id,
                                width: size.width as usize,
                                height: size.height as usize,
                                pixel_ratio: 1.0,
                                left: 0,
                                top: 0,
                                physical_view_inset_top: 0.0,
                                physical_view_inset_right: 0.0,
                                physical_view_inset_bottom: 0.0,
                                physical_view_inset_left: 0.0,
                                display_id: 0,
                            })
                            .unwrap();
                    }
                };
            })
            .map_err(|_| anyhow::anyhow!("Failed to insert message channel"))?;

        let registry_state = RegistryState::new(&globals);
        let shm = Shm::bind(&globals, &qh)?;
        let seat_state = SeatState::new(&globals, &qh);
        let output_state = OutputState::new(&globals, &qh);
        let compositor_state = CompositorState::bind(&globals, &qh)?;
        let xdg_state = XdgShell::bind(&globals, &qh)?;

        let mut this = Self {
            events,

            engine: None,

            registry_state,
            shm,
            seat_state,
            output_state,
            compositor_state,
            xdg_state,

            previous_size: None,
        };
        queue.roundtrip(&mut this)?;

        WaylandSource::new(connection, queue).insert(loop_handle.clone())?;

        let implicit_surface = Surface::with_data(
            &this.compositor_state,
            &qh,
            NellySurfaceData {
                view_id: ViewId::IMPLICIT,
                surface_data: Arc::new(SurfaceData::default()),
            },
        )?;

        let main_window =
            this.xdg_state
                .create_window(implicit_surface, WindowDecorations::ServerDefault, &qh);

        // This transform is necessary to make the window appear right-side up.
        // It will never change throughout the lifetime of the window.
        main_window
            .set_buffer_transform(wl_output::Transform::Flipped180)
            .unwrap();

        main_window.set_app_id("nelly");
        main_window.set_title("nelly");

        // We initialize everything as 1x1, and pray the compositor chooses a better size
        // upon the first configure event. This commit is necessary to receive that event.
        main_window.commit();

        this.engine = Some(embedder::init(
            config.clone(),
            loop_handle,
            &this.shm,
            qh,
            main_window.into(),
        )?);

        this.engine().notify_display_update(
            fluster::DisplaysUpdateType::Startup,
            &[fluster::Display {
                display_id: 0,
                single_display: true,
                refresh_rate: 0.0,
                width: 800 as _,
                height: 600 as _,
                device_pixel_ratio: 1.0,
            }],
        )?;

        debug!("wayland init");

        Ok(this)
    }

    pub fn send_event(&self, event: WaylandBackendEvent) {
        self.events
            .send(event)
            .expect("WaylandBackend event channel closed");
    }

    pub fn engine(&mut self) -> &mut Engine {
        self.engine.as_mut().unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct NellySurfaceData {
    view_id: ViewId,
    surface_data: Arc<SurfaceData>,
}

impl NellySurfaceData {
    pub fn view_id(&self) -> ViewId {
        self.view_id
    }
}
impl SurfaceDataExt for NellySurfaceData {
    fn surface_data(&self) -> &SurfaceData {
        &self.surface_data
    }
}
