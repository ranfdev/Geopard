#!/usr/bin/env bash
set -e

# Read data from manifest
folder=$1
manifest=$2;
name=$(< "$manifest" jq -r '.["modules"] | last | .["name"]');
appid=$(< "$manifest" jq -r '.["app-id"]');
runtime=$(< "$manifest" jq -r '"runtime/" + .["runtime"] + "/x86_64/" + .["runtime-version"]');
sdk=$(< "$manifest" jq -r '.["sdk"] + "/x86_64/" + .["runtime-version"]');
sdk_extensions=$(< "$manifest" jq -r '.["sdk-extensions"] | .[]');

# Install flathub repo
flatpak --verbose remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo;
flatpak install --user -y "$runtime";
flatpak install --user -y "$sdk";
for ext in $sdk_extensions
do
  echo "$ext";
  flatpak install --user -y "$ext";
done


# Prepare build folder
if [ -n "$(ls -A "$folder")" ]; then
  echo error: directory "$folder" is not empty;
  exit 1;
fi
rm -rf "$folder";
mkdir "$folder";

# Generate dist archive and the release manifest
flatpak-builder --user "$folder"/build "$manifest" --build-only --stop-at="$name" --keep-build-dirs --force-clean;
echo "meson dist --include-subprojects --no-tests" | flatpak-builder --user "$folder"/build "$manifest" --build-shell="$name" --keep-build-dirs --state-dir="$folder"/state;
< "$manifest" jq '(.["modules"] | last | .["sources"] | last) |= {type: "archive", path: "archive.tar.xz"}' > build-flatpak-auto/manifest-archive.json;

# Build the app from the dist archive, using the corrected manifest
cd "$folder"/;
mv state/build/"$name"/_flatpak_build/meson-dist/*.tar.xz archive.tar.xz;
flatpak-builder --user build --repo repo manifest-archive.json --force-clean;
flatpak build-bundle ./repo "$appid".flatpak "$appid"

# Put all the artifacts in a single folder
mkdir artifacts;
mv "$appid".flatpak artifacts/;
mv manifest-archive.json artifacts/"$appid".json;
mv archive.tar.xz artifacts/"$appid".tar.xz;
