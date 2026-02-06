#!/usr/bin/env bash
set -euo pipefail

# Build the relay server release binary and update the SHA256 hash file

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RELAY_DIR="$PROJECT_DIR/relay-server"
BIN_DIR="$RELAY_DIR/bin"

echo "[+] Building relay server (release)..."
cargo build --release --manifest-path "$RELAY_DIR/Cargo.toml"

echo "[+] Copying binary to $BIN_DIR..."
mkdir -p "$BIN_DIR"
cp "$RELAY_DIR/target/release/harbor-relay" "$BIN_DIR/harbor-relay"

echo "[+] Computing SHA256..."
SHA256=$(sha256sum "$BIN_DIR/harbor-relay" | awk '{print $1}')
echo "$SHA256  relay-server/bin/harbor-relay" > "$BIN_DIR/harbor-relay.sha256"

echo "[+] Done."
echo "    Binary: $BIN_DIR/harbor-relay"
echo "    SHA256: $SHA256"
