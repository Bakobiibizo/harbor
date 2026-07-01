# Harbor Release Capability Contract — Calls, Video, Wall, Feed, and Sync

**Status:** source-of-truth contract for the `calls-wall-sync-20260701-r3` release decomposition.
**Date:** 2026-07-01
**Scope owner ticket:** `ticket.0001-reconcile-release-capability-contract`
**Downstream docs correction ticket:** `ticket.0601-docs-and-release-gates-for-calls-wall-sync`

This document reconciles current source behavior, existing documentation, and the conduct ticket index. It is intentionally a contract, not an implementation patch: simulated, mock-only, or local-only behavior is not production-complete.

## Baseline evidence

The selected baseline is the existing Harbor Tauri/React/Rust/libp2p/SQLite/Zustand architecture. The contract extends the current implementation instead of redefining Harbor as a greenfield system.

Evidence inspected while reconciling this contract:

- `README.md`
- `SECURITY.md`
- `CHANGELOG.md`
- Parent conduct artifacts:
  - `run-3-1782931315265793072-source-spec.md`
  - `run-3-1782931405491008732-planning-ticket-index.md`
  - `run-3-1782931405498811124-planning-graph.md`
- Calling source:
  - `src-tauri/src/services/calling_service.rs`
  - `src-tauri/src/commands/calling.rs`
  - `src/services/calling.ts`
  - `src-tauri/src/p2p/protocols/mod.rs`
  - `src-tauri/src/p2p/behaviour.rs`
- Wall/feed/sync source:
  - `src/pages/Wall.tsx`
  - `src/pages/Feed.tsx`
  - `src/stores/wall.ts`
  - `src/stores/feed.ts`
  - `src/services/feed.ts`
  - `src-tauri/src/commands/posts.rs`
  - `src-tauri/src/services/feed_service.rs`
  - `src-tauri/src/services/content_sync_service.rs`
  - `src-tauri/src/commands/wall_sync.rs`
  - `relay-server/src/board_service.rs`

## Selected release contract

### Required for production-complete release claims

Harbor may claim production readiness for this release area only when all required items below have passed their mapped implementation and validation tickets:

1. **1:1 voice calling** is production-complete end to end.
2. **1:1 video calling** is production-complete end to end.
3. **Wall host views** are production-complete for local-first authoring, visibility, media integrity, edit/delete, preview/share/RSS surfaces, and real social state.
4. **Wall consumer views** are production-complete for feed and contact-wall reading, permissions, media retrieval, comments/reactions, and durable feed actions.
5. **Wall sync** is production-complete across direct P2P and relay-assisted paths, including permission enforcement, cursors, tombstones, media, social events, status, and multi-profile validation.
6. **Documentation and release gates** reflect the above precisely and do not describe placeholder, mock, local-only, or simulated behavior as complete.

### Optional group video contract

Group video is **optional** for the release. Harbor must not claim group video or group-call readiness until `ticket.0201-group-call-topology-contract` selects and documents the production topology and the dependent group implementation/validation tickets pass.

If group video is excluded from the release, `ticket.0601-docs-and-release-gates-for-calls-wall-sync` must document it as an explicit out-of-scope/future feature. No group topology is assumed here.

## Production-required behavior by area

### 1:1 voice calling

Production voice calling requires all of the following:

- Signed offer, answer, ICE, and hangup messages are transported over a registered libp2p signaling protocol, not only created/validated through local Tauri helpers.
- Network commands and events carry outgoing and incoming signaling across peers and emit lifecycle events to the frontend.
- `Capability::Call` checks protect both call initiation and incoming-call acceptance.
- Call session state is persisted, including ringing/incoming/connected/ended state, timestamps, and end reasons.
- A real WebRTC audio runtime uses microphone capture, `RTCPeerConnection`, SDP exchange, ICE candidate exchange, remote audio playback, hangup, and error handling.
- ICE/STUN/TURN configuration is user-configurable where required and does not silently depend on LAN-only success.
- The UI exposes incoming/outgoing/active call states, permission failures, media errors, and network errors.
- End-to-end validation demonstrates two real Harbor profiles completing a 1:1 voice call through the supported network path.

