#![feature(ptr_metadata)]
#![feature(integer_sign_cast)]
#![warn(clippy::pedantic)]
#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::too_many_lines,
    clippy::struct_field_names,
    clippy::missing_errors_doc,
    clippy::semicolon_if_nothing_returned, // this one is wrong imo
)]
#![deny(clippy::print_stderr, clippy::print_stdout)] // use tracing instead

use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use config::Config;
use nelly::Nelly;
use smithay_client_toolkit::reexports::calloop::{
    self,
    timer::{TimeoutAction, Timer},
    EventLoop,
};
use tracing::{debug, level_filters::LevelFilter, trace, Level, Metadata};
use tracing_log::{AsLog, LogTracer};
use tracing_subscriber::{filter::FilterExt, EnvFilter};

mod engine_meta {
    include!(concat!(env!("OUT_DIR"), "/engine_meta.rs"));
}

mod atomic_f64;
mod config;
mod embedder;
pub mod ffi;
mod nelly;
mod platform_message;
mod pool;
mod shell;

const DEFAULT_LOG_FILTER: &str = "nelly=trace,fluster=trace";

pub fn run(assets_path: &Path, app_library: Option<&Path>) -> anyhow::Result<()> {
    let rust_log = std::env::var("RUST_LOG").ok();
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(
            EnvFilter::builder().parse_lossy(rust_log.as_deref().unwrap_or(DEFAULT_LOG_FILTER)),
        )
        .init();
    let mut event_loop = EventLoop::try_new()?;

    let config = Arc::new(Mutex::new(Config::load()));

    trace!("main() on thread {:?}", std::thread::current().id());

    let mut nelly = Nelly::new(assets_path, app_library, &config, event_loop.handle())?;

    event_loop.run(None, &mut nelly, |state| {
        _ = state;
    })?;

    Ok(())
}
