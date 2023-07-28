{
  description = "A gemini browser";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem
      (with flake-utils.lib.system; [ x86_64-linux aarch64-linux ])
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          gtk_4_11 = pkgs.gtk4.overrideAttrs (old: rec {
            version = "4.11.4";
            src = pkgs.fetchFromGitLab {
              domain = "gitlab.gnome.org";
              owner = "GNOME";
              repo = "gtk";
              rev = version;
              hash = "sha256-YobWcLJm8owjrz6c6aPMCrVZqYDvNpjIt5Zea2CtAZY=";
            };
            postPatch = old.postPatch + ''
              patchShebangs build-aux/meson/gen-visibility-macros.py
            '';
          });
          wrapGAppsHook_4_11 = pkgs.wrapGAppsHook.override { gtk3 = gtk_4_11; };
          libadwaita_1_4 = (pkgs.libadwaita.override { gtk4 = gtk_4_11; }).overrideAttrs (old: rec {
            version = "1.4.alpha";
            src = pkgs.fetchFromGitLab {
              domain = "gitlab.gnome.org";
              owner = "GNOME";
              repo = "libadwaita";
              rev = version;
              hash = "sha256-UUS5b6diRenpxxmGvVJoc6mVjEVGS9afLd8UKu+CJvI=";
            };
            buildInputs = old.buildInputs ++ [ pkgs.appstream ];
          });
        in
        rec {
          packages.geopard = pkgs.callPackage ./nix/geopard.nix {
            gtk4 = gtk_4_11;
            wrapGAppsHook4 = wrapGAppsHook_4_11;
            libadwaita = libadwaita_1_4;
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
            nativeBuildInputs = with pkgs; [ openssl pkg-config glib gtk_4_11 libadwaita_1_4 clippy ];
            buildInputs = with pkgs; [
              clippy
              cargo
              rustc
              rustPlatform.cargoSetupHook
              rustfmt
            ];
          };
          packages.build-flatpak = pkgs.callPackage ./nix/build-flatpak.nix { };
          packages.default = packages.geopard;
        }
      );
}