Current source status: signed helper payloads exist, but production calling is incomplete because signaling transport, WebRTC runtime, call UI/events, and end-to-end validation are absent.

### 1:1 video calling

Production 1:1 video requires all voice-call requirements plus:

- Camera capture and video track negotiation are implemented in the WebRTC runtime.
- Local preview and remote video rendering are available in the call UI.
- Audio-only fallback, camera permission denial, device selection failures, and hangup/error transitions are handled.
- ICE/TURN behavior is validated for video media, not only signed SDP helpers.
- Multi-profile validation covers video offer/answer, media flow, and teardown.

Current source status: no production video runtime or UI is present.

### Optional group video

Production group video may ship only if the release explicitly includes it after the topology decision. Required behavior if included:

- A documented topology is selected before implementation: mesh, SFU/relay-assisted, or another explicit design.
- Membership and permissions are signed and enforced for every participant.
- Group signaling handles join, leave, renegotiation, participant failure, and call teardown.
- The UI renders participant membership and media state without relying on mock participants.
- Validation covers at least the selected minimum participant count and failure cases.

Current source status: group video topology is unresolved and must not be assumed.

### Wall host views

Production host wall behavior requires:

- Authors can create text/thought/image/video/audio/shared posts with explicit visibility controls instead of hidden hard-coded visibility.
- Local creation remains local-first and must not require relay availability.
- Media metadata that affects rendered content is bound to signed post integrity, or the release must explicitly document the signed event chain that protects it.
- Edit and delete operations propagate through the same sync model as post creation and are represented as durable events/tombstones.
- Author wall comments/reactions/like counts are real, durable state, not hard-coded defaults or local-only toggles.
- Preview, RSS, and share surfaces are reachable from production UI and follow the selected visibility contract.
- Placeholder messages such as “coming soon” are removed from release-claimed host functionality.

Current source status: local post/media authoring, visibility controls, and preview/RSS/share UI exist. Media signature integrity, durable social state, and edit/delete sync remain unfinished.

### Wall consumer views

Production consumer wall/feed behavior requires:

- Feed and contact-wall views show only posts the viewer is authorized to read.
- Contacts-only posts require author-granted `WallRead`; being an active contact alone is not sufficient unless represented by a signed grant.
- A dedicated contact-wall consumer path uses backend `getWall`/sync surfaces instead of relying only on the aggregate feed.
- Feed actions such as Like, Save, Hide, Snooze, Share, Comment, and media display are durable where release-claimed; unsupported actions are either removed or explicitly labelled out of scope.
- Comments and reactions use signed/syncable social events and are visible consistently between host and consumer views.
- Media fetch lifecycle handles missing local files, relay/direct fetch attempts, unavailable authors, progress/error states, and safe rendering.
- Mock peers and mock walls remain demo fixtures only and cannot satisfy production validation.

Current source status: aggregate feed and comments surfaces exist, but contact-wall view, durable feed interactions, syncable social events, and media lifecycle hardening remain unfinished.

### Wall sync

Production wall sync requires:

- Direct P2P sync and relay-assisted sync enforce the same visibility and permission rules.
- Relay reads for contacts-only posts require an author-granted `WallRead` proof or another documented permission mechanism; requester signatures alone are insufficient.
- Sync uses durable per-author cursors and pagination instead of fetching all contacts from cursor `0` on every relay sync.
- Post updates/deletes are reconciled through durable events/tombstones across local DB, direct P2P, relay storage, feed, and contact-wall views.
- Media metadata and bytes are synchronized with integrity checks and lifecycle status.
- Social events such as comments/reactions are signed, stored, synchronized, and reconciled consistently.
- Sync status and errors are observable by users and tests.
- Multi-profile validation demonstrates host/consumer synchronization through the selected supported paths.

Current source status: direct content sync and relay wall storage exist, but relay permission enforcement, cursor correctness, tombstones, media/social sync completeness, status, and validation are incomplete.

## Ticket mapping for unfinished or placeholder behavior

Every unfinished or placeholder behavior identified in the source-spec is mapped below to a conduct ticket, or explicitly excluded.

