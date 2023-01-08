{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "flake-utils";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    flake-compat.url = "github:edolstra/flake-compat";
    flake-compat.flake = false;
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    naersk,
    ...
  }: let
    name = "temps";
  in
    flake-utils.lib.eachDefaultSystem
    (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [rust-overlay.overlays.default];
        };

        rust = pkgs.rust-bin.stable.latest.default;

        naersk-lib = naersk.lib.${system}.override {
          cargo = rust;
          rustc = rust;
        };

        temps = naersk-lib.buildPackage {
          pname = name;
          root = ./.;
        };
      in rec {
        packages.${name} = temps;
        packages.default = packages.${name};

        apps.${name} = flake-utils.lib.mkApp {
          inherit name;
          drv = packages.${name};
        };
        apps.default = apps.${name};

        devShell = pkgs.mkShell {
          buildInputs = [
            rust
            pkgs.rust-analyzer
          ];
        };
      }
    );
}
