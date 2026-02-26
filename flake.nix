# flake.nix
{
  description = "Rust dev shell with OpenSSL and development tools";
  
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };
  
  outputs = { self, nixpkgs }: let
    pkgs = nixpkgs.legacyPackages."x86_64-linux";
    
    # Common packages for both shells
    commonBuildInputs = with pkgs; [
      cargo
      rustc
      rustfmt
      clippy
      rust-analyzer
    ];
    
    commonNativeBuildInputs = with pkgs; [
      # Remove rustup, rustc, cargo from here - they're already in buildInputs
      openssl
      pkg-config
      postgresql
      mariadb.client
    ];
    
    commonEnv = {
      RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    };
    
    commonShellHook = ''
      export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"
      export OPENSSL_DIR="${pkgs.openssl.dev}"
      export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
      export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
      export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH
    '';
    
  in {
    
    devShells."x86_64-linux" = {
      # Standard secure shell (default)
      default = pkgs.mkShell {
        buildInputs = commonBuildInputs;
        nativeBuildInputs = commonNativeBuildInputs;
        env = commonEnv;
        
        shellHook = commonShellHook + ''
          echo "Running in standard mode"
        '';
      };
      
      # Testing/insecure shell
      testing = pkgs.mkShell {
        buildInputs = commonBuildInputs;
        nativeBuildInputs = commonNativeBuildInputs;
        env = commonEnv;
        
        shellHook = commonShellHook + ''
          export ENABLE_ADMIN_USER="true"
          export ADMIN_USER="testing"
          export ADMIN_PASSWORD="test"
          export SECRET=ejCrnROblT0sRX6OQCLrANXtCxkeyUgG
          echo "Testing environment enabled (INSECURE)"
          echo "ADMIN_USER=$ADMIN_USER"
          echo "ADMIN_PASSWORD=$ADMIN_PASSWORD"
        '';
      };
    };
  };
}