| Unfinished / placeholder behavior | Contract disposition | Ticket(s) |
| --- | --- | --- |
| Calling has signed local helper payloads but no libp2p signaling request-response behavior. | Required for 1:1 voice/video. | `ticket.0101-calling-signaling-transport` |
| Calling has no network command/event path for incoming/outgoing signaling. | Required for call lifecycle. | `ticket.0101-calling-signaling-transport`, `ticket.0104-calling-ui-and-events` |
| Call state/history is represented by helper structs but not durable production session history. | Required for 1:1 voice/video. | `ticket.0102-call-session-state-history` |
| No real WebRTC audio runtime (`getUserMedia`/`RTCPeerConnection`/remote playback). | Required for 1:1 voice. | `ticket.0103-webrtc-audio-runtime` |
| No production call UI or Tauri event handling for incoming/active calls. | Required for 1:1 voice/video. | `ticket.0104-calling-ui-and-events` |
| ICE/STUN/TURN/NAT behavior for calls is not productized or validated. | Required for 1:1 voice/video. | `ticket.0105-call-ice-nat-configuration` |
| No end-to-end 1:1 voice validation with two profiles. | Release blocker for voice claim. | `ticket.0106-voice-call-integration-validation` |
| Group-call topology is undecided. | Required only before claiming group video; no topology assumed. | `ticket.0201-group-call-topology-contract` |
| No 1:1 video media runtime/UI. | Required for 1:1 video claim. | `ticket.0202-video-call-media-runtime`, `ticket.0205-video-group-call-validation` |
| No group signaling/membership model. | Optional; required only if group video ships. | `ticket.0203-group-call-signaling-membership` |
| No group media layout/runtime/UI. | Optional; required only if group video ships. | `ticket.0204-group-call-media-layout-runtime` |
| No video/group multi-profile validation. | Required for any video/group claim. | `ticket.0205-video-group-call-validation` |
| Wall composer hard-codes contacts visibility and lacks explicit production visibility controls. | Required for host wall. | `ticket.0301-wall-author-visibility-settings` |
| Wall post signatures do not currently bind post media metadata added after creation. | Required for host/consumer media integrity. | `ticket.0302-wall-media-signature-integrity` |
| Wall edit/delete operations do not propagate as synchronized durable events/tombstones. | Required for host/consumer sync. | `ticket.0303-wall-edit-delete-sync`, `ticket.0503-wall-event-reconciliation-tombstones` |
| Host wall comments/reactions/like counts are defaulted or local-only. | Required for host wall if social features are release-claimed. | `ticket.0304-wall-author-social-ui`, `ticket.0504-wall-social-event-model` |
| Preview/RSS/share surfaces must remain exposed through production UI with public-only RSS filtering. | Implemented by `ticket.0305-wall-preview-rss-share-ui`; keep covered by release tests/docs. | `ticket.0305-wall-preview-rss-share-ui` |
| No dedicated contact-wall consumer route/view. | Required for consumer wall. | `ticket.0401-contact-wall-view` |
| Feed Like/Save/Hide/Snooze interactions are placeholder/local-only where release-claimed. | Required for feed release claim. | `ticket.0402-feed-interactions-real` |
| Consumer comments/reactions are not syncable production social events. | Required for consumer social behavior. | `ticket.0403-consumer-comments-reactions-ui`, `ticket.0504-wall-social-event-model` |
| Consumer media fetch/rendering does not expose a complete lifecycle for missing/unavailable media. | Required for media feed/contact-wall release claim. | `ticket.0404-consumer-media-fetch-lifecycle` |
| Relay wall reads verify requester signature but do not enforce author grants for contacts-only posts. | Release blocker for wall sync privacy. | `ticket.0501-relay-wall-permission-enforcement` |
| Relay feed sync starts from cursor `0` for each active contact instead of durable per-author cursors. | Release blocker for scalable/correct sync. | `ticket.0502-wall-sync-cursors-pagination` |
| Wall update/delete reconciliation is incomplete across local/direct/relay paths. | Required for sync correctness. | `ticket.0503-wall-event-reconciliation-tombstones` |
| Wall comments/reactions/likes are not signed syncable social events. | Required for production social features. | `ticket.0504-wall-social-event-model` |
| Wall sync status/errors are not visible enough for users/tests. | Required for supportable sync. | `ticket.0505-wall-sync-status-observability` |
| No multi-profile validation for host/consumer wall sync. | Release blocker for wall sync claim. | `ticket.0506-wall-sync-multi-profile-validation` |
| README/SECURITY/CHANGELOG overclaims or drift from this contract. | Must be corrected after implementation decisions without silently changing scope. | `ticket.0601-docs-and-release-gates-for-calls-wall-sync` |
| Mock peers, mock walls, and mock auto-replies. | Demo-only fixture; never production evidence. Keep only if labelled demo/dev. | Explicit exclusion from production completion; docs labelling via `ticket.0601-docs-and-release-gates-for-calls-wall-sync` if retained. |
| Mobile app, call recording, screen sharing, rich-text collaborative editing, external analytics/telemetry. | Out of scope for this release contract. | Explicit exclusion per source-spec. |

