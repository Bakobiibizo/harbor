# Calls and wall-sync release gates

This document is the repository release gate for the calls/wall-sync release area. It complements the capability contract and the manual validation checklists; it does not replace them.

## Automated gates required for every release candidate

Before tagging, confirm `src-tauri/tauri.conf.json` points at the canonical release repository and does not contain `UPDATER_PUBKEY_PLACEHOLDER`; updater signing requirements must not be weakened or bypassed for calls/wall-sync release work.

Run from the repository root:

```bash
dev ci --language typescript
dev ci --language rust
cargo fmt --manifest-path relay-server/Cargo.toml -- --check
cargo check --manifest-path relay-server/Cargo.toml
cargo clippy --manifest-path relay-server/Cargo.toml -- -D warnings
cargo test --manifest-path relay-server/Cargo.toml
```

GitHub CI and the release workflow run these gates explicitly so frontend TypeScript, Tauri/Rust, and relay coverage cannot silently collapse to the default devkit language.

## Focused regression gates

Run the focused suites before broad gates when changing the corresponding area:

```bash
# 1:1 voice calls
pnpm exec vitest run src/services/voiceCallE2e.test.ts src/services/callingRuntime.test.ts src/stores/calling.test.ts
cargo test --manifest-path src-tauri/Cargo.toml offer_answer_ice_hangup_cross_libp2p_signaling_protocol

# video/group call UI/runtime
pnpm exec vitest run src/services/voiceCallE2e.test.ts src/services/callingRuntime.test.ts src/components/calling/CallOverlay.test.tsx src/stores/calling.test.ts
cargo test --manifest-path src-tauri/Cargo.toml offer_answer_ice_hangup_cross_libp2p_signaling_protocol

# wall/feed/contact-wall/social sync
pnpm exec vitest run src/stores/wall.test.ts src/stores/feed.test.ts src/stores/contactWall.test.ts src/services/comments.test.ts src/services/likes.test.ts
cargo test --manifest-path src-tauri/Cargo.toml content_sync wall_social p2p::types::tests::wall_sync_status_event_serializes_without_private_content
cargo test --manifest-path relay-server/Cargo.toml
```

## Manual multi-profile evidence required before production claims

Automated tests are necessary but not sufficient for release claims. Capture and attach the relevant evidence before describing these capabilities as production-ready:

- `docs/voice-call-e2e-validation.md` — two isolated Harbor profiles complete offer/answer/ICE/connected/hangup for a 1:1 voice call.
- `docs/video-group-call-validation.md` — two-profile video and selected 4-participant group-call scenarios validate media/UI/roster behavior.
- `docs/wall-sync-multi-profile-validation.md` — host, authorized consumer, and unauthorized consumer profiles validate direct/relay sync, permissions, media lifecycle, social events, edits/deletes, and offline catch-up.

If an interactive scenario cannot be run in the current environment, record the blocked step and queue follow-up work. Do not substitute screenshots, mock peer stores, or unit tests alone for the required multi-profile evidence.

## Release-note boundaries

Release notes may claim only behavior backed by the gates above:

- Mock peers, mock auto-replies, and mock walls are demo/test fixtures only.
- Harbor/libp2p relays carry app traffic and call signaling; they are not WebRTC TURN/SFU/MCU media relays.
- Strict/symmetric NAT support requires explicitly configured TURN and validation with that TURN path.
- Group calls are capped at 4 total participants by ADR-0001. Larger rooms, SFU/MCU behavior, server-side recording, mobile support, and screen sharing remain deferred until separate implementation and validation land.
