# Harbor beta code audit (2026-07-13)

Status: complete

## Purpose

This audit determines whether Harbor's implementation matches the behavior presented to beta users. It is deliberately broader than a build, CI, or test review. Passing automation is not treated as proof that a production-facing control is implemented or that a success message reflects durable state.

The review has two passes:

1. Functional completeness: fallbacks, mocks, placeholders, TODOs, incomplete wiring, false-success paths, swallowed errors, and code described as legacy or compatibility behavior.
2. Engineering quality: maintainability, complexity, duplication, coupling, inefficient work, observability, and testability.

Harbor has no customer compatibility obligation at this stage. A path described as legacy must therefore justify itself with a concrete current beta data, installation, or protocol requirement. Otherwise it should be removed rather than carried forward indefinitely.

## Review rules

- Production reachability is checked before assigning severity.
- Security and privacy decisions must fail closed.
- Best-effort background work may degrade gracefully, but it must leave useful diagnostics and must not produce false success.
- Unverified display names must never be presented as verified identity.
- A destructive, recovery, privacy, or security control must be backend-enforced and tested end to end before it is shown as available.
- Compatibility behavior must name the supported input/version and have a removal condition.
- Intentional privacy behavior, such as oracle-resistant relay responses, is distinguished from accidental error suppression.

## Severity model

- **Blocker:** can cause data loss, a false security/recovery guarantee, unauthorized access, or makes the beta materially unsafe to distribute.
- **High:** breaks a core workflow, creates inconsistent durable state, enables impersonation, or hides a failure behind apparent success.
- **Medium:** causes misleading state, silent degradation, corruption masking, or a substantial operational/debugging problem.
- **Low:** dead code, cleanup debt, weak diagnostics, or localized maintainability cost with limited immediate user impact.

## Executive status

The current beta is not release-ready. The completed audit records 140 findings: 12 Blockers, 62 High, 58 Medium, and 8 Low. The count includes three release-governance findings, 83 functional/fallback findings, and 54 engineering-quality findings.

The immediate stop-ship risks are broken direct-message cryptographic invariants; fake password, backup, recovery, and deletion controls; unenforced privacy behavior; authorization/replay defects; relay key exposure and cross-author overwrite; and an account/runtime lifecycle that is not actually isolated by profile. The second pass shows that these defects cluster around monolithic, partially initialized state machines, non-atomic persistence, duplicated protocols, stringly typed boundaries, and release gates that do not exercise production controls.

No production source was changed during this audit. Remediation should be decomposed from this document, and another beta release should not be promoted until every Blocker has verified end-to-end closure and each High has either closure or an explicit reviewed release disposition.

## Process and release-governance findings

### GOV-001: Known release blockers were advisory rather than gating

- Severity: Blocker
- Evidence: `docs/release-readiness-review-2026-07-01.md` explicitly identified simulated password change, recovery/import, deletion, and placeholder backup exports, then placed replacement or deferral in the release-preparation work order.
- Current state: those controls remain production-reachable in `src/pages/Settings.tsx`.
- Impact: a documented release risk survived work-item completion, CI, packaging, signing, and deployment. The release process was therefore able to report readiness while a known blocker remained.
- Required action: represent every Blocker/High release finding as a tracked gate with an owner, disposition, verification evidence, and a machine-checkable release assertion where feasible. A review document alone is not a gate.

### GOV-002: The live-beta acceptance gate omits destructive and recovery controls

- Severity: Blocker
- Evidence: `scripts/live-beta-acceptance.mjs` and `docs/live-beta-cross-platform-acceptance.md` define detailed scenarios for identity restart, password confirmation, contacts, media, calling, and presentation, but no scenario verifies password rotation, backup contents, restoration from an exported backup, or actual account deletion.
- Impact: the gate can pass while the application ships fake security and recovery controls. The extensive scenario count created confidence without covering some of the highest-consequence promises in the UI.
- Required action: build the release inventory from every production-facing command/control and require a disposition for each. Destructive, credential, privacy, and recovery controls need explicit packaged-app scenarios, not inferred coverage under general onboarding.

### GOV-003: The release workflow does not enforce the live-beta acceptance gate

- Severity: High
- Evidence: `.github/workflows/release.yml` runs frontend, Tauri, and relay CI, then builds and publishes. It never runs `scripts/live-beta-acceptance.mjs check` or requires an attested acceptance artifact. The workflow-dispatch version is also not checked against `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` before publication.
- Impact: a release can be published without the cross-platform P0 session, and a manually supplied release version can diverge from the version embedded in its binaries or expected artifact paths.
- Required action: add a release-candidate promotion workflow that consumes immutable acceptance evidence for the exact commit and artifact hashes, verifies all version sources, and is the only workflow permitted to publish.

## Pass 1: functional completeness and fallback behavior

### Blockers

#### FUN-001: Password change reports success without changing the password

- Location: `src/pages/Settings.tsx:352-380`
- Duplicate implementation: `src/pages/settings/SecuritySection.tsx:45-67`
- Reachability: the monolithic Settings page is routed by the production application.
- Evidence: the handler validates fields, waits on a timer, clears the form, and displays `Password changed successfully!`; it does not call a backend command.
- Impact: users can believe their encryption password was rotated when the old password remains effective.
- Required action: remove or disable the control until an atomic backend rotation exists and is verified by locking and unlocking with old and new credentials.

#### FUN-002: Exported identity backups contain placeholder key data

- Location: `src/pages/Settings.tsx:382-409`
- Duplicate implementation: `src/pages/settings/SecuritySection.tsx:82-107`
- Evidence: `encryptedKeys` is the literal string `ENCRYPTED_KEY_DATA_PLACEHOLDER`, followed by a success toast asserting that a backup was exported.
- Impact: a user may rely on an unrecoverable backup and later lose their identity permanently.
- Required action: immediately remove or visibly disable export. Reintroduce it only with versioned authenticated encryption, integrity validation, restoration tests, and wrong-password/tamper tests.

#### FUN-003: Import/recovery and account deletion are simulated

- Location: `src/pages/Settings.tsx:427-485`
- Duplicate implementation: `src/pages/settings/SecuritySection.tsx`
- Evidence: import checks only a caller-controlled JSON `type`, waits, and reports recovery. Delete waits and reports deletion without deleting state or redirecting.
- Impact: false recovery and destructive-operation guarantees. Import also reflects untrusted `displayName` from arbitrary JSON in a success message.
- Required action: remove or disable both controls until backend implementations and destructive/recovery acceptance tests exist.

#### FUN-004: Presented privacy controls are not enforced

- Locations: `src/pages/Settings.tsx:1708-1750`, `src/stores/messaging.ts:205-215`, `src/components/layout/MainLayout.tsx:127-132`, `src/pages/Chat.tsx:760-777`
- Evidence: `showReadReceipts` is only persisted and rendered; messaging never consults it. `markConversationRead` changes local database state, while `create_read_ack` has no production caller. `showOnlineStatus` only changes the local indicator color, and every chat contact is hardcoded `online: true`.
- Impact: the application promises privacy and presence behavior it does not implement.
- Required action: either remove these settings for beta or implement signed receipt delivery and a real presence policy end to end. Never label a local decoration as a remote privacy control.

#### FUN-025: Direct-message encryption reuses AES-GCM nonces across peers

- Location: `src-tauri/src/services/crypto_service.rs:227-249`, `src-tauri/src/services/crypto_service.rs:305-338`, `src-tauri/src/services/messaging_service.rs:126-151`, `src-tauri/src/db/connection.rs:471-494`
- Evidence: both peers derive the same conversation key by sorting their peer IDs. Each device maintains its own per-conversation send counter beginning at 1. The 96-bit AES-GCM nonce contains four zero bytes followed only by that counter, so Alice's message 1 and Bob's message 1 use the same key and nonce. The reserved direction bytes never encode direction.
- Impact: nonce reuse under AES-GCM breaks the scheme's confidentiality and authentication guarantees; observing bidirectional ciphertexts can reveal plaintext relationships and enable forgery analysis. Editing also reuses the original nonce again.
- Required action: stop using the current wire format. Derive independent directional keys/nonces or include a cryptographically bound sender/direction domain, use a versioned protocol, and add bidirectional/restart/rollback nonce-uniqueness tests before allowing messaging.

#### FUN-026: Message edits are unsigned plaintext and reuse the original encryption nonce

- Location: `src-tauri/src/p2p/network.rs:4294-4322`, `src-tauri/src/services/messaging_service.rs:629-753`
- Evidence: the network edit message carries `message_id`, plaintext `new_content`, and `edited_at` without an author signature. The receiver only checks that the stored original was not authored by itself; it does not bind the transport peer to the original sender. It then re-encrypts the replacement under the original message's nonce counter.
- Impact: any connected peer able to submit the protocol message can rewrite another contact's received message. Edit content loses application-layer end-to-end confidentiality in transit and compounds AES-GCM nonce reuse.
- Required action: disable edits immediately. Redesign edits as signed, encrypted, replay-protected immutable events bound to original author, recipient, message, revision, and a fresh nonce.

#### FUN-027: Invalid messages can consume replay nonces before authentication

- Location: `src-tauri/src/p2p/network.rs:4188-4219`, `src-tauri/src/services/messaging_service.rs:238-320`, `src-tauri/src/db/connection.rs:497-540`
- Evidence: the transport peer is not compared with `direct_msg.sender_peer_id`. `check_and_record_nonce` commits the claimed sender/counter before contact lookup and signature verification. A later verification failure does not roll it back.
- Impact: an attacker can claim a contact's identity and pre-consume counters, causing legitimate messages with those counters to be rejected as replay. This is a durable denial-of-service and corrupts replay state with unauthenticated input.
- Required action: bind the libp2p peer to the signed sender, verify recipient/conversation/signature/capability first, then record the nonce atomically with message persistence.

#### FUN-028: AWS bootstrap tracing can disclose the relay private key

- Location: `infrastructure/community-relay-cloudformation.yaml:245-246`, `infrastructure/community-relay-cloudformation.yaml:294-304`, `infrastructure/community-relay-cloudformation.yaml:439-446`; equivalent paths exist in `infrastructure/relay-cloudformation.yaml`.
- Evidence: user data runs with `#!/bin/bash -xe` and redirects the trace to `/var/log/user-data.log` and the instance console. Commands expand `EXISTING_KEY` and `IDENTITY_KEY_B64` while restoring or persisting the base64-encoded relay identity key.
- Impact: the long-lived relay signing/private identity key can be written into instance logs and console output accessible to operators or log collection systems.
- Required action: rotate deployed relay keys after fixing bootstrap, remove `-x`, bracket all secret operations with tracing disabled, avoid shell expansion of secret values, and add a deployment test that scans console/user-data output for the key material.

#### FUN-029: Relay wall authorization evaluates expiry using requester-controlled time

- Location: `relay-server/src/board_service.rs:922-965`, `relay-server/src/board_service.rs:1055-1079`
- Evidence: the requester's signed `timestamp` is passed to grant validation and `has_active_wall_read_grant`. The relay does not replace it with server time or enforce a narrow freshness window before using it for authorization.
- Impact: a grantee can continue reading contacts-only posts and social events after expiry by signing requests with an old timestamp from the grant's valid period.
- Required action: use relay server time for authorization, enforce signed-request freshness/replay constraints separately, and add expiry tests with deliberately stale but validly signed requests.

#### FUN-030: A registered relay peer can replace another author's wall post by reusing its ID

- Location: `relay-server/src/db.rs:677-723`
- Evidence: tombstone lookup includes `(post_id, author_peer_id)`, but storage then uses `INSERT OR REPLACE` against a table where `post_id` is globally unique. A different registered author can submit the victim's `post_id`; SQLite replaces the existing row, and replacement can trigger cascading deletion of its media.
- Impact: cross-author post overwrite, content integrity loss, and media deletion.
- Required action: replace with an author-bound upsert that rejects an existing ID owned by another author, performs monotonic revision checks, and updates in place without replacement cascades.

### High severity

#### FUN-005: Existing accounts collapse to the same chooser label

- Locations: `src/components/onboarding/AccountSelection.tsx:53-54`, `src-tauri/src/services/accounts_service.rs:88-104`, `src-tauri/src/services/accounts_service.rs:330-340`
- Evidence: the chooser uses only `verifiedQualifiedName`, otherwise `Local Harbor account`. New and migrated registry records begin with no verified qualified name, and current backfill occurs only after an account is selected and unlocked.
- Impact: users cannot reliably choose their own account. Falling back to the unverified display name would create an impersonation regression.
- Required action: hydrate the registry from locally verified signed claims before rendering the locked chooser, with an explicit migration for current beta profiles.

#### FUN-006: Contact state transitions discard persistence failures and still emit success

- Locations: `src-tauri/src/p2p/network.rs:3840-3935`, `src-tauri/src/p2p/network.rs:4112-4168`
- Evidence: request recording, status updates, promotion, removal, and revocation repeatedly use `let _ =`. `ContactAdded` can be emitted even if durable promotion failed.
- Impact: the two peers and the frontend can disagree about whether a request is pending, accepted, declined, or revoked.
- Required action: make each state transition atomic, propagate failures into an explicit failed state, and emit UI events only after durable completion.

#### FUN-007: Contact-link import can succeed without required grants

- Locations: `src-tauri/src/commands/network.rs:595-624`, `src/pages/Network.tsx:276-286`
- Evidence: contact storage succeeds first; Chat and WallRead grant errors are discarded; the frontend displays `Contact added successfully!`. An offline dial is intentionally nonfatal but is not distinguished from a completed connection.
- Impact: a contact appears added but messaging or wall access can remain broken.
- Required action: transact contact and grant persistence, return structured `added/offline/connected/failed` state, and reserve success language for the completed durable portion.

#### FUN-008: Relay ban lookup errors fail open

- Locations: `relay-server/src/board_service.rs:407`, `relay-server/src/board_service.rs:459`, `relay-server/src/board_service.rs:669`
- Evidence: `is_peer_banned(...).unwrap_or(false)` treats a database error as not banned.
- Impact: a banned peer may register or submit content during a ban-query failure.
- Required action: propagate the database error or treat it as denied. Authorization and moderation checks must fail closed.

#### FUN-009: Contact-request notifications present an unverified remote name without qualification

- Location: `src/hooks/useTauriEvents.ts:269-275`
- Evidence: the toast interpolates the requester-supplied `display_name` before a verified relay claim is available.
- Impact: phishing and impersonation surface at the moment a user decides whether to trust a stranger.
- Required action: show an unverified generic label until claim verification succeeds, or explicitly and prominently label the string as unverified.

#### FUN-010: Beta compatibility mode permits publishing without a verified relay name

- Locations: `src/components/identity/LegacyIdentityMigration.tsx:84-193`, `src-tauri/src/services/identity_publishing_policy.rs:7-15`, `src/App.tsx:211-236`
- Evidence: an unlocked account without a verified claim can choose `Continue in beta compatibility mode`; the backend explicitly authorizes publishing whenever mode is `compatibility`.
- Impact: the product's verified, relay-unique naming guarantee has a production bypass. Content can be published under an identity that normal UI cannot safely name.
- Required action: remove compatibility publishing. For the current pre-customer beta, perform one explicit migration of the retained local test identities or require them to claim a name before entering the application.

#### FUN-011: Identity creation succeeds even if the account registry write fails

- Location: `src-tauri/src/commands/identity.rs:43-75`
- Evidence: `register_account` errors are logged as informational and identity creation still returns success. The comment assumes the error may mean an existing account but does not distinguish that case from I/O, serialization, or permissions failure.
- Impact: a valid encrypted identity can be created without a chooser entry, producing an orphaned or apparently missing account after restart.
- Required action: make identity creation and registry registration an atomic/recoverable workflow. Handle `AlreadyExists` explicitly and fail or repair all other errors before reporting completion.

#### FUN-012: Profile photo upload is local decoration presented as a shared profile update

