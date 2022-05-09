{flatpak-builder, flatpak, stdenv, jq, writeShellApplication, writeTextFile}: 
let 
f = writeTextFile {
    name = "build-terminal.sh";
    text = ''
    meson dist --include-subprojects --no-tests;
    '';
};
in
writeShellApplication {
  name = "build-flatpak";
  runtimeInputs = [flatpak-builder flatpak jq];
  text = ''
    # Read data from manifest
    folder=$1
    manifest=$2;
    name=$(< "$manifest" jq -r '.["modules"] | last | .["name"]');
    appid=$(< "$manifest" jq -r '.["app-id"]');
    runtime=$(< "$manifest" jq -r '"runtime/" + .["runtime"] + "/x86_64/" + .["runtime-version"]');

    # Install flathub repo
    flatpak --verbose remote-add --system --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo;
    flatpak install --system -y "$runtime";

    # Prepare build folder
    if [ -n "$(ls -A "$folder")" ]; then
      echo error: directory "$folder" is not empty;
      exit 1;
    fi
    rm -rf "$folder";
    mkdir "$folder";

    # Generate dist archive and the release manifest
    flatpak-builder "$folder"/build "$manifest" --build-only --stop-at="$name" --keep-build-dirs --force-clean;
    < ${f} flatpak-builder "$folder"/build "$manifest" --build-shell="$name" --keep-build-dirs --state-dir="$folder"/state;
    < "$manifest" jq '(.["modules"] | last | .["sources"] | last) |= {type: "archive", path: "archive.tar.xz"}' > build-flatpak-auto/manifest-archive.json;

    # Build the app from the dist archive, using the corrected manifest
    cd "$folder"/;
    mv state/build/"$name"/_flatpak_build/meson-dist/*.tar.xz archive.tar.xz;
    flatpak-builder build --repo repo manifest-archive.json --force-clean;
    flatpak build-bundle ./repo "$appid".flatpak "$appid"
    
    # Put all the artifacts in a single folder
    mkdir artifacts;
    mv "$appid".flatpak artifacts/;
    mv manifest-archive.json artifacts/"$appid".json;
    mv archive.tar.xz artifacts/"$appid".tar.xz;
  '';
}
