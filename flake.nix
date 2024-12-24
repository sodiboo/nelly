{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;

      systems = lib.intersectLists lib.systems.flakeExposed lib.platforms.linux;

      forAllSystems = lib.genAttrs systems;
    in
    {
      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt-rfc-style);

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          rust-bin = rust-overlay.lib.mkRustBin { } pkgs;
          rust-nightly-toolchain = rust-bin.selectLatestNightlyWith (
            toolchain:
            toolchain.default.override {
              extensions = [
                "rust-analyzer"
                "rust-src"
              ];
            }
          );

          flutter = pkgs.flutterPackages-source.stable;
        in
        {
          default = pkgs.mkShell {

            nativeBuildInputs = [
              rust-nightly-toolchain
              pkgs.rustPlatform.bindgenHook
              flutter
              (pkgs.writeScriptBin "pub" ''
                exec "${flutter}/bin/flutter" --local-engine=host_release --local-engine-host=host_release pub "$@"
              '')
              pkgs.cargo-expand
              pkgs.rust-cbindgen
              # flutter_rust_bridge_codegen
            ];

            buildInputs = [
              pkgs.libxkbcommon
            ];

            # needed for ffi to work
            RUSTFLAGS = "-Z export-executable-symbols -C link-arg=-Wl,--export-dynamic";

            FLUTTER_ENGINE = "${flutter.engine}";

            FLUTTER_VERSION = "${flutter.version}";
          };
        }
      );
    };
}