- Locations: `src/pages/Settings.tsx:301-327`, `src/pages/Settings.tsx:768-833`, `src/stores/settings.ts:131`, `src/stores/settings.ts:240`
- Evidence: the UI says it manages “how others see you” and reports `Profile photo updated!`, but upload only stores a data URL in frontend settings/local storage. It does not hash media, update `local_identity.avatar_hash`, update the account registry, or distribute the avatar through contact/profile exchange.
- Impact: the local user sees a photo that contacts never receive, while the application asserts success. Large data URLs also consume local-storage quota.
- Required action: either label this as a device-only appearance setting or implement a real bounded avatar media pipeline and signed profile update.

#### FUN-013: Local network discovery toggle does not control mDNS

- Locations: `src/pages/Network.tsx:693-705`, `src-tauri/src/p2p/config.rs:11-12`, `src-tauri/src/p2p/behaviour.rs:228-230`
- Evidence: the frontend only persists `localDiscovery`. The backend always constructs mDNS behavior, and `enable_mdns` is not consulted outside configuration definitions.
- Impact: users cannot disable the advertised local-network discovery behavior.
- Required action: wire the setting into network construction/reconfiguration or remove the toggle until it is enforceable.

#### FUN-031: Blocking a contact does not override messaging, calling, or private-content authorization

- Locations: `src-tauri/src/commands/contacts.rs:164`, `src-tauri/src/services/contacts_service.rs:220`, `src-tauri/src/services/messaging_service.rs:239`, `src-tauri/src/services/calling_service.rs:437`, `src-tauri/src/services/content_sync_service.rs:169`, `src-tauri/src/services/permissions_service.rs:70`
- Evidence: blocking only sets `contacts.is_blocked`. Core authorization paths continue using stored keys and capability grants without checking the flag.
- Impact: a blocked peer can retain the ability to message, call, and fetch previously authorized contacts-only content.
- Required action: centralize authorization so blocked state is an overriding denial for every inbound protocol, disconnect the peer, stop background fetches, and test each capability after blocking.

#### FUN-032: Removing a contact leaves capability grants active

- Locations: `src-tauri/src/commands/contacts.rs:184`, `src-tauri/src/db/repositories/contacts_repo.rs:219`, `src-tauri/src/services/permissions_service.rs:70`
- Evidence: removal deletes the contact and contact-request records but does not create revocations for Chat, WallRead, or Call grants.
- Impact: local or relay authorization can outlive the contact relationship; re-adding the same peer can resurrect stale authority.
- Required action: atomically remove the relationship, apply immediate local denial, create signed grant revocations, and persist a durable revocation outbox.

#### FUN-033: Contact invites bypass the signed identity/contact handshake

- Locations: `src-tauri/src/commands/network.rs:517-617`, `src-tauri/src/services/contacts_service.rs:132`, `src-tauri/src/commands/contacts.rs:145`
- Evidence: the official-looking link carries unsigned identity metadata. The backend accepts two historical key encodings, does not prove that the supplied Ed25519 key derives the peer ID in the multiaddress, directly inserts the contact, and grants permissions.
- Impact: an invite is treated as authoritative identity and authorization material even though it is only caller-controlled data.
- Required action: make links discovery hints only. Bind the peer ID to the presented key and complete the signed request/acceptance exchange before storing trusted keys or issuing capabilities. Delete the pre-customer double-base64 compatibility decoder.

#### FUN-034: Locking an account leaves the network and private-data services active

- Locations: `src-tauri/src/commands/identity.rs:90`, `src-tauri/src/services/identity_service.rs:176`, `src-tauri/src/commands/network.rs:254`, `src-tauri/src/services/content_sync_service.rs:169`
- Evidence: lock clears cached private keys but does not stop networking, calls, relay/background workers, or content-serving handlers.
- Impact: “locked” does not mean offline or unable to serve locally stored private content; ongoing protocol activity can continue after the user expects the account to be secured.
- Required action: define lock semantics and implement an ordered shutdown barrier before clearing keys and presenting the locked UI.

#### FUN-035: The authoritative identity backend accepts empty or weak passwords

- Locations: `src-tauri/src/models/identity.rs:52`, `src-tauri/src/services/identity_service.rs:76`, `src-tauri/src/services/crypto_service.rs:76`
- Evidence: password policy exists only in the React form. Tauri commands and the local control surface can call identity creation without backend length/emptiness validation.
- Impact: encrypted identity keys can be protected by an empty password despite UI claims.
- Required action: enforce bounded password policy in the service boundary and test direct command/control invocation.

#### FUN-036: Unlock and network start tolerate inconsistent identity keys

- Locations: `src-tauri/src/services/identity_service.rs:138`, `src-tauri/src/commands/network.rs:160`
- Evidence: unlock does not verify that decrypted Ed25519/X25519 private keys reproduce the stored public keys and peer ID. Network startup detects a peer-ID mismatch but logs it and continues.
- Impact: corrupted or substituted identity material can enter active network state under contradictory identifiers.
- Required action: derive and compare all public identity material during unlock and abort closed on any mismatch.

#### FUN-037: Multi-account switching changes the registry but not the running application profile

- Locations: `src-tauri/src/commands/accounts.rs:40`, `src-tauri/src/services/accounts_service.rs:218`, `src-tauri/src/lib.rs:215-238`
- Evidence: switch updates `accounts.json`, while the database and all managed services remain bound to the profile selected at process startup. The command still reports success.
- Impact: the UI can claim another account is active while identity, database, network, and content remain attached to the previous profile.
- Required action: remove in-process switching for beta and perform a controlled full restart into the selected profile, or build a complete service-rebinding lifecycle with isolation tests.

#### FUN-038: Messaging reports queued requests as sent but has no delivery/retry lifecycle

- Locations: `src-tauri/src/commands/messaging.rs:128`, `src-tauri/src/p2p/network.rs:375`, `src-tauri/src/p2p/network.rs:2017`, `src-tauri/src/db/repositories/messages_repo.rs:357`, `src-tauri/src/services/messaging_service.rs:377`
- Evidence: send completes when libp2p queues a request. Responses and outbound failures are not used to transition the durable message, pending-message retrieval has no production consumer, and delivery/read acknowledgment constructors have no sender.
- Impact: messages can remain pending forever without retry while the user sees a normal send action.
- Required action: persist a bounded retry outbox, correlate libp2p request IDs, process responses and outbound failures, and implement delivery/read acknowledgments according to the privacy setting.

#### FUN-039: Post deletion can return success without ever deleting the relay copy

- Locations: `src-tauri/src/commands/posts.rs:287-296`, `src-tauri/src/commands/wall_sync.rs:29-33`
- Evidence: deletion stores a local tombstone, fire-and-forgets relay deletion, and returns success. Normal wall resync skips tombstones, leaving no automatic retry path.
- Impact: content the user believes deleted can remain available from the relay indefinitely.
- Required action: use a durable tombstone outbox, wait for or display relay acknowledgment state, retry across restarts, and make local-only versus network-complete deletion explicit.

#### FUN-044: Frontend identity mutations swallow backend failures

- Locations: `src/stores/identity.ts:254-315`, `src/components/layout/MainLayout.tsx:135-141`, `src/pages/Settings.tsx:338-350`
- Evidence: lock, bio, display-name, and hint actions catch errors without rejecting or returning failure. Callers then close the lock dialog, clear dirty state, or display success.
- Impact: the UI can claim the account is locked or a profile mutation is saved when the backend rejected it.
- Required action: state-changing store actions must rethrow or return a typed durable outcome; UI success follows only a successful authoritative response.

#### FUN-045: Remote call audio playback failure is silent and has no recovery UI

- Location: `src/services/callingRuntime.ts:231-233`, `src/services/callingRuntime.ts:545-558`
- Evidence: `audio.play()` rejection is swallowed with a comment that the overlay can retry, but the audio-element accessor has no production consumer and no resume control exists.
- Impact: a connected call can be completely silent, especially under autoplay restrictions, without explaining how to restore audio.
- Required action: surface a blocked-playback state and a user-gesture “Enable call audio” action.

#### FUN-046: User-triggered reads resolve successfully after failure

- Locations: `src/stores/messaging.ts:59-85`, `src/stores/feed.ts:328-353`, `src/stores/feed.ts:387-418`, `src/stores/contactWall.ts:108-170`, `src/stores/contacts.ts:31-59`
- Evidence: stores catch and retain errors but resolve their promises. Page-level catches therefore cannot run; Feed and Contact Wall report successful refreshes after failed loads, and several pages do not render the retained error.
- Impact: stale or empty data is presented as refreshed, obscuring network/database failures.
- Required action: use typed outcomes consistently. User-triggered actions reject or return failure; background reconciliation may be best effort but must expose degraded state.

#### FUN-047: Identity initialization converts arbitrary backend failure into “no identity”

- Locations: `src/stores/identity.ts:150-154`, `src/App.tsx:191-194`
- Evidence: initialization catches any error and sets `no_identity`; the application then presents account creation.
- Impact: transient IPC, permissions, corruption, or database errors look like permanent absence and can prompt the user to create conflicting state.
- Required action: distinguish authoritative absence from retryable/fatal initialization failure and never offer creation after an unsuccessful lookup.

#### FUN-048: Production event-listener registration can fail as an unhandled promise

- Locations: `src/hooks/useTauriEvents.ts:105-145`, `src/hooks/useTauriEvents.ts:457`
- Evidence: listeners for networking, deep links, calls, contacts, and control actions are registered sequentially, but `setupListeners()` is neither awaited nor caught.
- Impact: one registration failure can leave real-time application behavior partially unwired with no visible degraded state.
- Required action: await/catch setup, track which listeners are live, clean partial registration, and retry boundedly or block affected features.

#### FUN-049: Identity-specific frontend state is shared across all local accounts

- Locations: `src/stores/settings.ts:254-291`, `src/stores/feed.ts:16`, `src/stores/feed.ts:56-75`, `src/stores/messaging.ts:315-320`
- Evidence: avatar, default visibility, presence/read settings, relays, ICE servers, saved/hidden/snoozed posts, and archived conversations use global local-storage keys.
- Impact: switching identities leaks preferences and curation state across accounts and can apply another account's privacy/network choices.
- Required action: partition identity-specific persistence by active account/peer ID and reset in-memory stores at profile boundaries. Keep only truly device-wide appearance settings global.

#### FUN-050: Community author names remain peer-supplied and spoofable

- Locations: `relay-server/src/board_service.rs:399-430`, `relay-server/src/db.rs:325-410`
- Evidence: registration accepts an unconstrained, non-unique display name unrelated to the active relay-name claim, then board responses expose it as author identity.
- Impact: community posts can visually impersonate another user despite Harbor's relay-unique name model.
- Required action: derive presented names from active signed relay claims and stop storing/displaying arbitrary registration labels as identity.

#### FUN-051: Relay wall-post storage commits partial media and still reports success

- Location: `relay-server/src/board_service.rs:766-807`, `relay-server/src/board_service.rs:985-999`
- Evidence: the post commits first, media insertion failures are logged and swallowed, and the client receives `WallPostStored`. Media lookup errors during reads become empty media.
- Impact: signed attachment metadata can disappear while both writer and reader receive plausible success/empty results.
- Required action: transact post plus all media, roll back on failure, and distinguish database error from an actual empty attachment list.

#### FUN-052: Relay authentication and proof-of-work maps have no effective lifetime or capacity bound

- Locations: `relay-server/src/auth.rs:42-49`, `relay-server/src/auth.rs:99-100`, `relay-server/src/auth.rs:144-145`, `relay-server/src/abuse.rs:64-79`, `relay-server/src/abuse.rs:171-201`
- Evidence: expired outstanding challenges, used challenge IDs, peer pressure, and most work challenges are retained. Cheap new peer IDs bypass outer per-peer pressure.
- Impact: unauthenticated/Sybil traffic can cause sustained memory growth.
- Required action: add expiry pruning, hard capacities, source-network issuance quotas, and tests proving bounded memory under churn.

#### FUN-053: Introduction proof-of-work is not bound to the submitted envelope

- Location: `relay-server/src/introduction.rs:103-128`
- Evidence: the challenge is cryptographically validated, but its requester and target are not compared with the authenticated envelope's requester/target fields.
- Impact: work issued for one target can bypass target-specific accounting when submitting to another.
- Required action: require requester, target, action, relay, and audience to match exactly before admission.

#### FUN-054: Persistent relay storage has no global/source budgets

- Locations: `relay-server/src/main.rs:865-874`, `relay-server/src/db.rs:33-143`, `relay-server/src/board_service.rs:1002-1051`
- Evidence: peer registration, posts, grants, introductions, and social events lack a coherent global/source storage budget or retention policy. New peer IDs evade per-peer limits, and social events need not reference a valid authorized post.
- Impact: a Sybil attacker can exhaust disk and retain malformed data indefinitely.
- Required action: add source/global admission controls, per-author quotas, referential/visibility validation, database size thresholds, and retention/compaction.

#### FUN-055: Blocking database work and response delay run inside the swarm event loop

- Location: `relay-server/src/main.rs:857-886`
- Evidence: synchronous SQLite/mutex request processing executes in the libp2p polling loop, followed by a 20-40 ms sleep for generic introduction responses.
- Impact: modest concurrent or Sybil traffic can stall relay forwarding and every other protocol.
- Required action: move blocking work to bounded workers and schedule delayed responses without pausing swarm polling.

#### FUN-056: Relay circuits are configured with effectively unbounded resource lifetimes

- Locations: `relay-server/src/main.rs:724-735`, `relay-server/src/main.rs:771-773`, `relay-server/README.md:153-159`
- Evidence: circuits last seven days, bytes are unlimited, and idle connection duration is near one year, while documentation says circuits are limited to 1 MB.
- Impact: attackers can occupy circuit slots and bandwidth for prolonged periods.
- Required action: enforce configurable finite byte/duration/idle limits with production-safe defaults and accurate documentation.

#### FUN-057: Community board pagination repeats newer rows and can lose older history

- Location: `relay-server/src/db.rs:325-365`
- Evidence: rows are sorted newest-first while the cursor filter selects `created_at > cursor`.
- Impact: “load more” repeats already-seen rows; advancing an incremental cursor can permanently skip older rows outside the first page.
- Required action: use a stable `(created_at, post_id)` cursor whose comparison matches sort direction, with separate older-page and new-since operations.

#### FUN-058: Lightweight relay deployment surfaces advertise identity namespaces but disable identity transport

- Locations: `relay-server/src/main.rs:642-692`, `relay-server/src/main.rs:751-762`, `relay-server/README.md:43-47`, `infrastructure/relay-cloudformation.yaml:21-27`
- Evidence: identity/board request-response behavior is constructed only under `--community`, while relay-only documentation/templates expose an identity namespace without enabling community mode.
- Impact: a deployed “identity relay” can silently lack the identity functionality clients expect.
- Required action: make identity transport independent from community boards or remove identity claims/options from relay-only deployment surfaces.

#### FUN-059: Community data is lost on instance replacement while identity and IP survive

- Locations: `infrastructure/community-relay-cloudformation.yaml:237-242`, `infrastructure/community-relay-cloudformation.yaml:291-292`, `infrastructure/community-relay-cloudformation.yaml:390`
- Evidence: SQLite is stored on the disposable root volume; only identity and Elastic IP are preserved externally.
- Impact: replacement presents the same relay identity/address while losing boards, walls, grants, and queued introductions.
- Required action: use a retained encrypted data volume or managed database with tested backup/restore and explicit recovery semantics.

#### FUN-060: Relay identity-key files can be created world-readable

- Location: `relay-server/src/main.rs:588-604`
- Evidence: ordinary `fs::write` creation commonly follows a `0644` umask and existing permissions are not validated.
- Impact: other local users/processes can read the long-lived relay identity key on shared hosts or custom paths.
- Required action: use atomic create with `0600`, validate existing ownership/mode, and fail closed on unsafe permissions.

