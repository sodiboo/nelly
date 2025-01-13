//! This is just glue code that passes into `nelly::run` the generated artifacts.
//!    The project is structured this way, such that the main embedder library
//!          doesn't need to invoke `flutter build` in its build script.
//!               (because that massively slows down rust-analyzer)

use std::path::Path;

fn main() -> Result<(), impl std::fmt::Debug> {
    nelly::run(
        Path::new(generated::ASSETS),
        generated::APP_LIBRARY.map(Path::new),
    )
}
