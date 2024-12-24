use std::{fs::File, io::Write, path::PathBuf};

fn env(name: &str) -> Option<String> {
    println!("cargo::rerun-if-env-changed={name}");
    std::env::var(name).ok()
}

fn main() {
    let out_dir = PathBuf::from(env("OUT_DIR").unwrap());

    let flutter_engine = env("DEP_FLUTTER_ENGINE_PATH").unwrap();
    println!("cargo::rustc-link-search=native={flutter_engine}");
    println!("cargo::rustc-link-lib=flutter_engine");

    let icudtl_dat = env("DEP_FLUTTER_ENGINE_ICUDTL_DAT").unwrap();

    let mut generated = File::create(out_dir.join("engine_meta.rs")).unwrap();

    write!(
        generated,
        "
pub const FLUTTER_ENGINE_PATH: &str = {flutter_engine:?}; 
pub const ICUDTL_DAT: &str = {icudtl_dat:?};"
    )
    .unwrap()
}
