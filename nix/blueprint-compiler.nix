{ python3
, python3Packages
, stdenv
, fetchgit
, meson
, ninja
, gtk4
, glib
, gobject-introspection
, libadwaita
, wrapGAppsHook4
}:
python3Packages.buildPythonApplication {
  pname = "blueprint-compiler";
  version = "0.1.0";

  src = fetchgit {
    url = "https://gitlab.gnome.org/jwestman/blueprint-compiler.git";
    rev = "db2e662d3173f6348b86d2350d4e2f0340ec939c";
    sha256 = "sha256-aFx+aUDpc9wNgG/NUyIDgSZnURFShyTH1HhDjElTApY=";
  };

  preBuild = ''
    cat >setup.py <<'EOF'
    from setuptools import setup
    setup(
      name='blueprint-compiler',
      version='0.1.0',
      scripts=[
        "blueprint-compiler.py",
      ],
    )
    EOF
  '';
  doCheck = false;
  postInstall = ''
    mv -v $out/bin/blueprint-compiler.py $out/bin/blueprint-compiler
  '';
  nativeBuildInputs = [
    wrapGAppsHook4
  ];
  buildInputs = [
    gtk4
    glib
    gobject-introspection
    python3Packages.pygobject3
    libadwaita
    wrapGAppsHook4
  ];
  propagatedBuildInputs = [
    gobject-introspection
  ];
}
