{
  inputs = {
    utils.url = "github:numtide/flake-utils";

    naersk.url = "github:nmattia/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, utils, naersk, ... }:
  let
      pname = "sentinel";
  in {
    overlay = final: prev: {
      "${pname}" = naersk.lib."${final.system}".buildPackage {
        inherit pname;
        root = ./.;
      };
    };
  } // (
    utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ self.overlay ];
      };
    in rec {
      packages."${pname}" = pkgs."${pname}";
      defaultPackage = packages."${pname}";

      apps."${pname}" = utils.lib.mkApp {
        drv = packages."${pname}";
      };
      defaultApp = apps."${pname}";

      devShell = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [ rustc cargo ];
      };
    })
  );
}
