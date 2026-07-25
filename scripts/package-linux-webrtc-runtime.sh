#!/usr/bin/env bash
set -euo pipefail
umask 022

usage() {
  cat <<'EOF'
Usage: scripts/package-linux-webrtc-runtime.sh --runtime PATH --harbor PATH --output FILE

Create a relocatable Harbor Linux tarball containing the Harbor executable and
the pinned, WebRTC-enabled WebKitGTK runtime. FILE must end in .tar.gz.

Set SOURCE_DATE_EPOCH to a Unix timestamp for byte-for-byte reproducible output.
EOF
}

runtime=""
harbor_binary=""
output=""

while (($#)); do
  case "$1" in
    --runtime)
      runtime="${2:?--runtime requires a path}"
      shift 2
      ;;
    --harbor)
      harbor_binary="${2:?--harbor requires a path}"
      shift 2
      ;;
    --output)
      output="${2:?--output requires a path}"
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

if [[ -z "$runtime" || -z "$harbor_binary" || -z "$output" ]]; then
  usage >&2
  exit 64
fi
if [[ "$output" != *.tar.gz ]]; then
  printf '%s\n' '--output must end in .tar.gz.' >&2
  exit 64
fi
if [[ ! -x "$harbor_binary" ]]; then
  printf 'Harbor executable is missing or not executable: %s\n' "$harbor_binary" >&2
  exit 66
fi
if ! command -v readelf >/dev/null 2>&1; then
  printf '%s\n' 'readelf is required to verify package architecture.' >&2
  exit 69
fi

runtime="$(cd -- "$runtime" && pwd -P)"
harbor_binary="$(cd -- "$(dirname -- "$harbor_binary")" && pwd -P)/$(basename -- "$harbor_binary")"
output_dir="$(dirname -- "$output")"
mkdir -p -- "$output_dir"
output_dir="$(cd -- "$output_dir" && pwd -P)"
output="$output_dir/$(basename -- "$output")"

required_paths=(
  "lib/libwebkit2gtk-4.1.so.0"
  "lib/libjavascriptcoregtk-4.1.so.0"
  "lib/webkit2gtk-4.1/injected-bundle/libwebkit2gtkinjectedbundle.so"
  "libexec/webkit2gtk-4.1/WebKitWebProcess"
  "libexec/webkit2gtk-4.1/WebKitNetworkProcess"
  "libexec/webkit2gtk-4.1/WebKitGPUProcess"
  "share/harbor/webrtc-runtime.env"
)
for required_path in "${required_paths[@]}"; do
  if [[ ! -e "$runtime/$required_path" ]]; then
    printf 'WebRTC runtime is incomplete: %s is missing.\n' "$runtime/$required_path" >&2
    exit 66
  fi
done
if ! grep -qx 'ENABLE_WEB_RTC=ON' "$runtime/share/harbor/webrtc-runtime.env" || \
   ! grep -qx 'USE_GSTREAMER_WEBRTC=ON' "$runtime/share/harbor/webrtc-runtime.env"; then
  printf '%s\n' 'Runtime manifest does not attest WebRTC support.' >&2
  exit 65
fi

elf_architecture() {
  local machine
  machine="$(LC_ALL=C readelf -h "$1" 2>/dev/null | awk -F: '/^[[:space:]]*Machine:/{sub(/^[[:space:]]+/, "", $2); print $2; exit}')"
  case "$machine" in
    AArch64) printf '%s\n' 'aarch64' ;;
    *X86-64) printf '%s\n' 'x86_64' ;;
    *)
      printf 'Unsupported or invalid ELF machine for %s: %s\n' "$1" "${machine:-unknown}" >&2
      return 1
      ;;
  esac
}

harbor_architecture="$(elf_architecture "$harbor_binary")"
runtime_elf_paths=(
  "lib/libwebkit2gtk-4.1.so.0"
  "lib/libjavascriptcoregtk-4.1.so.0"
  "lib/webkit2gtk-4.1/injected-bundle/libwebkit2gtkinjectedbundle.so"
  "libexec/webkit2gtk-4.1/WebKitWebProcess"
  "libexec/webkit2gtk-4.1/WebKitNetworkProcess"
  "libexec/webkit2gtk-4.1/WebKitGPUProcess"
)
for runtime_elf_path in "${runtime_elf_paths[@]}"; do
  runtime_architecture="$(elf_architecture "$runtime/$runtime_elf_path")"
  if [[ "$harbor_architecture" != "$runtime_architecture" ]]; then
    printf 'Harbor is %s but %s is %s.\n' \
      "$harbor_architecture" "$runtime_elf_path" "$runtime_architecture" >&2
    exit 65
  fi
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
launcher="$script_dir/linux-webrtc/harbor"
test -x "$launcher"

stage="$(mktemp -d "$output_dir/.harbor-linux-package.XXXXXXXX")"
trap 'rm -rf -- "$stage"' EXIT
package_root="$stage/harbor"
mkdir -p -- "$package_root/bin" "$package_root/libexec" "$package_root/runtime"
install -m 0755 -- "$launcher" "$package_root/bin/harbor"
install -m 0755 -- "$harbor_binary" "$package_root/libexec/harbor-bin"
cp -a -- "$runtime/lib" "$runtime/libexec" "$package_root/runtime/"
mkdir -p -- "$package_root/runtime/share/harbor"
install -m 0644 -- "$runtime/share/harbor/webrtc-runtime.env" \
  "$package_root/runtime/share/harbor/webrtc-runtime.env"

source_date_epoch="${SOURCE_DATE_EPOCH:-0}"
if [[ ! "$source_date_epoch" =~ ^[0-9]+$ ]]; then
  printf '%s\n' 'SOURCE_DATE_EPOCH must be a non-negative integer.' >&2
  exit 64
fi

harbor_sha256="$(sha256sum "$package_root/libexec/harbor-bin" | cut -d ' ' -f 1)"
webkit_sha256="$(sha256sum "$package_root/runtime/lib/libwebkit2gtk-4.1.so.0" | cut -d ' ' -f 1)"
cat > "$package_root/MANIFEST" <<EOF
HARBOR_LINUX_PACKAGE_FORMAT=1
HARBOR_LINUX_ARCHITECTURE=$harbor_architecture
HARBOR_EXECUTABLE_SHA256=$harbor_sha256
WEBKITGTK_LIBRARY_SHA256=$webkit_sha256
EOF

temporary_output="$output.tmp.$$"
trap 'rm -rf -- "$stage"; rm -f -- "$temporary_output"' EXIT
tar --sort=name --mtime="@$source_date_epoch" --owner=0 --group=0 --numeric-owner \
  --directory "$stage" --create harbor | gzip -n > "$temporary_output"
mv -f -- "$temporary_output" "$output"

printf 'Created %s\n' "$output"
