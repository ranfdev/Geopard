{ stdenv
, cargo
, glib
, gtk4
, libadwaita
, pango
, rustc
, rustPlatform
, openssl
, pkg-config
, lib
, wrapGAppsHook
, meson
}:

rustPlatform.buildRustPackage rec {
  pname = "geopard";
  version = "1.0.0-alpha";

  src = lib.cleanSource ../.;
  cargoSha256 = "sha256-X2gVBKl37+FnDlzAQnLN6I99pEliytW/pMgMY6tJPd4=";

  nativeBuildInputs = [
    cargo
    rustc
    openssl
    pkg-config
  ];

  buildInputs = [
    glib
    gtk4
    libadwaita
    pango
    openssl
    wrapGAppsHook
    meson
  ];

  doCheck = false;

  meta = with lib; {
    homepage = "https://git.ranfdev.com/Geopard";
    description = "Browse the geminiverse";
    longDescription = ''
      Geopard is a gemini browser. It's colored and fast.
    '';
    maintainers = [ "ranfdev@gmail.com" ];
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
  };
}
