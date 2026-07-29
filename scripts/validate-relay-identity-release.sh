#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/harbor-relay-identity-validation}"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run_cargo_filter() {
  local manifest="$1" filter="$2" listed
  listed="$(cargo test --manifest-path "$manifest" "$filter" -- --list)"
  if ! grep -Eq '^[^[:space:]].*: test$' <<<"$listed"; then
    printf 'ERROR: Cargo filter matched zero tests: %s (%s)\n' "$filter" "$manifest" >&2
    return 1
  fi
  run cargo test --manifest-path "$manifest" "$filter"
}

cd "$ROOT"
printf 'Harbor commit: %s\n' "$(git rev-parse HEAD)"
printf 'Validation started (UTC): %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Match the three GitHub Actions CI jobs before running the named security
# regressions below. The full workspace tests include database migrations and
# the two-relay/three-identity integration harness.
run pnpm exec tsc --noEmit
run pnpm exec eslint src --max-warnings=0
run pnpm test -- --run
run pnpm build

run cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
run cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
run cargo check --manifest-path src-tauri/Cargo.toml
run cargo test --manifest-path src-tauri/Cargo.toml

run cargo fmt --manifest-path relay-server/Cargo.toml -- --check
run cargo check --manifest-path relay-server/Cargo.toml
run cargo clippy --manifest-path relay-server/Cargo.toml --all-targets -- -D warnings
run cargo test --manifest-path relay-server/Cargo.toml
run ./infrastructure/tests/relay-key-hardening.sh
run ./infrastructure/tests/relay-resource-limit-parity.py

run_cargo_filter src-tauri/Cargo.toml relay_name
run_cargo_filter src-tauri/Cargo.toml name_claim_service
run_cargo_filter src-tauri/Cargo.toml private_introduction_service
run_cargo_filter src-tauri/Cargo.toml private_introductions_repo
run_cargo_filter src-tauri/Cargo.toml contact_card
run_cargo_filter src-tauri/Cargo.toml expired_contact
run_cargo_filter src-tauri/Cargo.toml forged_author
run_cargo_filter src-tauri/Cargo.toml identity_publishing_policy
run_cargo_filter src-tauri/Cargo.toml unknown_name_sealed_delivery_round_trip
run_cargo_filter src-tauri/Cargo.toml relay_key_rotation_service

run_cargo_filter relay-server/Cargo.toml contact_card_wall
run_cargo_filter relay-server/Cargo.toml rejects_replay_expiry_wrong_key_and_tampering
run_cargo_filter relay-server/Cargo.toml unknown_invalid_and_replay_have_identical_response
run_cargo_filter relay-server/Cargo.toml ordinary_rotation_and_rollback
run_cargo_filter relay-server/Cargo.toml rejects_unknown_wrong_and_expired_replacements
run_cargo_filter relay-server/Cargo.toml registration_nonce_replay_leaves_assignment_unchanged
run_cargo_filter relay-server/Cargo.toml relay_restart_invalidates_prior_session_epoch
run_cargo_filter relay-server/Cargo.toml response_jitter_is_deterministic_and_bounded
run_cargo_filter relay-server/Cargo.toml delivery_key_is_real_for_a_claim_and_stable_decoy_otherwise
run_cargo_filter relay-server/Cargo.toml unauthorized_wall_response_excludes_private_posts_media_and_social_events
run_cargo_filter relay-server/Cargo.toml two_relays_three_identities_collision_restart_and_cross_namespace_intro
run_cargo_filter relay-server/Cargo.toml identity_key
run_cargo_filter relay-server/Cargo.toml resource_limits
run_cargo_filter relay-server/Cargo.toml storage_budget

printf '\nAutomated relay identity adversarial checks passed.\n'
printf 'Complete docs/relay-identity-release-validation.md before declaring the release gate passed.\n'
