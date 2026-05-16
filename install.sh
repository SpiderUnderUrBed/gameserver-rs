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

if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --no-modify-path --default-toolchain "$REQUIRED_RUST_VERSION" --profile minimal
    mv "$HOME/.cargo" "$LOCAL_RUST"
fi

export PATH="$LOCAL_RUST/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "Cargo installation failed"; exit 1; }
rustup override set "$REQUIRED_RUST_VERSION"

if $INSTALL_PANEL; then
    if [ -d "src/svelte" ]; then
        cd src/svelte
        mkdir -p build
        if ! command -v npm &> /dev/null; then
            curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
            sudo apt-get install -y nodejs
        fi
        npm install
        npm run build
        cp -r .svelte-kit/output/client/* build/
        cd ../..
    fi

    read -rp "Enter LOCALURL value for $MAIN_SERVICE_NAME (leave blank for none): " LOCALURL_MAIN
    read -rp "Enter TCPURL value for $MAIN_SERVICE_NAME (leave blank for none): " TCPURL_MAIN
fi

if $INSTALL_NODE; then
    read -rp "Enter LOCALURL value for $NODE_SERVICE_NAME (leave blank for none): " LOCALURL_NODE
fi

cargo build --release

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
    [ -n "$TCPURL_MAIN" ]   && echo "Environment=\"TCPURL=$TCPURL_MAIN\""     >> "$MAIN_SERVICE_FILE"

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
    cargo build --release
    cd ..

    NODE_SERVICE_FILE=$(mktemp)
    cat <<EOF > "$NODE_SERVICE_FILE"
[Unit]
Description=$NODE_SERVICE_NAME
After=network.target

[Service]
Type=simple
WorkingDirectory=$NODE_WORKDIR
ExecStart=$PWD/target/release/$NODE_BINARY_NAME
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