{
  description = "A gemini browser";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    naersk.url = "github:nix-community/naersk";
  };
  outputs = { self, nixpkgs, naersk }:
    with import nixpkgs { system = "x86_64-linux"; };
    rec {
      packages.x86_64-linux.blueprint-compiler = callPackage ./nix/blueprint-compiler.nix { };
      packages.x86_64-linux.geopard = callPackage ./nix/geopard.nix {
        naersk-lib = naersk.lib.x86_64-linux;
        blueprint-compiler = packages.x86_64-linux.blueprint-compiler;
      };
      checks.x86_64-linux.default = stdenv.mkDerivation {
        name = "geopard-checks";
        src = ./.;
        cargoDeps = packages.x86_64-linux.geopard.cargoDeps;
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
	nativeBuildInputs = with nixpkgs.legacyPackages.x86_64-linux; [openssl pkg-config glib gtk4 libadwaita clippy];
	buildInputs = with nixpkgs.legacyPackages.x86_64-linux; [
	  clippy
	  rustPlatform.rust.cargo
	  rustPlatform.rust.rustc
	  rustPlatform.cargoSetupHook
	  rustfmt
	];
      };
      packages.x86_64-linux.build-flatpak = callPackage ./nix/build-flatpak.nix { };
      packages.x86_64-linux.default = packages.x86_64-linux.geopard;
    };
}
