#![feature(ptr_metadata)]
#![feature(integer_sign_cast)]
#![warn(clippy::pedantic)]
#![allow(
    // unused_imports,
    dead_code,
    clippy::too_many_lines,
    clippy::struct_field_names,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned, // this one is wrong imo
)]
#![deny(clippy::print_stderr, clippy::print_stdout)] // use tracing instead

use std::{
    convert::Infallible,
    path::Path,
    sync::{Arc, Mutex},
};

use config::Config;
use halcyon_embedder::{EmbedderArgs, Halcyon, HalcyonHandler};
use platform_message::NellyPlatformRequest;
// use nelly::Nelly;
use smithay_client_toolkit::{
    reexports::{
        calloop::{EventLoop, LoopHandle, LoopSignal},
        calloop_wayland_source::WaylandSource,
        client::{globals::registry_queue_init, Connection, QueueHandle},
    },
    registry::{ProvidesRegistryState, RegistryState},
};
use tracing_subscriber::EnvFilter;
use volito::graphics::RendererConfig;

mod engine_meta {
    include!(concat!(env!("OUT_DIR"), "/engine_meta.rs"));
}

mod config;
mod platform_message;

const DEFAULT_LOG_FILTER: &str = "nelly=trace,halcyon=trace,volito=trace";

// this is the entrypoint.
// it just gets paths to the compile output of the Dart half of the app.
// the actual main() is in `/runner/src/main.rs`
// but distro packagers may wish to write a different runner to compile the Dart half without Cargo.
pub fn run(assets_path: &Path, app_library: Option<&Path>) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(
            EnvFilter::builder().parse_lossy(
                std::env::var("RUST_LOG")
                    .ok()
                    .as_deref()
                    .unwrap_or(DEFAULT_LOG_FILTER),
            ),
        )
        .init();

    let mut event_loop = EventLoop::try_new()?;

    event_loop
        .run(
            None,
            &mut Nelly::new(assets_path, app_library, &Config::load(), &event_loop)?,
            |nelly| {
                _ = nelly; // do absolutely nothing
            },
        )
        .map_err(Into::into)
}

struct Nelly {
    pub qh: QueueHandle<Self>,
    pub loop_handle: LoopHandle<'static, Nelly>,
    pub loop_signal: LoopSignal,

    // engine: Engine,
    // pub views: Arc<Mutex<HashMap<ViewId, FlutterWaylandSurface>>>,
    registry_state: RegistryState,

    halcyon: Halcyon<Nelly>,
    // halcyon: Halcy
    // shm: Shm,
    // seat_state: SeatState,
    // output_state: OutputState,
    // pub compositor_state: CompositorState,
    // pub xdg_state: XdgShell,
    // pub layer_shell: LayerShell,
}

impl ProvidesRegistryState for Nelly {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    smithay_client_toolkit::registry_handlers![Halcyon<Self>];
}
smithay_client_toolkit::delegate_registry!(Nelly);

impl HalcyonHandler for Nelly {
    type PlatformRequest = NellyPlatformRequest;

    fn halcyon(&mut self) -> &mut Halcyon<Self> {
        &mut self.halcyon
    }
}
halcyon_embedder::delegate_halcyon!(Nelly);

impl Nelly {
    pub fn new(
        assets_path: &Path,
        app_library: Option<&Path>,
        config: &Arc<Mutex<Config>>,
        event_loop: &EventLoop<'static, Nelly>,
    ) -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()?;

        let (globals, queue) = registry_queue_init::<Nelly>(&connection).unwrap();

        let qh = queue.handle();

        let registry_state = RegistryState::new(&globals);
        let halcyon = Halcyon::new(
            EmbedderArgs {
                assets_path,
                icu_data_path: Path::new(crate::engine_meta::ICUDTL_DAT),
                app_library,
                custom_dart_entrypoint: None,
                dart_entrypoint_argv: &[],
                renderer: halcyon_embedder::RendererArgs::Vulkan {
                    application_name: Some("nelly"),
                    application_version: 0,
                },
            },
            &globals,
            event_loop,
            qh.clone(),
        )?;

        WaylandSource::new(connection, queue).insert(event_loop.handle())?;

        Ok(Self {
            qh,
            loop_handle: event_loop.handle(),
            loop_signal: event_loop.get_signal(),

            // engine,
            // views,
            registry_state,
            halcyon,
            // shm,
            // seat_state,
            // output_state,
            // compositor_state,
            // xdg_state,
            // layer_shell,
        })
    }
}
