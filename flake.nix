# https://notes.srid.ca/rust-nix

{
  description = "A Discord Markov chain bot.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, crate2nix, ... }:
    let
      name = "sentinel";
    in
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          # Imports
          pkgs = import nixpkgs {
            inherit system;
          };

          inherit (import "${crate2nix}/tools.nix" { inherit pkgs; })
            generatedCargoNix;

          # Create the cargo2nix project
          project = pkgs.callPackage
            (generatedCargoNix {
              inherit name;
              src = ./.;
            })
            {
              # Individual crate overrides go here
              # Example: https://github.com/balsoft/simple-osd-daemons/blob/6f85144934c0c1382c7a4d3a2bbb80106776e270/flake.nix#L28-L50
              defaultCrateOverrides = pkgs.defaultCrateOverrides // {
                # The app crate itself is overriden here. Typically we
                # configure non-Rust dependencies (see below) here.
                ${name} = oldAttrs: {
                  inherit buildInputs nativeBuildInputs;
                } // buildEnvVars;
              };
            };

          # Configuration for the non-Rust dependencies
          buildInputs = with pkgs; [ openssl.dev ];
          nativeBuildInputs = with pkgs; [ rustc cargo pkgconfig nixpkgs-fmt ];
          buildEnvVars = {
            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          };
        in
        rec {
          packages.${name} = project.rootCrate.build;

          # `nix build`
          defaultPackage = packages.${name};

          # `nix run`
          apps.${name} = flake-utils.lib.mkApp {
            inherit name;
            drv = packages.${name};
          };
          defaultApp = apps.${name};

          # `nix develop`
          devShell = pkgs.mkShell
            {
              inherit buildInputs nativeBuildInputs;
              RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            } // buildEnvVars;
        }
      );
}
