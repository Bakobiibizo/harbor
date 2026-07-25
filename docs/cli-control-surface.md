# CLI control surface

Harbor provides an opt-in, authenticated loopback control socket for multi-profile validation and local automation. It is disabled unless `HARBOR_CONTROL_TOKEN` is set to at least 16 characters, and it only binds to `127.0.0.1`.

Start a controlled Harbor process:

```bash
HARBOR_PROFILE=control-a \
HARBOR_DATA_DIR=/tmp/harbor-control-a \
HARBOR_CONTROL_TOKEN='replace-with-a-long-random-token' \
HARBOR_CONTROL_PORT=19420 \
pnpm tauri dev
```

Run the client with the same token and port:

```bash
export HARBOR_CONTROL_TOKEN='replace-with-a-long-random-token'
export HARBOR_CONTROL_PORT=19420
/path/to/packaged/harborctl status
```

For automation, keep identity passwords out of argv and shell history:

```bash
export HARBOR_IDENTITY_PASSPHRASE_FILE='/secure/path/test-profile-password'
/path/to/packaged/harborctl identity-create 'Disposable test profile'
```

Both `HARBOR_CONTROL_TOKEN` and `HARBOR_IDENTITY_PASSPHRASE` support a `_FILE` alternative. Set exactly one value or file variable for each secret. Inline password arguments remain available for interactive compatibility but must not be used by automation.

Available commands:

```text
status
identity-create NAME PASS
identity-unlock PASS
identity-lock
network-start
network-stop
network-peers
network-addresses
network-connect MULTIADDR
contact-string
contact-add STRING
contact-request PEER
contact-accept PEER
contact-status PEER
permission-grant-all PEER
frontend ACTION [JSON]
shutdown
```

Frontend actions drive the same stores and WebRTC runtime used by the UI:

```bash
harborctl frontend identity.refresh
harborctl frontend state.snapshot
harborctl frontend contact.accept '{"peerId":"12D3..."}'
harborctl frontend call.start '{"peerId":"12D3...","video":true}'
harborctl frontend call.accept
harborctl frontend call.decline
harborctl frontend call.hangup
harborctl frontend group.start '{"peerIds":["12D3...","12D3..."],"video":true}'
harborctl frontend group.accept
harborctl frontend group.decline
harborctl frontend group.leave
```

On Windows, use `@file.json` for structured payloads so PowerShell/native argument parsing cannot remove JSON quotes:

```powershell
harborctl frontend group.start '@E:\harbor-validation\group-start.json'
```

The transport is newline-delimited JSON. Request command names and argument keys use snake case:

```json
{
  "id": "request-1",
  "token": "replace-with-a-long-random-token",
  "command": "contact_add",
  "contact_string": "harbor://..."
}
```

Every response echoes `id` and returns `ok`, `result`, and `error`. Treat the token as a secret. Use separate ports for concurrent profiles on the same machine.

`contact-request`, `contact-accept`, and `contact-status` are the preferred isolated validation flow once peers are connected. `contact-status` reports only whether the relationship exists and whether both directions of the call grant have converged. It does not expose grant signatures, identity keys, or contact records.
