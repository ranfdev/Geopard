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
, naersk-lib
, clippy
}:

stdenv.mkDerivation rec {
  pname = "geopard";
  version = "1.1.1";

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "cairo-rs-0.16.0" = "sha256-lCEOtFsuGKnYctVL5UzeFytIKIEIQU/DLr3mj+I8OEE=";
      "gdk4-0.5.0" = "sha256-j8RfllrPwcG/zxSMew/x435wYrR++z4csJ1p3wdNti0=";
      "gio-0.16.0" = "sha256-NKj1Yll7OIVQ5bi2H8EgRZyREl3mJ5ghgJ6T6xmcqOg=";
      "libadwaita-0.2.0" = "sha256-yg/Z2cK23R3NgJAziVOxjnPPmtTbBM6SNZhM9UFn+Gw=";
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
