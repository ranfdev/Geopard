{pkgs ? import <nixos-unstable> {}}:
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
    rustc
    rustfmt
    wrapGAppsHook
    dbus
    gdk-pixbuf
    glib
    gtk3
    openssl
    pkg-config
    rust-analyzer
  ];
}
