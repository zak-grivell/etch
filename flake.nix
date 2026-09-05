{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix.url = "github:nix-community/fenix";

    naersk.inputs.nixpkgs.follows = "nixpkgs";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self,
    flake-utils,
    naersk,
    nixpkgs,
    fenix,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [fenix.overlays.default];
        };

        rustToolchain = pkgs.fenix.latest.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
        ];

        naersk' = pkgs.callPackage naersk {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        etch = naersk'.buildPackage {
          pname = "etch";
          src = ./.;
        };
      in {
        packages = {
          inherit etch;
          default = etch;
        };

        apps = {
          etch = flake-utils.lib.mkApp {
            drv = etch;
            exePath = "/bin/etch";
          };
          default = self.apps.${system}.etch;
        };

        devShells.default = pkgs.mkShell {
          packages = [
            (pkgs.writeShellScriptBin "etch" ''
              exec cargo run --bin etch -- "$@"
            '')
            pkgs.python3
            pkgs.tree-sitter
            pkgs.nodejs
            pkgs.alejandra
            pkgs.rust-analyzer
            rustToolchain
          ];
        };
      }
    );
}
