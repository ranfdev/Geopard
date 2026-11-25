#!/usr/bin/env bash

read -p "Do you want to do a clean compilation? [N/y] " answer

if [[ "$answer" == "y" ]]; then
    rm -r _build
fi

export RUST_LOG=debug

meson setup _build -Dprefix="$(pwd)/_build" -Dprofile=development
ninja -C _build install
meson devenv -C _build ./src/geopard
