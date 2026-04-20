{
  description = "Sickgnal – end-to-end encrypted messaging client";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Runtime libraries needed by Slint/Winit (loaded via dlopen)
        runtimeLibs = with pkgs; [
          wayland
          libxkbcommon
          fontconfig
          libGL
          libGLU
          libx11
          libxcursor
          libxrandr
          libxi
          libxcb
          vulkan-loader
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Build tools
            rustc
            cargo
            pkg-config
            openssl

            # SQLite / SQLCipher
            sqlite
            sqlcipher

            # Fonts
            fontconfig
            dejavu_fonts

            # Runtime libs (for linking / dlopen)
          ] ++ runtimeLibs;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";

          # Ensure fontconfig finds system fonts + DejaVu
          FONTCONFIG_FILE = pkgs.makeFontsConf {
            fontDirectories = with pkgs; [
              dejavu_fonts
              noto-fonts
              liberation_ttf
            ];
          };
        };
      }
    );
}