### Medium severity

#### FUN-014: Unknown stored wall-event types become comments

- Location: `src-tauri/src/db/repositories/wall_social_repo.rs:182-195`
- Evidence: parse failure defaults to `LegacyCommentCreate`.
- Impact: corrupt or future event types can be materialized as the wrong action instead of being rejected or quarantined.
- Required action: return a typed conversion error and quarantine/report the row.

#### FUN-015: Message decryption failure is represented as ordinary message content

- Location: `src-tauri/src/services/messaging_service.rs:535-545`
- Evidence: every cryptographic failure becomes the string `[Decryption failed]` in a normal `DecryptedMessage`.
- Impact: tampering, wrong keys, corruption, and unsupported formats are indistinguishable and cannot drive recovery or security diagnostics.
- Required action: return a structured decryption status without exposing ciphertext or treating failure text as authored content.

#### FUN-016: Introduction queue write failures are silent internally

- Location: `relay-server/src/introduction.rs:153-165`
- Evidence: the public generic acceptance response is correctly oracle-resistant, but the database insert result is discarded without logging or a metric.
- Impact: the requester receives the intended opaque response, but operators cannot distinguish successful admission from storage failure.
- Required action: retain the identical public response while recording an internal structured failure metric/log.

#### FUN-017: Identity lookup errors collapse into an unverified-name state

- Locations: `src-tauri/src/services/feed_service.rs:108-129`, `src-tauri/src/services/feed_service.rs:171-189`
- Evidence: contact and verified-name repository errors are converted with `.ok().flatten()`.
- Impact: database failures masquerade as ordinary missing identity data and make trust-state failures difficult to diagnose.
- Required action: distinguish absent data from lookup failure in service results and UI state.

#### FUN-018: Background media preloading failures are discarded

- Locations: `src/stores/feed.ts:341-348`, `src/stores/feed.ts:441-442`, `src/stores/contactWall.ts:151-159`
- Evidence: preloading promises end in empty catches.
- Impact: media silently remains unavailable and the diagnostics cannot explain whether discovery, authorization, transfer, or cache admission failed.
- Required action: keep best-effort behavior but record structured diagnostics and expose retry/failure state where media is visible.

#### FUN-040: Network startup can leave a dead handle reported as running

- Locations: `src-tauri/src/commands/network.rs:61`, `src-tauri/src/commands/network.rs:195-231`, `src-tauri/src/p2p/network.rs:1297`
- Evidence: the handle is stored before the event loop successfully starts listening. If listener startup fails, the task exits but the stored handle remains and later starts return success.
- Impact: the UI can remain “running” while no network loop exists, with no automatic recovery.
- Required action: introduce an explicit readiness handshake, publish the handle only after listeners start, and clear/fail state when the task exits.

#### FUN-041: Persisted bootstrap-node configuration is not loaded at network startup

- Locations: `src-tauri/src/commands/bootstrap.rs:17`, `src-tauri/src/db/repositories/bootstrap_repo.rs:213`, `src-tauri/src/commands/network.rs:176`, `src-tauri/src/p2p/config.rs:27`
- Evidence: enabled/prioritized nodes can be persisted, but startup constructs `NetworkConfig::default()` with no database nodes. The enabled-address query has no production consumer.
- Impact: a presented network configuration surface does not affect startup connectivity.
- Required action: load and validate enabled nodes before constructing the network or remove the redundant persistence/API.

#### FUN-042: Database absence fallbacks also hide corruption and can regress cryptographic counters

- Locations: `src-tauri/src/db/connection.rs:114-125`, `src-tauri/src/db/connection.rs:419`, `src-tauri/src/db/connection.rs:459`, `src-tauri/src/db/connection.rs:471-481`, `src-tauri/src/db/migrations/011_posts_lamport_index.sql:9`
- Evidence: arbitrary schema-version and counter query errors become zero. Migrations contain partial-database repair branches, are not one global transaction, and migration 011 records the wrong schema version.
- Impact: corrupted state can be treated as a fresh database; Lamport clocks can regress and send counters can reuse AES-GCM nonces.
- Required action: only treat `QueryReturnedNoRows` as absence, otherwise fail closed. Because there are no customers, squash to one validated beta schema after deliberately preserving or discarding named tester profiles.

#### FUN-043: Media file storage can succeed without required transfer/cache state

- Locations: `src-tauri/src/services/media_service.rs:91`, `src-tauri/src/services/media_service.rs:127`
- Evidence: file storage discards failure from `ensure_transfer` and returns the new hash.
- Impact: later media lifecycle operations can be missing for a file the caller believes was fully stored.
- Required action: commit file and metadata coherently or remove the created file and return failure.

#### FUN-061: Sound notifications are implemented but their only setting is unreachable

- Locations: `src/services/audioNotifications.ts:59-60`, `src/pages/settings/NotificationsSection.tsx:7-38`, `src/pages/Settings.tsx`
- Evidence: sound honors `soundEnabled`, but the live monolithic Settings route does not render the modular Notifications section containing the switch.
- Impact: users cannot discover or change an active notification behavior through the production settings UI.
- Required action: consolidate Settings and expose the real toggle once.

#### FUN-062: Feed media-metadata failure erases evidence that an attachment exists

- Location: `src/pages/Feed.tsx:543-572`
- Evidence: `getPostMedia` errors are caught and converted to no media, so transfer/loading UI cannot distinguish an attachment metadata failure from a text-only post.
- Impact: users see missing media with no placeholder, retry, or explanation.
- Required action: preserve declared attachment state and surface metadata/transfer failure explicitly.

#### FUN-063: Empty-feed “Find Contacts” control does nothing

- Location: `src/pages/Feed.tsx:963-972`
- Evidence: the rendered button has no click handler or navigation.
- Impact: a primary empty-state onboarding action is inert.
- Required action: navigate to the contact discovery/request surface or remove the button.

#### FUN-064: Mentioned posts reject every attachment

- Location: `src/components/common/ComposePostModal.tsx:134-136`
- Evidence: mention presence causes attachment submission to fail rather than publishing a signed mention alongside media.
- Impact: an exposed composition combination is intentionally incomplete without explaining the limitation before submission.
- Required action: support signed media plus mention metadata or disable the combination in the composer with clear copy.

#### FUN-065: Clipboard operations report success before the browser confirms the write

- Locations: `src/pages/Settings.tsx:329-335`, `src/pages/Network.tsx:69-77`
- Evidence: clipboard promises are not awaited before success toasts.
- Impact: permission or platform clipboard failure is presented as success.
- Required action: await the write and report rejection.

#### FUN-066: Settings contains contradictory hard-coded and runtime versions

- Location: `src/pages/Settings.tsx:750-754`, `src/pages/Settings.tsx:1781-1788`
- Evidence: one production surface renders `Harbor v1.0.0` while another reads the installed application version.
- Impact: misleading diagnostics and support reports.
- Required action: use the single runtime version source everywhere.

#### FUN-067: Stale wall deletion reports success without changing relay state

- Location: `relay-server/src/db.rs:927-948`
- Evidence: a delete with an older Lamport clock returns `Ok(true)` while leaving the row unchanged; the service reports deletion.
- Impact: clients cannot distinguish idempotent completion from rejected stale state.
- Required action: return an explicit stale-clock/conflict outcome and propagate it to UI reconciliation.

#### FUN-068: Unchecked integer conversions can corrupt relay clocks and name sequences

- Locations: `relay-server/src/name_registration.rs:99`, `relay-server/src/name_registration.rs:144-150`, `relay-server/src/name_registration.rs:188`, `relay-server/src/db.rs:320`, `relay-server/src/db.rs:481`, `relay-server/src/db.rs:553`, `relay-server/src/db.rs:575`, `relay-server/src/db.rs:611-652`, `relay-server/src/db.rs:938-959`
- Evidence: protocol `u64` values are cast to SQLite `i64`; negative reads and signed subtraction are not checked.
- Impact: wraparound, panic, invisible posts, or permanently advanced/frozen streams.
- Required action: validate representable ranges at protocol boundaries and use checked arithmetic.

#### FUN-069: Relay media migration manufactures invalid signed state

- Location: `relay-server/src/db.rs:208-247`
- Evidence: missing signatures are filled with empty bytes and duplicate rows are deleted by retaining arbitrary `MIN(id)`.
- Impact: corrupted or unsigned metadata is made to look schema-valid while potentially deleting the authoritative duplicate.
- Required action: remove this pre-customer compatibility path and require a clean schema or explicit validated converter.

#### FUN-070: Relay key-rotation generator and verifier use incompatible models

- Locations: `relay-server/src/key_rotation.rs:6-27`, `relay-server/src/bin/relay-key-rotation.rs:38-57`, `relay-server/src/bin/relay-key-rotation.rs:106-126`
- Evidence: field names, time fields, nesting, and public-key encoding differ, and the library does not validate the version.
- Impact: generated rotation artifacts cannot reliably be consumed by the validating implementation.
- Required action: define one shared wire model and golden vectors; delete the alternate structure.

#### FUN-071: Relay updater fails to roll back after an immediate service crash

- Location: `infrastructure/scripts/update-relay.sh:125-145`
- Evidence: rollback occurs only when `systemctl start` itself fails. A process that starts and then exits makes `systemctl is-active` terminate the script under `set -e` before rollback.
- Impact: an update can leave the relay offline despite a retained previous binary.
- Required action: wrap a timed protocol/port health check in the rollback branch and verify identity/version before committing the update.

#### FUN-072: Docker identity persistence depends on a flag the image does not supply

- Location: `relay-server/Dockerfile:18-29`
- Evidence: the image sets `IDENTITY_KEY_PATH`, but Clap is not configured to read that environment variable. Direct image execution writes under the container user's home outside the declared volume.
- Impact: relay identity can change across container replacement despite apparent persistence configuration.
- Required action: pass `--identity-key-path` in a tested entrypoint or add explicit environment binding.

#### FUN-073: Relay advertises wildcard listen addresses externally

- Location: `relay-server/src/main.rs:802-813`
- Evidence: `/ip4/0.0.0.0/...` is added as an external address even when a real announced IP is configured.
- Impact: peers receive a non-routable address and diagnostics/address selection become unreliable.
- Required action: never publish wildcard listen addresses; advertise only validated reachable addresses.

#### FUN-074: Relay read requests are not bound to the connected transport peer

- Locations: `relay-server/src/main.rs:1175-1234`, `relay-server/src/main.rs:1318-1391`, `relay-server/src/main.rs:1467-1506`
- Evidence: mutations bind author to the Noise-authenticated peer, while list/wall/social reads only verify the supplied identity signature.
- Impact: captured signed read requests can be replayed by another connection and this compounds the requester-time expiry bypass.
- Required action: require requester identity to equal the connected peer and enforce freshness/nonces.

#### FUN-075: Community registration does not prove signing key ownership of the PeerId

- Location: `relay-server/src/board_service.rs:411-430`
- Evidence: the connection binds the claimed peer ID, but the stored Ed25519 signing key can be unrelated to that libp2p PeerId.
- Impact: Harbor has two contradictory identity roots for the same community author.
- Required action: enforce key-to-PeerId derivation or explicitly version a separate application signing identity with a signed binding.

#### FUN-076: Signed WallRead grant scope is ignored

- Locations: `relay-server/src/board_service.rs:810-848`, `relay-server/src/board_service.rs:961-965`, `relay-server/src/db.rs:514-531`
- Evidence: scope is signed and stored, but any active WallRead grant becomes global access.
- Impact: future or existing scoped grants are broader than their signed intent.
- Required action: reject non-null scopes until semantics are implemented, then enforce them on every read.

#### FUN-077: Relay stores malformed or unauthorized social-event junk

- Location: `relay-server/src/board_service.rs:1002-1051`
- Evidence: the relay verifies signatures but not recognized event type, canonical payload/field agreement, post existence, visibility/access, or canonical author name.
- Impact: permanent storage can be filled with inconsistent events that clients later reject.
- Required action: parse the canonical payload server-side, compare all fields, verify referenced post/access, and derive names from claims before storage.

#### FUN-078: Name registration maps operational SQLite failures to ordinary unavailability

- Location: `relay-server/src/name_registration.rs:181-202`
- Evidence: disk-full, corruption, and schema errors can be returned as if a name conflict occurred.
- Impact: operators and users diagnose an infrastructure failure as a normal naming decision.
- Required action: external errors may remain generic, but internal logs/metrics must retain the actual class and alert on operational failure.

#### FUN-079: AWS templates expose SSH globally even when the key parameter says SSH is disabled

- Locations: `infrastructure/relay-cloudformation.yaml:144-149`, `infrastructure/community-relay-cloudformation.yaml:153-158`
- Evidence: the ingress rule is unconditional; an empty key only prevents ordinary key-based login configuration.
- Impact: unnecessary public attack surface and misleading deployment controls.
- Required action: conditionally omit SSH ingress and require an explicit restricted administrator CIDR when enabled.

#### FUN-080: Repository parsers convert corrupt call/sync rows into plausible state

- Locations: `src-tauri/src/db/repositories/calls_repo.rs:276-288`, `src-tauri/src/db/repositories/boards_repo.rs:269`, `src-tauri/src/db/connection.rs:638`
- Evidence: unknown call direction/media/state becomes incoming/audio/ended, timestamps can become zero, and cursor/sync read errors become absent state.
- Impact: corruption is silently represented as normal history or causes sync replay rather than being quarantined.
- Required action: return typed conversion/query errors and separate missing rows from failed reads.

### Repository cleanup and deployment-surface findings

#### FUN-019: Demo data and unreachable demo branches remain in production source

- Severity: Low
- Locations: `src/stores/mockPeers.ts`, `src/stores/index.ts`, `src/pages/Chat.tsx:606`, `src/pages/Chat.tsx:2064-2069`
- Evidence: the mock store is no longer consumed by production pages, but it is exported and Chat retains `isReal: false` behavior and demo copy even though all constructed conversations are real.
- Impact: dead product modes increase cognitive load and can be accidentally reintroduced.
- Required action: delete the mock store and collapse the dead union/branches. Keep mock data in test fixtures or Storybook-only modules if needed.

#### FUN-020: Unused incompatible cryptographic helpers remain in the production service

- Severity: Low
- Location: `src-tauri/src/services/crypto_service.rs:63-74`, `src-tauri/src/services/crypto_service.rs:205-217`
- Evidence: the deprecated peer-ID helper explicitly produces a value that is not libp2p-compatible, and the old symmetric-key derivation is retained “for backwards compatibility.” Neither has a production caller outside its defining module.
- Impact: future code can accidentally select an invalid identity/key derivation path, and reviewers must continue reasoning about algorithms Harbor does not use.
- Required action: delete both helpers and their compatibility-only tests.

#### FUN-021: Public and agent-facing documentation asserts behavior that is not implemented

- Severity: High
- Locations: `README.md` Settings and usage sections; `CLAUDE.md`; `scripts/generate-signing-keys.md`
- Evidence: README says users can update the shared avatar, change passwords, export/import identities, control read receipts, and control mDNS. Those claims map to the fake or unwired controls above. `CLAUDE.md` still describes a nonexistent `D:\apps\chat-app` layout, mock-peer messaging/feed as current production behavior, and simulated security controls as implemented. Signing instructions still point at the old `nicholasoxford/harbor` updater endpoint despite that drift being recorded on July 1.
- Impact: users, contributors, and coding agents receive an inaccurate implementation contract and can reproduce the same release-readiness mistake.
- Required action: make current capability documentation generated or checked against the release inventory; delete stale session-history documentation and replace it with maintained architecture/operations guidance.

#### FUN-022: Checked-in local relay and two-profile scripts encode obsolete personal environments

