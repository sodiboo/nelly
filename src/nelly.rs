use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use volito::{Engine, ViewId};
use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use smithay_client_toolkit::reexports::calloop::{self, EventLoop, LoopHandle, LoopSignal};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::shm::Shm;
use tracing::{debug, error};

use crate::config::Config;
use crate::embedder::{self, FlutterWaylandSurface};
use crate::shell::compositor::CompositorState;
use crate::shell::layer::LayerShell;
use crate::shell::xdg::XdgShell;

use self::seat::SeatState;

#[path = "handlers.rs"]
mod handlers;
#[path = "seat/mod.rs"]
mod seat;

#[allow(dead_code)]
pub struct Nelly {
    events: Sender<NellyEvent>,
    pub qh: QueueHandle<Self>,
    pub loop_handle: LoopHandle<'static, Nelly>,
    pub loop_signal: LoopSignal,

    engine: Engine,
    pub views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,

    registry_state: RegistryState,
    shm: Shm,
    seat_state: SeatState,
    output_state: OutputState,
    pub compositor_state: CompositorState,
    pub xdg_state: XdgShell,
    pub layer_shell: LayerShell,
}

pub enum NellyEvent {
    Frame,
    ViewRemoved(ViewId),
}

impl Nelly {
    pub fn new(
        assets_path: &Path,
        app_library: Option<&Path>,
        config: &Arc<Mutex<Config>>,
        event_loop: &EventLoop<'static, Nelly>,
    ) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;

        let (globals, queue) = registry_queue_init::<Nelly>(&connection).unwrap();
        debug!("init wayland");

        let qh = queue.handle();

        let (events, channel) = calloop::channel::channel();

        event_loop
            .handle()
            .insert_source(channel, |event, (), nelly| {
                let calloop::channel::Event::Msg(event) = event else {
                    return;
                };
                match event {
                    NellyEvent::Frame => {}
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
        let layer_shell = LayerShell::bind(&globals, &qh)?;

        WaylandSource::new(connection, queue).insert(event_loop.handle())?;

        let views = Arc::new(Mutex::new(HashMap::new()));

        let engine = embedder::init(
            assets_path,
            app_library,
            config,
            event_loop,
            &shm,
            &qh,
            views.clone(),
        )?;

        Ok(Self {
            events,
            qh,
            loop_handle: event_loop.handle(),
            loop_signal: event_loop.get_signal(),

            engine,
            views,

            registry_state,
            shm,
            seat_state,
            output_state,
            compositor_state,
            xdg_state,
            layer_shell,
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

    pub fn remove_view(&mut self, view_id: ViewId) -> volito::Result<()> {
        let events = self.events().clone();
        self.engine().remove_view(view_id, move |success| {
            if success {
                events
                    .send(NellyEvent::ViewRemoved(view_id))
                    .expect("Nelly event channel closed");
            } else {
                error!("Failed to remove view {:?}", view_id);
            }
        })
    }
}
