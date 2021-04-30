{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust
    cargo
    rustc
    rustfmt
    clang
    # Deps
    pkg-config
    stdenv.cc.libc
    lua5_4
    openssl
    sqlite
    libopus
    libwebp
    ffmpeg
    youtube-dl
    graphicsmagick
  ];

  shellHook = ''
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang}/lib"
    export BINDGEN_EXTRA_CLANG_ARGS="-I${pkgs.stdenv.cc.libc}/include"
  '';
}
