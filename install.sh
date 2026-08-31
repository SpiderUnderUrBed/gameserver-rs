#!/usr/bin/env bash
# note to developers working on this, no need to have the postgres db script ran within here, as only json is avalible atm
set -e

REPO_URL="https://github.com/SpiderUnderUrBed/gameserver-rs"
MAIN_SERVICE_NAME="gameserver-rs"
NODE_SERVICE_NAME="gameserver-rs-node"
MAIN_BINARY_NAME="$MAIN_SERVICE_NAME"
NODE_BINARY_NAME="$NODE_SERVICE_NAME"
LOCAL_RUST="$PWD/.rust"
REQUIRED_RUST_VERSION="1.88.0"

echo "What would you like to do?"
echo "  1) Install"
echo "  2) Update"
read -rp "Selection [1]: " MODE_SELECTION
MODE_SELECTION="${MODE_SELECTION:-1}"

read -rp "Project build flags (passed to cargo build, leave blank for none): " CARGO_BUILD_FLAGS
CARGO_FEATURE_ARGS=()
[ -n "$CARGO_BUILD_FLAGS" ] && read -ra CARGO_FEATURE_ARGS <<< "$CARGO_BUILD_FLAGS"

ensure_cargo() {
    if [ -d "$LOCAL_RUST" ]; then
        export PATH="$LOCAL_RUST/bin:$PATH"
    fi
    if ! command -v cargo &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
            sh -s -- -y --no-modify-path --default-toolchain "$REQUIRED_RUST_VERSION" --profile minimal
        mv "$HOME/.cargo" "$LOCAL_RUST"
        export PATH="$LOCAL_RUST/bin:$PATH"
    fi
    command -v cargo >/dev/null 2>&1 || { echo "Cargo installation failed"; exit 1; }
}

update_unit() {
    local name="$1"
    local subdir="$2"
    local unit_file="/etc/systemd/system/$name.service"

    if [ ! -f "$unit_file" ]; then
        echo "Unit file not found for $name, skipping"
        return
    fi

    local exec_start
    exec_start=$(grep -m1 '^ExecStart=' "$unit_file" | cut -d'=' -f2-)
    local repo_dir="${exec_start%/target/release/*}"
    local bin_name
    bin_name=$(basename "$exec_start")

    if [ ! -d "$repo_dir" ]; then
        echo "Could not determine source directory for $name, skipping"
        return
    fi

    if [ -n "$subdir" ] && [ -d "$repo_dir/$subdir" ]; then
        (cd "$repo_dir/$subdir" && cargo build --release "${CARGO_FEATURE_ARGS[@]}")
    else
        (cd "$repo_dir" && cargo build --release "${CARGO_FEATURE_ARGS[@]}")
    fi

    local built_bin="$repo_dir/target/release/$bin_name"
    if [ -f "$built_bin" ] && [ "$built_bin" != "$exec_start" ]; then
        sudo cp "$built_bin" "$exec_start"
    fi

    sudo systemctl restart "$name.service"
    echo "Updated and restarted $name"
}

if [ "$MODE_SELECTION" = "2" ]; then
    read -rp "Enter $MAIN_SERVICE_NAME (server) service name(s) to update, space separated (leave blank for none): " PANEL_SERVICE_NAMES
    read -rp "Enter $NODE_SERVICE_NAME service name(s) to update, space separated (leave blank for none): " NODE_SERVICE_NAMES

    ensure_cargo

    for name in $PANEL_SERVICE_NAMES; do
        update_unit "$name" ""
    done

    for name in $NODE_SERVICE_NAMES; do
        update_unit "$name" "gameserver"
    done

    echo "Done."
    exit 0
fi

# Ask what to install
echo "What would you like to install?"
echo "  1) Panel"
echo "  2) Node"
echo "  3) Both"
read -rp "Selection [3]: " INSTALL_SELECTION
INSTALL_SELECTION="${INSTALL_SELECTION:-3}"

case "$INSTALL_SELECTION" in
    1) INSTALL_PANEL=true;  INSTALL_NODE=false ;;
    2) INSTALL_PANEL=false; INSTALL_NODE=true  ;;
    3) INSTALL_PANEL=true;  INSTALL_NODE=true  ;;
    *) echo "Invalid selection"; exit 1 ;;
esac

# Ask for service names
if $INSTALL_PANEL; then
    read -rp "Panel service name [$MAIN_SERVICE_NAME]: " MAIN_SERVICE_NAME_INPUT
    MAIN_SERVICE_NAME="${MAIN_SERVICE_NAME_INPUT:-$MAIN_SERVICE_NAME}"
fi

