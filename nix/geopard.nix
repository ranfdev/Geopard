{ stdenv
, cargo
, glib
, gtk3
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
  cargoSha256 = "1sq16n8hbvl9yy95z381yd8d1lay0vmcappwx9xsi8srrmm0cd3s";

  nativeBuildInputs = [
    cargo
    rustc
    openssl
    pkg-config
  ];

  buildInputs = [
    glib
    gtk3
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
