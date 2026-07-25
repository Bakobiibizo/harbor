#!/usr/bin/env bash
set -euo pipefail

readonly WEBKITGTK_VERSION="2.52.3"
readonly WEBKITGTK_SHA256="5b3e0d174e63dcc28848b1194e0e7448d5948c3c2427ecd931c2c5be5261aebb"

usage() {
  cat <<'EOF'
Usage: scripts/attest-linux-webrtc-runtime.sh --runtime PATH --cmake-cache FILE --source FILE

Verify a completed WebKitGTK build and write the runtime manifest required by
the Harbor packager. The source archive must be the pinned WebKitGTK 2.52.3
archive; the CMake cache must name this exact install prefix and enable WebRTC.
EOF
}

runtime=""
cmake_cache=""
source_archive=""
while (($#)); do
  case "$1" in
    --runtime)
      runtime="${2:?--runtime requires a path}"
      shift 2
      ;;
    --cmake-cache)
      cmake_cache="${2:?--cmake-cache requires a file}"
      shift 2
      ;;
    --source)
      source_archive="${2:?--source requires a file}"
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

if [[ -z "$runtime" || -z "$cmake_cache" || -z "$source_archive" ]]; then
  usage >&2
  exit 64
fi
if [[ ! -d "$runtime" || ! -f "$cmake_cache" || ! -f "$source_archive" ]]; then
  printf '%s\n' 'Runtime, CMake cache, or source archive is missing.' >&2
  exit 66
fi

runtime="$(cd -- "$runtime" && pwd -P)"
cmake_cache="$(cd -- "$(dirname -- "$cmake_cache")" && pwd -P)/$(basename -- "$cmake_cache")"
source_archive="$(cd -- "$(dirname -- "$source_archive")" && pwd -P)/$(basename -- "$source_archive")"

printf '%s  %s\n' "$WEBKITGTK_SHA256" "$source_archive" | sha256sum --check --status

required_cache_values=(
  "CMAKE_INSTALL_PREFIX:PATH=$runtime"
  "ENABLE_WEB_RTC:BOOL=ON"
  "PORT:STRING=GTK"
  "USE_GSTREAMER_WEBRTC:BOOL=ON"
  "USE_GTK4:BOOL=OFF"
)
for cache_value in "${required_cache_values[@]}"; do
  if ! grep -Fqx "$cache_value" "$cmake_cache"; then
    printf 'CMake cache does not contain required setting: %s\n' "$cache_value" >&2
    exit 65
  fi
done

required_runtime_paths=(
  "lib/libwebkit2gtk-4.1.so.0"
  "lib/libjavascriptcoregtk-4.1.so.0"
  "lib/webkit2gtk-4.1/injected-bundle/libwebkit2gtkinjectedbundle.so"
  "libexec/webkit2gtk-4.1/WebKitWebProcess"
  "libexec/webkit2gtk-4.1/WebKitNetworkProcess"
  "libexec/webkit2gtk-4.1/WebKitGPUProcess"
)
for runtime_path in "${required_runtime_paths[@]}"; do
  if [[ ! -e "$runtime/$runtime_path" ]]; then
    printf 'Installed runtime is incomplete: %s\n' "$runtime/$runtime_path" >&2
    exit 66
  fi
done

manifest_dir="$runtime/share/harbor"
mkdir -p -- "$manifest_dir"
cat > "$manifest_dir/webrtc-runtime.env" <<EOF
HARBOR_WEBKIT_RUNTIME_FORMAT=1
WEBKITGTK_VERSION=$WEBKITGTK_VERSION
WEBKITGTK_SOURCE_SHA256=$WEBKITGTK_SHA256
WEBKITGTK_API=4.1
ENABLE_WEB_RTC=ON
USE_GSTREAMER_WEBRTC=ON
USE_GTK4=OFF
EOF

printf 'Attested WebRTC-enabled WebKitGTK runtime at %s\n' "$runtime"
