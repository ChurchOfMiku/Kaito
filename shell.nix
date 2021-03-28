{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    pkg-config
    lua5_4
    openssl
    sqlite
    libwebp
    graphicsmagick
  ];

  shellHook = ''
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang}/lib";
  '';
}
