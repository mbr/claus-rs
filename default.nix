{ pkgs ? import <nixpkgs> {} }:
let
  cargoToml = pkgs.lib.importTOML ./Cargo.toml;
  fenix = import (fetchGit {
    url = "https://github.com/nix-community/fenix";
    rev = "c3c27e603b0d9b5aac8a16236586696338856fbb";
  }) { };
  toolchain = fenix.stable.toolchain;
  platform = (pkgs.makeRustPlatform {
    cargo = toolchain;
    rustc = toolchain;
  });
in
platform.buildRustPackage rec {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = pkgs.lib.cleanSource ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };
}
