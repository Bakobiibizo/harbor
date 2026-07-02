# Changelog

All notable Harbor changes are summarized here. Older generated merge-history entries were consolidated during release-hardening documentation cleanup so release notes do not imply unvalidated production behavior.

## Unreleased — calls and wall sync release hardening

### Added
- Production-oriented one-to-one call documentation for signed libp2p signaling, WebRTC runtime expectations, call history, UI states, and ICE/STUN/TURN configuration.
- Group-call topology contract: relay-assisted small-group full mesh with a hard 4-participant cap and no SFU/MCU/media relay behavior.
- Wall host/consumer documentation for visibility controls, media posts, preview/RSS/share surfaces, contact-wall/feed reads, signed comments/reactions, edit/delete reconciliation, and relay-assisted sync.
- Release validation gates for frontend TypeScript CI, Tauri/Rust CI, relay formatting/check/clippy/tests, plus documented multi-profile voice/video/group/wall validation checklists.

### Changed
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
