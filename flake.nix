{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
      in {
        nixosModules.default = import ./nix/module.nix;
        packages = {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "secretservice";
            version = "0.1";
            cargoLock.lockFile = ./Cargo.lock;
            src = pkgs.lib.cleanSource ./.;
          };
        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [cargo rustc clippy rustfmt rust-analyzer];
        };
      }
    );
}
