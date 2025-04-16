# flake.nix
{
  description = "Rust dev shell with OpenSSL";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      devShells.${system}.default = pkgs.mkShell {
        
        nativeBuildInputs = with pkgs; [ pkg-config ];
        buildInputs = with pkgs; [ openssl];

        shellHook = ''
          export OPENSSL_LIB_DIR=${pkgs.openssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${pkgs.openssl.dev}/include
          export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH
        '';
      };
    };
}
