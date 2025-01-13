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

use std::path::Path;

use config::Config;
use nelly::Nelly;
use smithay_client_toolkit::reexports::calloop::EventLoop;
use tracing_subscriber::EnvFilter;

mod engine_meta {
    include!(concat!(env!("OUT_DIR"), "/engine_meta.rs"));
}

mod atomic_f64;
mod config;
mod embedder;
mod nelly;
mod platform_message;
mod pool;
mod shell;

const DEFAULT_LOG_FILTER: &str = "nelly=trace,volito=trace";

// this is the entrypoint.
// it just gets paths to the compile output of the Dart half of the app.
// the actual main() is in `/runner/src/main.rs`
// but distro packagers may wish to write a different runner to compile the Dart half without Cargo.
pub fn run(assets_path: &Path, app_library: Option<&Path>) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .pretty()
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
