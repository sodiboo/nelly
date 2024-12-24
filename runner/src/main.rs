use std::path::Path;

#[cfg(feature = "build-in-cargo")]
mod app {
    include!(concat!(env!("OUT_DIR"), "/app.rs"));
}

#[cfg(not(feature = "build-in-cargo"))]
mod app {
    pub const ASSETS: &str = env!("NELLY_ASSETS");
    pub const APP_LIBRARY: Option<&str> = option_env!("NELLY_APP_LIBRARY");
}

fn main() -> anyhow::Result<()> {
    nelly::run(Path::new(app::ASSETS), app::APP_LIBRARY.map(Path::new))
}
