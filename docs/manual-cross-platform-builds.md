# Project-owned cross-platform builds

Harbor does not use GitHub Actions runners. Both workflow definitions are retained under
`.github/disabled-workflows/` as historical references, where GitHub cannot execute them.

Run the build matrix controller from WSL. It resolves a remote branch, tag, or SHA to one exact
commit, then builds that commit in dedicated checkouts on the following project-owned systems:

| Target | Build host | Default checkout |
| --- | --- | --- |
| Linux x86_64 | Local WSL | `~/.cache/harbor-build/linux-x86_64/repo` |
| Windows x86_64 | Windows through `powershell.exe` | `E:\apps\builds\harbor-windows\repo` |
| Linux ARM64 | `ssh gx10` | `~/.cache/harbor-build/linux-aarch64/repo` |

The controller never modifies the developer checkout on Windows, WSL, or `gx10`. It fetches the
latest remote commit for the selected ref and verifies that every checkout is detached at the same
SHA before compiling.

Preview the plan without building:

```bash
pnpm ci:platforms -- --ref main --dry-run
```

Build all supported platforms:

```bash
pnpm ci:platforms -- --ref main
```

Build one platform or a release tag:

```bash
pnpm ci:platforms -- --ref feature/live-beta-remediation --platform linux-aarch64
pnpm ci:platforms -- --ref v1.4.1-beta.8 --platform windows-x86_64
```

By default, artifacts are copied to `artifacts/manual-ci/<short-sha>/`, including `harbor`,
`harborctl`, SHA-256 checksums, and build metadata. The `artifacts/` directory is ignored by Git.

These are compile outputs, not published releases. Windows Authenticode signing, Tauri updater
signing, installer bundling, release-manifest generation, and upload remain explicit promotion
steps. macOS is not part of the matrix until project-owned macOS hardware is available.

## Host prerequisites

All hosts need Git, Node.js, pnpm, a current stable Rust toolchain, and the platform-specific Tauri
2 prerequisites. WSL must be able to launch `powershell.exe`; `gx10` must be reachable with
non-interactive SSH authentication. Override checkout or host locations with the environment
variables shown by `scripts/build-platform-matrix.sh --help`.
