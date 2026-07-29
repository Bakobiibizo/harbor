#!/usr/bin/env bash
set -euo pipefail

readonly WEBKITGTK_VERSION="2.52.3"
readonly WEBKITGTK_SHA256="5b3e0d174e63dcc28848b1194e0e7448d5948c3c2427ecd931c2c5be5261aebb"
readonly WEBKITGTK_URL="https://webkitgtk.org/releases/webkitgtk-${WEBKITGTK_VERSION}.tar.xz"

usage() {
  cat <<'EOF'
Usage: scripts/build-linux-webrtc-runtime.sh --prefix PATH [options]

Build Harbor's pinned WebKitGTK 4.1 runtime with WebRTC enabled.

Options:
  --prefix PATH      Install prefix. Must not already contain files.
  --work-dir PATH    Download/build directory (default: ./build/linux-webrtc-runtime).
  --jobs N           Parallel build jobs (default: detected CPU count).
  --source PATH      Use an existing webkitgtk-2.52.3.tar.xz archive.
  --help             Show this help.

The script never invokes sudo or installs system packages. See
docs/linux-arm64-webrtc-runtime.md for build prerequisites.
EOF
}

prefix=""
work_dir="$PWD/build/linux-webrtc-runtime"
source_archive=""
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '2')"

while (($#)); do
  case "$1" in
    --prefix)
      prefix="${2:?--prefix requires a path}"
      shift 2
      ;;
    --work-dir)
      work_dir="${2:?--work-dir requires a path}"
      shift 2
      ;;
    --jobs)
      jobs="${2:?--jobs requires a number}"
      shift 2
      ;;
    --source)
      source_archive="${2:?--source requires a path}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

if [[ -z "$prefix" ]]; then
  printf '%s\n' '--prefix is required.' >&2
  exit 64
fi
if [[ ! "$jobs" =~ ^[1-9][0-9]*$ ]]; then
  printf '%s\n' '--jobs must be a positive integer.' >&2
  exit 64
fi

for command_name in cmake curl sha256sum tar xz; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Required build command is unavailable: %s\n' "$command_name" >&2
    exit 69
  fi
done

mkdir -p -- "$work_dir"
work_dir="$(cd -- "$work_dir" && pwd -P)"
mkdir -p -- "$prefix"
prefix="$(cd -- "$prefix" && pwd -P)"

if [[ "$prefix" == "/" || "$prefix" == "/usr" || "$prefix" == "/usr/local" ]]; then
  printf 'Refusing unsafe install prefix: %s\n' "$prefix" >&2
  exit 64
fi
if find "$prefix" -mindepth 1 -print -quit | grep -q .; then
  printf 'Install prefix is not empty: %s\n' "$prefix" >&2
  exit 73
fi

if [[ -z "$source_archive" ]]; then
  source_archive="$work_dir/webkitgtk-${WEBKITGTK_VERSION}.tar.xz"
  if [[ ! -f "$source_archive" ]]; then
    curl --fail --location --proto '=https' --tlsv1.2 \
      --output "$source_archive" "$WEBKITGTK_URL"
  fi
fi
source_archive="$(cd -- "$(dirname -- "$source_archive")" && pwd -P)/$(basename -- "$source_archive")"

printf '%s  %s\n' "$WEBKITGTK_SHA256" "$source_archive" | sha256sum --check --status

source_dir="$work_dir/webkitgtk-${WEBKITGTK_VERSION}"
build_dir="$work_dir/build-webkitgtk-${WEBKITGTK_VERSION}"
rm -rf -- "$source_dir" "$build_dir"
tar --extract --xz --file "$source_archive" --directory "$work_dir"

cmake -S "$source_dir" -B "$build_dir" \
  -DPORT=GTK \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DENABLE_DOCUMENTATION=OFF \
  -DENABLE_GAMEPAD=OFF \
  -DENABLE_MINIBROWSER=OFF \
  -DENABLE_SPEECH_SYNTHESIS=OFF \
  -DENABLE_WEB_RTC=ON \
  -DUSE_AVIF=OFF \
  -DUSE_GTK4=OFF \
  -DUSE_LIBBACKTRACE=OFF \
  -DUSE_GSTREAMER_WEBRTC=ON

cmake --build "$build_dir" --target install --parallel "$jobs"

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
"$script_dir/attest-linux-webrtc-runtime.sh" \
  --runtime "$prefix" \
  --cmake-cache "$build_dir/CMakeCache.txt" \
  --source "$source_archive"

printf 'WebRTC-enabled WebKitGTK %s installed at %s\n' "$WEBKITGTK_VERSION" "$prefix"
