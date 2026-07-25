#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
MAIN="$ROOT/relay-server/src/main.rs"

fail() {
  printf 'relay mode/address parity failed: %s\n' "$1" >&2
  exit 1
}

if grep -Eq 'add_external_address\([^)]*0\.0\.0\.0' "$MAIN"; then
  fail 'wildcard address is published as external'
fi
grep -Fq 'validated_external_addresses(announce_ip' "$MAIN" ||
  fail 'announced addresses do not cross the validation boundary'
grep -Fq 'request_enabled_in_mode(&request, community_mode)' "$MAIN" ||
  fail 'protocol requests are not checked against deployed mode'

for template in \
  "$ROOT/infrastructure/relay-cloudformation.yaml" \
  "$ROOT/src-tauri/harbor-relay-cloudformation.yaml"; do
  service=$(grep -F 'ExecStart=/usr/local/bin/harbor-relay' "$template")
  [[ "$service" == *'--identity-namespace'* ]] ||
    fail "$(basename "$template") omits identity namespace"
  [[ "$service" == *'--data-dir /var/lib/harbor-relay/data'* ]] ||
    fail "$(basename "$template") does not persist identity state"
  [[ "$service" != *'--community'* ]] ||
    fail "$(basename "$template") unexpectedly enables community boards"
done

community=$(grep -F 'ExecStart=/usr/local/bin/harbor-relay' \
  "$ROOT/infrastructure/community-relay-cloudformation.yaml")
[[ "$community" == *'--identity-namespace'* ]] ||
  fail 'community template omits identity namespace'
[[ "$community" == *'--data-dir /var/lib/harbor-relay/data'* ]] ||
  fail 'community template does not persist relay state'
[[ "$community" == *'--community'* ]] ||
  fail 'community template does not enable community boards'

printf 'relay mode/address parity passed\n'
