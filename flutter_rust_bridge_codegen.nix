{
  lib,
  flutter_rust_bridge,
  rustPlatform,
  cargo-expand,
}:
rustPlatform.buildRustPackage {
  pname = "flutter_rust_bridge_codegen";
  version = "2.6.0";

  src = flutter_rust_bridge;

  cargoLock = {
    allowBuiltinFetchGit = true;
    lockFile = "${flutter_rust_bridge}/Cargo.lock";
  };

  doCheck = false;

  # needed to get tests running
  nativeBuildInputs = [ cargo-expand ];

  # needed to run text (see https://github.com/fzyzcjy/flutter_rust_bridge/blob/ae970bfafdf80b9eb283a2167b972fb2e6504511/frb_codegen/src/library/utils/logs.rs#L43)
  logLevel = "debug";
  checkFlags = [
    # Disabled because these tests need a different version of anyhow than the package itself
    "--skip=tests::test_execute_generate_on_frb_example_dart_minimal"
    "--skip=tests::test_execute_generate_on_frb_example_pure_dart"
    # Disabled because of modifications to use local engine breaks these tests
    "--skip-tests::test_parse_single_rust_input"
    "--skip-tests::test_parse_wildcard_rust_input"
  ];

  meta = {
    mainProgram = "flutter_rust_bridge_codegen";
    description = "Flutter/Dart <-> Rust binding generator, feature-rich, but seamless and simple";
    homepage = "https://fzyzcjy.github.io/flutter_rust_bridge";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ sodiboo ];
  };
}
