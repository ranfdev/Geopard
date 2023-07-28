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
, wrapGAppsHook4
, meson
, ninja
, gdk-pixbuf
, cmake
, desktop-file-utils
, gettext
, blueprint-compiler
, appstream-glib
, rust-analyzer
}:

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
    wrapGAppsHook4
  ];

  buildInputs = [
    rust-analyzer
    meson
    ninja
    desktop-file-utils
    gdk-pixbuf
    glib
    gtk4
    libadwaita
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
