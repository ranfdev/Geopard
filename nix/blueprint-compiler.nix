{ python3
, stdenv
, fetchFromGitLab
, gobject-introspection
, lib
, meson
, ninja
}:
stdenv.mkDerivation rec {
  pname = "blueprint-compiler";
  version = "0.pre+date=2022-05-17";

  src = fetchFromGitLab {
    domain = "gitlab.gnome.org";
    owner = "jwestman";
    repo = pname;
    rev = "06278a64248cec92bb95a958eadfba453943c061";
    sha256 = "sha256-ukvqWvYl4NohKEm6W50eRZ9fmKD4vhH0yzgrCf5HVts=";
  };

  # Requires pythonfuzz, which I've found difficult to package
  doCheck = false;

  nativeBuildInputs = [
    meson
    ninja
    python3.pkgs.wrapPython
  ];
  propagatedBuildInputs = [
    gobject-introspection
    python3
  ];

  postFixup = ''
    wrapPythonPrograms
  '';

  meta = with lib; {
    description = "A markup language for GTK user interface files";
    homepage = "https://gitlab.gnome.org/jwestman/blueprint-compiler";
    license = licenses.lgpl3Plus;
    maintainers = [ maintainers.ranfdev ];
    platforms = platforms.all;
  };
}