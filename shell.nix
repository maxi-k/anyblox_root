{ pkgs ? import (fetchTarball channel:nixos-24.11) {} }:

with pkgs;

mkShell {
  buildInputs = [
    rustc
    rust-analyzer
    cargo
    openssl
    pkg-config
  ];
}
