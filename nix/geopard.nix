{ stdenv
, cargo
, glib
, gtk4
, libadwaita
, pango
, rustPlatform
, rustfmt
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
, gobject-introspection
, appstream-glib
, clippy
}:

stdenv.mkDerivation rec {
  pname = "geopard";
  version = "1.2.0";

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "cairo-rs-0.16.0" = "sha256-Y0qRUliZRuEYvLje2ld75BDgSM7lHOnWITyuI/RoxwI=";
      "gdk4-0.5.0" = "sha256-cRZS8csxpPZm6yxyb6MYiGO7rdw207E4w4uiuJqJoaU=";
      "gio-0.16.0" = "sha256-wENBSDGVUQIa6CK4d5oZ9ih0h1SY1CKWBKVtVcxsXP0=";
      "libadwaita-0.2.0" = "sha256-+ATfy8QIgpoifSCrcqdoub1ust3pEdU3skjOPfIaDQc=";
    };
  };

  src = ../.;

  nativeBuildInputs = [
    openssl
    gettext
    glib # for glib-compile-schemas
    meson
    ninja
    pkg-config
    wrapGAppsHook4
    cmake
    blueprint-compiler
    desktop-file-utils
    appstream-glib
    blueprint-compiler
    rustPlatform.rust.cargo
    rustPlatform.cargoSetupHook
    rustPlatform.rust.rustc
  ];

  buildInputs = [
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
