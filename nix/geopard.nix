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

naersk-lib.buildPackage rec {
  pname = "geopard";
  version = "1.0.1";

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
    rustfmt
    clippy
  ];

  buildInputs = [
    desktop-file-utils
    gdk-pixbuf
    glib
    gtk4
    libadwaita
    openssl
  ];
  checkPhase = ''
  '';
  doCheck = true;
  meta = with lib; {
    homepage = "https://github.com/ranfdev/Geopard";
    description = "Colorful, adaptive gemini browser";
    maintainers = with maintainers; [ ranfdev ];
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
  };
}
