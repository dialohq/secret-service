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
    self,
    nixpkgs,
    flake-utils,
    naersk,
    ...
  }:
    {
      nixosModules.default = {
        config,
        pkgs,
        lib,
        ...
      }:
        import ./nix/module.nix {
          inherit config pkgs lib;
          secret-service = self.packages.${pkgs.system}.default;
        };
    }
    // flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        naersk' = pkgs.callPackage naersk {};
      in {
        packages.default = naersk'.buildPackage {
          src = ./.;
          buildInputs = [pkgs.sqlite];
        };
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [cargo rustc clippy rustfmt rust-analyzer sqlite];
        };
      }
    );
}