## Source-spec coverage matrix

| Source-spec section | Contract coverage | Ticket / exclusion |
| --- | --- | --- |
| Current documented intent | Reconciled in “Selected release contract” and “Documentation drift to correct later”. | `ticket.0601-docs-and-release-gates-for-calls-wall-sync` |
| Calling | 1:1 voice requirements, 1:1 video requirements, and current incomplete status. | `ticket.0101-calling-signaling-transport`, `ticket.0102-call-session-state-history`, `ticket.0103-webrtc-audio-runtime`, `ticket.0104-calling-ui-and-events`, `ticket.0105-call-ice-nat-configuration`, `ticket.0106-voice-call-integration-validation`, `ticket.0202-video-call-media-runtime`, `ticket.0205-video-group-call-validation` |
| Wall host behavior | Host wall production requirements and unfinished mapping. | `ticket.0301-wall-author-visibility-settings`, `ticket.0302-wall-media-signature-integrity`, `ticket.0303-wall-edit-delete-sync`, `ticket.0304-wall-author-social-ui`, `ticket.0305-wall-preview-rss-share-ui`, `ticket.0503-wall-event-reconciliation-tombstones`, `ticket.0504-wall-social-event-model` |
| Wall consumer/feed behavior | Consumer/feed production requirements and unfinished mapping. | `ticket.0401-contact-wall-view`, `ticket.0402-feed-interactions-real`, `ticket.0403-consumer-comments-reactions-ui`, `ticket.0404-consumer-media-fetch-lifecycle`, `ticket.0504-wall-social-event-model` |
| Wall sync | Direct/relay sync production requirements and unfinished mapping. | `ticket.0501-relay-wall-permission-enforcement`, `ticket.0502-wall-sync-cursors-pagination`, `ticket.0503-wall-event-reconciliation-tombstones`, `ticket.0504-wall-social-event-model`, `ticket.0505-wall-sync-status-observability`, `ticket.0506-wall-sync-multi-profile-validation` |
| Non-negotiable constraints | Preserved in baseline and release blocker sections. | Applies to all implementation tickets. |
| Explicit exclusions | Preserved in out-of-scope mapping. | Explicit exclusion unless contract changes. |
| Optional group video | No topology assumed; decision required before implementation. | `ticket.0201-group-call-topology-contract`; optional dependent `ticket.0203-group-call-signaling-membership`, `ticket.0204-group-call-media-layout-runtime`, and `ticket.0205-video-group-call-validation` |

## Documentation drift to correct later

These are identified for later correction. This ticket does **not** silently change feature scope by editing README, SECURITY, or CHANGELOG claims.

### `README.md`

