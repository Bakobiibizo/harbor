#!/usr/bin/env bash
set -euo pipefail

# Build the relay server release binary, update SHA256 hash, and patch templates.
#
# Usage: ./scripts/build-relay.sh
#
# This script:
#   1. Builds relay-server in release mode
#   2. Copies the binary to relay-server/bin/
#   3. Computes the SHA256 hash
#   4. Updates the hash in:
#      - relay-server/bin/harbor-relay.sha256
#      - infrastructure/community-relay-cloudformation.yaml
#      - src/constants/cloudformation-template.ts

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RELAY_DIR="$PROJECT_DIR/relay-server"
BIN_DIR="$RELAY_DIR/bin"
INFRA_DIR="$PROJECT_DIR/infrastructure"
TS_TEMPLATE="$PROJECT_DIR/src/constants/cloudformation-template.ts"
COMMUNITY_YAML="$INFRA_DIR/community-relay-cloudformation.yaml"

echo "[+] Building relay server (release)..."
cargo build --release --manifest-path "$RELAY_DIR/Cargo.toml"

echo "[+] Copying binary to $BIN_DIR..."
mkdir -p "$BIN_DIR"
cp "$RELAY_DIR/target/release/harbor-relay" "$BIN_DIR/harbor-relay"

echo "[+] Computing SHA256..."
SHA256=$(sha256sum "$BIN_DIR/harbor-relay" | awk '{print $1}')
echo "$SHA256  relay-server/bin/harbor-relay" > "$BIN_DIR/harbor-relay.sha256"

echo "[+] Updating community CloudFormation template..."
sed -i "s/EXPECTED_SHA256=\"[a-f0-9]*\"/EXPECTED_SHA256=\"$SHA256\"/" "$COMMUNITY_YAML"
# Also catch the placeholder
sed -i "s/EXPECTED_SHA256=\"PLACEHOLDER_UPDATE_WITH_BUILD_RELAY_SH\"/EXPECTED_SHA256=\"$SHA256\"/" "$COMMUNITY_YAML"

echo "[+] Updating embedded TypeScript template..."
# Match any 64-char hex hash or the placeholder in the TS file's EXPECTED_SHA256 line
sed -i "s/EXPECTED_SHA256=\\\\\"[a-f0-9]*\\\\\"/EXPECTED_SHA256=\\\\\"$SHA256\\\\\"/g" "$TS_TEMPLATE"
sed -i "s/EXPECTED_SHA256=\\\\\"PLACEHOLDER_UPDATE_WITH_BUILD_RELAY_SH\\\\\"/EXPECTED_SHA256=\\\\\"$SHA256\\\\\"/g" "$TS_TEMPLATE"

echo "[+] Done."
echo "    Binary:  $BIN_DIR/harbor-relay"
echo "    SHA256:  $SHA256"
echo ""
echo "    Next steps:"
echo "      1. git add relay-server/bin/ infrastructure/ src/constants/"
echo "      2. git commit -m 'chore: rebuild relay binary'"
echo "      3. git push"
echo "      4. Deploy the CloudFormation stack"
