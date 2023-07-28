{ stdenv
, cargo
, rustc
, glib
, gtk4
, libadwaita
, rustPlatform
, openssl
, pkg-config
, lib
, wrapGAppsHook
, meson
, ninja
, gdk-pixbuf
, cmake
, desktop-file-utils
, gettext
, blueprint-compiler
, appstream-glib
, rust-analyzer
, fetchFromGitLab
, appstream
}:

let
  gtk_4_11 = gtk4.overrideAttrs (old: rec {
    version = "4.11.4";
    src = fetchFromGitLab {
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
  wrapGAppsHook_4_11 = wrapGAppsHook.override { gtk3 = gtk_4_11; };

  libadwaita_1_4 = (libadwaita.override { gtk4 = gtk_4_11; }).overrideAttrs (old: rec {
    version = "1.4.alpha";
    src = fetchFromGitLab {
      domain = "gitlab.gnome.org";
      owner = "GNOME";
      repo = "libadwaita";
      rev = version;
      hash = "sha256-UUS5b6diRenpxxmGvVJoc6mVjEVGS9afLd8UKu+CJvI=";
    };
    buildInputs = old.buildInputs ++ [ appstream ];
  });
in

stdenv.mkDerivation {
  pname = "geopard";
  version = "1.4.0";

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ../Cargo.lock;
  };

  src = ../.;

  nativeBuildInputs = [
    openssl
    gettext
    glib # for glib-compile-schemas
    meson
    ninja
    pkg-config
    cmake
    blueprint-compiler
    desktop-file-utils
    appstream-glib
    blueprint-compiler
    cargo
    rustPlatform.cargoSetupHook
    rustc
    wrapGAppsHook_4_11
  ];

  buildInputs = [
    rust-analyzer
    meson
    ninja
    desktop-file-utils
    gdk-pixbuf
    glib
    gtk_4_11
    libadwaita_1_4
    openssl
  ];

  meta = with lib; {
    homepage = "https://github.com/ranfdev/Geopard";
    description = "Colorful, adaptive gemini browser";
    maintainers = with maintainers; [ ranfdev ];
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
  };
}
