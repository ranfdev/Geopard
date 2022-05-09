{
  description = "A gemini browser";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
  };
  outputs = { self, nixpkgs }:
  with import nixpkgs { system = "x86_64-linux"; };
  {
    # geopard = callPackage ./nix/geopard.nix {};
    build-flatpak = callPackage ./nix/build-flatpak.nix {};
  };
}
