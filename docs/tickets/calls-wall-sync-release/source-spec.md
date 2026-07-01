---
ldgr_doc: 1
kind: source_spec
id: source_spec.calls-wall-sync-release.v1
schema: ldgr.source_spec.v1
status: ready
tags:
- harbor
- source-spec
---

# Calls, Video, and Wall Sync Source Spec

## User request

Evaluate Harbor for unfinished features and decompose release-quality work into tickets for:

- voice calling;
- video calling, preferably group-capable;
- wall functionality from both host and consumer perspectives;
- wall synchronization between host/consumer views;
- using repository docs as source of truth where possible.

## Current documented intent

- `README.md` describes Harbor as decentralized, local-first, permission-based chat with WallRead and Call capabilities.
- `README.md` marks voice calling as completed as “signaling” and lists video calling/screen sharing and group chats as future/stretch goals.
- `README.md` says posts are stored locally and shared with contacts who have permission; Feed should show posts from contacts who have granted WallRead.
- `CHANGELOG.md` references wall preview, RSS, likes, relay/community, and feed content sync work, but it is noisy and duplicated.
- `docs/release-readiness-review-2026-07-01.md` identifies stale docs, placeholder updater config, simulated security flows, placeholder Feed actions, Wall comments placeholder, and relay clippy failures.

## Source findings

### Calling

- `src-tauri/src/services/calling_service.rs` can sign/validate offer, answer, ICE, and hangup payloads and checks `Capability::Call`.
- `src-tauri/src/commands/calling.rs` exposes Tauri commands for local signing/validation.
- `src/services/calling.ts` wraps those commands.
- `src-tauri/src/p2p/protocols/mod.rs` declares `SIGNALING_PROTOCOL`, but `src-tauri/src/p2p/behaviour.rs` does not register a signaling request-response behavior.
- `src-tauri/src/p2p/types.rs` has no call/signaling network command or event.
- `src/hooks/useTauriEvents.ts` has no incoming-call handling.
- `src/pages/Chat.tsx` has no production call UI; source search found no `RTCPeerConnection` or `getUserMedia` usage.

Conclusion: production voice calling is not complete; the app has signed signaling helpers but no transport, WebRTC runtime, UI, or end-to-end validation.

### Wall host behavior

- `src/stores/wall.ts` creates posts and media, but hard-codes `visibility: 'contacts'` and notes likes/comments are local/defaulted rather than backend-backed.
- `src/pages/Wall.tsx` still uses “Comments coming soon!” for wall post comments.
- `src-tauri/src/commands/feed.rs` includes wall preview commands and `src-tauri/src/commands/rss.rs` includes RSS generation, but source search did not find production UI wrappers using them.
- `src-tauri/src/services/posts_service.rs` signs new posts with `media_hashes: Vec::new()` while media is added afterward, leaving media metadata outside the signed post payload.

Conclusion: host wall posting exists, but visibility controls, social actions, preview/RSS UI, media integrity, and update/delete propagation need release work.

### Wall consumer/feed behavior

- `src/pages/Feed.tsx` maps real feed items, comments, and media, but Like, Save, Hide, and Snooze handlers still show “coming soon” toasts.
- `src/stores/feed.ts` loads comments through `commentsService`, but comments/likes are local backend tables and not syncable production social events.
- There is no dedicated contact wall route in `src/App.tsx`; only `/wall` and `/feed` exist.
- `src/services/feed.ts` wraps `getWall` and relay sync commands, so backend surface exists but consumer wall UI is incomplete.

Conclusion: feed viewing exists, but consumer wall view and durable feed interactions are incomplete.

### Wall sync

- Direct content sync exists in `src-tauri/src/services/content_sync_service.rs` with WallRead checks and lamport cursors.
- Relay wall sync exists in `src-tauri/src/commands/wall_sync.rs` and `relay-server/src/board_service.rs`.
- `sync_feed_from_relay` fetches every active contact from cursor `0` on each call instead of using stored per-author relay cursors.
- Relay `process_get_wall_posts` verifies the requester signature but does not enforce author-granted WallRead for contacts-only posts.
- Relay wall media metadata currently filters to images in `sync_wall_to_relay`.
- Feed service includes all active contacts in allowed authors and then allows contacts-only posts, which conflicts with README language requiring WallRead.

Conclusion: wall sync needs permission enforcement, cursor correctness, event reconciliation/tombstones, media/social event sync, and multi-profile validation.

## Non-negotiable constraints

- Extend existing Tauri/Rust/React/Zustand/libp2p/SQLite architecture.
- Preserve local-first behavior: local creation must not require relay availability.
- Preserve signed identity, permission, and content verification boundaries.
- No production feature may be completed with mock peers, placeholders, fake SDP/media, or UI-only state.
- Group video topology is a major decision and must be selected before group runtime implementation.

## Explicit exclusions unless the release contract changes

- Mobile app.
- Call recording.
- Screen sharing.
- Rich-text collaborative editing.
- External analytics/telemetry.
