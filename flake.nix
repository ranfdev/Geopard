{
  description = "A gemini browser";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    naersk.url = "github:nix-community/naersk";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, naersk, flake-utils }:
    flake-utils.lib.eachSystem
      (with flake-utils.lib.system; [ x86_64-linux aarch64-linux ])
      (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        rec {
          packages.blueprint-compiler = pkgs.callPackage ./nix/blueprint-compiler.nix { };
          packages.geopard = pkgs.callPackage ./nix/geopard.nix {
            naersk-lib = naersk.lib.${system};
            blueprint-compiler = packages.blueprint-compiler;
          };
          checks.default = pkgs.stdenv.mkDerivation {
            name = "geopard-checks";
            src = ./.;
            cargoDeps = packages.geopard.cargoDeps;
            configurePhase = ''
              # These are replaced during the real build by meson
              sed \
                -e 's/str =.*;/str = "";/g' \
                -e 's/i32 =.*;/i32 = 0;/g' \
                src/build_config.rs.in \
                > src/build_config.rs
            '';
            checkPhase = ''
              cargo fmt --check;
              cargo clippy -- -D warnings
          	'';
            doCheck = true;
            installPhase = ''echo "" > $out'';
            nativeBuildInputs = with pkgs; [ openssl pkg-config glib gtk4 libadwaita clippy ];
            buildInputs = with pkgs; [
              clippy
              rustPlatform.rust.cargo
              rustPlatform.rust.rustc
              rustPlatform.cargoSetupHook
              rustfmt
            ];
          };
          packages.build-flatpak = pkgs.callPackage ./nix/build-flatpak.nix { };
          packages.default = packages.geopard;
        }
      );
}
