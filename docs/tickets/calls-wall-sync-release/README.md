---
ldgr_doc: 1
kind: ticket_index_readme
id: ticket_readme.calls-wall-sync-release.v1
schema: ldgr.readme.v1
status: ready
tags:
- harbor
- ticket-index
---

# Harbor Calls, Video, and Wall Sync Ticket Decomposition

## Source spec

- Primary request: production decomposition for voice calling, video/group calling, wall host/consumer functionality, and host/consumer wall synchronization.
- Local source spec: [`source-spec.md`](source-spec.md).
- Repository evidence: `README.md`, `CHANGELOG.md`, `docs/release-readiness-review-2026-07-01.md`, `src-tauri/src/services/calling_service.rs`, `src/services/calling.ts`, `src-tauri/src/p2p/behaviour.rs`, `src/pages/Feed.tsx`, `src/pages/Wall.tsx`, `src/stores/wall.ts`, `src/stores/feed.ts`, `src-tauri/src/commands/wall_sync.rs`, and `relay-server/src/board_service.rs`.

## Summary

This decomposition treats Harbor as an existing application. The tickets extend current identity, permission, libp2p request-response, relay, SQLite, Tauri command, React, and Zustand systems. They do **not** permit mock-only, placeholder, simulated, or documentation-only completion for production behavior.

## Ticket count by epoch

- Epoch 0 — Source of Truth and Release Contract: 1
- Epoch 1 — Calling Foundation and Voice: 6
- Epoch 2 — Video and Group Calling: 5
- Epoch 3 — Wall Host Experience: 5
- Epoch 4 — Wall Consumer Experience: 4
- Epoch 5 — Wall Sync Integrity: 6
- Epoch 6 — Release Hardening: 1

Total tickets: 28

## Coverage map

| Spec area | Tickets |
| --- | --- |
| Source-of-truth / release contract | `0001`, `0601` |
| 1:1 voice calling | `0101`-`0106` |
| Video calling | `0202`, `0205` |
| Group-capable video | `0201`, `0203`, `0204`, `0205` |
| Wall host creation/visibility/media/edit/delete/social/preview | `0301`-`0305` |
| Wall consumer/feed/contact-wall interactions | `0401`-`0404` |
| Wall sync permissions/cursors/reconciliation/social/observability/validation | `0501`-`0506` |


## Tickets

