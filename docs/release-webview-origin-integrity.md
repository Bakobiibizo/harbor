# Release webview origin integrity

Harbor release builds load the frontend from Tauri's embedded asset protocol. They do not use a
loopback web server.

| Platform | Packaged top-level origin |
| -------- | ------------------------- |
| Windows  | `https://tauri.localhost` |
| Linux    | `tauri://localhost`       |
| macOS    | `tauri://localhost`       |

Windows opts into Tauri's HTTPS custom-protocol workaround. Linux and macOS use the native
`tauri:` scheme. The main capability is local-only and grants no remote URL access.

## Why the configuration is split

Tauri's Rust runtime uses its `custom-protocol` feature to distinguish packaged and development
behavior. Rust's `--release` optimization flag alone does not select the packaged origin. When
`devUrl` lived in `tauri.conf.json`, a binary produced by a direct Cargo invocation or another
incorrect release path could still load `http://localhost:1420`. That also explains a Windows
camera or microphone prompt naming that origin: it is evidence that the running executable is not
using Harbor's bundled release origin.

`tauri.conf.json` now contains only the local `frontendDist`. `tauri.dev.conf.json` contains the
Vite URL and is injected by Harbor's package-script wrapper only for `pnpm tauri dev`. A correct
release, and even a direct Cargo build without Tauri's production feature, therefore has no
development URL available and falls back to embedded assets.

## Automated release gate

Run:

```bash
pnpm validate:release-origin
pnpm test:release-origin
```

The gate rejects a release `devUrl`, a remote `frontendDist`, an explicit window URL, remote IPC
capability access, or a Windows webview without the HTTPS scheme. Release jobs also scan the built
executable for ASCII and UTF-16 copies of the Vite origin before publication.

The packaged Linux smoke exercises real WebKitGTK IPC through the frontend control surface and
fails on origin-related diagnostics. It still requires a Linux graphical/WebKitGTK host; the
static gate does not replace that installed-runtime check.