if $INSTALL_NODE; then
    read -rp "Node service name [$NODE_SERVICE_NAME]: " NODE_SERVICE_NAME_INPUT
    NODE_SERVICE_NAME="${NODE_SERVICE_NAME_INPUT:-$NODE_SERVICE_NAME}"

    echo "How should the node binary be deployed?"
    echo "  1) Shared binary (single build, all nodes on this host use the same binary)"
    echo "  2) Copied binary per node (required for gRPC nodes)"
    read -rp "Selection [1]: " NODE_BINARY_MODE_SELECTION
    NODE_BINARY_MODE_SELECTION="${NODE_BINARY_MODE_SELECTION:-1}"
    case "$NODE_BINARY_MODE_SELECTION" in
        1) NODE_BINARY_MODE="shared" ;;
        2) NODE_BINARY_MODE="copied" ;;
        *) echo "Invalid selection"; exit 1 ;;
    esac

    DEFAULT_NODE_WORKDIR="$PWD/gameserver"
    read -rp "Node working directory [$DEFAULT_NODE_WORKDIR]: " NODE_WORKDIR_INPUT
    NODE_WORKDIR="${NODE_WORKDIR_INPUT:-$DEFAULT_NODE_WORKDIR}"

    if [ "$NODE_WORKDIR" != "$DEFAULT_NODE_WORKDIR" ]; then
        read -rp "Create core files/directories in '$NODE_WORKDIR'? [Y/n]: " CREATE_CORE
        CREATE_CORE="${CREATE_CORE:-Y}"
        if [[ "$CREATE_CORE" =~ ^[Yy]$ ]]; then
            mkdir -p "$NODE_WORKDIR/server"
            echo "Created server/ directory in $NODE_WORKDIR"
            if [ -f "$DEFAULT_NODE_WORKDIR/provider-db.json" ]; then
                cp "$DEFAULT_NODE_WORKDIR/provider-db.json" "$NODE_WORKDIR/provider-db.json"
                echo "Copied provider-db.json to $NODE_WORKDIR"
            else
                echo "Warning: provider-db.json not found at $DEFAULT_NODE_WORKDIR/provider-db.json, skipping"
            fi
        fi
    fi
fi

if [ ! -d "gameserver-rs" ] && [ "$(basename "$PWD")" != "gameserver-rs" ]; then
    git clone "$REPO_URL"
fi

if [ "$(basename "$PWD")" != "gameserver-rs" ]; then
    cd gameserver-rs
fi

git checkout testing

read -rp "Pull latest updates? [y/N]: " PULL_UPDATE
if [[ "$PULL_UPDATE" =~ ^[Yy]$ ]]; then
    git pull
fi

ensure_cargo
rustup override set "$REQUIRED_RUST_VERSION"

if $INSTALL_PANEL; then
    if [ -d "src/frontend" ]; then
        cd src/frontend
        if ! command -v npm &> /dev/null; then
            curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
            sudo apt-get install -y nodejs
        fi
        npm install
        npm run build
        cd ../..
    fi

    read -rp "Enter LOCALURL value for $MAIN_SERVICE_NAME (leave blank for none): " LOCALURL_MAIN
    read -rp "Enter NODEURL value for $MAIN_SERVICE_NAME (leave blank for none): " NODEURL_MAIN
fi

if $INSTALL_NODE; then
    read -rp "Enter LOCALURL value for $NODE_SERVICE_NAME (leave blank for none): " LOCALURL_NODE
fi

git submodule sync --recursive
git submodule update --init --remote --recursive

cargo build --release "${CARGO_FEATURE_ARGS[@]}"

if $INSTALL_PANEL; then
    MAIN_SERVICE_FILE=$(mktemp)
    cat <<EOF > "$MAIN_SERVICE_FILE"
[Unit]
Description=$MAIN_SERVICE_NAME
After=network.target

[Service]
Type=simple
WorkingDirectory=$PWD
ExecStart=$PWD/target/release/$MAIN_BINARY_NAME
Restart=on-failure
EOF

    [ -n "$LOCALURL_MAIN" ] && echo "Environment=\"LOCALURL=$LOCALURL_MAIN\"" >> "$MAIN_SERVICE_FILE"
    [ -n "$NODEURL_MAIN" ]   && echo "Environment=\"NODEURL=$NODEURL_MAIN\""     >> "$MAIN_SERVICE_FILE"

    echo "[Install]
WantedBy=multi-user.target" >> "$MAIN_SERVICE_FILE"

    sudo cp "$MAIN_SERVICE_FILE" "/etc/systemd/system/$MAIN_SERVICE_NAME.service"
    sudo systemctl daemon-reload
    sudo systemctl enable "$MAIN_SERVICE_NAME.service"
    sudo systemctl restart "$MAIN_SERVICE_NAME.service"
    rm "$MAIN_SERVICE_FILE"
    echo "Panel service '$MAIN_SERVICE_NAME' started and enabled."
fi

if $INSTALL_NODE; then
    cd gameserver
    cargo build --release "${CARGO_FEATURE_ARGS[@]}"
    cd ..

    NODE_BIN_SRC="$PWD/target/release/$NODE_BINARY_NAME"
    if [ "$NODE_BINARY_MODE" = "copied" ]; then
        NODE_BIN_DEST="$NODE_WORKDIR/$NODE_BINARY_NAME"
        cp "$NODE_BIN_SRC" "$NODE_BIN_DEST"
        chmod +x "$NODE_BIN_DEST"
        NODE_EXEC_START="$NODE_BIN_DEST"
    else
        NODE_EXEC_START="$NODE_BIN_SRC"
    fi

    NODE_SERVICE_FILE=$(mktemp)
    cat <<EOF > "$NODE_SERVICE_FILE"
[Unit]
Description=$NODE_SERVICE_NAME
After=network.target

[Service]
Type=simple
WorkingDirectory=$NODE_WORKDIR
ExecStart=$NODE_EXEC_START
Restart=on-failure
EOF

    [ -n "$LOCALURL_NODE" ] && echo "Environment=\"LOCALURL=$LOCALURL_NODE\"" >> "$NODE_SERVICE_FILE"

    echo "[Install]
WantedBy=multi-user.target" >> "$NODE_SERVICE_FILE"

    sudo cp "$NODE_SERVICE_FILE" "/etc/systemd/system/$NODE_SERVICE_NAME.service"
    sudo systemctl daemon-reload
    sudo systemctl enable "$NODE_SERVICE_NAME.service"
    sudo systemctl restart "$NODE_SERVICE_NAME.service"
    rm "$NODE_SERVICE_FILE"
    echo "Node service '$NODE_SERVICE_NAME' started and enabled."
fi

echo "Done."