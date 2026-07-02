# Host/consumer wall sync multi-profile validation

This checklist validates Harbor wall synchronization across isolated host and consumer profiles using real Harbor identities, SQLite databases, libp2p networking, and relay paths. It must not be satisfied by mock peer stores or direct repository mutation alone.

## Preconditions

- Run from the repository root.
- Use disposable profiles and data directories only.
- Start a local Harbor community relay or provide an operator-owned test relay multiaddr. Do not use production user data.
- Capture redacted logs/events only. Do not attach private post body/media contents unless the data was generated solely for this test.

## Automated regression gates

Run these before and after the manual multi-profile scenario:

```bash
pnpm exec vitest run src/stores/wall.test.ts src/stores/feed.test.ts src/stores/contactWall.test.ts src/services/comments.test.ts src/services/likes.test.ts
cargo test --manifest-path src-tauri/Cargo.toml content_sync wall_social p2p::types::tests::wall_sync_status_event_serializes_without_private_content
cargo test --manifest-path relay-server/Cargo.toml
dev check
dev ci --language typescript
```

Expected coverage:

- wall/feed/contact-wall stores load backend state, request relay sync, preserve local-first fallback, and surface sync status;
- comments/reactions use signed backend social-event commands rather than mock-only state;
- Rust content sync enforces permissions, media metadata signatures, cursors, tombstones, social events, and redacted status events;
- relay server persistence and permission enforcement checks continue to pass.

## Local relay setup

Build and start the relay with an isolated database:

```bash
cargo build --manifest-path relay-server/Cargo.toml
HARBOR_RELAY_DB=/tmp/harbor-wall-sync-relay.sqlite \
  cargo run --manifest-path relay-server/Cargo.toml -- \
  --listen /ip4/127.0.0.1/tcp/0
```

Record the relay peer ID and listen multiaddr printed at startup. Use that relay address in both Harbor profiles.

## Three-profile wall sync scenario

Use three isolated app profiles:

```bash
HARBOR_PROFILE=wall-host HARBOR_DATA_DIR=/tmp/harbor-wall-host pnpm tauri dev
HARBOR_PROFILE=wall-authorized HARBOR_DATA_DIR=/tmp/harbor-wall-authorized pnpm tauri dev
HARBOR_PROFILE=wall-unauthorized HARBOR_DATA_DIR=/tmp/harbor-wall-unauthorized pnpm tauri dev
```

1. Create or unlock disposable identities in all profiles.
2. Start Peer-to-Peer networking for all profiles.
3. Add the relay address in all profiles and wait for relay-connected/circuit-address UI or logs.
4. Add `wall-authorized` as a host contact and grant `WallRead`. Do not grant `WallRead` to `wall-unauthorized`.
5. On `wall-host`, create:
   - one public text post with image or video media;
   - one contacts-only post with image or video media.
6. Confirm host-side observations:
   - post creation succeeds locally before relay sync completes;
   - `wall_sync_status` emits or UI displays `sync_started`, `media_queued`/`media_fetched` where applicable, `posts_stored`, and `cursor_advanced` or equivalent success state;
   - relay submission events identify post IDs and relay peer ID without leaking private body/media content in logs.
7. On `wall-authorized`, refresh the host contact wall and feed.
8. Confirm authorized consumer observations:
   - public and contacts-only posts appear in contact wall/feed after relay sync;
   - media fetch state transitions from pending/loading to available, or records a per-item fetch failure without dropping the post;
   - final local SQLite/UI state converges with the host for visible posts.
9. On `wall-unauthorized`, refresh the host contact wall and feed.
10. Confirm unauthorized consumer observations:
    - public post appears;
    - contacts-only post does not appear;
    - logs/status include a permission-denied/filtered result without private content leakage.
11. On `wall-authorized`, add a signed comment and like/reaction to the public post. Refresh `wall-host` and confirm social counts/events converge.
12. On `wall-host`, edit the public post. Refresh both consumers and confirm the edited public post converges while the unauthorized profile still cannot see contacts-only content.
13. On `wall-host`, delete the public post. Refresh both consumers and confirm tombstone/delete reconciliation removes it from wall/feed views. For the two-profile relay tombstone check, keep only `wall-host` and `wall-authorized` running, record the deleted post ID, relay cursor, tombstone lamport clock, deleted_at, and delete signature, then force `wall-authorized` to refresh from a stale cursor lower than the original create lamport. Confirm the relay returns the signed tombstone event and the authorized profile does not resurrect the deleted post in contact wall/feed or SQLite.
14. Stop `wall-authorized`, create another public post on `wall-host`, and ensure it is submitted to the relay.
15. Restart `wall-authorized`, refresh the contact wall/feed, and confirm offline relay availability catches up using cursors without duplicating earlier posts.

## Evidence to record

For each run, attach or record:

- automated gate commands and pass/fail output;
- profile names and data directories;
- relay database path, relay peer ID, and relay multiaddr;
- timestamps for post create, relay submit, authorized fetch, unauthorized fetch, social event, edit, delete, two-profile stale-cursor tombstone refresh, offline catch-up, and final refresh;
- redacted `wall_sync_status` events and relevant UI status text;
- final SQLite/UI counts for host, authorized consumer, and unauthorized consumer:
  - public posts visible;
  - contacts-only posts visible;
  - deleted posts visible;
  - relay tombstone lamport/deleted_at/delete-signature observed;
  - comment/reaction counts;
  - media item states;
  - last relay cursor/sync time.

If the interactive scenario cannot be completed, record the blocked step and queue follow-up validation work. Automated unit/integration tests alone are not release-ready evidence for this ticket.

## Run 48 evidence

This run added the repeatable validation checklist above and executed non-interactive automated gates in the agent environment. Results:

- `pnpm exec vitest run src/stores/wall.test.ts src/stores/feed.test.ts src/stores/contactWall.test.ts src/services/comments.test.ts src/services/likes.test.ts` — passed, 5 files / 62 tests.
- `cargo test --manifest-path src-tauri/Cargo.toml content_sync -- --nocapture` — passed, 21 tests.
- `cargo test --manifest-path src-tauri/Cargo.toml wall_social -- --nocapture` — passed, 3 tests.
- `cargo test --manifest-path src-tauri/Cargo.toml p2p::types::tests::wall_sync_status_event_serializes_without_private_content -- --nocapture` — passed, 1 test.
- `cargo test --manifest-path relay-server/Cargo.toml` — passed, 5 relay tests.
- `dev check --language rust` — passed, including `cargo fmt --check`, `cargo clippy -D warnings`, `cargo check`, and full `src-tauri` tests: 231 passed.
- `dev ci --language typescript` — passed, including TypeScript, ESLint, Vitest, and frontend build.
- `cargo check --manifest-path relay-server/Cargo.toml && cargo clippy --manifest-path relay-server/Cargo.toml -- -D warnings` — passed.

The interactive three-profile Tauri scenario was not run because this session has no desktop/WebView interaction channel; it remains required before release readiness can be claimed.
