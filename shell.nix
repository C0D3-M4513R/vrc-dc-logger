{
  pkgs ? import <nixpkgs> {
    localSystem = "x86_64-linux";
    crossSystem = "aarch64-linux";

  },
}:
pkgs.callPackage (
  {
    mkShell,
    musl
  }:
  mkShell {
    # By default this provides gcc, ar, ld, and some other bare minimum tools
    buildInputs = [
     musl
   ];
  }
) { }
