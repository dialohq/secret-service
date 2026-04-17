{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    flake-utils,
    naersk,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        naersk' = pkgs.callPackage naersk {};
      in {
        nixosModules.default = import ./nix/module.nix;
        packages = {
          default = naersk'.buildPackage {
            src = ./.;
            buildInputs = [pkgs.sqlite];
          };
        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [cargo rustc clippy rustfmt rust-analyzer sqlite];
        };
      }
    );
}
