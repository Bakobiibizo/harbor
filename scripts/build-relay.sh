#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

cargo build --release --manifest-path relay-server/Cargo.toml
EXPECTED_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' relay-server/Cargo.toml | head -n 1)
ACTUAL_VERSION=$(relay-server/target/release/harbor-relay --version)
if [[ "$ACTUAL_VERSION" != *" $EXPECTED_VERSION" ]]; then
  echo "Relay version mismatch: expected $EXPECTED_VERSION, got $ACTUAL_VERSION" >&2
  exit 1
fi
if ! relay-server/target/release/harbor-relay --help | grep -q -- '--identity-namespace'; then
  echo "Relay artifact is missing identity namespace support" >&2
  exit 1
fi
cp relay-server/target/release/harbor-relay relay-server/bin/harbor-relay
SHA256=$(sha256sum "relay-server/bin/harbor-relay" | awk '{print $1}')
echo "$SHA256  relay-server/bin/harbor-relay" > "relay-server/bin/harbor-relay.sha256"

python3 - <<'PY'
from pathlib import Path
import re
sha = Path('relay-server/bin/harbor-relay.sha256').read_text().split()[0]
paths = [
    Path('infrastructure/community-relay-cloudformation.yaml'),
    Path('infrastructure/relay-cloudformation.yaml'),
    Path('infrastructure/scripts/update-relay.sh'),
]
for path in paths:
    text = path.read_text()
    if not re.search(r'EXPECTED_SHA256="[0-9a-f]{64}"', text):
        raise SystemExit(f'No EXPECTED_SHA256 assignment found in {path}')
    updated = re.sub(r'EXPECTED_SHA256="[0-9a-f]{64}"', f'EXPECTED_SHA256="{sha}"', text)
    path.write_text(updated)
PY

echo "Built relay-server/bin/harbor-relay"
echo "SHA256: $SHA256"