- Severity: Low
- Locations: `scripts/build-local-relay.sh`, `scripts/run-bob.ps1`, `scripts/run-alice.bat`, `scripts/run-bob.bat`, `scripts/run-bob-dev.bat`, `src-tauri/tauri.bob.json`
- Evidence: scripts hardcode `/home/bakobi/repos/harbor`, `D:\apps\chat-app`, personal profile names, old npm commands, an old `0.1.3` app version, and fixed ports. No maintained documentation or package script references them.
- Impact: repository cruft and misleading test instructions; accidental execution can write to unexpected profile paths.
- Required action: delete them or replace the entire set with one platform-neutral isolated-profile harness used by acceptance tests.

#### FUN-023: Default relay Docker Compose configuration is not a generic deployable relay

- Severity: High
- Location: `relay-server/docker-compose.yml:14`
- Evidence: the default announcement address is the maintainer-specific `154.5.126.219`, and the command does not configure an identity namespace even though relay-scoped naming depends on one.
- Impact: a semi-technical operator following the checked-in deployment surface can announce an incorrect address and launch a relay missing the identity behavior the product expects.
- Required action: require `ANNOUNCE_IP` and `IDENTITY_NAMESPACE` explicitly, validate both at container startup, and use the same production artifact/config contract as the documented cloud deployment.

#### FUN-024: Development bootstrap scripts use npm against a pnpm-only repository

- Severity: Low
- Locations: `scripts/setup-dev.ps1`, `scripts/setup-dev.sh`, `src-tauri/tauri.bob.json`, older run scripts
- Evidence: the repository tracks `pnpm-lock.yaml` and CI uses pnpm, while setup scripts run `npm install` and advertise `npm run` commands.
- Impact: contributors can create a divergent dependency graph and hit the same native optional-dependency inconsistencies seen during this audit.
- Required action: standardize bootstrap and all examples on the pinned pnpm version and frozen lockfile behavior.

#### FUN-081: A stale third CloudFormation template bypasses maintained deployment controls

- Severity: Low
- Location: `src-tauri/harbor-relay-cloudformation.yaml`
- Evidence: this unreferenced copy clones mutable `main`, does not preserve relay identity, and uses globally fixed resource names. The application and documentation use the templates under `infrastructure/` instead.
- Impact: operators or contributors can deploy an obsolete and materially less safe relay by selecting the wrong checked-in template.
- Required action: delete the duplicate.

#### FUN-082: Relay build scripts produce inconsistent artifacts and checksums

- Severity: Low
- Locations: `scripts/build-relay.sh`, `scripts/build-relay.ps1`
- Evidence: the Linux script verifies version/help and updates deployment hashes; the PowerShell script skips those checks and overwrites the shared checksum with a Windows executable checksum.
- Impact: platform choice changes what “the relay checksum” means and can invalidate deployment metadata.
- Required action: replace both with one cross-platform build contract producing platform-qualified artifacts/manifests.

#### FUN-083: Public relay events are logged at info level without volume control

- Severity: Low
- Location: `relay-server/src/main.rs:847-849`
- Evidence: every relay event is formatted at info level.
- Impact: routine or hostile traffic can generate excessive journal volume and obscure actionable events.
- Required action: move high-volume events to debug/trace and add structured sampling/rate limiting.

## Intentional fallbacks reviewed and accepted

The following are not defects in their present direction, although some should gain diagnostics:

- Invalid stored post visibility defaults to contacts-only rather than public.
- Media authorization treats lookup failures as unknown or blocked.
- Relay introduction responses deliberately do not reveal whether a target exists.
- Decoy delivery keys prevent target enumeration.
- Introduction queue-count errors behave as a full queue.
- Provider embeds default to per-use consent.
- New posts default to contacts-only visibility.
- Offline dialing after durable contact addition may be nonfatal, provided the returned state says that the contact is saved but offline.

## Pass 1 legacy and compatibility inventory

Harbor has no customer compatibility obligation. The following paths should not remain permanent production behavior:

| Compatibility path | Current purpose | Disposition |
| --- | --- | --- |
| `LegacyIdentityMigration`, `compatibility` publishing mode, and `identity_migration_state` compatibility value | Allows pre-name identities to enter and publish without a verified name | Remove from production. Run a one-time explicit converter for the named retained tester profiles, or require those profiles to claim names. |
| Raw `/p2p/` multiaddress contact path | Bypasses the current invite/request UX | Remove from normal UI. If direct dialing is useful, keep it only in an explicitly labeled developer diagnostics surface without contact trust/grants. |
| Double-base64 invite key decoder | Accepts output from an earlier encoding bug | Remove; accept one canonical versioned invite encoding. |
| Direct-add unsigned contact bundles | Old “no handshake needed” workflow | Remove entirely; invite data is discovery-only. |
| Migrations 001-023 and partial-database repair branches | Supports every development schema accumulated before public beta | Squash to one clean beta schema. Preserve only explicitly selected local tester profiles through a reviewed one-time conversion. |
| `legacy_comment_create`, `legacy_reaction_add`, unsigned bridge rows | Preserves pre-signature local social rows | Remove variants, bridge, and exporter behavior; do not publish unsigned history. |
| Relay media signature backfill/dedup compatibility | Repairs old unsigned/duplicate relay rows | Remove and reset/convert the pre-customer relay database explicitly. |
| Deprecated incompatible peer-ID and old symmetric-key helpers | No production caller | Delete. |
| Mock peer store and `isReal` UI unions | Earlier demo mode | Delete from production; retain only test fixtures where needed. |
| Duplicate modular `src/pages/settings/` tree | Abandoned Settings decomposition | Choose one implementation and delete the other; do not merge fake controls forward. |
| `tauri.bob.json`, Alice/Bob launch scripts, local relay script | Ad hoc early testing | Replace with the maintained isolated-profile acceptance harness. |
| Alternate relay key-rotation model | Unused/incompatible implementation | Delete after one canonical model and golden vector exist. |

The term “compatibility” remains legitimate where it means current interoperability with an external standard, such as deriving a libp2p-compatible PeerId. Those paths are not legacy product modes and should be named after the invariant they satisfy rather than grouped with migration behavior.

## Pass 1 remaining gaps and TODO inventory

The marker scan found very few conventional `TODO`/`FIXME` comments. The incomplete work is predominantly presented as finished behavior, swallowed errors, dead duplicate implementations, or permissive compatibility branches. The functional findings above are therefore the authoritative gap inventory.

Explicit marker-backed gaps found in production source:

- simulated password change, recovery, and deletion;
- placeholder backup key material;
- hardcoded “assume online” presence;
- demo/mock conversation and feed branches;
- incomplete mention-plus-media composition;
- legacy/raw contact paths and compatibility identity publishing.

Systematic coverage used `dev walk` manifests for `src/`, `src-tauri/src/`, and `relay-server/`, followed by production-only searches and call-site tracing. Deployment templates, workflows, setup/build scripts, Docker surfaces, public capability docs, and release gates were also reviewed.

## Pass 2: engineering quality, maintainability, complexity, and efficiency

### Structural baseline

The production source inventory contains 225 TypeScript/TSX/Rust files and approximately 73,556 lines excluding standalone frontend test files. Raw line count is not itself a defect, but Harbor concentrates unrelated responsibilities in a small set of unusually large modules:

| Module | Lines | Responsibilities currently combined |
| --- | ---: | --- |
| `src-tauri/src/p2p/network.rs` | 5,597 | network actor, every protocol dispatcher, pending-operation state, authorization orchestration, media I/O, relay discovery, and application events |
| `src/pages/Settings.tsx` | 2,370 | profile, security, privacy, appearance, network, media cache, updates, bug reports, and about UI |
| `src/pages/Chat.tsx` | 2,084 | conversation indexing, message loading/search/rendering, attachments, calling, menus, shortcuts, and dialogs |
| `src/pages/Network.tsx` | 1,671 | diagnostics, polling, contacts, relays, invite links, and deployment UI |
| `relay-server/src/board_service.rs` | 1,715 | registration, boards, walls, grants, social events, authorization, and protocol-to-database mapping |
| `relay-server/src/main.rs` | 1,509 | CLI/configuration, identity, swarm construction, event loop, protocol schema, authentication, and all request dispatch |
| `src-tauri/src/services/content_sync_service.rs` | 1,483 | manifest construction, authorization, ingest, cursor management, and tests |
| `src/pages/Wall.tsx` | 1,449 | composition, wall modes, preview, filtering, rendering, social actions, and local UI state |
| `src-tauri/src/services/media_service.rs` | 1,346 | file storage, transfer state, cache policy, eviction, database access, and tests |
| `src/pages/Feed.tsx` | 1,206 | post rendering, social controls, link previews, media loading, filtering, and feed orchestration |
| `src/services/callingRuntime.ts` | 1,163 | peer-connection lifecycle, media devices, signaling, timers, group-call state, and UI snapshots |

The first-pass defects are not isolated accidents. They cluster at these responsibility boundaries, where a single change must preserve UI state, persistence, cryptographic invariants, protocol correlation, and network event ordering at once.

### High severity

#### QUAL-001: The network actor is a service locator and multi-protocol state machine in one file

- Locations: `src-tauri/src/p2p/network.rs:917-971`, `src-tauri/src/p2p/network.rs:995-1096`, `src-tauri/src/p2p/network.rs:3072-3823`, `src-tauri/src/p2p/network.rs:4357-5441`
- Evidence: `NetworkService` owns the swarm, ten optional application services, eleven collections of connection/pending-operation state, and all event routing. Dependencies are injected later through setters, so valid construction leaves every application service absent. One response handler is roughly 750 lines and the command match is roughly 1,085 lines.
- Impact: protocol invariants are distributed across distant match arms and optional-service branches. Missing wiring becomes an ordinary runtime error or silent return rather than an impossible construction state. The nonce, contact transition, media, and pending-request defects in Pass 1 all cross this boundary.
- Required action: split transport ownership from protocol handlers. Give each protocol a typed state machine with required constructor dependencies, bounded pending operations, explicit timeout/cancellation, and a small command/event interface. Keep the swarm loop responsible only for scheduling and dispatch.

#### QUAL-002: Client and relay maintain separate copies of the same wire protocol

- Locations: `src-tauri/src/p2p/protocols/board_sync.rs:123-416`, `relay-server/src/main.rs:248-566`
- Evidence: `BoardSyncRequest`, `BoardSyncResponse`, and their post/media/social DTOs are independently declared in the client and relay crates. They already use different local type names and module paths. There is no shared protocol crate or client-to-relay golden-vector compatibility gate.
- Impact: either side can compile and pass its own unit tests while producing a protocol the other side cannot decode or authorize. The incompatible relay key-rotation structures in `FUN-070` demonstrate that this drift is already occurring elsewhere in the same boundary.
- Required action: move versioned wire types, canonical signing bytes, limits, and golden vectors into one dependency used by both binaries. Test every request and response by encoding on one side and decoding/verifying on the other.

#### QUAL-003: The account registry uses unsynchronized, non-atomic read-modify-write

- Locations: `src-tauri/src/services/accounts_service.rs:51-85`, `src-tauri/src/services/accounts_service.rs:123-215`, `src-tauri/src/services/accounts_service.rs:218-271`
- Evidence: every operation independently reads `accounts.json`, mutates an in-memory copy, and overwrites the live file with `fs::write`. `AccountsService` has no lock, temporary-file/rename commit, file sync, generation check, or recovery copy.
- Impact: concurrent metadata, active-account, or registration writes can lose one another. A crash or full disk during overwrite can corrupt the only chooser registry, making valid encrypted profiles appear missing. Removal commits the registry change before optional data deletion, so a deletion failure also leaves an unlisted profile directory.
- Required action: serialize registry mutations, write and sync a same-directory temporary file, atomically replace the live registry, retain a validated recovery copy, and make account removal a recoverable staged operation. Longer term, store this metadata transactionally with the profile index rather than in an ad hoc JSON database.

#### QUAL-004: Media is repeatedly copied in full across disk, Rust, IPC, and the webview

- Locations: `src/services/media.ts:23-35`, `src/stores/wall.ts:216-247`, `src/pages/Chat.tsx:931-940`, `src-tauri/src/commands/media.rs:31-96`, `src/components/common/PostMedia.tsx:51-119`, `src-tauri/src/p2p/network.rs:2389-2444`
- Evidence: upload reads the entire file into JavaScript, converts the typed array into a number array for IPC, buffers it again in Rust, and writes it synchronously. Display reads the entire file, base64-expands it into a data URL, returns it through IPC, and repeats this per mounted media item. P2P serving synchronously reads and serializes the entire attachment inside the network actor. The documented 10 MB limit still permits large transient allocations and event-loop stalls.
- Impact: media-heavy feeds multiply memory, serialization, and disk costs; video cannot be streamed or range-read; and serving one attachment can delay messages, calls, and other swarm events. The frontend comment claiming an `asset://` URL disagrees with the data-URL implementation.
- Required action: use Tauri's scoped asset/custom protocol with range support for display, pass selected file paths or streaming handles rather than arrays of numbers, use bounded streaming/chunks for P2P, and move blocking filesystem work off async/network event loops. Cache resolved URLs with explicit lifetime management.

#### QUAL-005: SQLite access serializes unrelated work and blocks async networking

- Locations: `src-tauri/src/db/connection.rs:30-89`, `relay-server/src/db.rs:146-180`, `src-tauri/src/p2p/network.rs:1297-1331`, `relay-server/src/main.rs:835-904`
- Evidence: each process shares one synchronous `rusqlite::Connection` behind a standard mutex. Network and relay event handlers call services that lock that connection and perform queries/transactions inline. The relay additionally uses `unwrap`/`expect` on the database mutex throughout `relay-server/src/db.rs:169-948`.
- Impact: a slow write, schema operation, or long query stops every other database consumer and can stall the single swarm event loop. In the relay, any panic while holding the mutex poisons it; the next routine request then panics the server rather than returning an operational error.
- Required action: keep database work outside the swarm loop through a bounded worker interface, establish short measured transaction scopes, configure SQLite for the chosen concurrency model, and return typed poisoned/storage errors. Add latency and queue-depth instrumentation before selecting pooling or a dedicated database actor.

#### QUAL-014: Relay database work includes request-time DDL, N+1 reads, and autocommit loops

- Locations: `relay-server/src/introduction.rs:79-86`, `relay-server/src/introduction.rs:197-217`, `relay-server/src/board_service.rs:766-797`, `relay-server/src/board_service.rs:985-996`
- Evidence: constructing `IntroductionService` executes schema DDL during introduction requests; one acknowledgement performs up to 100 independent deletes; wall media is inserted one row at a time; and wall reads query media once per returned post.
- Impact: public requests create unpredictable lock duration and write amplification on the single relay connection. Partial autocommit completion also compounds the consistency findings from Pass 1.
- Required action: perform versioned schema work only at startup, batch acknowledgements, and fetch/store a post plus media in one bounded transaction/query plan.

#### QUAL-015: The deployment helper converts real update failures into success

- Location: `infrastructure/scripts/deploy-stack.sh:15-22`, `infrastructure/scripts/deploy-stack.sh:79-85`, `infrastructure/scripts/deploy-stack.sh:94-117`
- Evidence: CloudFormation parameters are assembled in an unquoted string, so the default `Harbor Community` is split into multiple shell arguments. Every `update-stack` failure is then reported as `No updates needed` and exits successfully, without checking AWS's specific no-update response.
- Impact: invalid parameters, templates, authorization, credentials, and AWS failures can all look like a successful deployment. Operators can proceed to DNS or testing against unchanged/broken infrastructure.
- Required action: build arguments as a Bash array, preserve stderr and exit status, and match only the documented no-update condition. Add ShellCheck and mocked failure-path tests.

#### QUAL-016: Stack updates can silently reset operator configuration

