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
  version = "1.3.0";

  cargoDeps = rustPlatform.importCargoLock {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "cairo-rs-0.16.0" = "sha256-cqWL8PKmakObkHZWFtfLQtMKKBfSPQNvqLFkh0JaBGY=";
      "gdk4-0.5.0" = "sha256-ICUZ8Y/4D4iAzZosusZL2sB/EXGkWarWk5ZIW84crg4=";
      "gio-0.16.0" = "sha256-34RcAmMozLAqrGPalGvyBdtTwurcoYs2VKtNenDOK3E=";
      "libadwaita-0.2.0" = "sha256-47GmghKbdaK+5F7GeeMEczDqMP3X+daHY2UEhmC5Qtc=";
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
