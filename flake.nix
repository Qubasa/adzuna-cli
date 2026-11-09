{
  description = "adzuna: command-line client for the Adzuna job search API";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1";
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, ... }@inputs:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [ self.overlays.default ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        rustToolchain =
          with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable;
            [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
            ]
          );
      };

      packages = forEachSupportedSystem (
        { pkgs }:
        rec {
          adzuna-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "adzuna-cli";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            meta.mainProgram = "adzuna";
          };
          default = adzuna-cli;
        }
      );

      checks = forEachSupportedSystem (
        { pkgs }:
        let
          package = self.packages.${pkgs.stdenv.hostPlatform.system}.adzuna-cli;
        in
        {
          inherit package;
          clippy = package.overrideAttrs (old: {
            pname = "adzuna-cli-clippy";
            nativeBuildInputs = old.nativeBuildInputs ++ [ pkgs.clippy ];
            buildPhase = "cargo clippy --offline --all-targets --all-features -- -D warnings";
            doCheck = false;
            installPhase = "touch $out";
          });
          fmt = pkgs.runCommand "adzuna-cli-fmt" { nativeBuildInputs = [ pkgs.rustToolchain ]; } ''
            cd ${self}
            cargo fmt --check
            touch $out
          '';
        }
      );

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              cargo-deny
              rust-analyzer
            ];
            env.RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
          };
        }
      );

      formatter = forEachSupportedSystem ({ pkgs }: pkgs.nixfmt-tree);
    };
}
