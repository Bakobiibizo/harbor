#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

BINARY="${HARBOR_SMOKE_BINARY:-$ROOT_DIR/src-tauri/target/release/harbor}"
NAME="${HARBOR_SMOKE_NAME:-}"
NAMESPACE="${HARBOR_SMOKE_NAMESPACE:-harbor.social}"
PASSPHRASE="${HARBOR_SMOKE_PASSPHRASE:-}"
PROFILE_ROOT="${HARBOR_SMOKE_PROFILE_ROOT:-/tmp/harbor-packaged-smoke-${NAME:-unset}}"
OUTPUT_DIR="${HARBOR_SMOKE_OUTPUT_DIR:-/tmp/harbor-packaged-smoke-evidence}"
CONTROL_PORT="${HARBOR_SMOKE_CONTROL_PORT:-19620}"
CONTROL_TOKEN="harbor-packaged-smoke-control-${CONTROL_PORT}"

if [ -z "$NAME" ] || [ -z "$PASSPHRASE" ]; then
  echo "HARBOR_SMOKE_NAME and HARBOR_SMOKE_PASSPHRASE are required." >&2
  echo "The name is registered permanently with the configured relay; choose it intentionally." >&2
  exit 2
fi
if [[ ! "$NAME" =~ ^[a-z0-9]([a-z0-9-]{1,30}[a-z0-9])$ ]] || [[ "$NAME" == *--* ]]; then
  echo "HARBOR_SMOKE_NAME must be a canonical 3-32 character Harbor name." >&2
  exit 2
fi
if [ ! -x "$BINARY" ]; then
  echo "Packaged Harbor binary not found or not executable: $BINARY" >&2
  exit 2
fi

for command in jq nc python3 scrot xdotool; do
  command -v "$command" >/dev/null || {
    echo "Missing packaged-smoke dependency: $command" >&2
    exit 2
  }
done

if [ "${HARBOR_SMOKE_UNDER_XVFB:-0}" != 1 ]; then
  command -v xvfb-run >/dev/null || {
    echo "Missing packaged-smoke dependency: xvfb-run" >&2
    exit 2
  }
  if [ "${HARBOR_SMOKE_SKIP_COMPONENT_TESTS:-0}" != 1 ]; then
    pnpm exec vitest run \
      src/components/identity/MentionInbox.test.tsx \
      src/components/identity/LegacyIdentityMigration.test.tsx \
      src/pages/ContactWall.test.tsx
  fi
  mkdir -p "$OUTPUT_DIR"
  exec xvfb-run -a -s '-screen 0 1280x900x24' env HARBOR_SMOKE_UNDER_XVFB=1 "$0"
fi

mkdir -p "$PROFILE_ROOT" "$OUTPUT_DIR"
APP_PID=""

