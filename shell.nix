{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    pkg-config
    lua5_4
    openssl
    sqlite
    libwebp
    (graphicsmagick.overrideAttrs (
      oldAttrs: rec {
        with_windows_font_dir = "${pkgs.corefonts}/share/fonts/truetype";
      }
    ))
  ];

  shellHook = ''
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang}/lib";
  '';
}
