{
  description = "A gemini browser";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };
  outputs = { self, nixpkgs, microvm }:
    with import nixpkgs { system = "x86_64-linux"; };
    rec {
      packages.x86_64-linux.blueprint-compiler = callPackage ./nix/blueprint-compiler.nix { };
      packages.x86_64-linux.geopard = callPackage ./nix/geopard.nix { blueprint-compiler = packages.x86_64-linux.blueprint-compiler; };
      packages.x86_64-linux.build-flatpak = callPackage ./nix/build-flatpak.nix { };
      packages.x86_64-linux.default = packages.x86_64-linux.geopard;
    };
}
