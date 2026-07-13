# ADR-0002: Relay-scoped communities as portable signed forums

- **Status:** Proposed; implementation is gated on the acceptance questions below
- **Date:** 2026-07-12
- **Scope:** Community identity, discovery, membership, forum content, moderation, synchronization, and client experience
- **Related:** [Relay names and private introductions](relay-names-and-private-introductions.md), [community wireflow](community-forum-wireflow.md), [implementation program](../tickets/live-beta-remediation/community-forum/README.md)

## Context

Harbor's current community implementation identifies a community by a libp2p relay peer ID and an
operator-provided `community_name`. Joining requires a relay multiaddress. The relay creates a
single General board, registers any signing peer that connects, and accepts a flat stream of signed
text posts. Authors can delete their own posts and the relay can ban peers. There is no community
manifest, thread/reply relationship, membership record, portable role authority, or community-level
governance contract.

This makes Communities feel like another feed while quietly assigning significant practical power
to the relay process and its database operator. It also exposes peer IDs when a human community name
is absent. Harbor needs a forum-shaped place for durable, topic-oriented discussion among people who
do not need to be contacts, without introducing a global community registry or pretending that a
relay host cannot affect availability.

### Existing architecture evidence

| Surface                                                      | Current behavior and design constraint                                                                                                                                                              |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/types/boards.ts`, `src/stores/boards.ts`                | `relayPeerId` is the community key; a nullable operator label is the only human identity; state contains boards and one flat `boardPosts` list.                                                     |
| `src/services/boards.ts`, `src-tauri/src/commands/boards.rs` | Join accepts a relay multiaddress, extracts its peer ID, dials it, and stores that relay as the community. There is no manifest/invite verification or membership object.                           |
| `src-tauri/src/db/migrations/008_boards.sql`                 | Local cache has relay communities, boards, posts, and timestamp cursors but no topics, threads, replies, roles, membership, or portable community ID.                                               |
| `src-tauri/src/services/board_service.rs`                    | Posts and requests are signed with existing identity keys. New board posts are text-only and author deletion is signed. This signing foundation should be extended rather than replaced.            |
| `relay-server/src/db.rs`                                     | Community mode creates a General board and stores known peers, bans, posts, and Lamport clocks. Board creation/governance is not exposed as a community protocol.                                   |
| `relay-server/src/board_service.rs`                          | Peer registration proves possession of a public key; any registered, unbanned peer may post. The relay verifies signatures and may ban peers, but membership and moderator authority are undefined. |
| `src-tauri/src/p2p/protocols/board_sync.rs`                  | Existing canonical signed request/response and relay-name primitives provide the base for versioned community events, replay protection, and qualified author names.                                |

The current implementation can be migrated incrementally, but its relay peer IDs, display labels,
and flat rows are not sufficient evidence for the new community identity or event graph.

## Decision

Harbor communities will be **relay-scoped, cryptographically identified, portable signed forums**.
The naming relay attests that a local community slug is unique in its namespace and may host the
first replica, but it does not receive an implicit owner role over community content or governance.

### 1. Community identity and names

A community has two identifiers:

- A human address: `community:<slug>@<relay-namespace>`, for example
  `community:harbor@harbor.social`.
- An immutable `community_id`, computed as the BLAKE3 digest of the canonical CBOR genesis
  manifest, including its signature set and relay name attestation.

The local slug follows the existing relay-name normalization rules: 3 to 32 lowercase ASCII
letters, digits, and single hyphens. It is unique only within the relay namespace. Clients always
show the qualified address on unfamiliar or security-sensitive surfaces. A bare title is decorative
metadata and may not replace the qualified address as identity.

The canonical genesis manifest contains:

- signature domain and protocol version;
- local slug and relay namespace;
- immutable community ID inputs;
- title, summary, rules digest, and initial public/private policy;
- initial topic catalog for the MVP;
- initial governance mode and, when enabled, steward keys and threshold;
- creation time and nonce;
- naming-relay attestation proving slug uniqueness; and
- creator/founder signatures proving consent to the manifest.

The relay namespace and signing key use the existing relay-key pinning and rotation contract. A
relay peer ID or multiaddress is a transport locator, not the community identity. Locator changes do
not rename the community or change `community_id`.

### 2. Relay responsibility is hosting, not ownership

A relay may provide four separable functions:

1. attest a relay-unique community slug;
2. advertise signed transport locators;
3. store and replay verified community events; and
4. enforce a published host abuse policy for its own infrastructure.

It cannot forge member events, rewrite signed history, grant itself a community role, or silently
change the manifest. It can censor, omit, or stop serving data, because no protocol can force an
operator to remain available. Harbor must present that as a hosting/availability risk rather than
claiming the relay is powerless.

Host actions such as rate limiting, quarantine, or refusing service are signed and logged separately
from community governance events. Clients label them as **Host action**, not **Moderator action**.
This distinction remains even if the same person operates a relay and serves as a steward.

### 3. Discovery and invitations

There is no mandatory global directory. Discovery is user-driven through:

- `harbor://community/...` invitation URIs;
- equivalent HTTPS handoff URLs;
- QR codes or copied invitation bundles; and
- optional user-selected directories in a later release.