- [`0001-reconcile-release-capability-contract`](epoch-0-source-of-truth/0001-reconcile-release-capability-contract.md) — Reconcile Harbor release capability contract
- [`0101-calling-signaling-transport`](epoch-1-calling-foundation/0101-calling-signaling-transport.md) — Implement signed calling signaling transport
- [`0102-call-session-state-history`](epoch-1-calling-foundation/0102-call-session-state-history.md) — Persist call session state and history
- [`0103-webrtc-audio-runtime`](epoch-1-calling-foundation/0103-webrtc-audio-runtime.md) — Implement 1:1 WebRTC audio runtime
- [`0104-calling-ui-and-events`](epoch-1-calling-foundation/0104-calling-ui-and-events.md) — Add production call UI and event handling
- [`0105-call-ice-nat-configuration`](epoch-1-calling-foundation/0105-call-ice-nat-configuration.md) — Implement ICE, STUN, and TURN configuration for calls
- [`0106-voice-call-integration-validation`](epoch-1-calling-foundation/0106-voice-call-integration-validation.md) — Validate end-to-end 1:1 voice calling
- [`0201-group-call-topology-contract`](epoch-2-video-and-group-calling/0201-group-call-topology-contract.md) — Select and document production group-call topology
- [`0202-video-call-media-runtime`](epoch-2-video-and-group-calling/0202-video-call-media-runtime.md) — Implement one-to-one video call runtime
- [`0203-group-call-signaling-membership`](epoch-2-video-and-group-calling/0203-group-call-signaling-membership.md) — Implement group-call signaling and membership control
- [`0204-group-call-media-layout-runtime`](epoch-2-video-and-group-calling/0204-group-call-media-layout-runtime.md) — Implement group call media runtime and UI
- [`0205-video-group-call-validation`](epoch-2-video-and-group-calling/0205-video-group-call-validation.md) — Validate video and group calling release readiness
- [`0301-wall-author-visibility-settings`](epoch-3-wall-host-experience/0301-wall-author-visibility-settings.md) — Implement wall author visibility controls
- [`0302-wall-media-signature-integrity`](epoch-3-wall-host-experience/0302-wall-media-signature-integrity.md) — Bind wall media to signed post integrity
- [`0303-wall-edit-delete-sync`](epoch-3-wall-host-experience/0303-wall-edit-delete-sync.md) — Synchronize wall edits and deletes
- [`0304-wall-author-social-ui`](epoch-3-wall-host-experience/0304-wall-author-social-ui.md) — Show real comments and reactions on author wall
- [`0305-wall-preview-rss-share-ui`](epoch-3-wall-host-experience/0305-wall-preview-rss-share-ui.md) — Expose wall preview, RSS, and share surfaces
- [`0401-contact-wall-view`](epoch-4-wall-consumer-experience/0401-contact-wall-view.md) — Implement contact wall consumer view
- [`0402-feed-interactions-real`](epoch-4-wall-consumer-experience/0402-feed-interactions-real.md) — Replace feed placeholder interactions with durable behavior
- [`0403-consumer-comments-reactions-ui`](epoch-4-wall-consumer-experience/0403-consumer-comments-reactions-ui.md) — Implement consumer comments and reactions on feed/contact walls
- [`0404-consumer-media-fetch-lifecycle`](epoch-4-wall-consumer-experience/0404-consumer-media-fetch-lifecycle.md) — Harden consumer media fetching and rendering lifecycle
- [`0501-relay-wall-permission-enforcement`](epoch-5-wall-sync-integrity/0501-relay-wall-permission-enforcement.md) — Enforce wall visibility and permissions through relay sync
- [`0502-wall-sync-cursors-pagination`](epoch-5-wall-sync-integrity/0502-wall-sync-cursors-pagination.md) — Implement durable wall sync cursors and pagination
- [`0503-wall-event-reconciliation-tombstones`](epoch-5-wall-sync-integrity/0503-wall-event-reconciliation-tombstones.md) — Implement wall event reconciliation and tombstones
- [`0504-wall-social-event-model`](epoch-5-wall-sync-integrity/0504-wall-social-event-model.md) — Implement signed wall social event model
- [`0505-wall-sync-status-observability`](epoch-5-wall-sync-integrity/0505-wall-sync-status-observability.md) — Expose wall sync status and observability
- [`0506-wall-sync-multi-profile-validation`](epoch-5-wall-sync-integrity/0506-wall-sync-multi-profile-validation.md) — Validate host/consumer wall synchronization
- [`0601-docs-and-release-gates-for-calls-wall-sync`](epoch-6-release-hardening/0601-docs-and-release-gates-for-calls-wall-sync.md) — Update docs and release gates for calls and wall sync

## Dependency/orchestration notes

- Start with `0001-reconcile-release-capability-contract` to lock the release contract.
- Voice calling should land in order: signaling transport (`0101`), session state (`0102`), WebRTC audio (`0103`), UI/events (`0104`), ICE/TURN config (`0105`), validation (`0106`).
- Group video is intentionally gated by `0201-group-call-topology-contract`; downstream group tickets must implement the chosen topology rather than assuming mesh or SFU.
- Wall permission enforcement (`0501`), event reconciliation (`0503`), and social event model (`0504`) are foundational for both host and consumer social UI tickets.
- Multi-profile validation tickets (`0106`, `0205`, `0506`) should run after their corresponding implementation waves and before `0601` release docs/gates.
- `planning-ticket-index.md` and `planning-graph.md` use file-backed artifact references, not LDGR artifact IDs. Record these files as LDGR artifacts before launching conduct batches that require artifact IDs.

## Validation strategy

- Use focused Vitest tests for frontend services/stores/components.
- Use focused Cargo tests for Rust services, commands, migrations, protocols, relay behavior, and reconciliation logic.
- Use two or more Harbor profiles (`HARBOR_PROFILE` or `HARBOR_DATA_DIR`) for production-path call and wall-sync validation.
- Use a local or deployed Harbor relay for relay permission, offline wall availability, and relay cursor scenarios.
- Run broad gates after relevant waves: `dev check`, `dev ci --language typescript`, and relay `cargo check`/tests/clippy as release policy requires.

## Explicit exclusions / out of scope for this decomposition

- Mobile app work.
- Screen sharing unless the release contract changes.
- Call recording.
- Rich-text collaborative wall editing.
- External telemetry/analytics.
- Cross-device syncing of local saved/hidden/snoozed feed preferences.

## Known ambiguity / required operator decision

- Group video topology is unresolved. Ticket `0201-group-call-topology-contract` must decide whether Harbor ships small-group P2P mesh, SFU, MCU, or another topology before group runtime tickets proceed.
