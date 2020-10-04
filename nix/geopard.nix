{ stdenv
, cargo
, glib
, gtk3
, pango
, rust
, rustc
, rustPlatform
, openssl
, pkg-config
, lib
}:

rustPlatform.buildRustPackage rec {
  pname = "geopard";
  version = "0.1.0";

  src = lib.cleanSource ../.;
  cargoSha256 = "0rnfacbf20wq638gfndchynmap28b21jwa0wdxz2b2wgqi7nahcv";

  nativeBuildInputs = [
    cargo
    rustc
    openssl
    pkg-config
  ];

  buildInputs = [
    glib
    gtk3
    pango
    openssl
  ];

  doCheck = false;

  meta = with stdenv.lib; {
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