An invitation bundle contains the canonical manifest or its digest, qualified community address,
relay-key proof, one or more signed locators, and an optional membership invitation. The client
validates the manifest, relay attestation, locator expiry, and expected `community_id` before showing
a join confirmation. Changing `https://` to `harbor://` manually is never required.

Open-community invitations are discovery links, not bearer authorization. Later private-community
invitations are single-purpose, bounded, signed capability objects and must not contain plaintext
membership lists or reusable secrets.

### 4. Membership

Membership is an explicit signed event, not a side effect of peer registration.

For an open community, a user signs `CommunityJoin` over the community ID, verified qualified user
name claim digest, identity key, sequence, and time. `CommunityLeave` is a newer signed tombstone.
The relay stores only membership in that community; Harbor never uploads a user's full contact graph
or list of other communities.

MVP roles are deliberately small:

- `reader`: may sync public community metadata and content;
- `member`: may create threads and replies under rate/content limits; and
- `host`: an infrastructure label, never a community capability.

The MVP has no mutable privileged community role. This avoids shipping an accidental permanent
owner before threshold governance is implemented. The genesis topic catalog and rules are fixed for
the MVP. Authors may tombstone their own content. Readers control local hide/block/report choices.

Later governance adds signed, scoped, expiring `steward` and `moderator` capabilities. Community-wide
changes require a manifest-defined threshold of active steward signatures. No single implicit global
owner exists. Moderator actions are signed, scoped to a topic or action class, appealable, and never
erase the underlying signed event from another replica.

### 5. Forum content model

Communities use a forum hierarchy, not a second social feed:

```text
Community
└── Topic (stable category from the manifest in MVP)
    └── Thread (title + opening body/media)
        └── Reply
            └── optional parent reply (one displayed nesting level in MVP)
```

Every content mutation is a signed append-only event bound to `community_id` and includes an event
ID, author peer ID and verified name-claim digest, author sequence/Lamport clock, creation time,
content modality, and relevant parent IDs. Edits and deletes are newer signed events. A reply may
name another reply as its parent, but the MVP UI renders at most one indentation level to avoid
unreadable deep trees.

Threads have a required title and opening body or media. Replies do not require titles. Topic,
thread, and reply IDs are immutable. A client rejects cross-community parents, missing parents after
the bounded replay window, invalid author signatures, stale sequences, and impossible event types.

The value beyond a personal feed is explicit:

- durable topic archives rather than time-only scrolling;
- question/discussion threads with coherent replies;
- participation beyond the contact graph;
- stable links to useful discussions;
- unread/followed-thread state; and
- community knowledge that remains locally readable when the host is offline.

### 6. Modality filters and defaults

The forum offers compact `All`, `Images`, `Video`, and `Audio` filters using the same canonical
post-modality rules as Feed and personal posts. Filtering never changes topic/thread order or hides
the text-only replies required to understand a selected thread.

Community selection is independent from Feed/personal-wall selection. The preferred landing topic,
sort, and modality are persisted per `community_id`, not as one global community preference. Within
an open thread, modality filtering applies to thread discovery/results, not to individual replies.

### 7. Privacy contract

MVP communities are public-content communities with explicit membership for posting. Joining leaks
that membership to the selected host relay and other peers can infer participation from signed
posts. The join confirmation must say so. The relay must not expose a browsable member list through
the normal protocol.

MVP does not claim private community content. Private/invite-only communities require a separate
protocol slice with encrypted group content keys, epoch rotation on membership changes, bounded
history sharing, revocation behavior, metadata disclosure analysis, and recovery. Until that lands,
the UI must not label a community private merely because joining requires a link.

Local caches are profile-scoped. Leaving removes active credentials and offers cache deletion.
Blocking a member hides their content locally without publishing the user's block list. Reports are
explicit, minimize quoted private data, and state whether they go only to a host or to recognized
community moderators.

### 8. Abuse, moderation, and recovery

MVP controls:

- authenticated requests, signature verification, replay rejection, size limits, rate limits, and
  bounded proof of work where the existing relay abuse policy calls for it;
