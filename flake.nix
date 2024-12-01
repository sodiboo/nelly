{
  inputs = {
    nixpkgs.url = "github:sodiboo/nixpkgs/flutter";

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
            buildInputs = [
              rust-nightly-toolchain
              flutter
              pkgs.libxkbcommon
            ];

            LIBCLANG_PATH = lib.makeLibraryPath [
              pkgs.libclang
            ];

            FLUTTER_ENGINE = "${flutter.engine}";

            FLUTTER_VERSION = "${flutter.version}";
          };
        }
      );
    };
}
