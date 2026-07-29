#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  cat <<'EOF'
Usage: build-unix-platform.sh --repo-url URL --commit SHA --worktree PATH \
  --output PATH --platform linux-x86_64|linux-aarch64

Builds Harbor from an exact remote commit in a dedicated checkout. This worker
is used locally in WSL and is streamed over SSH to the ARM64 build host.
EOF
}

repo_url=
commit=
worktree=
output=
platform=

while (($#)); do
  case "$1" in
    --repo-url) repo_url=${2:?}; shift 2 ;;
    --commit) commit=${2:?}; shift 2 ;;
    --worktree) worktree=${2:?}; shift 2 ;;
    --output) output=${2:?}; shift 2 ;;
    --platform) platform=${2:?}; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

for value in repo_url commit worktree output platform; do
  if [[ -z ${!value} ]]; then
    printf 'Missing required option: %s\n' "$value" >&2
    usage >&2
    exit 2
  fi
done

if [[ ! $commit =~ ^[0-9a-fA-F]{40}$ ]]; then
  printf 'Commit must be a full 40-character Git SHA: %s\n' "$commit" >&2
  exit 2
fi

case "$platform" in
  linux-x86_64) expected_machine=x86_64 ;;
  linux-aarch64) expected_machine=aarch64 ;;
  *) printf 'Unsupported Unix platform: %s\n' "$platform" >&2; exit 2 ;;
esac

actual_machine=$(uname -m)
if [[ $actual_machine != "$expected_machine" ]]; then
  printf 'Platform %s requires %s, but this host is %s\n' \
    "$platform" "$expected_machine" "$actual_machine" >&2
  exit 1
fi

# Project-owned build hosts use user-scoped Node/pnpm and rustup toolchains when
# available. These take precedence over missing or older distribution packages.
if [[ -d $HOME/.local/bin ]]; then
  export PATH="$HOME/.local/bin:$PATH"
fi
if [[ -d $HOME/.cargo/bin ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

for command in git node pnpm cargo sha256sum file; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'Required command is unavailable: %s\n' "$command" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "$worktree")" "$output"

if [[ ! -d $worktree/.git ]]; then
  if [[ -e $worktree ]]; then
    if [[ -n $(find "$worktree" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null) ]]; then
      printf 'Build checkout exists but is not an empty Git repository: %s\n' "$worktree" >&2
      exit 1
    fi
  fi
  git clone --filter=blob:none --no-checkout "$repo_url" "$worktree"
fi

checkout_origin=$(git -C "$worktree" remote get-url origin)
if [[ $checkout_origin != "$repo_url" ]]; then
  printf 'Build checkout origin mismatch: expected %s, found %s\n' \
    "$repo_url" "$checkout_origin" >&2
  exit 1
fi

git -C "$worktree" fetch --no-tags --prune origin "$commit"
git -C "$worktree" checkout --detach --force FETCH_HEAD
git -C "$worktree" reset --hard "$commit"

actual_commit=$(git -C "$worktree" rev-parse HEAD)
if [[ $actual_commit != "$commit" ]]; then
  printf 'Checkout resolved to %s instead of %s\n' "$actual_commit" "$commit" >&2
  exit 1
fi

(
  cd "$worktree"
  export VITE_HARBOR_RELAY_NAMESPACE=harbor.social
  pnpm install --frozen-lockfile
  pnpm exec tauri build --no-bundle
)

harbor_binary=$worktree/src-tauri/target/release/harbor
harborctl_binary=$worktree/src-tauri/target/release/harborctl
for binary in "$harbor_binary" "$harborctl_binary"; do
  if [[ ! -x $binary ]]; then
    printf 'Expected build output is missing or not executable: %s\n' "$binary" >&2
    exit 1
  fi
done

rm -f "$output/harbor" "$output/harborctl" "$output/SHA256SUMS" "$output/build-info.txt"
install -m 0755 "$harbor_binary" "$output/harbor"
install -m 0755 "$harborctl_binary" "$output/harborctl"

(
  cd "$output"
  sha256sum harbor harborctl > SHA256SUMS
)

version=$(node -e \
  "const fs = require('node:fs'); console.log(JSON.parse(fs.readFileSync(process.argv[1], 'utf8')).version)" \
  "$worktree/package.json")
{
  printf 'platform=%s\n' "$platform"
  printf 'architecture=%s\n' "$actual_machine"
  printf 'commit=%s\n' "$commit"
  printf 'version=%s\n' "$version"
  printf 'built_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  file "$output/harbor"
  file "$output/harborctl"
} > "$output/build-info.txt"

printf 'Built %s at commit %s\nArtifacts: %s\n' "$platform" "$commit" "$output"
