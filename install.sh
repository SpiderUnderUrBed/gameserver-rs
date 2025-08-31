#!/usr/bin/env bash
set -e

REPO_URL="https://github.com/SpiderUnderUrBed/gameserver-rs"
MAIN_SERVICE_NAME="gameserver-rs"
NODE_SERVICE_NAME="gameserver-rs-node"
LOCAL_RUST="$PWD/.rust"
REQUIRED_RUST_VERSION="1.88.0"

if [ ! -d "gameserver-rs" ]; then
    git clone "$REPO_URL"
fi

cd gameserver-rs

if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --no-modify-path --default-toolchain "$REQUIRED_RUST_VERSION" --profile minimal
    mv "$HOME/.cargo" "$LOCAL_RUST"
fi

export PATH="$LOCAL_RUST/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "Cargo installation failed"; exit 1; }
rustup override set "$REQUIRED_RUST_VERSION"

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

read -rp "Do you want to set a custom LOCALURL for $MAIN_SERVICE_NAME? [y/N]: " USE_LOCALURL_MAIN
if [[ "$USE_LOCALURL_MAIN" =~ ^[Yy]$ ]]; then
    read -rp "Enter LOCALURL value for $MAIN_SERVICE_NAME: " LOCALURL_MAIN
else
    LOCALURL_MAIN=""
fi

cargo build --release

MAIN_SERVICE_FILE=$(mktemp)
cat <<EOF > "$MAIN_SERVICE_FILE"
[Unit]
Description=$MAIN_SERVICE_NAME
After=network.target

[Service]
Type=simple
WorkingDirectory=$PWD
ExecStart=$PWD/target/release/$MAIN_SERVICE_NAME
Restart=on-failure
EOF

if [ -n "$LOCALURL_MAIN" ]; then
    echo "Environment=\"LOCALURL=$LOCALURL_MAIN\"" >> "$MAIN_SERVICE_FILE"
fi

echo "[Install]
WantedBy=multi-user.target" >> "$MAIN_SERVICE_FILE"

sudo cp "$MAIN_SERVICE_FILE" "/etc/systemd/system/$MAIN_SERVICE_NAME.service"
sudo systemctl daemon-reload
sudo systemctl enable "$MAIN_SERVICE_NAME.service"
sudo systemctl restart "$MAIN_SERVICE_NAME.service"
rm "$MAIN_SERVICE_FILE"

cd src/gameserver
read -rp "Do you want to set a custom LOCALURL for $NODE_SERVICE_NAME? [y/N]: " USE_LOCALURL_NODE
if [[ "$USE_LOCALURL_NODE" =~ ^[Yy]$ ]]; then
    read -rp "Enter LOCALURL value for $NODE_SERVICE_NAME: " LOCALURL_NODE
else
    LOCALURL_NODE=""
fi

cargo build --release

NODE_SERVICE_FILE=$(mktemp)
cat <<EOF > "$NODE_SERVICE_FILE"
[Unit]
Description=$NODE_SERVICE_NAME
After=network.target

[Service]
Type=simple
WorkingDirectory=$PWD
ExecStart=$PWD/target/release/$NODE_SERVICE_NAME
Restart=on-failure
EOF

if [ -n "$LOCALURL_NODE" ]; then
    echo "Environment=\"LOCALURL=$LOCALURL_NODE\"" >> "$NODE_SERVICE_FILE"
fi

echo "[Install]
WantedBy=multi-user.target" >> "$NODE_SERVICE_FILE"

sudo cp "$NODE_SERVICE_FILE" "/etc/systemd/system/$NODE_SERVICE_NAME.service"
sudo systemctl daemon-reload
sudo systemctl enable "$NODE_SERVICE_NAME.service"
sudo systemctl restart "$NODE_SERVICE_NAME.service"
rm "$NODE_SERVICE_FILE"

echo "Both services are now started and enabled."
