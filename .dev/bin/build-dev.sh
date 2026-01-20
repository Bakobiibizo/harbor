#!/usr/bin/env bash
set -euo pipefail

DEVKIT_DIR="/home/bakobi/repos/Erasmus/erasmus-3/tools/devkit"
OUT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo build --release -p devkit-cli --manifest-path "$DEVKIT_DIR/Cargo.toml"

cp "$DEVKIT_DIR/target/release/dev" "$OUT_DIR/dev"
chmod +x "$OUT_DIR/dev"

echo "built dev -> $OUT_DIR/dev"
