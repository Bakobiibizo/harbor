# Changelog

All notable Harbor changes are summarized here. Older generated merge-history entries were consolidated during release-hardening documentation cleanup so release notes do not imply unvalidated production behavior.

## 1.4.0 — calls, wall sync, contact links, and relay release hardening

### Added
- Shareable contact invite links that use `https://social-harbor.com/add-friend/...` as a browser handoff and embed the full `harbor://` contact bundle for the desktop app.
- Deep-link routing for `harbor://add-friend/...`, including single-instance forwarding and queued contact invites while the identity is locked.
- One-to-one call runtime work: signed libp2p signaling, call history, visible call UI states, configurable ICE/STUN/TURN settings, and two-profile validation documentation.
- Group-call topology contract: relay-assisted small-group full mesh with a hard 4-participant cap and no SFU/MCU/media relay behavior.
- Wall and feed sync hardening: wall visibility controls, media posts, preview/RSS/share surfaces, contact-wall/feed reads, signed comments/reactions, edit/delete reconciliation, tombstones, and relay-assisted sync paths.
- Harbor-operated community relay documentation, default relay branding, AWS SSM relay-address instructions, and in-place relay binary update workflow.
- Release validation gates for frontend TypeScript CI, Tauri/Rust CI, relay formatting/check/clippy/tests, plus documented multi-profile voice/video/group/wall validation checklists.

### Changed
- Default app relay now points at the Harbor Community Relay at `/ip4/100.49.236.191/tcp/4001/p2p/12D3KooWMfwHKfzDrZ2V3Zniw3Qu797bHrKsFKAdG9CtQiaEhbQ3`.
- Relay deployment artifacts and checksums were refreshed, and `update-relay.sh` now verifies the new binary checksum before stopping the running service.
- GitHub CI and release validation now use the bundled ARM64 `dev` binary with the current devkit command shape.
- Mock peers, mock auto-replies, and mock walls are documented as demo/test fixtures only; they are not release evidence for production calling or wall sync.
- Release notes now distinguish validated automated coverage from required interactive multi-profile evidence.

### Known limitations / deferred
- Strict/symmetric NAT media connectivity requires operator-configured TURN; Harbor libp2p relays carry signaling and app data, not WebRTC media.
- Group calls are limited to 4 total participants by ADR-0001. Larger rooms, SFU/MCU routing, server-side recording, and webinar behavior are out of scope.
- Screen sharing and mobile applications remain stretch goals until explicitly implemented and validated.
- Production readiness for voice/video/group calls and wall sync must not be claimed until the corresponding two-/three-/four-profile validation artifacts are captured.

## 1.3.0 — media and release pipeline updates

- Improved media-post handling.
- Updated relay binary packaging and hashes.
- Continued CI and release-pipeline refinement.

## 1.2.0 — v1 release follow-up

- Follow-up media-post fixes and release-candidate integration updates.

## 1.1.0 — CI and template updates

- Updated infrastructure templates and relay artifacts.
- Improved frontend/backend CI reliability.
- Fixed frontend test mock data types.

## 1.0.0 — initial v1 feature set

- Added broad backend and frontend test coverage.
- Implemented the v1 desktop application baseline: encrypted identity, peer networking, contacts, direct messaging, wall posts, feed surfaces, and polished UI.

## 0.2.0 and earlier

- Added relay/community relay support, NAT traversal improvements, feed content sync protocol groundwork, multi-user account selection, updater/signing pipeline setup, wall preview/RSS/share surfaces, and like/reaction backend support.
