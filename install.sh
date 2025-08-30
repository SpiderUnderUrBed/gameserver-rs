#!/usr/bin/env bash
set -e

REPO_URL="https://github.com/SpiderUnderUrBed/gameserver-rs"
MAIN_SERVICE_NAME="gameserver-rs"
NODE_SERVICE_NAME="gameserver-rs-node"
SYSTEMD_FILE="gameserver-rs-control"

if [ ! -f "Cargo.toml" ] || ! grep -q '\[package\]' Cargo.toml; then
    if [ ! -d "gameserver-rs" ]; then
        git clone "$REPO_URL"
    fi
    cd gameserver-rs
fi

read -rp "Do you want to set a custom LOCALURL for $MAIN_SERVICE_NAME? [y/N]: " USE_LOCALURL_MAIN
if [[ "$USE_LOCALURL_MAIN" =~ ^[Yy]$ ]]; then
    read -rp "Enter LOCALURL value for $MAIN_SERVICE_NAME: " LOCALURL_MAIN
    echo "LOCALURL for $MAIN_SERVICE_NAME set to '$LOCALURL_MAIN'"
else
    LOCALURL_MAIN=""
fi

cargo build --release

if [ ! -f "$SYSTEMD_FILE" ]; then
    echo "Systemd file '$SYSTEMD_FILE' not found!"
    exit 1
fi

TMP_MAIN_SERVICE=$(mktemp)
if [ -n "$LOCALURL_MAIN" ]; then
    awk -v env="LOCALURL=$LOCALURL_MAIN" '/^\[Service\]/ {print; print "Environment=\"" env "\""; next}1' "$SYSTEMD_FILE" > "$TMP_MAIN_SERVICE"
else
    cp "$SYSTEMD_FILE" "$TMP_MAIN_SERVICE"
fi

sudo cp "$TMP_MAIN_SERVICE" "/etc/systemd/system/$MAIN_SERVICE_NAME.service"
sudo systemctl daemon-reload
sudo systemctl enable "$MAIN_SERVICE_NAME.service"
sudo systemctl restart "$MAIN_SERVICE_NAME.service"
rm "$TMP_MAIN_SERVICE"

cd src/gameserver

read -rp "Do you want to set a custom LOCALURL for $NODE_SERVICE_NAME? [y/N]: " USE_LOCALURL_NODE
if [[ "$USE_LOCALURL_NODE" =~ ^[Yy]$ ]]; then
    read -rp "Enter LOCALURL value for $NODE_SERVICE_NAME: " LOCALURL_NODE
    echo "LOCALURL for $NODE_SERVICE_NAME set to '$LOCALURL_NODE'"
else
    LOCALURL_NODE=""
fi

cargo build --release

TMP_NODE_SERVICE=$(mktemp)
if [ -n "$LOCALURL_NODE" ]; then
    awk -v env="LOCALURL=$LOCALURL_NODE" '/^\[Service\]/ {print; print "Environment=\"" env "\""; next}1' "../../$SYSTEMD_FILE" > "$TMP_NODE_SERVICE"
else
    cp "../../$SYSTEMD_FILE" "$TMP_NODE_SERVICE"
fi

sudo cp "$TMP_NODE_SERVICE" "/etc/systemd/system/$NODE_SERVICE_NAME.service"
sudo systemctl daemon-reload
sudo systemctl enable "$NODE_SERVICE_NAME.service"
sudo systemctl restart "$NODE_SERVICE_NAME.service"
rm "$TMP_NODE_SERVICE"

echo "Both services are now started and enabled."