cleanup() {
  if [ -n "$APP_PID" ] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

control() {
  local port="$1"
  local request="$2"
  printf '%s\n' "$request" | nc -q 0 -w 5 127.0.0.1 "$port"
}

launch() {
  local port="$1"
  local log="$2"
  export GDK_BACKEND=x11
  export LIBGL_ALWAYS_SOFTWARE=1
  export WEBKIT_DISABLE_COMPOSITING_MODE=1
  export HARBOR_PROFILE=packaged-smoke
  export HARBOR_DATA_DIR="$PROFILE_ROOT"
  export HARBOR_CONTROL_TOKEN="$CONTROL_TOKEN"
  export HARBOR_CONTROL_PORT="$port"
  export RUST_LOG=harbor_lib=info
  "$BINARY" >"$log" 2>&1 &
  APP_PID=$!
  local window=""
  for _ in $(seq 1 30); do
    window="$(xdotool search --name 'Harbor' 2>/dev/null | head -n 1 || true)"
    [ -n "$window" ] && break
    sleep 1
  done
  [ -n "$window" ] || {
    echo "Harbor window did not appear; see $log" >&2
    return 1
  }
  xdotool windowsize "$window" 1200 800
  xdotool windowmove "$window" 40 40
  sleep 3
}

shutdown() {
  local port="$1"
  control "$port" "{\"id\":\"shutdown\",\"token\":\"$CONTROL_TOKEN\",\"command\":\"shutdown\"}" >/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
}

frontend_snapshot() {
  local port="$1"
  control "$port" "{\"id\":\"snapshot\",\"token\":\"$CONTROL_TOKEN\",\"command\":\"frontend\",\"action\":\"state.snapshot\",\"payload\":{}}"
}

wait_for_verified_frontend() {
  local port="$1"
  local output="$2"
  for _ in $(seq 1 75); do
    local response
    response="$(frontend_snapshot "$port" 2>/dev/null || true)"
    if printf '%s' "$response" | jq -e \
      --arg relay "$NAMESPACE" \
      '.ok == true and .result.identity.status == "unlocked" and .result.identity.identity.relayNameClaim.request.relay == $relay' \
      >/dev/null 2>&1; then
      printf '%s\n' "$response" >"$output"
      return 0
    fi
    sleep 2
  done
  return 1
}

has_active_claim() {
  [ -s "$PROFILE_ROOT/harbor.db" ] && python3 - "$PROFILE_ROOT/harbor.db" "$NAME" "$NAMESPACE" <<'PY'
import sqlite3, sys
db, name, relay = sys.argv[1:]
connection = sqlite3.connect(db)
qualified = f"@{name}@{relay}"
count = connection.execute(
    "SELECT COUNT(*) FROM relay_name_claims WHERE qualified_name=? AND status='active'",
    (qualified,),
).fetchone()[0]
raise SystemExit(0 if count == 1 else 1)
PY
}

EXISTING_CLAIM=0
if has_active_claim; then
  EXISTING_CLAIM=1
fi

FIRST_LOG="$OUTPUT_DIR/first-launch.log"
launch "$CONTROL_PORT" "$FIRST_LOG"

if [ "$EXISTING_CLAIM" = 1 ]; then
  control "$CONTROL_PORT" "{\"id\":\"unlock\",\"token\":\"$CONTROL_TOKEN\",\"command\":\"identity_unlock\",\"passphrase\":\"$PASSPHRASE\"}" >"$OUTPUT_DIR/first-launch-unlock.json"
  control "$CONTROL_PORT" "{\"id\":\"refresh\",\"token\":\"$CONTROL_TOKEN\",\"command\":\"frontend\",\"action\":\"identity.refresh\",\"payload\":{}}" >"$OUTPUT_DIR/first-launch-refresh.json"
else
  scrot "$OUTPUT_DIR/onboarding-profile.png"
  xdotool mousemove 900 348 click 1 type --delay 20 "$NAME"
  xdotool key Tab Tab Return
  sleep 2
  xdotool type --delay 15 "$PASSPHRASE"
  xdotool key Tab
  xdotool type --delay 15 "$PASSPHRASE"
  xdotool key Tab
  xdotool type --delay 15 packaged-smoke-only
  xdotool key Tab Tab Return
fi

if ! wait_for_verified_frontend "$CONTROL_PORT" "$OUTPUT_DIR/first-launch-state.json"; then
  scrot "$OUTPUT_DIR/first-launch-failed.png"
  echo "Verified onboarding did not complete; see $OUTPUT_DIR" >&2
  exit 1
fi
scrot "$OUTPUT_DIR/verified-onboarding.png"
shutdown "$CONTROL_PORT"

python3 - "$PROFILE_ROOT" "$NAME" "$NAMESPACE" <<'PY'
import json, pathlib, sqlite3, sys
root, name, relay = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
assert (root / "harbor.db").is_file()
assert (root / "accounts.json").is_file()
assert (root / "logs").is_dir()
connection = sqlite3.connect(root / "harbor.db")
qualified = f"@{name}@{relay}"
row = connection.execute(
    "SELECT qualified_name, peer_id, status FROM relay_name_claims WHERE qualified_name=?",
    (qualified,),
).fetchone()
assert row and row[0] == qualified and row[2] == "active", row
mode = connection.execute(
    "SELECT mode FROM identity_migration_state WHERE peer_id=?", (row[1],)
).fetchone()
assert mode == ("verified",), mode
print(json.dumps({"qualifiedName": qualified, "peerId": row[1], "mode": mode[0]}))
PY

RESTART_PORT=$((CONTROL_PORT + 1))
RESTART_LOG="$OUTPUT_DIR/restart.log"
launch "$RESTART_PORT" "$RESTART_LOG"
control "$RESTART_PORT" "{\"id\":\"unlock\",\"token\":\"$CONTROL_TOKEN\",\"command\":\"identity_unlock\",\"passphrase\":\"$PASSPHRASE\"}" >"$OUTPUT_DIR/restart-unlock.json"
control "$RESTART_PORT" "{\"id\":\"refresh\",\"token\":\"$CONTROL_TOKEN\",\"command\":\"frontend\",\"action\":\"identity.refresh\",\"payload\":{}}" >"$OUTPUT_DIR/restart-refresh.json"
if ! wait_for_verified_frontend "$RESTART_PORT" "$OUTPUT_DIR/restart-state.json"; then
  scrot "$OUTPUT_DIR/restart-failed.png"
  echo "Verified name did not recover after restart; see $OUTPUT_DIR" >&2
  exit 1
fi
scrot "$OUTPUT_DIR/restart-recovered.png"
shutdown "$RESTART_PORT"

{
  echo "commit=$(git rev-parse HEAD)"
  echo "binary_sha256=$(sha256sum "$BINARY" | awk '{print $1}')"
  echo "os=$(uname -srmo)"
  echo "qualified_name=@$NAME@$NAMESPACE"
  echo "profile_root=$PROFILE_ROOT"
  echo "completed_at_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >"$OUTPUT_DIR/evidence.txt"

echo "Packaged Harbor UX smoke passed. Evidence: $OUTPUT_DIR"