- Location: `infrastructure/scripts/deploy-stack.sh:79-85`, `infrastructure/scripts/deploy-stack.sh:107-122`
- Evidence: update calls supply only a subset of template parameters rather than `UsePreviousValue` for omitted values, including key-pair and circuit configuration. The script applies the update directly instead of presenting a change set.
- Impact: a routine update can revert console-customized values or replace resources without an operator seeing the effective plan.
- Required action: explicitly preserve every omitted parameter and create, inspect, and approve a change set before execution when replacement or network/identity change is possible.

#### QUAL-017: CloudFormation completion is not tied to a working relay

- Locations: `infrastructure/relay-cloudformation.yaml:215-234`, `infrastructure/relay-cloudformation.yaml:390-423`, `infrastructure/community-relay-cloudformation.yaml:224-243`, `infrastructure/community-relay-cloudformation.yaml:401-434`
- Evidence: the instance has no `CreationPolicy`/`cfn-signal`. Bootstrap's port-check loop falls through after ten failures and still prints/publishes the relay address as success.
- Impact: AWS can report `CREATE_COMPLETE` and expose an address even though user-data, the service, or the protocol is not operational.
- Required action: signal stack success only after an authenticated protocol-level readiness check; make exhausted health checks fail bootstrap and surface the relevant logs.

#### QUAL-018: Relay release artifacts are manual, architecture-ambiguous, and mutable

- Locations: `scripts/build-relay.sh:7-37`, `.github/workflows/ci.yml:67-80`, `.github/workflows/release.yml:23-63`, `infrastructure/relay-cloudformation.yaml:219-270`, `infrastructure/community-relay-cloudformation.yaml:228-279`
- Evidence: the build script compiles for its current host and copies to the generic tracked path `relay-server/bin/harbor-relay`; CI checks relay source but does not prove that binary/checksum/template metadata match it; application releases do not build a relay; and templates fetch from `raw/main` while selecting a moving AL2023 image.
- Impact: an ARM development host can populate a path consumed by x86-64 AWS instances, retained templates can fetch different future bytes, and source review does not establish what operators deploy.
- Required action: produce versioned OS/architecture-qualified relay artifacts in CI, verify ELF architecture and smoke tests, publish them immutably with generated checksums/SBOM/provenance, and reference an immutable artifact and tested image version from generated templates.

#### QUAL-019: Relay updates take unsafe live database copies and retain them without limit

- Location: `infrastructure/scripts/update-relay.sh:125-140`
- Evidence: the community data directory is recursively copied before the service is stopped, so SQLite files may change during the copy. Each update retains another full database, binary, and unit backup without a retention or disk-space policy.
- Impact: the rollback copy can be internally inconsistent, while repeated updates can consume the same disk that holds the live community database.
- Required action: stop/checkpoint or use SQLite's online backup API, validate the recovery copy, check available space, and enforce tested retention.

#### QUAL-020: Community relay data has no recovery or availability design

- Location: `infrastructure/community-relay-cloudformation.yaml:237-242`, `infrastructure/community-relay-cloudformation.yaml:291-292`, `infrastructure/community-relay-cloudformation.yaml:390`
- Evidence: the only database resides on the instance root volume. There is no retained data volume, snapshot/backup schedule, restore validation, standby, or declared recovery objective.
- Impact: instance or volume loss destroys community names, boards, posts, grants, and queued introductions even if the relay identity key survives separately.
- Required action: use retained encrypted state storage, automate backups/snapshots, define recovery-point/time objectives, and continuously test restoration with identity continuity.

#### QUAL-021: The relay has no production health or operational telemetry

- Locations: `relay-server/src/main.rs:607-614`, `relay-server/src/main.rs:835-904`, `infrastructure/relay-cloudformation.yaml:374-381`, `infrastructure/community-relay-cloudformation.yaml:385-392`
- Evidence: operations expose human-readable journal logs only. There is no readiness/liveness surface, request/database latency, queue depth, rate-limit counts, storage/disk monitoring, alarms, or bounded log-retention configuration.
- Impact: overload, stuck database work, disk exhaustion, failed introductions, and a dead protocol can remain invisible until a user reports failure.
- Required action: add a local authenticated admin/health surface, structured metrics/traces, bounded logs, and infrastructure alarms tied to explicit service-level objectives.

#### QUAL-022: Relay tests do not cross the actual server/network boundary

- Locations: `relay-server/tests/identity_multi_relay.rs:88-239`, `relay-server/src/bin/relay-smoke.rs:119-218`
- Evidence: the integration test calls services/databases directly instead of launching `harbor-relay`, serializing the real wire types, and exercising transport-peer binding/event-loop behavior. The circuit smoke utility loops rather than providing a bounded CI pass/fail result.
- Impact: both sides can pass unit/integration tests while being wire-incompatible, deadlocking under real scheduling, or accepting an identity on a connection they would reject in process.
- Required action: add process-level tests with real libp2p clients, cross-crate golden vectors, concurrent/slow request cases, restart/recovery, and bounded smoke exits.

#### QUAL-023: Infrastructure and relay packaging have no automated quality gate

- Location: `.github/workflows/ci.yml:67-80`
- Evidence: relay CI runs Rust checks only. It does not validate CloudFormation, ShellCheck deployment/update scripts, build the Docker image, verify artifact checksums/architecture, scan bootstrap output for secrets, or exercise updater rollback.
- Impact: operationally fatal or insecure changes can merge while all named relay checks are green.
- Required action: add dedicated infrastructure and artifact jobs using `cfn-lint`/AWS validation, ShellCheck plus failure tests, container builds, checksum/architecture assertions, secret-log scanning, and update/rollback scenarios.

#### QUAL-032: A remote relay can make the client solve effectively unbounded proof-of-work

- Locations: `src-tauri/src/p2p/network.rs:51-77`, `src-tauri/src/p2p/protocols/board_sync.rs:50-60`, `src-tauri/src/p2p/network.rs:3203-3210`
- Evidence: `solve_work` searches toward `u64::MAX` using a remote `difficulty: u8` with no authenticated maximum, deadline, or work budget. The only swarm actor awaits the `spawn_blocking` result rather than continuing to poll networking.
- Impact: a malicious or misconfigured relay can freeze all client networking indefinitely and occupy a blocking worker.
- Required action: authenticate and strictly cap challenge parameters, enforce time/work budgets, and return computation completion to the actor as an asynchronous correlated event without awaiting it in the swarm loop.

#### QUAL-033: Relay operations are correlated by peer rather than request

- Locations: `src-tauri/src/p2p/network.rs:966-969`, `src-tauri/src/p2p/network.rs:2072-2101`, `src-tauri/src/p2p/network.rs:3080-3086`, `src-tauri/src/p2p/network.rs:3115-3148`
- Evidence: pending name, introduction, and delivery operations use separate maps keyed only by relay `PeerId`. Response dispatch discards request correlation and chooses an operation using priority/containment heuristics; outbound failure can fail a name request while leaving the actual failed introduction pending.
- Impact: concurrent operations against the same relay can consume one another's challenges/sessions/responses, produce cross-talk, or hang permanently.
- Required action: use one typed operation state machine keyed by the libp2p outbound request/correlation ID, with expected response phase, operation kind, caller, and deadline.

#### QUAL-034: Network stop/restart does not supervise all background workers

- Locations: `src-tauri/src/commands/network.rs:195-229`, `src-tauri/src/commands/network.rs:254-267`, `src-tauri/src/p2p/network.rs:364-372`
- Evidence: each network start spawns an uncancellable infinite private-mention worker. Stop removes the handle and sends shutdown but receives no acknowledgement and does not await either the swarm actor or ancillary workers.
- Impact: repeated starts leak zombie mention loops and can overlap old/new swarms for the same profile, duplicating work and racing durable state.
- Required action: introduce a profile-scoped runtime supervisor owning cancellation tokens and join handles. Stop must reject new work, cancel workers, receive actor acknowledgement, and await bounded termination.

#### QUAL-035: Most relay-facing client operations have no deadline or cleanup

- Locations: `src-tauri/src/p2p/network.rs:164-271`, `src-tauri/src/p2p/network.rs:4472-4479`, `src-tauri/src/commands/network.rs:209-227`
- Evidence: delivery resolution and introduction fetch/submit await oneshots without timeouts. Only name registration has a caller timeout, and abandoned actor state is removed only on a later retry. The mention worker processes work sequentially, so one missing response stalls the queue.
- Impact: packet loss, disconnect, relay restart, or correlation failure can leave the UI/background worker hung forever and block subsequent work.
- Required action: make deadlines actor-owned for every operation; complete callers and remove state on timeout, disconnect, outbound failure, cancellation, and shutdown; apply bounded retry/backoff outside the actor.

#### QUAL-036: Domain mutations commit counters, events, and projections separately

- Locations: `src-tauri/src/services/posts_service.rs:373-458`, `src-tauri/src/services/posts_service.rs:497-558`, `src-tauri/src/services/posts_service.rs:589-632`, `src-tauri/src/services/wall_social_service.rs:84-203`, `src-tauri/src/services/permissions_service.rs:160-293`
- Evidence: post create/update/delete, media rows, Lamport counters, social events/materialized likes/comments, and permission events/projections are persisted through separate repository calls and commits.
- Impact: any later failure leaves event history, materialized state, counters, media, or outbound intent inconsistent. Restart/retry cannot reliably determine which stage completed.
- Required action: expose repository operations over a shared `Transaction`; atomically write the counter, immutable event, materialized projection, and durable outbox item for each domain command.

#### QUAL-044: Account lifecycle is not an explicit frontend state boundary

- Locations: `src/stores/settings.ts:254-291`, `src/stores/feed.ts:16-75`, `src/stores/messaging.ts:315-320`, `src/hooks/useTauriEvents.ts:64-74`, `src/pages/Feed.tsx:532-537`, `src/pages/Chat.tsx:679-683`, `src/pages/Boards.tsx:359-361`
- Evidence: identity-specific settings, feed preferences, and archived peer IDs use device-global storage keys. An identity change resets only media transfers; page hydration effects are not keyed to the active profile.
- Impact: after switching/locking accounts, another profile's avatar, privacy defaults, relays, ICE configuration, archived peers, or cached entities can remain visible or active until incidental reconciliation. This makes account-switch correctness impossible to enforce store by store.
- Required action: introduce one profile-session coordinator that atomically stops the old runtime, clears every identity-scoped store, and hydrates the new profile. Namespace profile data by verified local account ID and retain only genuine device preferences globally.

#### QUAL-045: Large pages subscribe to entire mutable stores

- Locations: `src/pages/Settings.tsx:177-195`, `src/pages/Chat.tsx:613-629`, `src/pages/Network.tsx:100-131`, `src/pages/Feed.tsx:490-520`, `src/pages/Wall.tsx:371-393`, `src/components/layout/MainLayout.tsx:77-81`
- Evidence: primary pages destructure whole Zustand stores rather than subscribing through narrow selectors. Any field change therefore rerenders the complete component; the Network page also refreshes store state every five seconds.
- Impact: status ticks, message delivery state, transfer progress, or unrelated settings rebuild 1,000-2,000-line component trees and their derived arrays. This amplifies the polling and N+1 work documented elsewhere.
- Required action: use narrow selectors and shallow equality, normalize entity state, and split stable memoized feature components so high-frequency state updates touch only their consumers.

#### QUAL-046: Async view/store reads can commit results for a stale selection

- Locations: `src/stores/boards.ts:84-155`, `src/stores/contactWall.ts:108-169`, `src/pages/Network.tsx:184-197`
- Evidence: boards and contact-wall actions capture a selected community/board/author, await remote/local work, then unconditionally replace current state. The shareable-contact effect likewise has no cancellation/generation check after relay status changes.
- Impact: rapid navigation or disconnect can display posts/addresses belonging to the previously selected context, creating both confusing and potentially privacy-sensitive cross-context UI.
- Required action: attach request generations or abort signals to every selection-scoped load and verify the context key before each commit. Normalize cached entities separately from the active selection.

#### QUAL-047: Production calls bypass tested frontend service adapters

- Locations: `src/services/messaging.ts:6-50`, `src/stores/messaging.ts:58-292`, `src/services/mentions.ts:11-23`
- Evidence: the messaging service adapter applies publishing policy and has unit tests, but the production store directly invokes every Tauri command. Mentions exposes another direct command path. Similar command ownership is split between service and store layers elsewhere.
- Impact: policy, error mapping, cancellation, telemetry, and tests apply only to whichever duplicate path a caller chooses; a passing adapter test may cover no production call.
- Required action: make backend authorization authoritative and route each frontend capability through one generated/typed adapter. Stores should consume adapters and own view state, not duplicate IPC contracts.

### Medium severity

#### QUAL-006: Event-driven refresh still performs broad full reloads and redundant polling

- Locations: `src/hooks/useTauriEvents.ts:33-63`, `src/services/reactiveRefresh.ts:47-93`, `src/pages/Network.tsx:145-168`
- Evidence: any post hint reloads the first 50 feed items and sometimes a contact wall; the fallback refresh reloads contacts, requests, messages, and posts every 60 seconds. While the Network page is open it also invokes four status/list commands every five seconds despite receiving network events.
- Impact: one small event causes repeated full SQLite queries, IPC serialization, store replacement, media discovery, and React rendering. Activity scales with open screens and polling intervals rather than with changed records.
- Required action: emit typed deltas/revisions, update normalized entities in place, reserve reconciliation for detected revision gaps, and make diagnostics polling slow/adaptive or push-driven. Instrument refresh cause and rows/bytes changed.

#### QUAL-007: Feed assembly contains database and IPC N+1 paths

- Locations: `src-tauri/src/services/feed_service.rs:84-133`, `src-tauri/src/commands/feed.rs:115-137`, `src/pages/Feed.tsx:539-580`, `src/stores/wall.ts:152-167`, `src/pages/ContactWall.tsx:78-114`, `src/stores/feed.ts:204-221`, `src/stores/contactWall.ts:228-243`
- Evidence: verified-name resolution performs a database lookup per post even though display names have a cache; wall preview resolves the same local name claim inside every post mapping; and Feed, Wall, and ContactWall issue one `get_post_media` command per item. Reactions replace the containing item array, retriggering media effects for unchanged posts.
- Impact: loading or merely liking an item in a 50-item feed can produce dozens of serialized database locks and IPC round trips before media bytes are resolved. Reactive refresh multiplies the work.
- Required action: return posts, canonical author labels, social counts, and attachment metadata from bounded batch queries. Cache per author within a request and provide a batch media endpoint or joined read model.

#### QUAL-008: Selecting a conversation loads the same messages twice and builds contacts quadratically

- Locations: `src/pages/Chat.tsx:634-643`, `src/pages/Chat.tsx:760-784`, `src/pages/Chat.tsx:905-950`, `src/stores/messaging.ts:97-140`
- Evidence: the selection effect calls `setActiveConversation`, whose store implementation loads messages, while a second page effect loads the same conversation again. Sending optimistically inserts one message and then reloads the complete 100-message window. Conversation assembly calls `realConversations.find` for every contact and array-backed archive checks for every conversation.
- Impact: every selection duplicates SQLite/IPC work and races two state replacements; every send discards the benefit of its targeted optimistic update. The conversation list is O(contacts × conversations/archives).
- Required action: make one owner load a conversation, deduplicate in-flight reads by peer/revision, and index conversations by peer ID before mapping contacts.

#### QUAL-009: Frontend quality gates do not enforce the failure patterns found in Pass 1

- Locations: `eslint.config.ts:5-25`, `vitest.config.ts:4-23`, `.dev/config.toml:33-45`
- Evidence: ESLint enables no recommended or type-aware rule set, treats unused values only as warnings, permits explicit `any`, and does not check floating/misused promises or complexity. Coverage can be generated but has no thresholds and is not part of `ts_ci`.
- Impact: silent promise rejection, dead branches, duplicated effects, and untested production controls all pass the nominal frontend CI gate. The presence of many unit tests therefore does not establish meaningful production-path coverage.
- Required action: enable type-aware recommended rules, fail CI on warnings, add floating/misused-promise and unsafe-boundary rules, set ratcheting coverage thresholds for production modules, and add complexity/size budgets as review signals rather than arbitrary formatting targets.

