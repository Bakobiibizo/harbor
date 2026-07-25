#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)

usage() {
  cat <<'EOF'
Usage: scripts/build-platform-matrix.sh [options]

Build Harbor on project-owned infrastructure without GitHub Actions.

Options:
  --ref REF             Remote branch, tag, or commit. Defaults to the current branch.
  --platform PLATFORM   Build only one platform. May be repeated. Supported values:
                        linux-x86_64, windows-x86_64, linux-aarch64, all.
  --output PATH         Artifact root. Defaults to artifacts/manual-ci/<short-sha>.
  --dry-run             Resolve the commit and print the build plan without compiling.
  -h, --help            Show this help.

Environment overrides:
  HARBOR_GX10_HOST              SSH host (default: gx10)
  HARBOR_LINUX_WORKTREE         WSL build checkout
  HARBOR_WINDOWS_WORKTREE       Windows build checkout, in Windows path syntax
  HARBOR_GX10_WORKTREE          ARM64 checkout on gx10
  HARBOR_GX10_OUTPUT            ARM64 artifact directory on gx10
EOF
}

ref=
output_root=
dry_run=0
declare -a platforms=()

while (($#)); do
  case "$1" in
    --) shift ;;
    --ref) ref=${2:?}; shift 2 ;;
    --platform) platforms+=("${2:?}"); shift 2 ;;
    --output) output_root=${2:?}; shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

if ((${#platforms[@]} == 0)); then
  platforms=(all)
fi
if [[ " ${platforms[*]} " == *' all '* ]]; then
  platforms=(linux-x86_64 windows-x86_64 linux-aarch64)
fi

for platform in "${platforms[@]}"; do
  case "$platform" in
    linux-x86_64|windows-x86_64|linux-aarch64) ;;
    *) printf 'Unsupported platform: %s\n' "$platform" >&2; exit 2 ;;
  esac
done

repository_url=$(git -C "$repo_root" remote get-url origin)
if [[ -z $ref ]]; then
  ref=$(git -C "$repo_root" symbolic-ref --quiet --short HEAD || true)
  if [[ -z $ref ]]; then
    printf 'Detached checkout: pass --ref explicitly.\n' >&2
    exit 2
  fi
fi

printf 'Resolving %s from %s...\n' "$ref" "$repository_url"
git -C "$repo_root" fetch --quiet --no-tags "$repository_url" "$ref"
commit=$(git -C "$repo_root" rev-parse 'FETCH_HEAD^{commit}')
short_commit=${commit:0:12}

if [[ -z $output_root ]]; then
  output_root=$repo_root/artifacts/manual-ci/$short_commit
elif [[ $output_root != /* ]]; then
  output_root=$repo_root/$output_root
fi

gx10_host=${HARBOR_GX10_HOST:-gx10}
linux_worktree=${HARBOR_LINUX_WORKTREE:-$HOME/.cache/harbor-build/linux-x86_64/repo}
windows_worktree=${HARBOR_WINDOWS_WORKTREE:-E:\\apps\\builds\\harbor-windows\\repo}
gx10_worktree=${HARBOR_GX10_WORKTREE:-.cache/harbor-build/linux-aarch64/repo}
gx10_output=${HARBOR_GX10_OUTPUT:-.cache/harbor-build/linux-aarch64/output/$short_commit}

printf 'Build ref:    %s\n' "$ref"
printf 'Build commit: %s\n' "$commit"
printf 'Platforms:    %s\n' "${platforms[*]}"
printf 'Artifacts:    %s\n' "$output_root"

if ((dry_run)); then
  printf 'WSL checkout:     %s\n' "$linux_worktree"
  printf 'Windows checkout: %s\n' "$windows_worktree"
  printf 'ARM64 checkout:   %s:%s\n' "$gx10_host" "$gx10_worktree"
  exit 0
fi

mkdir -p "$output_root"

for platform in "${platforms[@]}"; do
  case "$platform" in
    linux-x86_64)
      "$script_dir/build-unix-platform.sh" \
        --repo-url "$repository_url" \
        --commit "$commit" \
        --worktree "$linux_worktree" \
        --output "$output_root/linux-x86_64" \
        --platform linux-x86_64
      ;;
    windows-x86_64)
      if ! command -v powershell.exe >/dev/null 2>&1; then
        printf 'powershell.exe is unavailable; run the matrix controller from WSL.\n' >&2
        exit 1
      fi
      windows_script=$(wslpath -w "$script_dir/build-windows-platform.ps1")
      windows_output=$(wslpath -w "$output_root/windows-x86_64")
      powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$windows_script" \
        -RepositoryUrl "$repository_url" \
        -Commit "$commit" \
        -Worktree "$windows_worktree" \
        -Output "$windows_output"
      ;;
    linux-aarch64)
      quoted_args=$(printf '%q ' \
        --repo-url "$repository_url" \
        --commit "$commit" \
        --worktree "$gx10_worktree" \
        --output "$gx10_output" \
        --platform linux-aarch64)
      ssh "$gx10_host" "bash -s -- $quoted_args" < "$script_dir/build-unix-platform.sh"
      mkdir -p "$output_root/linux-aarch64"
      scp -q -r "$gx10_host:$gx10_output/." "$output_root/linux-aarch64/"
      ;;
  esac
done

declare -a available_platforms=()
for platform in linux-x86_64 windows-x86_64 linux-aarch64; do
  build_info=$output_root/$platform/build-info.txt
  if [[ -f $build_info ]] && grep -Fq "commit=$commit" "$build_info"; then
    available_platforms+=("$platform")
  fi
done

{
  printf 'ref=%s\n' "$ref"
  printf 'commit=%s\n' "$commit"
  printf 'requested_platforms=%s\n' "${platforms[*]}"
  printf 'available_platforms=%s\n' "${available_platforms[*]}"
  printf 'completed_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$output_root/matrix-info.txt"

printf '\nAll requested builds completed from %s.\nArtifacts: %s\n' "$commit" "$output_root"
