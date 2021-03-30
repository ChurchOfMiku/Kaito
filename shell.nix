{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    pkg-config
    lua5_4
    openssl
    sqlite
    libopus
    libwebp
    ffmpeg
    youtube-dl
    (graphicsmagick.overrideAttrs (
      oldAttrs: rec {
        with_windows_font_dir = "${pkgs.corefonts}/share/fonts/truetype";
        configureFlags = oldAttrs.configureFlags ++ [
          "--with-librsvg"
        ];
        buildInputs = oldAttrs.buildInputs ++ [
          pkgs.gnome3.librsvg
        ];
      }
    ))
  ];

  shellHook = ''
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang}/lib";
  '';
}