#### QUAL-010: Windows development runs a different and weaker toolchain contract

- Locations: `.dev/config.toml:9-45`, `.dev/config.toml:83-109`, `.dev/config.windows.toml:9-45`, `.dev/config.windows.toml:83-100`, `.dev/config.windows.toml:128-150`
- Evidence: Linux/macOS uses the frozen pnpm lockfile and defines relay check, format, Clippy, and test tasks. Windows uses npm/npx, performs a mutable `npm install`, has no relay test task/pipeline, and advertises npm-based Tauri commands.
- Impact: code can pass on one development platform with a different dependency graph and without testing the relay. This directly undermines the requested ability to develop and build Harbor from both Windows and WSL.
- Required action: generate platform adapters from one task definition, use the same pinned pnpm and Rust commands everywhere, and add a CI assertion that every required pipeline exists and is equivalent on each supported platform.

#### QUAL-011: Release automation trusts mutable third-party action tags

- Locations: `.github/workflows/ci.yml:24-77`, `.github/workflows/release.yml:27-277`
- Evidence: checkout, package setup, toolchain, 1Password, Tauri, Azure login, and Artifact Signing actions are referenced by moving major tags such as `@v4`, `@stable`, `@v0`, and `@v2`, including jobs with `contents: write`, `id-token: write`, signing material, and release publication authority.
- Impact: an upstream tag move or compromise changes executable release code without a Harbor commit, affecting binaries and credentials.
- Required action: pin actions to reviewed commit SHAs, use an automated controlled update process, minimize permissions per job, and keep signing/publishing behind protected environments.

#### QUAL-012: Deployment templates duplicate nearly all bootstrap logic

- Locations: `infrastructure/relay-cloudformation.yaml:1-486`, `infrastructure/community-relay-cloudformation.yaml:1-502`
- Evidence: the two templates separately carry the same VPC, security group, IAM, updater, identity persistence, service, and diagnostics shell program; their primary intended difference is community mode/data configuration.
- Impact: security and reliability fixes must be copied exactly between roughly 500-line files. The duplicated private-key tracing defect in `FUN-028` shows that this is already a shared failure multiplier.
- Required action: produce both deployable artifacts from one reviewed source with parameterized mode, or move common bootstrap into a versioned immutable artifact and keep templates declarative. Test generated templates for semantic equivalence of shared controls.

#### QUAL-024: The relay binary and library compile separate module compositions

- Locations: `relay-server/src/main.rs:6-11`, `relay-server/src/lib.rs:3-10`
- Evidence: the binary declares local modules instead of using the package library's modules. Production and library/test composition can therefore diverge even when the source filenames overlap.
- Impact: duplicate compilation and a misleading test boundary; testing the library does not prove that the production binary wires the same implementation.
- Required action: make the binary a thin entrypoint over one library-owned server/runtime implementation.

#### QUAL-025: Relay schema evolution is ad hoc and unversioned

- Locations: `relay-server/src/db.rs:7-144`, `relay-server/src/db.rs:172-248`
- Evidence: startup applies a monolithic `CREATE IF NOT EXISTS` block plus manual column probes and data rewrites, with no schema version table, ordered migration ledger, or upgrade fixture suite.
- Impact: an interrupted or partially compatible upgrade is difficult to identify, reproduce, or repair, and compatibility branches accumulate inside normal startup.
- Required action: adopt numbered transactional migrations, declare the supported starting schema, and test upgrade/restart from every retained version. Given the pre-customer state, start from one clean baseline after explicitly preserving selected tester data.

#### QUAL-026: Runtime configuration is fragmented across CLI, templates, and hard-coded constants

- Locations: `relay-server/src/main.rs:33-43`, `relay-server/src/main.rs:524-568`, `relay-server/src/main.rs:680-687`, `relay-server/src/main.rs:724-735`, `infrastructure/relay-cloudformation.yaml:51-59`, `infrastructure/community-relay-cloudformation.yaml:55-63`
- Evidence: abuse limits, total circuits, circuit duration/bytes, cleanup cadence, and operational limits are hard-coded or exposed inconsistently. Templates expose per-peer circuits but not all effective limits.
- Impact: operators cannot reason from deployment parameters to actual resource/security behavior, and defaults drift independently.
- Required action: define one validated configuration model with documented defaults and file/environment/CLI binding; generate or verify deployment parameter parity against it.

#### QUAL-027: Container deployment is neither productionized nor clearly non-production

- Locations: `relay-server/Dockerfile:3-29`, `relay-server/docker-compose.yml:1-23`
- Evidence: there is no `.dockerignore`, health check, resource budget, community data/backup contract, multi-architecture publication, or CI image build. The identity path environment variable is not consumed as described in `FUN-072`.
- Impact: the checked-in files look deployable but bypass the production lifecycle and can lose identity/state or ship an untested image.
- Required action: either add the same immutable artifacts, health, persistence, limits, CI, and recovery contract as the supported deployment, or label/remove the container path for this beta.

#### QUAL-028: Generated systemd services run as unrestricted root

- Locations: `infrastructure/relay-cloudformation.yaml:369-385`, `infrastructure/community-relay-cloudformation.yaml:380-396`
- Evidence: services use root with no dedicated account, state-directory ownership, filesystem protection, privilege restrictions, descriptor/resource limits, or graceful timeout policy.
- Impact: a relay compromise has full instance privileges, and runaway connections/logs/resources have few containment boundaries.
- Required action: create a relay user, grant only required state/network access, and add tested systemd hardening and resource directives.

#### QUAL-029: Persistent cloud resources are created outside the stack lifecycle

- Locations: `infrastructure/relay-cloudformation.yaml:279-363`, `infrastructure/scripts/teardown-stack.sh:66-102`
- Evidence: user-data creates the EIP and SSM identity parameter dynamically, so CloudFormation cannot model, update, drift-detect, or reliably clean them. Default teardown retains an unattached billable EIP.
- Impact: orphaned cost and identity resources are easy to miss, and operators cannot understand the full deployment from stack state.
- Required action: model retained resources declaratively with explicit retention/deletion policy, or provide an idempotent lifecycle controller and prominent cost/ownership reporting.

#### QUAL-030: Cross-layer errors are stringly typed

- Locations: `relay-server/src/board_service.rs:399-1156`, `relay-server/src/auth.rs:72-230`, `src-tauri/src/p2p/types.rs:373-397`, `src/utils/callErrors.ts:89-122`
- Evidence: domain, validation, authorization, transport, storage, and retry states collapse into strings and are later interpreted through formatting or regular expressions.
- Impact: retryability, safe user copy, metrics, and UI transitions depend on text remaining stable. This is the same class of boundary that produced the beta's `[object Object]` call failure.
- Required action: define versioned error codes and structured details at each wire/IPC boundary, retain causal typed errors internally, and map to localized user messages only at the UI edge.

#### QUAL-031: The relay has no graceful shutdown path

- Location: `relay-server/src/main.rs:835-905`
- Evidence: the event loop runs forever without signal handling, work draining, SQLite checkpoint, or shutdown telemetry.
- Impact: deployment updates and instance termination can interrupt writes/responses and provide no evidence of whether shutdown completed cleanly.
- Required action: handle SIGTERM/SIGINT, stop accepting requests, drain bounded work, checkpoint/close state, and exit within the systemd/cloud timeout.

#### QUAL-037: Frontend event delivery can backpressure the only swarm actor

- Locations: `src-tauri/src/p2p/network.rs:1297-1330`, `src-tauri/src/p2p/network.rs:1338-1382`, `src-tauri/src/commands/network.rs:238-247`
- Evidence: the actor awaits writes to a bounded 256-event frontend channel. Media events instead use `try_send`, so reliability and overload semantics vary by event type.
- Impact: a slow or stopped webview consumer can pause transport processing; switching selected events to lossy delivery without a policy can instead hide required state.
- Required action: separate reliable state transitions from coalescible telemetry, persist/reconcile durable state, and ensure swarm polling never waits on UI delivery.

#### QUAL-038: Blocking CPU work runs directly in async identity commands

- Locations: `src-tauri/src/commands/identity.rs:45-55`, `src-tauri/src/commands/identity.rs:80-86`, `src-tauri/src/services/crypto_service.rs:76-187`
- Evidence: identity creation and unlock perform Argon2 key derivation synchronously on Tokio command workers. Media has the same blocking pattern captured in `QUAL-004`.
- Impact: intentionally expensive password derivation can starve unrelated async commands and amplify concurrent unlock attempts into application-wide latency.
- Required action: run password KDF work in a bounded blocking pool with concurrency limits and cancellation-aware UI state; measure target-platform latency and memory settings.

#### QUAL-039: Content synchronization and media preload contain additional N+1/quadratic paths

- Locations: `src-tauri/src/services/content_sync_service.rs:338-358`, `src-tauri/src/commands/media.rs:316-469`, `src-tauri/src/p2p/network.rs:5195-5201`
- Evidence: manifest construction fetches media hashes once per post, permitting roughly 1,001 serialized queries for a 1,000-post manifest. Preload repeatedly queries/mutates up to 512 transfer rows and rereads state, while pending media requests are linearly scanned by hash.
- Impact: background synchronization competes for the single database/network actor and grows far faster than the number of new objects.
- Required action: use joined/batched queries, bulk transfer mutations, and maps indexed by the actual lookup key; enforce per-cycle work budgets.

#### QUAL-040: Replay and delivery-key caches grow without bounds

- Locations: `src-tauri/src/services/calling_service.rs:43-50`, `src-tauri/src/services/calling_service.rs:480-500`, `src-tauri/src/services/calling_service.rs:1150-1290`, `src-tauri/src/services/mentions_service.rs:94-100`, `src-tauri/src/services/mentions_service.rs:234-252`
- Evidence: every calling signaling fingerprint is retained for the process lifetime, with replay recording after some signaling side effects; the cache disappears on restart. Expired mention delivery keys are retained rather than pruned.
- Impact: long sessions leak memory, restart weakens replay protection, and duplicated signaling can perform side effects before being rejected.
- Required action: deduplicate before side effects and use bounded TTL/LRU or persisted replay state with explicit pruning; remove expired delivery keys on access and maintenance.

#### QUAL-041: Tauri migration application is a hand-written compatibility ladder

- Locations: `src-tauri/src/db/connection.rs:6-28`, `src-tauri/src/db/connection.rs:114-393`, `src-tauri/src/db/migrations/011_posts_lamport_index.sql:9`
- Evidence: 23 migrations are individually wired into one large function using one captured starting version, without a migration registry or checksums. Migration 011 writes schema version 9, and compatibility startup creates substitute tables.
- Impact: ordering/version mistakes are difficult to detect, migration identity cannot be audited, and normal startup silently invents partial schemas.
- Required action: for this pre-customer beta, squash to a verified baseline after explicitly converting retained tester profiles. Thereafter use a transactional version/name/checksum registry and upgrade fixtures.

#### QUAL-042: Core networking cannot be tested deterministically

- Locations: `src-tauri/src/p2p/network.rs:5451-5574`; direct time/ID generation occurs throughout `src-tauri/src/p2p/network.rs` and `src-tauri/src/services/`
- Evidence: the 5,597-line actor has only a handful of helper/codec-oriented tests and no injectable clock, ID source, transport, scheduler, or repository. Production networking/services directly call wall clock and random UUID generation extensively.
- Impact: timeout, retry, disconnect, response-reordering, restart, and cancellation interleavings cannot be reproduced reliably, precisely where the beta failures concentrate.
- Required action: extract pure protocol reducers and inject clock/ID/transport/repository ports; add deterministic state-machine and lifecycle tests before more features are layered onto the actor.

#### QUAL-043: Profile service construction and publication workflows are duplicated

- Locations: `src-tauri/src/lib.rs:234-318`, `src-tauri/src/commands/posts.rs:152-322`
- Evidence: startup manually constructs an order-sensitive graph of concrete services with overlapping identity/contact/permission dependencies. Relay publication logic is repeated separately for create, update, and delete.
- Impact: account/profile switching and new protocol behavior require coordinated edits across wiring and command paths; one mutation can omit the durability/error behavior added to another.
- Required action: introduce a profile-scoped application context with mandatory narrow ports and one durable relay outbox/publisher shared by all post mutations.

#### QUAL-048: Frontend page controllers mix state, orchestration, and entire feature trees

- Locations: `src/pages/Settings.tsx:175-604`, `src/pages/Settings.tsx:761-2085`, `src/pages/Chat.tsx:645-669`, `src/pages/Chat.tsx:1664-2084`, `src/pages/Wall.tsx:394-404`, `src/pages/Wall.tsx:1090-1449`, `src/pages/ContactWall.tsx:65-278`
- Evidence: Settings owns roughly 31 local state values and eight large conditional sections. Chat owns composer/search/call/dialog state and maps every message in the parent; Wall/ContactWall retain per-post drafts in the page that maps all cards.
- Impact: a composer keystroke or per-card draft change rebuilds large unrelated view trees, while tests must construct nearly the entire feature to exercise one handler.
- Required action: split controller hooks from memoized lists/cards/composers, keep per-card ephemeral state with the card, and give each feature one narrow async boundary.

#### QUAL-049: Settings has two diverging implementations totaling over 4,000 lines

- Locations: `src/pages/Settings.tsx`, `src/pages/settings/index.ts:1-5`, `src/pages/settings/`
- Evidence: the 2,370-line routed page coexists with an unreachable 1,994-line modular tree containing its own profile, security, privacy, notification, network, appearance, and about behavior.
- Impact: fixes can land in the wrong implementation and appear tested while production remains unchanged; duplicate fake controls already did so in Pass 1.
- Required action: choose the intended design, migrate only real controls section by section with route-level tests, and delete the other implementation immediately after each slice.

#### QUAL-050: Async listeners and object URLs have lifecycle leaks

- Locations: `src/pages/Chat.tsx:854-905`, `src/components/common/NotificationCenter.tsx:21-38`
- Evidence: replacing a pending Chat attachment creates a new object URL without revoking the previous one; unmount cleanup captures the initial attachment due to an empty dependency list. NotificationCenter can unmount before asynchronous listener registration returns, leaving no disposer for the late listener.
- Impact: repeated attachment selection leaks blobs, and remounts can accumulate native notification handlers with duplicate actions/state updates.
- Required action: keep current resources/disposers in refs, revoke before replacement, and make asynchronous registration cancellation-aware so late resources dispose immediately.

#### QUAL-051: Mention resolution invokes backend work on composer keystrokes

- Locations: `src/components/identity/MentionResolution.tsx:15-37`, `src/components/common/ComposePostModal.tsx:245-260`
- Evidence: every text change extracts all names and resolves them again; once a mention exists, ordinary following characters repeat the remote/database work.
- Impact: typing speed drives request volume, state churn, and stale-result opportunities.
- Required action: debounce, cache by normalized qualified name/claim revision, resolve only when the extracted-name set changes, and support a bounded batch resolver.

#### QUAL-052: Calling lifecycle state is split between Zustand and module globals

- Location: `src/stores/calling.ts:90-112`
- Evidence: runtime instances, pending envelopes, group invitations, and roster versions live outside observable store state; correctness depends on every exit path calling manual disposal.
- Impact: account switches and tests cannot inspect or reset calling atomically, and missed cleanup can retain peer connections/media across sessions.
- Required action: make one profile-session-owned calling controller hold opaque runtime resources with explicit start/stop/dispose and expose only immutable snapshots to the store.

#### QUAL-053: Common UI primitives and formatting are repeatedly reimplemented

