use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use fluster::{Engine, ViewId, WindowMetricsEvent};
use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::reexports::calloop::{self, LoopHandle};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::protocol::{wl_output, wl_shm};
use smithay_client_toolkit::reexports::client::{Connection, Proxy, QueueHandle};
use smithay_client_toolkit::reexports::protocols::wp::viewporter;
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::shm::Shm;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::embedder::{self, FlutterWaylandSurface};
use crate::pool::SinglePool;
use crate::shell::compositor::CompositorState;
use crate::shell::xdg::window::{Window, WindowDecorations};
use crate::shell::xdg::XdgShell;
use crate::shell::WaylandSurface;

use self::seat::SeatState;

#[path = "handlers.rs"]
mod handlers;
#[path = "seat/mod.rs"]
mod seat;

#[allow(dead_code)]
pub struct Nelly {
    events: Sender<NellyEvent>,
    pub qh: QueueHandle<Self>,
    loop_handle: LoopHandle<'static, Nelly>,

    engine: Engine,
    pub views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,

    registry_state: RegistryState,
    shm: Shm,
    seat_state: SeatState,
    output_state: OutputState,
    pub compositor_state: CompositorState,
    pub xdg_state: XdgShell,
}

pub enum NellyEvent {
    Frame,
    Close(Window),
    Resize(Window, fluster::Size<u32>),
    ViewRemoved(ViewId),
}

impl Nelly {
    pub fn new(
        assets_path: &Path,
        app_library: Option<&Path>,
        config: &Arc<Mutex<Config>>,
        loop_handle: LoopHandle<'static, Nelly>,
    ) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;

        let (globals, queue) = registry_queue_init::<Nelly>(&connection).unwrap();
        debug!("init wayland");

        let qh = queue.handle();

        let (events, channel) = calloop::channel::channel();

        loop_handle
            .insert_source(channel, |event, (), nelly| {
                let calloop::channel::Event::Msg(event) = event else {
                    return;
                };
                match event {
                    NellyEvent::Frame => {}
                    NellyEvent::Close(window) => {
                        let view_id = window.view_id();
                        debug!("Closing window {view_id:?}");
                    }
                    NellyEvent::Resize(window, size) => {
                        let view_id = window.view_id();
                        debug!(
                            "Resizing window {view_id:?} to {}",
                            format_args!("{}x{}", size.width, size.height)
                        );

                        let pixel_ratio = window.surface().data().scale_factor();

                        let width = (f64::from(size.width) * pixel_ratio).round();
                        let height = (f64::from(size.height) * pixel_ratio).round();

                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "i promise you it's fine"
                        )]
                        let (width, height) = (width as usize, height as usize);

                        let view_metrics = WindowMetricsEvent {
                            view_id,
                            width,
                            height,
                            pixel_ratio,
                            left: 0,
                            top: 0,
                            physical_view_inset_top: 0.0,
                            physical_view_inset_right: 0.0,
                            physical_view_inset_bottom: 0.0,
                            physical_view_inset_left: 0.0,
                            display_id: 0,
                        };

                        if window.was_mapped().swap(true, Ordering::Relaxed) {
                            nelly
                                .engine()
                                .send_window_metrics_event(view_metrics)
                                .unwrap();
                        } else {
                            nelly
                                .engine()
                                .add_view(view_id, view_metrics, move |success| {
                                    if success {
                                        info!("Added view {view_id:?}");
                                    } else {
                                        error!("Failed to add view {view_id:?}");
                                    }
                                })
                                .unwrap()
                        }
                    }
                    NellyEvent::ViewRemoved(view_id) => {
                        debug!("View {view_id:?} removed");
                        nelly.views.lock().unwrap().remove(&view_id);
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

        WaylandSource::new(connection, queue).insert(loop_handle.clone())?;

        let mut views = HashMap::new();

        {
            let view_id = ViewId::IMPLICIT;
            let surface = compositor_state.create_surface(&qh, view_id);

            let window = xdg_state.create_window(surface, WindowDecorations::ServerDefault, &qh);

            window.set_app_id("nelly");
            window.set_title("nelly");
            window.commit();

            views.insert(view_id, FlutterWaylandSurface::from(window));
        }

        let views = Arc::new(Mutex::new(views));

        let mut engine = embedder::init(
            assets_path,
            app_library,
            config,
            &loop_handle,
            &shm,
            &qh,
            views.clone(),
        )?;

        engine.notify_display_update(
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

        Ok(Self {
            events,
            qh,
            loop_handle,

            engine,
            views,

            registry_state,
            shm,
            seat_state,
            output_state,
            compositor_state,
            xdg_state,
        })
    }

    pub fn events(&self) -> &Sender<NellyEvent> {
        &self.events
    }

    pub fn send_event(&self, event: NellyEvent) {
        self.events.send(event).expect("Nelly event channel closed");
    }

    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }
}
