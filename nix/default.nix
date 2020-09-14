{pkgs ? import <nixpkgs> {}}:
let
  geopard = pkgs.callPackage ./geopard.nix {};
in geopard
