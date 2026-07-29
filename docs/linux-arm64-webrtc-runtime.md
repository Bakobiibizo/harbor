# ARM64 Linux WebRTC runtime

Harbor must not use the Ubuntu or Debian WebKitGTK package for calls unless that build exposes
`RTCPeerConnection`. WebKitGTK can report that the `enable-webrtc` setting is enabled while the
JavaScript API is absent because WebRTC was disabled when the library was compiled. Harbor's ARM64
Linux release therefore ships a private, pinned WebKitGTK 4.1 runtime.

The private runtime does not replace system packages. The package launcher sets
`LD_LIBRARY_PATH`, `WEBKIT_EXEC_PATH`, and `WEBKIT_INJECTED_BUNDLE_PATH` before the Harbor process
starts. The runtime is used by Harbor and its WebKit child processes only.

## Build the runtime

Use Ubuntu 24.04 ARM64 as the release baseline. Build on the oldest distribution supported by the
artifact, because the private WebKitGTK libraries still use GTK, GStreamer, GLib, graphics, and
codec libraries supplied by the host.

Enable source packages for the Ubuntu release, then install the WebKitGTK build dependencies and
the media components used by Harbor:

```bash
sudo sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources
sudo apt-get update
sudo apt-get build-dep -y webkit2gtk
sudo apt-get install -y cmake curl gstreamer1.0-libav gstreamer1.0-nice \
  gstreamer1.0-plugins-bad libgstreamer-plugins-bad1.0-dev ninja-build xz-utils
```

The build script does not run `sudo`. It downloads WebKitGTK 2.52.3 over HTTPS, verifies the pinned
SHA-256 digest before extraction, enables `ENABLE_WEB_RTC` and `USE_GSTREAMER_WEBRTC`, and emits a
runtime manifest used by the packager.

```bash
scripts/build-linux-webrtc-runtime.sh \
  --prefix "$PWD/build/harbor-webkitgtk" \
  --work-dir "$PWD/build/webkitgtk-source" \
  --jobs "$(nproc)"
```

The prefix must be empty. Builds never install to `/`, `/usr`, or `/usr/local`.

If WebKitGTK was built with the same pinned source and options before this script was added, verify
the completed build and create its package manifest without rebuilding:

```bash
scripts/attest-linux-webrtc-runtime.sh \
  --runtime "$HOME/.local/harbor-webkitgtk" \
  --cmake-cache "$HOME/harbor-webkit-build/webkitgtk-2.52.3/build-harbor/CMakeCache.txt" \
  --source "$HOME/harbor-webkit-build/webkitgtk-2.52.3.tar.xz"
```

Attestation fails unless the archive digest, install prefix, GTK API, WebRTC configuration, and
installed process/library layout all match.

## Package Harbor

Build Harbor on the same release baseline, then combine the executable and private runtime:

```bash
pnpm tauri build --no-bundle
SOURCE_DATE_EPOCH="$(git show -s --format=%ct HEAD)" \
  scripts/package-linux-webrtc-runtime.sh \
    --runtime "$PWD/build/harbor-webkitgtk" \
    --harbor "$PWD/src-tauri/target/release/harbor" \
    --output "$PWD/dist/Harbor_linux_aarch64_webrtc.tar.gz"
```

The archive expands to a `harbor` directory. Start it with `harbor/bin/harbor`; do not invoke
`harbor/libexec/harbor-bin` directly. The launcher fails closed if its runtime is incomplete.
Using a fixed `SOURCE_DATE_EPOCH` makes repeated packages from identical inputs byte-for-byte
reproducible.

## Clean-host release gate

Install the normal runtime dependencies on a clean Ubuntu 24.04 ARM64 host, including PipeWire or
PulseAudio and the GStreamer WebRTC, libnice, and codec plugins. Do not install a custom WebKitGTK
globally. Extract the archive and confirm that the executable resolves the packaged WebKitGTK:

```bash
LD_DEBUG=libs harbor/bin/harbor 2>&1 | grep 'harbor/runtime/lib/libwebkit2gtk-4.1.so.0'
```

Run Harbor with disposable profiles. The frontend diagnostic must report all of
`hasRTCPeerConnection`, `hasMediaDevices`, and `hasGetUserMedia` as `true`. Then complete the signed
cross-host voice scenario in [Cross-host call harness](cross-host-call-harness.md): audio capture,
offer/answer, ICE connected, remote audio, and clean hangup. A source build or a run using a library
under `/opt` is useful diagnosis but is not packaged-release evidence.

The tarball intentionally does not bundle the broad GTK/GStreamer/graphics dependency closure.
It is supported on the Ubuntu 24.04 ARM64 baseline, not as a distribution-independent Linux binary.