- local mute, block, hide, and report;
- author-signed edit/delete tombstones;
- signed host quarantine/refusal events under a published host policy; and
- export of sanitized diagnostics without content or private membership graphs.

MVP explicitly does not provide democratic governance, moderator election, appeals adjudication,
role recovery, or portable host consensus. Those claims are forbidden in UI and release notes.

Later threshold governance supports scoped role grants/revocations, policy revisions, moderator
actions, appeals, steward-set changes, and emergency freezes. Key loss never causes silent role or
community-name reassignment. Recovery uses a newer threshold-signed key rotation. If quorum is lost,
the old community remains verifiable and readable but may become governance-frozen; users may fork
it under a new qualified name with an explicit ancestry record.

### 9. Offline operation, replay, and relay loss

Clients store verified events locally and maintain a cursor per community replica. Sync is
incremental, idempotent, and ordered by a deterministic event key rather than wall-clock time alone.
Unknown-parent events are held in a bounded quarantine until parents arrive. Tombstones and role or
membership revocations are retained so stale replicas cannot resurrect content or authority.

While offline, users can read cached topics/threads and queue signed drafts. The UI distinguishes
`local draft`, `queued`, `submitted to host`, and `confirmed in replica log`. A queued event is
revalidated against current membership and protocol state before submission. Failure does not erase
the draft.

In MVP, loss of the only host makes the community cached/read-only until it returns. Harbor exports
the signed manifest and event archive so users retain evidence and content. Later replica locator
records and threshold-approved migration allow multiple hosts without changing `community_id`.

## MVP boundary

The first implementation includes:

- canonical community manifest and relay-unique name attestation;
- verified invitation/handoff and explicit open-community join/leave;
- public topics, threads, one-level replies, edits, and tombstones;
- signed append-only events and incremental offline replay;
- qualified user/community names in all normal UI;
- per-community landing/filter preferences;
- local abuse controls and transparent host actions; and
- multi-profile validation including unauthorized posting and offline catch-up.

It excludes private content, mutable moderator/steward roles, quorum governance, relay replication,
community migration, global discovery, role recovery, and community forks. These are separate later
tickets and may not be implied by MVP terminology.

## Alternatives rejected

### Relay peer ID as community identity

This is the current model. It is unreadable, couples identity to one transport key/address, and
makes relay replacement look like a different community.

### Relay operator as permanent owner

It is simple but conflicts with Harbor's user-control goals and conflates hosting power with social
authority. Hosting power remains real and visible, but it is not silently converted into governance.

### Global community-name directory

It creates a central availability, censorship, enumeration, and governance dependency. Relay-scoped
names provide usable uniqueness without a global account database.

### Blockchain or smart-contract governance

It adds cost, consensus, key-recovery, and operational complexity that the forum MVP does not need.
Signed event logs and threshold capabilities provide the necessary verifiability more directly.

### Ship mutable single-founder administration first

This would create an owner role that is difficult to remove safely later. The MVP instead freezes
community-wide settings and defers privileged mutations until threshold governance is specified and
implemented.

## Consequences

The design gives communities a stable human and cryptographic identity, a distinct forum purpose,
and verifiable content independent of a particular transport address. It also makes honest limits
visible: an MVP host can still deny availability, public participation leaks metadata, and useful
moderation/governance requires later signing work.

Implementation is larger than a UI restyle. It requires versioned protocol objects, new persistence,
relay handling, migration from legacy relay boards, and multi-profile negative tests. Existing flat
boards cannot be silently interpreted as trusted community manifests.

## Acceptance questions

1. Is `community:<slug>@<relay-namespace>` acceptable as the canonical visible address, or should
   the UI use another prefix while preserving the same scoped identity?
2. Is a public-content, open-membership MVP acceptable, with private/invite-only content explicitly
   deferred until group-key rotation is designed?
3. Is fixed MVP configuration acceptable so Harbor avoids a temporary single-owner role, or must
   threshold steward governance ship in the first community release?
4. Should legacy relay boards be offered as an explicitly unverified read-only import, or should
   users start new signed communities and leave old board caches untouched?

Until these are answered and this ADR becomes **Accepted**, only the protocol-freeze ticket may be
scheduled. Product implementation tickets remain proposed.

## Requirement mapping

- Community identity/name, discovery, membership, forum structure, filters, privacy, abuse,
  recovery, offline replay, value, and MVP/later boundaries are defined above.
- The [wireflow](community-forum-wireflow.md) maps the decision to user-visible states.
- The [ticket program](../tickets/live-beta-remediation/community-forum/README.md) decomposes the
  accepted path without modifying LDGR state.
