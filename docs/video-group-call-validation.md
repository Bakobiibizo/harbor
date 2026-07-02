# Video and group call release validation

This checklist validates Harbor one-to-one video calls and the selected small-group calling scenario through production UI, Tauri commands, signed libp2p signaling, WebRTC media setup, roster/UI state, and cleanup. Passing automated tests alone is not release-ready evidence for this ticket; record the two-profile and group-call observations below when an interactive desktop session is available.

## Preconditions

- Use disposable profiles and data directories only.
- Use a machine with microphone/camera permissions available to every Harbor window, or browser/WebView fake media flags when the platform supports them.
- Do not store real camera feeds in artifacts. Prefer logs, redacted screenshots, timestamps, and state/event observations.
- Default LAN/mDNS signaling is acceptable for local validation. If peers are separated by NAT, configure a known TURN server in Settings before starting the call.
- Group calls currently use the `relay_assisted_mesh_v1` topology and are capped at **4 total participants**: the local user plus at most 3 remote peers. Larger groups must be rejected by UI/runtime errors.

## Automated regression gates

Run these before and after the manual app scenarios:

```bash
pnpm exec vitest run src/services/voiceCallE2e.test.ts src/services/callingRuntime.test.ts src/components/calling/CallOverlay.test.tsx src/stores/calling.test.ts
cargo test --manifest-path src-tauri/Cargo.toml offer_answer_ice_hangup_cross_libp2p_signaling_protocol
dev check
dev ci --language typescript
```

Expected coverage:

- frontend call runtimes: offer/answer, ICE, connected, hangup, video toggles, group participant cap, partial participant failure, and group overlay controls;
- Rust/libp2p signaling protocol: cross-swarm offer, answer, ICE, and hangup request-response flow;
- full project Rust and TypeScript gates.

## Fake-media setup where available

When the WebView/browser runtime supports flags, run test profiles with synthetic devices instead of real camera/microphone input. Examples for Chromium-based harnesses:

```bash
--use-fake-ui-for-media-stream --use-fake-device-for-media-stream
```

For Tauri desktop runs where command-line fake-device flags are unavailable or platform-specific, grant OS camera/microphone permissions to each Harbor app window and record that permission prompts were accepted. A virtual camera/audio device is acceptable.

## Two-profile one-to-one video scenario

1. Start two isolated Harbor instances with disposable data:

   ```bash
   HARBOR_PROFILE=video-a HARBOR_DATA_DIR=/tmp/harbor-video-a pnpm tauri dev
   HARBOR_PROFILE=video-b HARBOR_DATA_DIR=/tmp/harbor-video-b pnpm tauri dev
   ```

2. Create or unlock disposable identities in both profiles.
3. Start Peer-to-Peer networking in both profiles and wait until each profile sees the other peer connected.
4. Add each peer as a contact and grant call permission if the current build exposes permission controls.
5. In `video-a`, open Chat for `video-b` and click the video-call button.
6. Verify `video-a` shows outgoing/ringing state and `video-b` shows an incoming call overlay.
7. Accept from `video-b`.
8. Confirm both profiles transition through connecting to connected.
9. Toggle microphone mute and camera enable/disable from both sides; confirm UI state and media runtime state update without dropping the call.
10. Hang up from either side and confirm both profiles leave the active call UI and persist a terminal history entry with reason `normal`.

Record timestamps for: call started, incoming offer observed, answer accepted, first ICE candidate on each side, connected media state on each side, camera toggle on/off, microphone mute/unmute, and hangup on each side.

## Selected group participant scenario

Validate the release topology with four total participants: one local caller and three remote peers.

1. Start four isolated profiles:

   ```bash
   HARBOR_PROFILE=group-a HARBOR_DATA_DIR=/tmp/harbor-group-a pnpm tauri dev
   HARBOR_PROFILE=group-b HARBOR_DATA_DIR=/tmp/harbor-group-b pnpm tauri dev
   HARBOR_PROFILE=group-c HARBOR_DATA_DIR=/tmp/harbor-group-c pnpm tauri dev
   HARBOR_PROFILE=group-d HARBOR_DATA_DIR=/tmp/harbor-group-d pnpm tauri dev
   ```

2. Create/unlock disposable identities, start Peer-to-Peer networking, and ensure all participants needed by the caller are reachable.
3. From `group-a`, start a group video call with `group-b`, `group-c`, and `group-d`.
4. Verify roster tiles are created for each remote participant and move through invited/ringing/connecting/connected states independently.
5. Accept on every remote profile and confirm the caller sees all connected tiles.
6. Toggle local mute/camera controls and confirm group overlay state updates.
7. Disconnect or reject one remote participant only if performing degradation validation; confirm the failed tile is isolated and the remaining participants stay connected.
8. Leave the group call and confirm all active legs are hung up and UI state is cleaned up.
9. Attempt to exceed the cap with more than 3 remote peers, where practical, and confirm the UI/runtime shows the mesh topology cap error instead of starting the call.

Record timestamps for: group room creation, roster displayed, each offer/answer/ICE exchange, each participant connected, any degraded participant state, cap rejection, and group cleanup.

## Evidence to attach to LDGR/release notes

- Commands and pass/fail output for the automated gates.
- Profile names and data directories used.
- Approximate timestamps for the two-profile and group scenarios.
- Redacted logs or observations showing `call_signaling_received` offer/answer/ICE/hangup events.
- UI observations for outgoing/incoming/connecting/connected states, roster tiles, controls, errors, and cleanup.
- NAT/TURN configuration used, or an explicit note that local LAN/mDNS was used.

If any manual scenario cannot be completed, record the blocked step and queue follow-up validation work rather than treating unit tests as production readiness evidence.
