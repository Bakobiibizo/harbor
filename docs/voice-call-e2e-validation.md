# 1:1 voice call end-to-end validation

This scenario validates the production voice-call path across two isolated Harbor profiles. It is intentionally not satisfied by command-signing tests alone: the evidence must show two profiles, libp2p signaling, persisted call state, WebRTC runtime state, UI state transitions, ICE exchange, connected media state, and clean hangup.

## Preconditions

- Run from the repository root.
- Use disposable profile names and data directories; do not use personal Harbor identities.
- No production relay credentials are required. The default LAN/mDNS path is acceptable; an operator-provided test TURN server may be added in Settings when validating strict NAT behavior.
- Use a machine with microphone permission available to both app windows, or a controlled virtual audio device.
- For headless Linux/Xvfb/WebKitGTK validation only, launch Harbor with `HARBOR_HEADLESS_MEDIA_CAPTURE=1` after provisioning a virtual PulseAudio source. This opt-in flag enables WebKitGTK WebRTC/media-stream settings, mirrors frontend WebRTC availability to stdout, and auto-allows WebKitGTK permission requests in the validation environment. It is not needed for normal desktop use.
- The validation host's WebKitGTK build must expose `RTCPeerConnection` to JavaScript. On Ubuntu/WebKitGTK hosts this may also require GStreamer WebRTC packages such as `gstreamer1.0-plugins-bad`, `gstreamer1.0-nice`, and `gstreamer1.0-libav`; if Harbor logs `hasRTCPeerConnection:false`, record the runtime as unsupported and use a WebKitGTK build/display stack that exposes WebRTC before continuing the manual scenario.

## Automated regression gates

Run these before and after the manual two-profile scenario:

```bash
pnpm exec vitest run src/services/voiceCallE2e.test.ts src/services/callingRuntime.test.ts src/stores/calling.test.ts
cargo test --manifest-path src-tauri/Cargo.toml offer_answer_ice_hangup_cross_libp2p_signaling_protocol
```

The Vitest regression drives two isolated frontend audio runtimes through offer, answer, ICE, connected, and hangup states. The Cargo regression drives the real libp2p request-response signaling protocol across two swarms. These are not a substitute for the two-profile app scenario below, but they prevent closing voice work with only local signing tests.

## Two-profile app scenario

1. Start two isolated Harbor instances:

   ```bash
   HARBOR_PROFILE=voice-a HARBOR_DATA_DIR=/tmp/harbor-voice-a pnpm tauri dev
   HARBOR_PROFILE=voice-b HARBOR_DATA_DIR=/tmp/harbor-voice-b pnpm tauri dev
   ```

2. In each window, create or unlock a disposable identity.
3. Start the Peer-to-Peer network in both profiles and wait until each profile shows the other peer connected. If auto-discovery does not find the peer, use the Network page's available connection/relay controls for the local test environment.
4. Exchange contact details between the two profiles and grant the contact permission that allows voice calls.
5. Open Chat in profile `voice-a`, select profile `voice-b`, and click the phone button.
6. Verify profile `voice-a` shows an outgoing/ringing call state and profile `voice-b` shows the incoming call overlay.
7. Accept the call in profile `voice-b`.
8. Confirm both profiles transition through connecting to connected. Evidence should include:
   - frontend call overlay state (`ringing`/`incoming` -> `connecting` -> `connected`),
   - backend call history/active call state,
   - `call_signaling_received` events for offer, answer, and ICE,
   - libp2p signaling request/response logs,
   - ICE connection state becoming connected.
9. Hang up from either profile.
10. Confirm both profiles leave the active call UI and the call appears in history with a terminal reason of `normal`.

## Evidence to record

Record the commands, profile names, data directories, approximate timestamps, and observed logs/events. At minimum, capture:

- `pnpm exec vitest ...` result.
- `cargo test ... offer_answer_ice_hangup_cross_libp2p_signaling_protocol` result.
- `dev check` result.
- `dev ci --language typescript` result.
- For the two-profile run: timestamps for call started, incoming offer observed, call accepted, first ICE candidate observed on each side, connected state observed on each side, hangup observed on each side.

If the two-profile scenario cannot be completed, record the failed step and queue follow-up work rather than treating automated unit tests as production readiness evidence.
