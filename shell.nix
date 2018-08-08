with import <nixpkgs> {};

stdenv.mkDerivation {
  name = "rust-discord-env";

  buildInputs = [
    rustChannels.stable.rust
    #rustChannels.nightly.rust
    
    pkgconfig

    openssl
    libsodium
    libopus

    ffmpeg
    espeak
  ];

  DISCORD_TOKEN = lib.readFile ./token;
  RUST_LOG = "info";
}