- Locations: `src/pages/Settings.tsx:93-168`, `src/pages/Network.tsx:49-64`, `src/pages/settings/shared.tsx:69-126`, `src/utils/formatting.ts:1-22`
- Evidence: Toggle and PasswordInput have multiple implementations; initials formatting is duplicated across major profile surfaces despite an existing helper; the same primary gradient literal appears broadly across production code.
- Impact: accessibility, theming, sizing, and interaction fixes drift between pages, as seen in the styling issues raised during beta preparation.
- Required action: consolidate design-system controls and semantic formatting/avatar components, then prohibit page-local copies through review/lint conventions.

#### QUAL-054: Critical frontend controllers have no direct tests

- Locations: `src/pages/Settings.tsx`, `src/pages/Chat.tsx`, `src/pages/Network.tsx`, `src/pages/Feed.tsx`, `src/pages/Boards.tsx`, `src/components/layout/MainLayout.tsx`, `src/hooks/useTauriEvents.ts`
- Evidence: there are no direct component/controller tests for these five pages (about 7,900 lines combined), MainLayout, or the application event hook. Existing store tests sometimes assert swallowed-error behavior rather than caller-visible failure. Coverage configuration has no threshold (`QUAL-009`).
- Impact: the highest-risk orchestration, lifecycle, and presented controls are less tested than small adapters/components, allowing false success and stale session behavior through CI.
- Required action: decompose first, then add focused controller tests for account switching, listener teardown, stale-request suppression, mutation failures, media batching, and production control reachability; ratchet critical-module coverage.

### Low severity and cleanup

#### QUAL-013: A generic devkit template is committed as `temp_config.toml`

- Location: `temp_config.toml:1-240`
- Evidence: the root file defines unrelated Python pipelines, nightly Rust tooling, generic comments, and commands that do not match Harbor's maintained `.dev/config*.toml`. No repository task references it.
- Impact: it is indistinguishable from a scratch/config candidate and adds another contradictory development contract.
- Required action: delete it. If any generally useful tasks are wanted, deliberately port them into the maintained `.dev` configuration and CI.

## Detection-pattern catalog

This catalog turns the findings into repeatable review tripwires. Commands assume the repository root and are read-only. They intentionally return candidates rather than pass/fail verdicts: a match can be safe, and an implementation can still be unsafe without containing a convenient keyword.

Each pattern therefore has two parts:

1. a mechanical search for cheap, repeatable candidate discovery;
2. a confirmation rule or test that establishes whether the candidate is a real failure.

The finding index on each pattern is the coverage contract. When a finding is remediated, keep its pattern in CI/review guidance so the same class does not return.

<!-- Keep DP identifiers stable because work items and CI may refer to them. -->

### DP-001: Presented controls without authoritative behavior

Findings: `GOV-001`, `GOV-002`, `FUN-001`, `FUN-002`, `FUN-003`, `FUN-004`, `FUN-012`, `FUN-013`, `FUN-021`, `FUN-061`, `FUN-063`, `FUN-064`

Search for timers, placeholders, local-only persistence, hard-coded success copy, and settings with no enforcement consumer:

```bash
rg -n -i 'placeholder|simulat|setTimeout|changed successfully|exported successfully|recovered successfully|deleted successfully|localStorage|showReadReceipts|showOnlineStatus|localDiscovery|soundEnabled|not.*yet' src src-tauri/src scripts docs README.md
```

Confirmation: inventory every routed control and trace `interaction -> authoritative command -> durable mutation -> restart -> externally observable effect`. Reject the dependency and prove that success UI does not appear. Privacy/shared behavior must be observed from a second peer, not inferred from the local decoration.

### DP-002: Release claims that are advisory or absent from the publishing gate

Findings: `GOV-001`, `GOV-002`, `GOV-003`, `QUAL-009`, `QUAL-011`, `QUAL-018`, `QUAL-022`, `QUAL-023`, `QUAL-054`

```bash
rg -n 'live-beta-acceptance|release-readiness|workflow_dispatch|needs:|coverage|threshold|cfn-lint|shellcheck|artifact|checksum|publish|createRelease|updateRelease' .github scripts docs package.json vitest.config.ts eslint.config.ts
```

Confirmation: deliberately break one production control, one packaged-app scenario, relay startup, wire compatibility, and artifact/version equality. Each must prevent publication. Acceptance evidence must identify the exact commit and artifact hashes consumed by the publishing job.

### DP-003: Ignored errors, permissive defaults, and false success

Findings: `FUN-006`, `FUN-007`, `FUN-008`, `FUN-011`, `FUN-016`, `FUN-017`, `FUN-018`, `FUN-039`, `FUN-040`, `FUN-042`, `FUN-043`, `FUN-044`, `FUN-045`, `FUN-046`, `FUN-047`, `FUN-048`, `FUN-051`, `FUN-062`, `FUN-065`, `FUN-067`, `FUN-078`, `FUN-080`, `QUAL-015`, `QUAL-030`

```bash
rg -n -U 'let _\s*=|\.ok\(\)\.flatten\(\)|unwrap_or\((false|true|0)\)|unwrap_or_default\(\)|catch\s*(\([^)]*\))?\s*\{\s*(//[^\n]*)?\s*\}|\.catch\([^\n]*(undefined|\{\s*\})|Ok\(true\)|No updates needed|toast\.success|success:' src src-tauri/src relay-server/src infrastructure scripts
```

Confirmation: inject I/O, database, permission, transport, clipboard, autoplay, malformed-row, and stale-revision failure at every stage. User actions must return a typed failure or explicit partial state; background degradation must retain diagnostics; no success event/toast may precede the defined durable outcome.

### DP-004: Signed data not bound to the authenticated transport or semantic subject

Findings: `FUN-009`, `FUN-027`, `FUN-029`, `FUN-033`, `FUN-050`, `FUN-053`, `FUN-074`, `FUN-075`, `FUN-077`

```bash
rg -n 'sender_peer_id|author_peer_id|requester_peer_id|target_peer_id|recipient|remote_peer|connected_peer|PeerId|public_key|display_name|verify|signature|challenge|envelope' src-tauri/src relay-server/src
```

Confirmation: replay a valid signed request over a different Noise-authenticated connection and independently substitute requester, target, author, recipient, relay, display name, or signing key. Every mismatch must fail before replay state, authorization, notification, or storage changes.

### DP-005: Authorization ignores block, revocation, expiry, scope, or lookup failure

Findings: `FUN-008`, `FUN-029`, `FUN-031`, `FUN-032`, `FUN-076`

```bash
rg -n 'is_blocked|is_peer_banned|has_active|grant|revok|scope|expires|expiry|timestamp|Utc::now|unwrap_or\(false\)' src-tauri/src relay-server/src
```

Confirmation: exercise every inbound capability after block, contact removal, revocation, expiry, restart, non-null scope, and authorization database failure. Authoritative local/server time must decide expiry, and denial must override cached contact/key/grant state.

### DP-006: Nonce, replay, edit, and cryptographic-domain mistakes

Findings: `FUN-015`, `FUN-020`, `FUN-025`, `FUN-026`, `FUN-027`, `FUN-042`, `QUAL-040`

```bash
rg -n -i 'nonce|counter|replay|fingerprint|aes.?gcm|encrypt|decrypt|derive.*key|legacy|backwards compatibility|check_and_record|edited|new_content' src-tauri/src
```

Confirmation: property-test simultaneous first messages in both directions, restarts, counter rollback, repeated edits, invalid signatures with future counters, and replay before/after restart. Assert key/nonce uniqueness, directional domain separation, authentication before replay commitment, fresh edit nonces, and bounded durable replay protection.

### DP-007: Account/profile lifecycle is not one isolated runtime boundary

Findings: `FUN-005`, `FUN-011`, `FUN-034`, `FUN-035`, `FUN-036`, `FUN-037`, `FUN-049`, `QUAL-003`, `QUAL-034`, `QUAL-043`, `QUAL-044`, `QUAL-052`

```bash
rg -n 'accounts\.json|active_account|switch_account|register_account|lock_identity|start_network|stop_network|spawn\(|JoinHandle|CancellationToken|localStorage|persist\(|profile|set_.*service|new Map\(' src src-tauri/src
```

Confirmation: create two profiles with conflicting names, data, settings, relays, calls, and archives; rapidly switch, lock, restart, and fail registry writes. Assert one active context, atomic chooser metadata, backend password policy, key/PeerId consistency, complete worker teardown, no cross-profile state, and no private serving while locked.

### DP-008: Multi-stage mutations without a transaction or durable outbox

Findings: `FUN-006`, `FUN-007`, `FUN-032`, `FUN-038`, `FUN-039`, `FUN-043`, `FUN-051`, `FUN-067`, `QUAL-036`, `QUAL-043`

```bash
rg -n 'transaction|execute\(|insert_|update_|delete_|store_|grant|revoke|tombstone|outbox|pending|send_request|emit|WallPostStored|ContactAdded' src-tauri/src relay-server/src
```

Confirmation: fail after each counter, event, projection, media, permission, tombstone, and network-publication stage, then restart and retry. The result must be one atomic durable mutation plus a restart-safe outbox, or no mutation. Retries must be idempotent and expose pending/acknowledged/conflicted state.

### DP-009: Absence fallbacks, corrupt-row coercion, and schema repair during normal startup

Findings: `FUN-014`, `FUN-017`, `FUN-042`, `FUN-068`, `FUN-069`, `FUN-080`, `QUAL-025`, `QUAL-041`

```bash
rg -n 'QueryReturnedNoRows|unwrap_or|unwrap_or_default|CREATE TABLE IF NOT EXISTS|ALTER TABLE|PRAGMA table_info|schema_version|migration|Legacy|as i64|as u64|MIN\(id\)|empty bytes' src-tauri/src/db relay-server/src
```

Confirmation: test clean creation, every retained upgrade path, interrupted migration, corrupt version/schema, malformed enums/timestamps, negative values, and protocol integers beyond `i64::MAX`. Only genuine absence may default; corruption must fail or quarantine rather than become plausible state.

### DP-010: Replace/upsert semantics weaken ownership or revision integrity

Findings: `FUN-030`, `FUN-051`, `FUN-067`, `FUN-069`

```bash
rg -n 'INSERT OR REPLACE|REPLACE INTO|ON CONFLICT|post_id.*UNIQUE|ON DELETE CASCADE|lamport|stale|Ok\(true\)|MIN\(id\)|store.*media' relay-server/src
```

Confirmation: have author B submit author A's ID; submit equal, stale, and newer clocks; force failure between post and media; seed conflicting migration rows. Ownership must never change, stale mutation must be distinct from idempotent success, and media must commit atomically.

### DP-011: Unbounded caches, challenges, queues, storage, work, or circuit lifetimes

Findings: `FUN-052`, `FUN-054`, `FUN-056`, `QUAL-032`, `QUAL-035`, `QUAL-040`

```bash
rg -n 'HashMap|HashSet|VecDeque|insert\(|retain\(|remove\(|u64::MAX|difficulty|timeout|deadline|max_.*bytes|max_.*duration|pending|challenge|cache|quota|unlimited' src-tauri/src relay-server/src infrastructure
```

Confirmation: churn unique peers/challenges/fingerprints/keys/content past expected volume using accelerated time, maximum difficulty, and missing responses. Assert hard memory/disk/work limits, TTL pruning, cancellation/deadlines, finite circuits, and source plus global quotas.

### DP-012: Blocking CPU, filesystem, database, channel, or sleep work inside async networking

Findings: `FUN-055`, `QUAL-004`, `QUAL-005`, `QUAL-014`, `QUAL-031`, `QUAL-032`, `QUAL-037`, `QUAL-038`, `QUAL-039`

```bash
rg -n 'std::fs|fs::read|fs::write|rusqlite|Mutex<Connection>|thread::sleep|sleep\(|Argon2|spawn_blocking|\.send\([^;]*\.await|lock\(\).*unwrap|select_next_some' src-tauri/src relay-server/src
```

Confirmation: run slow disk/database/KDF/media work and a delayed response while messages, calls, pings, and circuits continue. Measure swarm poll gaps, queue depth, and tail latency. Shutdown must cancel/drain bounded work without blocking protocol progress.

### DP-013: Pending operations correlated by peer instead of request, with no deadline

Findings: `FUN-038`, `QUAL-033`, `QUAL-034`, `QUAL-035`, `QUAL-042`

```bash
rg -n 'pending_.*HashMap|HashMap<PeerId|OutboundRequestId|request_id|oneshot|recv\(\)\.await|send_request|OutboundFailure|ConnectionClosed|loop \{' src-tauri/src/p2p src-tauri/src/commands
```

Confirmation: issue simultaneous name/introduction/delivery operations to one relay, reorder responses, disconnect, omit a response, cancel callers, stall the frontend channel, and stop/restart. Each operation must correlate by request ID, expire, complete its caller, clean state, and join all workers deterministically.

### DP-014: Pagination, N+1 queries, linear scans, duplicate loads, and broad reconciliation

Findings: `FUN-057`, `QUAL-006`, `QUAL-007`, `QUAL-008`, `QUAL-014`, `QUAL-039`

```bash
rg -n 'ORDER BY|created_at\s*[<>]|LIMIT|cursor|for .* in |map\(async|iter\(\)\.find|\.find\(|\.includes\(|get_.*media|get_.*name|loadFeed|loadMessages|refresh|setInterval' src src-tauri/src relay-server/src
```

Confirmation: instrument SQL, IPC, bytes, and renders for 1, 50, and 1,000 entities; growth should be batched rather than nested-linear. Page duplicate timestamps with a compound cursor exactly once. Selecting a conversation should load once, and a delta event must not reload an entire domain absent a revision gap.

### DP-015: Full media copies, erased attachment state, and partial metadata

Findings: `FUN-012`, `FUN-018`, `FUN-043`, `FUN-045`, `FUN-051`, `FUN-062`, `FUN-064`, `QUAL-004`, `QUAL-007`, `QUAL-014`, `QUAL-039`

```bash
rg -n 'Vec<u8>|number\[\]|Array\.from|Uint8Array|fs::read|base64|data:|getPostMedia|get_post_media|ensure_transfer|preload|store.*media|insert.*media|audio\.play|\.play\(\)|attachment' src src-tauri/src relay-server/src
```

Confirmation: load many maximum-size attachments under slow disk/network and separately fail metadata, authorization, transfer, cache, object resolution, and autoplay. Require bounded streaming/ranges, preserved attachment existence, visible progress/failure/retry, transactional metadata, and no swarm/webview stall.

### DP-016: Duplicated or incompatible wire, signing, rotation, and schema models

Findings: `FUN-020`, `FUN-033`, `FUN-058`, `FUN-069`, `FUN-070`, `QUAL-002`, `QUAL-024`, `QUAL-025`, `QUAL-041`

```bash
rg -n 'struct (BoardSync|.*Request|.*Response|.*Rotation)|enum (BoardSync|.*Request|.*Response)|mod (auth|db|board_service|introduction)|canonical|serde|base64.*decode|legacy|compatibility|version' src-tauri/src relay-server/src
```

Confirmation: maintain golden vectors produced by one real binary/crate and decoded/verified by the other for every message/artifact, including malformed and unknown versions. Production binary and tests must use the same library composition and one canonical model.

### DP-017: Persisted, documented, or advertised configuration is not consumed

Findings: `FUN-013`, `FUN-041`, `FUN-058`, `FUN-072`, `FUN-073`, `QUAL-026`, `QUAL-027`

```bash
rg -n 'enable_mdns|localDiscovery|bootstrap|IDENTITY_KEY_PATH|identity-key-path|identity_namespace|community|NetworkConfig::default|add_external_address|0\.0\.0\.0|Arg::|env\(' src src-tauri/src relay-server infrastructure
```

Confirmation: toggle every exposed input and inspect effective behavior after restart using the real protocol. Deployment parity tests must prove each parameter is consumed, identity/state persist, the advertised mode is active, and only validated routable addresses are published.

### DP-018: Secrets exposed through tracing, logging, or unsafe key-file creation

Findings: `FUN-028`, `FUN-060`, `FUN-072`

