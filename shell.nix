{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustup
    openssl
    pkg-config
    postgresql 
    mariadb.client  
  ];

  shellHook = ''
    export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"
    export OPENSSL_DIR="${pkgs.openssl.dev}"
    export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
    # rustup default nightly
    echo "Enable testing environment? (insecure) [y/N]"
    read -r -n 1 reply
    if [[ "$reply" =~ ^[Yy]$ ]]; then
      export ENABLE_ADMIN_USER="true"
      export ADMIN_USER="testing"
      export ADMIN_PASSWORD="test"
      export SECRET=ejCrnROblT0sRX6OQCLrANXtCxkeyUgG

      echo "ADMIN_USER=$ADMIN_USER"
      echo "ADMIN_PASSWORD=$ADMIN_PASSWORD"
    else
      echo ""
      echo "Running in standard mode"
    fi
  '';
}