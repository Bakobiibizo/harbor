#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
TEMPLATES=(
  "$ROOT/infrastructure/relay-cloudformation.yaml"
  "$ROOT/infrastructure/community-relay-cloudformation.yaml"
  "$ROOT/src-tauri/harbor-relay-cloudformation.yaml"
)
KEY_TEMPLATES=("${TEMPLATES[@]:0:2}")
SENTINEL='HARBOR_RELAY_PRIVATE_KEY_SENTINEL_8f21c947'
SENTINEL_B64=$(printf '%s' "$SENTINEL" | base64 -w0)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

fail() {
  printf 'relay key hardening test failed: %s\n' "$*" >&2
  exit 1
}

extract_block() {
  local template=$1
  local block=$2
  awk -v begin="# HARBOR_KEY_${block}_BEGIN" -v end="# HARBOR_KEY_${block}_END" '
    index($0, begin) { capture = 1; next }
    index($0, end) { capture = 0 }
    capture { sub(/^          /, ""); print }
  ' "$template"
}

for template in "${TEMPLATES[@]}"; do
  if grep -Eq '#!/[^[:space:]]+[[:space:]]+-[^[:space:]]*x|^[[:space:]]*set[[:space:]]+-[^[:space:]]*x' "$template"; then
    fail "$template enables shell tracing"
  fi
  grep -Fq 'set -euo pipefail' "$template" || fail "$template does not fail closed"
  grep -Fq 'umask 077' "$template" || fail "$template does not set an owner-only umask"
  grep -Fq 'UMask=0077' "$template" || fail "$template does not constrain the service umask"
  if grep -Eq 'EXISTING_KEY|IDENTITY_KEY_B64|echo[[:space:]].*identity.*key.*\$' "$template"; then
    fail "$template expands identity key material into a loggable shell command"
  fi
  if grep -Eq '\$\{IDENTITY_(KEY_PATH|ENCODED|RESTORE|FETCH_ERROR|EXPORT)\}' "$template"; then
    fail "$template exposes a shell variable to CloudFormation substitution"
  fi
done

for template in "${KEY_TEMPLATES[@]}"; do
  grep -Fq -- '--value "file://$IDENTITY_EXPORT"' "$template" \
    || fail "$template does not pass SecureString material by owner-only parameter file"
  grep -Fq 'mv -T "$IDENTITY_RESTORE" "$IDENTITY_KEY_PATH"' "$template" \
    || fail "$template does not publish restored key material atomically"

  key_dir="$WORK/$(basename "$template" .yaml)"
  key_path="$key_dir/id.key"
  capture="$key_dir/captured-cloud-init-console-journal.log"
  put_ok="$key_dir/put-ok"
  mkdir -p "$key_dir"

  aws() {
    if [ "$1 $2" = "ssm get-parameter" ]; then
      case "${AWS_GET_MODE:-ok}" in
        ok) printf '%s' "$SENTINEL_B64"; return 0 ;;
        missing) printf 'ParameterNotFound' >&2; return 1 ;;
        denied) printf 'AccessDeniedException' >&2; return 1 ;;
        invalid) printf 'not-valid-base64!' ; return 0 ;;
        *) return 94 ;;
      esac
    fi
    if [ "$1 $2" = "ssm put-parameter" ]; then
      local argument value_path=''
      for argument in "$@"; do
        case "$argument" in
          file://*) value_path=${argument#file://} ;;
        esac
      done
      [ -n "$value_path" ] || return 91
      [ "$(cat "$value_path")" = "$SENTINEL_B64" ] || return 92
      : > "$AWS_PUT_OK"
      return 0
    fi
    return 93
  }
  export SENTINEL_B64 AWS_PUT_OK="$put_ok" AWS_GET_MODE=ok
  export -f aws

  (
    set -euo pipefail
    umask 077
    IDENTITY_KEY_PATH="$key_path"
    IDENTITY_SSM_PARAM='/harbor/test/identity-key'
    REGION='test-1'
    source <(extract_block "$template" RESTORE)
  ) > "$capture" 2>&1

  [ "$(cat "$key_path")" = "$SENTINEL" ] || fail "$template did not restore the sentinel key"
  [ "$(stat -c '%a' "$key_path")" = '600' ] || fail "$template restored a non-0600 key"

  (
    set -euo pipefail
    umask 077
    IDENTITY_KEY_PATH="$key_path"
    IDENTITY_SSM_PARAM='/harbor/test/identity-key'
    REGION='test-1'
    KEY_IS_NEW=true
    IDENTITY_EXPORT=''
    source <(extract_block "$template" PERSIST)
  ) >> "$capture" 2>&1

  [ -f "$put_ok" ] || fail "$template did not persist through the parameter file"
  if grep -Fq "$SENTINEL" "$capture" || grep -Fq "$SENTINEL_B64" "$capture"; then
    fail "$template disclosed sentinel material to captured cloud-init/console/journal output"
  fi

  chmod 0644 "$key_path"
  if (
    set -euo pipefail
    IDENTITY_KEY_PATH="$key_path"
    IDENTITY_SSM_PARAM='/harbor/test/identity-key'
    REGION='test-1'
    KEY_IS_NEW=true
    IDENTITY_EXPORT=''
    source <(extract_block "$template" PERSIST)
  ) >> "$capture" 2>&1; then
    fail "$template accepted an identity key with unsafe mode 0644"
  fi

  rm -f "$key_path"
  for failure_mode in denied invalid; do
    if (
      set -euo pipefail
      umask 077
      export AWS_GET_MODE="$failure_mode"
      IDENTITY_KEY_PATH="$key_path"
      IDENTITY_SSM_PARAM='/harbor/test/identity-key'
      REGION='test-1'
      source <(extract_block "$template" RESTORE)
    ) >> "$capture" 2>&1; then
      fail "$template did not fail closed for SSM mode $failure_mode"
    fi
    [ ! -e "$key_path" ] || fail "$template published a key after SSM mode $failure_mode"
  done

  (
    set -euo pipefail
    umask 077
    export AWS_GET_MODE=missing
    IDENTITY_KEY_PATH="$key_path"
    IDENTITY_SSM_PARAM='/harbor/test/identity-key'
    REGION='test-1'
    source <(extract_block "$template" RESTORE)
    [ "$KEY_IS_NEW" = true ]
  ) >> "$capture" 2>&1
done

printf 'relay key hardening sentinel tests passed\n'
