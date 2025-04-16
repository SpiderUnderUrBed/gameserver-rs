{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # rustc
    # cargo
   # rust-analyzer
    # rust-src
    openssl
    pkg-config
  ];

  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
  OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

 # TMPDIR = "${builtins.getEnv "HOME"}/tmp";
}
