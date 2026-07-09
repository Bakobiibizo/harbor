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
cargo run --manifest-path src-tauri/Cargo.toml --bin harborctl -- status
```

Available commands:

```text
status
identity-create NAME PASS
identity-unlock PASS
identity-lock
network-start
network-stop
network-peers
contact-string
contact-add STRING
permission-grant-all PEER
frontend ACTION [JSON]
shutdown
```

Frontend actions drive the same stores and WebRTC runtime used by the UI:

```bash
harborctl frontend identity.refresh
harborctl frontend call.start '{"peerId":"12D3...","mediaMode":"video"}'
harborctl frontend call.accept
harborctl frontend call.decline
harborctl frontend call.hangup
harborctl frontend group.start '{"peerIds":["12D3...","12D3..."],"mediaMode":"video"}'
harborctl frontend group.accept
harborctl frontend group.decline
harborctl frontend group.leave
```

The transport is newline-delimited JSON. Request command names and argument keys use snake case:

```json
{"id":"request-1","token":"replace-with-a-long-random-token","command":"contact_add","contact_string":"harbor://..."}
```

Every response echoes `id` and returns `ok`, `result`, and `error`. Treat the token as a secret. Use separate ports for concurrent profiles on the same machine.