```bash
rg -n '#!/bin/(ba)?sh -[^[:space:]]*x|set -x|EXISTING_KEY|IDENTITY_KEY_B64|private.*key|identity.*key|fs::write|OpenOptions|PermissionsExt|set_mode|0o600|user-data\.log|console' infrastructure relay-server
```

Confirmation: deploy with a recognizable canary secret and scan console, journal, cloud-init, shell-error, and support output. Verify atomic owner-only creation under representative umasks, existing-file ownership/mode validation, stable identity, and fail-closed unsafe paths.

### DP-019: Deployment/updater converts failure into success, resets state, or skips rollback

Findings: `FUN-071`, `QUAL-015`, `QUAL-016`, `QUAL-017`

```bash
rg -n 'set -e|update-stack|No updates needed|PARAMETERS=|is-active|rollback|cfn-signal|CreationPolicy|change-set|UsePreviousValue|health|CREATE_COMPLETE' infrastructure
```

Confirmation: simulate invalid credentials/parameters/templates, a true no-op, immediate and delayed crashes, protocol-dead/port-open state, customized prior parameters, and bootstrap exhaustion. Only the documented no-op may succeed; other failures must preserve settings, fail health, or roll back.

### DP-020: Missing durability, safe backup, recovery, and cloud-resource ownership

Findings: `FUN-059`, `FUN-071`, `QUAL-019`, `QUAL-020`, `QUAL-029`, `QUAL-031`

```bash
rg -n 'sqlite|community-data|cp -r|cp -a|backup|snapshot|Volume|EBS|DeletionPolicy|UpdateReplacePolicy|allocate-address|put-parameter|ctrl_c|SIGTERM|checkpoint|shutdown|teardown' infrastructure relay-server/src
```

Confirmation: terminate/replace during writes and updates, restore newest and previous backups onto a fresh instance, and verify identity plus all data. Repeated update/teardown must leave no unexpected EIP/parameter/snapshot/backup, and SIGTERM must drain/checkpoint within the service timeout.

### DP-021: Excessive service privilege or unintended public ingress

Findings: `FUN-079`, `QUAL-028`

```bash
rg -n 'FromPort:\s*22|ToPort:\s*22|0\.0\.0\.0/0|User=root|ExecStart|NoNewPrivileges|ProtectSystem|ProtectHome|PrivateTmp|CapabilityBoundingSet|LimitNOFILE|KeyName' infrastructure
```

Confirmation: render with SSH disabled and enabled. Disabled must create no rule; enabled must require a restricted CIDR. Run under a dedicated account with owned state directories, minimal filesystem/capabilities, and explicit resource budgets.

### DP-022: Mutable, architecture-ambiguous, or platform-divergent build/release inputs

Findings: `FUN-023`, `FUN-024`, `FUN-066`, `FUN-081`, `FUN-082`, `QUAL-010`, `QUAL-011`, `QUAL-018`, `QUAL-023`, `QUAL-027`

```bash
rg -n 'uses:.*@(v[0-9]+|stable|main|master|latest)|raw\.githubusercontent\.com/.*/(main|master)|bin/harbor-relay|checksum|sha256|cargo build --release|npm (install|run)|pnpm|Harbor v[0-9]|harbor-relay-cloudformation' .github scripts infrastructure relay-server src src-tauri .dev
```

Confirmation: build all supported targets in CI, inspect architecture, smoke-test, and verify immutable platform-qualified checksums, SBOM/provenance, version equality, and reviewed action SHAs. Windows/WSL task graphs must use the same frozen dependency graph and required tests.

### DP-023: Duplicated deployment bootstrap and fragmented effective configuration

Findings: `FUN-023`, `FUN-056`, `FUN-058`, `QUAL-012`, `QUAL-016`, `QUAL-026`, `QUAL-027`

```bash
rg -n 'UserData|Fn::Base64|AWS::EC2::Instance|MaxCircuits|circuit|duration|bytes|identity.namespace|--community|--identity|Default:|hard.?coded' infrastructure relay-server/src/main.rs relay-server/docker-compose.yml src-tauri --glob '*.yaml' --glob '*.yml' --glob '*.rs'
```

Confirmation: enumerate/render every deployment surface and compare shared security, identity, updater, systemd, health, and effective limits. Shared controls need one source or generated-equivalence test; updates must preserve operator values.

### DP-024: Operational failures hidden by sparse, noisy, or unbounded observability

Findings: `FUN-016`, `FUN-078`, `FUN-083`, `QUAL-021`, `QUAL-031`

```bash
rg -n 'info!.*event|tracing::(info|warn|error|debug)|let _\s*=|health|ready|liveness|metrics|histogram|counter|queue_depth|disk|SIGTERM|ctrl_c|journal|SystemMaxUse' relay-server/src infrastructure
```

Confirmation: induce database/admission failure, queue saturation, disk pressure, protocol deadlock, hostile event volume, and termination. Operators need structured bounded latency/error/queue/storage signals and alarms without weakening oracle-resistant public responses.

### DP-025: Stringly typed errors and corrupt state normalized into ordinary behavior

Findings: `FUN-014`, `FUN-015`, `FUN-067`, `FUN-078`, `FUN-080`, `QUAL-030`

```bash
rg -n 'Result<[^>]*String>|map_err\([^;]*(format!|to_string)|error:\s*String|String\(.*error|\[object Object\]|contains\(|Regex|Decryption failed|LegacyCommentCreate|unwrap_or\(.*(Incoming|Audio|Ended)' src src-tauri/src relay-server/src
```

Confirmation: enumerate stable codes at storage, domain, transport, IPC, and UI boundaries. Fuzz malformed rows and remote errors; callers must distinguish absent, invalid, unauthorized, conflict, retryable transport, and fatal storage states without parsing prose.

### DP-026: Compatibility, demo, duplicate, personal-environment, or scratch paths in production

Findings: `FUN-010`, `FUN-019`, `FUN-020`, `FUN-022`, `FUN-024`, `FUN-081`, `QUAL-010`, `QUAL-013`, `QUAL-049`

```bash
rg -n -i 'legacy|compatibility|migration|mockPeers|isReal|demo|D:\\apps|chat-app|run-(alice|bob)|tauri\.bob|/home/bakobi|npm (install|run)|temp_config|src/pages/settings|raw.*address|multiaddr|/p2p/' src src-tauri scripts README.md CLAUDE.md .dev temp_config.toml
```

Confirmation: trace reachability from router, command registration, imports, docs, and deployment entry points. Every production feature needs one owner. Compatibility must name a supported external/current input and removal condition; otherwise delete it after explicitly converting selected tester data.

### DP-027: Inert controls or unsupported field combinations exposed as available

Findings: `FUN-061`, `FUN-063`, `FUN-064`

```bash
rg -n -i 'Find Contacts|soundEnabled|NotificationsSection|mention|attachment|not supported|cannot.*attachment|disabled=|onClick=' src --glob '*.{ts,tsx}'
```

Confirmation: keyboard-activate and click every primary action/empty-state control. Exercise pairwise composer combinations for text, mention, image, video, and audio. Unsupported combinations must be disabled and explained before submission, not rejected afterward.

### DP-028: God components/objects, whole-store subscriptions, and duplicated UI primitives

Findings: `QUAL-001`, `QUAL-045`, `QUAL-048`, `QUAL-049`, `QUAL-053`

```bash
rg -n 'use[A-Za-z]+Store\(\)|useState\(|function (Toggle|PasswordInput)|const (Toggle|PasswordInput)|set_.*service|Option<Arc<|linear-gradient|getInitials' src src-tauri/src/p2p/network.rs --glob '*.{ts,tsx,rs}'
wc -l src-tauri/src/p2p/network.rs src/pages/{Settings,Chat,Network,Feed,Wall,ContactWall}.tsx src/pages/settings/*.{ts,tsx} 2>/dev/null
```

Confirmation: construction must make missing required dependencies impossible. Profile/render one narrow update such as transfer progress or a draft keystroke; only its owner should run. Route tests must identify one Settings implementation, and complexity/size trends should be review gates rather than correctness claims.

### DP-029: Stale async commits, listener races, and object-URL lifecycle leaks

Findings: `FUN-048`, `QUAL-046`, `QUAL-050`

```bash
rg -n 'listen\(|unlisten|createObjectURL|revokeObjectURL|dispose|cleanup|AbortController|generation|useEffect|set\(' src --glob '*.{ts,tsx}'
```

Confirmation: hold request A, select B, resolve B, then A; state must remain B. Begin delayed listener registration, unmount, then resolve; the late disposer must run. Replace attachments repeatedly and prove every prior/current URL is revoked exactly once.

### DP-030: Generic, unverified, or fabricated identity labels

Findings: `FUN-005`, `FUN-009`, `FUN-017`, `FUN-047`, `FUN-050`

```bash
rg -n 'Local Harbor account|verifiedQualifiedName|display_name|displayName|no_identity|no identity|unverified|safePeerLabel|legacy name' src src-tauri/src
```

Confirmation: render multiple locked accounts before unlock and inject IPC/corruption errors during identity initialization. Labels must derive only from locally verified signed claims; absence must be distinct from failure; requester-supplied names must never be presented as trusted.

### DP-031: Production stores/components bypass the tested IPC adapter or failure contract

Findings: `FUN-044`, `FUN-046`, `QUAL-047`

```bash
rg -n 'invoke<|invoke\(' src/stores src/pages src/components src/services --glob '*.{ts,tsx}'
```

Confirmation: each backend capability should have one typed/generated adapter and authoritative backend policy. Reject the adapter dependency and confirm production callers expose failure. Tests must exercise the same path used by routed stores/components, not an unused parallel service.

### DP-032: Backend/name resolution work driven by composer keystrokes

Findings: `QUAL-051`

```bash
rg -n 'onChange|MentionResolution|extractMention|resolveMention|resolve.*Name|qualifiedName|debounce' src --glob '*.{ts,tsx}'
```

Confirmation: type 100 ordinary characters after one mention and count resolver calls. Resolution should occur only when the normalized mention set changes, be debounced/batched/cached, and discard stale results.

### DP-033: Tests and static analysis cover adapters but not critical production controllers

Findings: `GOV-002`, `GOV-003`, `QUAL-009`, `QUAL-022`, `QUAL-023`, `QUAL-042`, `QUAL-054`

```bash
rg -n '#\[test\]|#\[tokio::test\]|Command::new|harbor-relay|libp2p|SettingsPage|ChatPage|NetworkPage|FeedPage|BoardsPage|useTauriEvents|coverage|threshold|no-floating-promises|no-misused-promises|cfn-lint|shellcheck|smoke' src src-tauri/src relay-server .github scripts vitest.config.ts eslint.config.ts
```

Confirmation: CI must launch the production relay and real clients, exercise packaged controllers and account/network lifecycle, and include deterministic clocks/transports for reorder/timeout tests. Critical modules require ratcheting coverage and type-aware linting; infrastructure requires template/script/artifact failure cases.

### DP-034: Architecture boundaries permit partial initialization and duplicated orchestration

Findings: `QUAL-001`, `QUAL-005`, `QUAL-024`, `QUAL-043`, `QUAL-045`, `QUAL-048`

```bash
rg -n 'Option<Arc<|set_.*service|pub struct .*Service|impl .*Service|match command|handle_.*event|use[A-Za-z]+Store\(\)|invoke\(|send_request|publish' src src-tauri/src/p2p/network.rs relay-server/src
```

Confirmation: construct each subsystem with required typed dependencies, one lifecycle owner, one command adapter, and narrow protocol/domain ports. Tests should substitute clock, transport, repository, and publisher independently; missing dependencies must fail construction rather than a live request.

### Coverage check

Every audit finding is intentionally referenced by at least one `DP-*` pattern. Keep this mechanical check with the document when findings or patterns change:

```bash
audit=docs/audits/beta-code-audit-2026-07-13.md
sed -n '1,/^## Detection-pattern catalog$/p' "$audit" \
  | rg -o '(GOV|FUN|QUAL)-[0-9]{3}' | sort -u > /tmp/harbor-finding-ids
sed -n '/^## Detection-pattern catalog$/,/^## Verification log$/p' "$audit" \
  | rg -o '(GOV|FUN|QUAL)-[0-9]{3}' | sort -u > /tmp/harbor-pattern-ids
comm -23 /tmp/harbor-finding-ids /tmp/harbor-pattern-ids
```

Expected output: empty. A non-empty line is a finding without a documented regression pattern.

## Verification log

- Three `dev walk` manifests covered `src/`, `src-tauri/src/`, and `relay-server/`: 393 files across the source roots. Production-code filtering identified 225 TypeScript/TSX/Rust files and approximately 73,556 lines excluding standalone frontend tests.
- Pass 1 traced markers, fallbacks, error suppression, compatibility branches, production reachability, deployment surfaces, capability documentation, and release gates. The small conventional TODO count was not treated as evidence of completeness.
- Pass 2 independently reviewed frontend, Tauri/backend, and relay/infrastructure boundaries, then reconciled overlaps into 54 distinct engineering findings.
- All 140 finding IDs are unique at their headings. Referenced repository paths were checked for existence; the only apparent exception from the mechanical extractor was the intentional glob notation `.dev/config*.toml`.
- The detection catalog contains 34 stable pattern groups. Its coverage check maps all 140 finding IDs with no missing or unknown IDs; all 34 Bash blocks passed shell parsing and every included `rg` expression compiled successfully.
- Relay introduction privacy tests: three scenarios passed in both the relay library and binary targets.
- Frontend targeted tests could not start from the existing dependency tree because the Linux Rolldown native binding is absent. Dependencies were not changed during the read-only audit.
- The audit document is the only intentional repository addition. No production source, dependency, generated artifact, configuration, or lockfile was changed.
- Pre-existing untracked file preserved: `pnpm-workspace.yaml`.

## Remediation order

1. Freeze beta promotion and disable updater publication until stop-ship controls have closure evidence tied to an exact commit and artifact hashes.
2. Replace the direct-message nonce/edit/replay design with a versioned reviewed protocol; bind every signed request to transport peer, recipient, freshness, and durable replay state.
3. Remove fake password, backup, recovery, deletion, read-receipt, presence, avatar-sharing, and mDNS controls. Reintroduce only backend-enforced behavior with packaged-app acceptance tests.
4. Repair profile isolation and lifecycle: trusted chooser labels, atomic account registry, one active profile context, namespaced frontend persistence, runtime teardown, and supervised network/background workers.
5. Make contact/block/grant/post/social mutations atomic and observable. Blocking/removal must revoke all effective access, and no UI success may precede durable completion.
6. Close relay authorization and integrity blockers: server-time expiry, author-bound post IDs, fail-closed moderation/storage errors, key-to-PeerId binding, scoped grants, replay protection, and immediate key rotation after removing bootstrap secret tracing.
7. Establish safe relay operations: immutable architecture-qualified artifacts, real readiness signaling, preserved change sets, hardened non-root services, bounded resources, recovery/backup testing, health metrics, and infrastructure CI.
8. Remove pre-customer compatibility modes, mock/dead branches, duplicate Settings/protocol/deployment implementations, stale scripts, and scratch configuration. Convert only explicitly retained tester data into one clean beta schema.
9. Decompose the network/relay/frontend monoliths behind shared typed protocol and error contracts; add operation correlation/deadlines, transactional repositories/outboxes, and deterministic state-machine tests.
10. Batch feed/media/name/sync reads, stream media instead of full-buffer IPC, narrow store subscriptions, cancel stale async loads, and eliminate redundant polling/refresh.
11. Strengthen CI with type-aware frontend linting, critical-module coverage, process-level client/relay tests, lifecycle/restart tests, platform-equivalent dev pipelines, pinned actions/toolchains, and infrastructure/artifact validation.
12. Promote a new beta only after the release inventory maps every production-facing control to an implemented command, failure contract, automated coverage, and completed cross-platform human acceptance scenario.
