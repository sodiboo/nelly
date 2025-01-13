use std::{fs::File, io::Write, path::PathBuf};

use volito_build_support::{BuildError, FlutterApp};

fn env(name: &str) -> Option<String> {
    println!("cargo::rerun-if-env-changed={name}");
    std::env::var(name).ok()
}

fn main() {
    let out_dir = PathBuf::from(env("OUT_DIR").unwrap());

    let manifest_dir = env("CARGO_MANIFEST_DIR").unwrap();

    let workspace_dir = &manifest_dir[..manifest_dir.rfind('/').unwrap()];

    {
        let p = PathBuf::from(&manifest_dir).join("lib").join("gen.dart");

        let mut f = File::create(p).expect("Failed to create gen.dart");

        writeln!(f, r#"const WORKSPACE_DIR = "{workspace_dir}";"#).unwrap()
    }

    if let Some(app) = build_flutter_app() {
        let mut generated = File::create(out_dir.join("gen.rs")).unwrap();

        write!(
            generated,
            "
pub const ASSETS: &str = {assets:?};
pub const APP_LIBRARY: Option<&str> = {app_library:?};
            ",
            assets = app.assets().to_str().unwrap(),
            app_library = app.app_library().map(|lib| lib.to_str().unwrap()),
        )
        .unwrap()
    }
}

fn dump(raw: &[u8]) {
    dump_str(String::from_utf8_lossy(raw).as_ref())
}

fn dump_str(raw: &str) {
    raw.lines().for_each(|line| {
        println!("cargo::error={}", line);
    });
}

fn build_flutter_app() -> Option<FlutterApp> {
    FlutterApp::builder().project_root(PathBuf::from(env("CARGO_MANIFEST_DIR").unwrap()).parent().unwrap()).entrypoint("src/entrypoint.dart").with_experimental_feature("macros").build().inspect_err(|err| {
        match err {
        BuildError::FlutterNotFound => {
            println!("cargo::error=flutter was not found in PATH");
        }
        BuildError::FlutterBundleBuildFailed(output) => {
            println!("cargo::error=flutter bundle build failed");
            dump(&output.stdout);
            dump(&output.stderr);
        }
        BuildError::FrontendServerNotFound => {
            println!(
                "cargo::error=the frontend server could not be located in the flutter engine build"
            );
        }
        BuildError::DartNotFound { .. } => {
            println!(
                "cargo::error=the dart binary could not be located in the flutter engine build"
            );
        }
        BuildError::KernelSnapshotBuildFailed(output) => {
            println!("cargo::error=the kernel snapshot build failed");
            dump(&output.stdout);
            dump(&output.stderr);
        }
        BuildError::GenSnapshotNotFound => {
            println!("cargo::error=the gen_snapshot binary could not be located in the flutter engine build");
        }
        BuildError::DartAotBuildFailed(output) => {
            println!("cargo::error=dart AOT build failed");
            dump(&output.stdout);
            dump(&output.stderr);
        }
    }}).ok()
}
