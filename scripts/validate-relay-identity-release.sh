#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/harbor-relay-identity-validation}"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

cd "$ROOT"
run npm test -- --run src/services/mentions.test.ts src/utils/mentions.test.ts

run cargo test --manifest-path src-tauri/Cargo.toml --lib relay_name
run cargo test --manifest-path src-tauri/Cargo.toml --lib private_introduction_service
run cargo test --manifest-path src-tauri/Cargo.toml --lib contact_card
run cargo test --manifest-path src-tauri/Cargo.toml --lib expired_contact
run cargo test --manifest-path src-tauri/Cargo.toml --lib forged_author

run cargo test --manifest-path relay-server/Cargo.toml contact_card_wall
run cargo test --manifest-path relay-server/Cargo.toml rejects_replay_expiry_wrong_key_and_tampering
run cargo test --manifest-path relay-server/Cargo.toml unknown_invalid_and_replay_have_identical_response
run cargo test --manifest-path relay-server/Cargo.toml ordinary_rotation_and_rollback
run cargo test --manifest-path relay-server/Cargo.toml rejects_unknown_wrong_and_expired_replacements

printf '\nAutomated relay identity adversarial checks passed.\n'
printf 'Complete docs/relay-identity-release-validation.md before declaring the release gate passed.\n'
