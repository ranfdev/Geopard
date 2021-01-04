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
  version = "1.0.0-alpha";

  src = lib.cleanSource ../.;
  cargoSha256 = "0b77w95bj6avnxgs5ia93hhq3jr9cmbpa5zw8i37s688633il15x";

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