- Feature list says **“Voice Calling: WebRTC signaling through libp2p”**. Current source only has signed Tauri helper payloads and a declared protocol constant; there is no registered signaling request-response behavior, WebRTC runtime, or production UI. Correct after `ticket.0101`–`ticket.0106`.
- Usage guide says users can click a phone icon to initiate a voice call “if supported”. Current production call UI/runtime is absent. Correct or gate behind implementation in `ticket.0104`/`ticket.0106`.
- Architecture and protocol sections list voice signaling messages as though they are part of the runtime protocol surface. Correct after signaling transport lands in `ticket.0101`.
- Roadmap marks **Voice calling (signaling)** as completed. This may remain only if explicitly scoped to signed helper generation/validation; it must not imply production voice calls before `ticket.0106`.
- Wall/feed language says posts are shared with contacts who have permission and feed shows contacts who granted `WallRead`. This is the selected contract, but source currently allows active-contact inclusion and relay reads without author-grant enforcement. Keep the contract language but update release status only after `ticket.0501` and `ticket.0506`.
- Roadmap marks wall/blog posts with media and feed aggregation as complete. Clarify which pieces are local/backend-complete versus production sync/social complete after wall tickets finish.

### `SECURITY.md`

- Supported versions list only `0.1.x`, while `CHANGELOG.md` contains later `v1.x` releases and this contract targets release-completion work. Decide/update supported versions in `ticket.0601-docs-and-release-gates-for-calls-wall-sync`.
- Security limitations should be revisited after call work lands to accurately describe call metadata, STUN/TURN/relay exposure, and WebRTC media limitations. This is a docs release-gate item, not a new feature decision.

### `CHANGELOG.md`

- Changelog entries are duplicated/noisy and can imply completed release-quality work for feed content sync, wall preview/RSS, likes, relay/community, and v1 feature enhancements. `ticket.0601-docs-and-release-gates-for-calls-wall-sync` must distinguish implemented local/backend pieces from production-complete claims validated by this contract.
- Existing release notes should not claim production-complete voice, video, group calls, wall sync privacy, or syncable social behavior until the mapped validation tickets pass.

## Release blockers

A release may not claim production completion for this scope while any of the following remain true:

- 1:1 voice lacks transport, WebRTC runtime, UI/events, ICE configuration, or end-to-end validation.
- 1:1 video lacks media runtime/UI or validation.
- Group video is claimed before `ticket.0201` chooses topology and group implementation/validation tickets pass.
- Wall host/consumer views rely on placeholder “coming soon” interactions or local-only social state for release-claimed behavior.
- Relay sync returns contacts-only posts without author-granted `WallRead` enforcement.
- Sync lacks durable cursors, tombstones, social events, media lifecycle handling, status observability, or multi-profile validation.
- README, SECURITY, or CHANGELOG describe any simulated, mock-only, local-only, or placeholder behavior as production-complete.

## Requirement and required-test evidence

- `req.01`: This durable contract enumerates production-required behavior for 1:1 voice, 1:1 video, optional group video, wall host views, wall consumer views, wall sync, and release blockers in the sections above.
- `req.02`: The “Ticket mapping for unfinished or placeholder behavior” table maps every currently unfinished or placeholder behavior from the source-spec to a conduct ticket, or records an explicit exclusion with rationale for demo/mock fixtures and out-of-scope release features.
- `req.03`: The “Documentation drift to correct later” section identifies README, SECURITY, and CHANGELOG references that contradict or can overclaim the selected release contract, and assigns later correction to `ticket.0601-docs-and-release-gates-for-calls-wall-sync` without silently changing feature scope in this ticket.
- `test.01`: Manual review confirms every source-spec coverage row maps to at least one ready ticket or explicit exclusion in the source-spec coverage matrix and ticket mapping table.
- `test.02`: Worker LDGR status was reviewed for this isolated worker DB before recording the planning artifact; the worker owned only `0001-reconcile-release-capability-contract`, with no conflicting active Harbor conduct work in the worker ledger.

## Validation expectation for downstream tickets

Implementation tickets should validate at the narrowest practical layer first, then run broader checks when cross-stack behavior changes:

- frontend store/service/UI tests for Zustand and React behavior;
- Rust unit tests for command/service/protocol changes;
- relay-server tests for relay permission and wall storage behavior;
- two-profile Harbor validation with relay/direct paths for calling and wall sync release claims;
- documentation/release-gate validation in `ticket.0601-docs-and-release-gates-for-calls-wall-sync` after implementation tickets pass.
