{pkgs ? import <nixpkgs-unstable> {}}:
pkgs.mkShell {
  buildInputs = with pkgs; [
    cargo
    clippy
    desktop-file-utils
    gettext
    glib # for glib-compile-schemas
    meson
    ninja
    python3
    gtk4
    libadwaita
    rustc
    rustfmt
    wrapGAppsHook
    dbus
    gdk-pixbuf
    glib
    openssl
    pkg-config
    rust-analyzer
  ];
}
