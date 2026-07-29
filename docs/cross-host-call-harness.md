# Cross-host packaged call harness

`scripts/cross-host-call-harness.mjs` is the repeatable preflight for Harbor voice, video, and group-call lifecycle behavior across disposable packaged profiles. It drives the authenticated `harborctl` surface used by the application UI. It does not run `cargo`, `pnpm`, `tauri dev`, or infer a development binary.

This harness complements, but does not replace, real packaged hardware-media acceptance. A passing run proves signaling and frontend state convergence on the configured hosts. It does not prove that a human can hear audio, see video, approve an OS permission prompt, or pass Windows/macOS signing gates.

## Safety contract

- `check` only validates configuration and does not connect to or launch a host.
- `run` requires the exact `--execute-real-hosts` flag.
- Every endpoint requires explicit packaged Harbor and `harborctl` paths.
- Profile names must start with `harness-`. Disposable data paths must be absolute and contain `harbor-call-harness-`; the harness refuses broader cleanup paths.
- Control tokens and identity passwords may only come from named environment variables or token files. They are never accepted inline in JSON.
- Secret files are operator-owned by default and are never deleted. A file is deleted only when its source declares `"ownership": "harness-ephemeral"`; that opt-in is accepted only for an absolute file whose name starts with `harbor-call-harness-`. Once a real run starts, the operator has transferred ownership of that file to the harness.
- SSH endpoints require files already present on the remote host. This prevents secrets entering local SSH arguments or the remote command text.
- Identity passwords are supplied to `harborctl` through `HARBOR_IDENTITY_PASSPHRASE` or `HARBOR_IDENTITY_PASSPHRASE_FILE`, not argv.
- Application stdout and stderr remain inside each disposable profile and are deleted during cleanup. Retained evidence is an allowlisted lifecycle summary.
- Cleanup runs in reverse endpoint order after success, command failure, convergence timeout, or scenario failure. It stops networking, requests shutdown, terminates the recorded process, removes the isolated data root, deletes every harness-owned ephemeral secret file, and verifies those paths no longer exist. A failed deletion makes cleanup and the run fail.

Generate unique random control tokens and passwords for every profile. Do not use personal Harbor identities or reuse a normal Harbor data directory. Prefer files marked `harness-ephemeral` for one-run credentials. Environment variables and files without that marker remain operator-owned; unset or remove them yourself after the run.

## Supported endpoint adapters

| `kind`               | Execution path                                     | Secret source                                        |
| -------------------- | -------------------------------------------------- | ---------------------------------------------------- |
| `local-wsl`          | Direct Linux process launched from the current WSL | Environment variable or local file                   |
| `windows-powershell` | Native Windows process launched through PowerShell | Inherited environment variable or Windows-local file |
| `remote-linux-ssh`   | Remote packaged process launched through `ssh`     | Remote host-local file only                          |

Each endpoint has its own profile, data directory, control port, Harbor binary, `harborctl` binary, token source, and password source. Use `dialAddress` when mDNS cannot bridge hosts. It must be a reachable libp2p multiaddress containing the literal `{peerId}` placeholder. Harbor normally chooses a random listen port, so `/ip4/203.0.113.20/tcp/{tcpPort}/p2p/{peerId}` is the usual form; the harness resolves `{tcpPort}` from that profile's `network-addresses` response after startup. When `dialAddress` is omitted, the harness waits for normal relay or mDNS connectivity.

The secret-file ownership forms are:

```json
{ "file": "/secure/operator-password", "ownership": "operator" }
{ "file": "/run/user/1000/harbor-call-harness-password", "ownership": "harness-ephemeral" }
```

`ownership` defaults to `operator`. Do not mark a reusable credential or a path managed by another secret service as `harness-ephemeral`. The harness removes only the exact opted-in file, never its parent directory. SSH cleanup happens on the remote host and Windows cleanup happens through PowerShell on Windows.

For a Linux host launched outside its desktop login shell, set `runtimeEnvironment` to the non-secret session variables needed by the packaged app. The harness accepts only `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`, `GDK_BACKEND`, `PULSE_SERVER`, `PIPEWIRE_REMOTE`, and the three WebKit runtime path variables. It rejects Harbor control variables and arbitrary environment keys. A gx10-style Wayland launch normally needs `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`, and `GDK_BACKEND`; a private WebKitGTK package also needs `LD_LIBRARY_PATH`, `WEBKIT_EXEC_PATH`, and `WEBKIT_INJECTED_BUNDLE_PATH`.

See [the example configuration](examples/cross-host-call-harness.config.json). Paths in that file are placeholders and must point to the exact packaged candidate under test.

## Commands

Validate the file without reading secrets or contacting hosts:

```bash
node scripts/cross-host-call-harness.mjs check \
  --config docs/examples/cross-host-call-harness.config.json
```

Run the real hosts only after verifying paths, disposable data roots, OS media permissions, reachability, and secret sources:

```bash
node scripts/cross-host-call-harness.mjs run \
  --config /secure/path/call-harness.json \
  --execute-real-hosts
```

Run the deterministic fake-adapter regression without launching Harbor, PowerShell, or SSH:

```bash
node --test scripts/cross-host-call-harness.node-test.mjs
```

## Provisioning and assertions

The harness performs these bounded operations:

1. Clears and launches three isolated packaged profiles.
2. Creates disposable identities without putting passwords in process arguments.
3. Starts networking, waits for the frontend control listener, and converges peer connectivity.
4. Exchanges signed contact requests directly by connected peer ID, accepts each request, and verifies both issued and received call grants on both profiles.
5. Runs and tears down a one-to-one voice call.
6. Runs and tears down a reverse-direction video call.
7. Lets a call expire, attempts a late answer, and verifies it cannot reconnect the expired call.
8. Starts a three-profile video group, removes one participant's network, verifies the remaining leg stays connected while the failed leg degrades, and leaves the room.
9. Starts a clean one-to-one call after group cleanup.
10. Stops networks, shuts down all apps, terminates any remaining recorded processes, removes all disposable roots, and removes all explicitly harness-owned secret files.

Every control command and state poll has a deadline. A state that does not converge is a failed run, not an indefinite wait.

## Evidence and privacy

Set `evidenceFile` to retain a JSON result. The document contains endpoint labels and adapter kinds, scenario outcomes, safe state names, timestamps, cleanup outcomes, and a redacted failure category/message. It intentionally omits:

- tokens, passwords, TURN credentials, and private keys;
- raw identity keys, contact bundles, contact graphs, and peer IDs;
- SDP, ICE credentials/candidates, DTLS fingerprints, signatures, and group nonces;
- media streams and application stdout/stderr.

Do not attach the disposable profile logs as evidence. If deeper debugging is necessary, inspect them locally before cleanup with a separately reviewed procedure and never publish raw signaling output.

## Required packaged `harborctl` contract

The packaged `harborctl` paired with the app must expose:

```text
status
identity-create NAME
network-start
network-stop
network-peers
network-connect MULTIADDR
contact-request PEER
contact-accept PEER
contact-status PEER
frontend state.snapshot
frontend call.start|call.accept|call.hangup
frontend group.start|group.accept|group.leave
shutdown
```

`contact-status` returns only relationship and call-grant booleans. The harness consumes complete control responses in memory but persists only the allowlisted evidence described above.
