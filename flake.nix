# Based on: https://github.com/akirak/flake-templates/blob/master/rust/flake.nix
# https://akirak.github.io/flake-templates/
{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # systems.url = "github:nix-systems/default";

    treefmt-nix.url = "github:numtide/treefmt-nix";

    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
  };

  # Add settings for your binary cache.
  # nixConfig = {
  #   extra-substituters = [
  #   ];
  #   extra-trusted-public-keys = [
  #   ];
  # };

  outputs =
    inputs@{ nixpkgs, flake-parts, ... }:
    let
      # For details on these options, See
      # https://github.com/oxalica/rust-overlay?tab=readme-ov-file#cheat-sheet-common-usage-of-rust-bin

      # NOTE: Specify the Rust toolchain below

      # Channel of the Rust toolchain (stable or beta).
      rustChannel = "stable";
      # Version (latest or specific date/semantic version)
      rustVersion = "latest";
      # Profile (default or minimal)
      rustProfile = "default";
    in
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = nixpkgs.lib.systems.flakeExposed;

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      perSystem =
        {
          config,
          system,
          pkgs,
          lib,
          craneLib,
          commonArgs,
          ...
        }:
        {
          _module.args = {
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ inputs.rust-overlay.overlays.default ];
            };
            craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
              pkgs: pkgs.rust-bin.${rustChannel}.${rustVersion}.${rustProfile}
            );
            commonArgs = {
              # Depending on your code base, you may have to customize the
              # source filtering to include non-standard files during the build.
              # See
              # https://crane.dev/source-filtering.html?highlight=source#source-filtering
              src = craneLib.cleanCargoSource (craneLib.path ./.);

              # NOTE: Specify build inputs below

              nativeBuildInputs = with pkgs; [
                pkg-config
              ];

              # buildInputs = with pkgs; [
              #   glib
              #   dbus
              # ];

              devShellBuildInputs = with pkgs; [
                rust-analyzer-unwrapped
                bacon
              ];
            };
          };

          # Build the executable package.
          packages.default = craneLib.buildPackage (
            commonArgs
            // {
              cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            }
          );

          devShells.default = craneLib.devShell {
            packages =
              (commonArgs.nativeBuildInputs or [ ])
              ++ (commonArgs.buildInputs or [ ])
              ++ (commonArgs.devShellBuildInputs or [ ]);

            RUST_SRC_PATH = "${
              pkgs.rust-bin.${rustChannel}.${rustVersion}.rust-src
            }/lib/rustlib/src/rust/library";
          };

          treefmt = {
            projectRootFile = "Cargo.toml";
            programs = {
              actionlint.enable = true;
              nixfmt.enable = true;
              rustfmt.enable = true;
            };
          };
        };
    };
}
