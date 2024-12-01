use std::{cell::RefCell, rc::Rc};

use config::Config;
use nelly::Nelly;
use smithay_client_toolkit::reexports::calloop::EventLoop;
use tracing::Level;
use tracing_subscriber::EnvFilter;

mod config;
mod embedder;
mod nelly;
mod pool;

fn main() -> anyhow::Result<()> {
    let rust_log = std::env::var("RUST_LOG").ok();
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(Level::TRACE)
        .with_env_filter(
            EnvFilter::builder().parse_lossy(rust_log.as_deref().unwrap_or("nelly=debug")),
        )
        .init();

    let mut event_loop = EventLoop::try_new().unwrap();

    let config = Rc::new(RefCell::new(Config::load()));

    let mut wayland = Nelly::new(config.clone(), event_loop.handle()).unwrap();

    event_loop
        .run(None, &mut wayland, |state| {
            _ = state;
        })
        .unwrap();

    Ok(())
}
