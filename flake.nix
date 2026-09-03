# flake.nix
{
  description = "Rust dev shell with OpenSSL and development tools";
  
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  
  outputs = { self, nixpkgs, rust-overlay }: let
    pkgs = import nixpkgs {
      system = "x86_64-linux";
      overlays = [ rust-overlay.overlays.default ];
    };
    rustToolchain = pkgs.rust-bin.fromRustupToolchainFile
      ./rust-toolchain.toml;
      
    # Common packages for both shells
    commonBuildInputs = with pkgs; [
      rustToolchain
      cargo
      rustc
      rustfmt
      bubblewrap
      protobuf
      clippy
      rust-analyzer
      ripgrep
      act
      playwright-driver.browsers
    ];
    
    commonNativeBuildInputs = with pkgs; [
      openssl
      pkg-config
      postgresql
      mariadb.client
    ];
    
    commonEnv = {
      RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
      PROTOC = "${pkgs.protobuf}/bin/protoc";
      PROTOC_INCLUDE = "${pkgs.protobuf}/include";
      PROTOBUF_LOCATION = "${pkgs.protobuf}";
    };
    
    commonShellHook = ''
      export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"
      export OPENSSL_DIR="${pkgs.openssl.dev}"
      export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
      export OPENSSL_INCLUDE_DIR="${pkgs.openssl.dev}/include"
      export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH

      # export PROTOC="${pkgs.protobuf}/bin/protoc"
      # export PROTOC_INCLUDE="${pkgs.protobuf}/include"
    '';
    
  in {
    # export PLAYWRIGHT_LAUNCH_OPTIONS_EXECUTABLE_PATH = "${pkgs-playwright.playwright.browsers}/chromium-${chromium-rev}/chrome-linux/chrome"
    
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
          # export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
          export PLAYWRIGHT_BROWSERS_PATH=${pkgs.playwright-driver.browsers}
          export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
          export PLAYWRIGHT_LAUNCH_OPTIONS_EXECUTABLE_PATH="${pkgs.playwright-driver.browsers}/chromium-${
            (builtins.head (builtins.filter (x: x.name == "chromium")
              (builtins.fromJSON (builtins.readFile "${pkgs.playwright-driver}/browsers.json")).browsers
            )).revision
          }/chrome-linux/chrome"
          export TMPDIR=/var/tmp

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
