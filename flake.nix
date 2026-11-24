{
  description = "simplex-client: a SimpleX Chat protocol client in Rust (native and wasm)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rust
            pkgs.cargo-deny
            pkgs.cargo-nextest
            pkgs.wasm-bindgen-cli
            pkgs.binaryen
            pkgs.wasm-pack
            pkgs.nodejs
            pkgs.clang
            pkgs.lld
            pkgs.pkg-config
          ];
        };
      }
    );
}
