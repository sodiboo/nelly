use std::{fs::File, io::Write, path::PathBuf};

use fluster::build::{BuildError, FlutterApp};

fn main() {
    match FlutterApp::builder().entrypoint("src/main.dart").build() {
        Ok(app) => {
            let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

            let mut generated =
                File::create(out_dir.join("generated.rs")).expect("failed to create generated.rs");

            write!(
                generated,
                r#"
                   pub const ASSETS: &str = {assets:?};
                   pub const APP_LIBRARY: Option<&str> = {app_library};
                "#,
                assets = app.assets().to_str().unwrap(),
                app_library = match app.app_library() {
                    Some(lib) => format!("Some({:?})", lib.to_str().unwrap()),
                    None => "None".into(),
                }
            )
            .unwrap();
        }
        Err(BuildError::FlutterNotFound) => {
            println!("cargo::error=flutter was not found in PATH");
        }
        Err(BuildError::FlutterBundleBuildFailed(output)) => {
            println!("cargo::error=flutter bundle build failed");
            dump(&output.stdout);
            dump(&output.stderr);
        }
        Err(BuildError::FrontendServerNotFound) => {
            println!(
                "cargo::error=the frontend server could not be located in the flutter engine build"
            );
        }
        Err(BuildError::DartNotFound { .. }) => {
            println!(
                "cargo::error=the dart binary could not be located in the flutter engine build"
            );
        }
        Err(BuildError::KernelSnapshotBuildFailed(output)) => {
            println!("cargo::error=the kernel snapshot build failed");
            dump(&output.stdout);
            dump(&output.stderr);
        }
        Err(BuildError::GenSnapshotNotFound) => {
            println!("cargo::error=the gen_snapshot binary could not be located in the flutter engine build");
        }
        Err(BuildError::DartAotBuildFailed(output)) => {
            println!("cargo::error=dart AOT build failed");
            dump(&output.stdout);
            dump(&output.stderr);
        }
    }
}

fn dump(raw: &[u8]) {
    String::from_utf8_lossy(raw).lines().for_each(|line| {
        println!("cargo::error={}", line);
    });
}
