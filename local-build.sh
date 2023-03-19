#!/usr/bin/bash

read -p "Do you want to do a clean compilation? [N/y] " answer

if [[ "$answer" == "y" ]]; then
    rm -r _builddir
fi

meson setup _builddir
meson configure _builddir -Dprofile=development

meson compile -C _builddir --verbose
meson devenv -C _builddir ./src/geopard
