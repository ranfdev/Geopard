{flatpak-builder, flatpak, stdenv, jq, writeShellApplication}: 
writeShellApplication {
  name = "build-flatpak";
  runtimeInputs = [flatpak-builder flatpak jq];
  text = builtins.readFile ../build-aux/build-flatpak.sh;
}
