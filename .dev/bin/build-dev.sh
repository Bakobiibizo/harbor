#!/usr/bin/env bash
set -euo pipefail

DEVKIT_DIR="${DEVKIT_DIR:-/mnt/d/apps/devkit}"
TARGET="${DEVKIT_TARGET:-aarch64-unknown-linux-gnu}"
OUT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo build --release -p devkit-cli --target "$TARGET" --manifest-path "$DEVKIT_DIR/Cargo.toml"

install -m 0755 "$DEVKIT_DIR/target/$TARGET/release/dev" "$OUT_DIR/dev"

echo "built dev ($TARGET) -> $OUT_DIR/dev"
