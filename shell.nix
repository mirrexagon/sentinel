let
  pkgs = import <nixpkgs> {};
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    rls
    cargo-edit
  ];

  RUST_LOG = "debug";
}
