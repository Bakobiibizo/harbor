#!/usr/bin/env bash
set -euo pipefail

LIMITS_FILE=${HARBOR_RELAY_RESOURCE_LIMITS_FILE:-/etc/harbor-relay/resource-limits.env}
while IFS='=' read -r name value; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  [[ "$name" =~ ^HARBOR_RELAY_[A-Z0-9_]+$ ]] || {
    printf 'invalid relay resource variable in %s: %s\n' "$LIMITS_FILE" "$name" >&2
    exit 1
  }
  [[ "$value" =~ ^[0-9]+$ ]] || {
    printf 'invalid relay resource value in %s for %s\n' "$LIMITS_FILE" "$name" >&2
    exit 1
  }
  if [[ -z "${!name+x}" ]]; then
    export "$name=$value"
  fi
done < "$LIMITS_FILE"

exec /usr/local/bin/harbor-relay "$@"
