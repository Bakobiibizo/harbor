#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SRC_BIN="$ROOT_DIR/.dev/bin/dev"
DEST_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
DEST_BIN="$DEST_DIR/dev"

if [[ ! -f "$SRC_BIN" ]]; then
  echo "error: expected $SRC_BIN to exist"
  echo "hint: run .dev/bin/build-dev.sh first"
  exit 1
fi

mkdir -p "$DEST_DIR"
install -m 0755 "$SRC_BIN" "$DEST_BIN"

echo "installed dev -> $DEST_BIN"
