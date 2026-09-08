# Harbor games platform contract

Status: accepted feature contract for implementation

## Source and provenance

This contract was produced from:

- the operator's Harbor games requirements discussion on 2026-09-08;
- the Harbor source at `v1.4.1-beta.7` (`63a70bfc90cf9907b3e63dd1bd4eac3de5a0fd25`);
- Neo Grounds `main` at `a5cfa14`, inspected from the operator-provided `gx10` checkout;
- Neo Grounds' `docs/runtime-package-manifest.md`, `docs/runtime-technical-spike.md`, and `docs/browser-wasm-player-core.md`.

The Neo Grounds revision above is the protocol-design input. Later changes must update the compatibility fixtures and this provenance record.

## Objective

Add an integrated games platform to Harbor. People discover approved games through an embedded Neo Grounds store, install them, and play them locally in a sandboxed WASM runtime. Neo Grounds remains the game creation and build application. Multiplayer over Harbor networking is a planned extension, not part of this delivery.

## Accepted product requirements

### HG-001 — Integrated games area

Harbor provides a first-class **Games** navigation item and route. The Games area contains:

- an embedded Neo Grounds store for browsing and installation;
- a local library of installed games;
- manual `.harborgame` import;
- discovery of `.harborgame` files in a configured games folder;
- local launch, stop, expand, fullscreen, and uninstall controls.

### HG-002 — Neo Grounds owns creation

Neo Grounds owns game generation, editing, building, previewing, submission, and moderation. A separate Harbor game studio is out of scope. Games may remain usable within Neo Grounds without a Harbor identity.

Publishing a game to the Harbor store requires signing in with Harbor and signing the finalized package with the creator's unlocked Harbor identity.

### HG-003 — Existing runtime contract

Harbor adopts the Neo Grounds v1 runtime direction rather than inventing another game ABI:

- package format: `neo-grounds-runtime-package`;
- WASM ABI: `neo-grounds-wasm-component-v1`;
- renderer: `canvas2d-command-buffer`;
- host integration: `worker-host-canvas-proxy`;
- lifecycle exports: `ng_init`, `ng_update`, `ng_render`, and `ng_shutdown`;
- package contents: `runtime.json`, `game.wasm`, `assets.json`, `package.sig`, and content-addressed assets.

The user-facing `.harborgame` file is a deterministic archive containing that package. The archive contract must define canonical path ordering, normalized metadata, duplicate-path rejection, decompression limits, and canonical package hashing. Harbor rejects unknown required compatibility fields instead of guessing.

A canonical schema and golden package fixtures are authoritative across both repositories. Cross-repository implementations must consume those fixtures so format drift fails tests.

### HG-004 — Creator signatures

The creator signature uses the creator's Harbor Ed25519 identity. Private identity material never leaves Harbor.

The signed payload is domain-separated and includes at least:

- protocol domain and version;
- canonical package digest;
- game and version identifiers;
- title;
- requested permissions;
- creator peer ID and public key;
- issuance timestamp;
- a bounded signing-request identifier.

Harbor displays those fields and requires explicit approval before signing. Neo Grounds verifies that the peer ID derives from the supplied public key and that the signature covers the exact submitted package digest. Package mutation after signing invalidates submission.

Store approval is a separate server-side attestation over an immutable package digest. Approval never replaces or weakens the creator signature. Every changed package is a new version and requires a new creator signature and approval.

### HG-005 — Sign in with Harbor

Neo Grounds offers **Sign in with Harbor** as an optional account-linking provider. The flow uses a short-lived, single-use, origin-bound challenge:

1. Neo Grounds creates a challenge for `https://games.social-harbor.com`.
2. The browser opens a versioned `harbor://games/auth/...` deep link.
3. Harbor fetches or decodes only bounded challenge metadata, verifies the exact trusted origin, and shows an approval screen.
4. The unlocked Harbor identity signs the domain-separated challenge.
5. Harbor submits the proof directly to the trusted Neo Grounds API.
6. Neo Grounds verifies the proof, consumes the nonce atomically, and links the Harbor public identity to the Neo Grounds account.
7. The browser obtains its normal secure Neo Grounds session without receiving Harbor private material.

Challenges expire, are single-use, bind the intended Neo Grounds account/session and audience, and cannot authorize package signing implicitly. Authentication and package signing are separate confirmations and signature domains.

### HG-006 — Moderated store

Neo Grounds has distinct publication states for its own platform and the Harbor store. Harbor-store submission requires a linked Harbor identity and valid creator package signature.

The Harbor-store lifecycle is:

`draft -> submitted -> approved | rejected -> withdrawn`

Only approved, non-withdrawn versions appear in Harbor's catalog or can be downloaded through Harbor. The initial approver is the operator through the Neo Grounds admin interface. Rejection does not remove the creator's Neo Grounds game. Approval records reviewer identity, package digest, decision timestamp, and an optional sanitized reason.

### HG-007 — Store service and storage

The store web application and API run on `gx10` and use durable storage under `/mnt/nas/neo-grounds`. Public origin:

- UI: `https://games.social-harbor.com`
- API: `https://games.social-harbor.com/api`

The service uses a file-backed or relational metadata store with atomic state transitions; in-memory demo repositories are not production storage. Package blobs are immutable and content-addressed. Temporary uploads remain outside the approved catalog and are deleted through a bounded retention policy.

Observed deployment state on 2026-09-08: `social-harbor.com` was served by Vercel, the games paths did not exist, and the `gx10` tunnel did not route the games subdomain. DNS, tunnel ingress, TLS, service supervision, backups, health checks, and rollback therefore remain implementation work rather than assumed infrastructure.

### HG-008 — Embedded store boundary

Harbor embeds a dedicated Neo Grounds store route in a sandboxed iframe. The iframe handles catalog browsing, search, game details, creator sign-in state, and install intent. It receives no Tauri API, filesystem access, Harbor database access, identity keys, contact graph, or unrestricted navigation.

The only initial bridge message is a versioned install intent carrying a store game/version identifier. Harbor:

- checks the exact `https://games.social-harbor.com` message origin;
- validates a closed message schema;
- resolves package metadata independently through the trusted API;
- asks the user to confirm creator, permissions, download size, and version;
- downloads and validates the package itself.

Games do not execute in the store iframe. They execute locally through Harbor's runtime.

### HG-009 — Local library

Harbor supports both:

1. manual `.harborgame` import through a file picker; and
2. non-recursive discovery in a configurable games folder.

The games folder is a discovery source, not an execution root. Harbor copies validated packages into its managed package store and never executes mutable files in place. Symlinks, path traversal, duplicate archive paths, special files, and files outside declared manifests are rejected.

Assumption: the configured discovery folder is machine-visible, while installation approval, permissions, saves, and play history are profile-scoped. This preserves Harbor's existing profile isolation. This assumption must be surfaced in UI copy and can be changed only by an explicit product decision.

### HG-010 — Runtime isolation and permissions

WASM executes in a dedicated Worker behind the versioned Neo Grounds ABI. The host owns DOM, Canvas 2D, WebAudio, input, fullscreen, package I/O, and all platform APIs. Game code receives no direct DOM, network, filesystem, Tauri, credential, or Harbor-service access.

Permissions are deny-by-default and declared in `runtime.json`. Initial Harbor support is limited to capabilities implemented and reviewed in this delivery. Unsupported permissions fail before launch; they are never silently ignored.

Resource limits must cover archive size, decompressed size, file count, individual assets, WASM memory, frame/update budget, platform-call rate, save size, and crash/restart behavior. Numeric limits must be set from measured Neo Grounds fixture/build output plus an explicit safety margin, and the evidence and protected failure mode must be recorded beside each chosen value.

### HG-011 — Integrity and installation

Before installation Harbor verifies:

- archive structure and bounded extraction;
- schema and ABI compatibility;
- every declared byte length and SHA-256 digest;
- no undeclared files;
- creator peer ID/public-key derivation;
- creator signature;
- store approval attestation for store downloads;
- API metadata/package digest agreement.

An installation is committed atomically by package digest. Failed validation leaves no runnable partial install. Existing installed versions remain usable if an update fails. Errors identify the failed check without exposing private material.

### HG-012 — Offline behavior

Installed games launch without the store or network when their declared capabilities permit it. The catalog reports offline state honestly. Saves are local and profile-scoped. Uninstalling a package does not delete saves without a separate explicit choice.

### HG-013 — Deferred multiplayer

The ABI may reserve the existing `multiplayer_signals` permission, but this delivery does not bind it to Harbor networking. Multiplayer, lobbies, state synchronization, cheating policy, host migration, and contact/session invitations require a separate protocol and threat-model contract. Games requesting multiplayer fail with a clear unsupported-capability error in this version.

## Neo Grounds API surface

The implementation may refine transport details while preserving these responsibilities:

- `POST /api/auth/harbor/challenges`
- `GET /api/auth/harbor/challenges/:id`
- `POST /api/auth/harbor/challenges/:id/proof`
- `GET /api/auth/harbor/challenges/:id/status`
- `POST /api/harbor-store/submissions`
- `GET /api/harbor-store/submissions/:id`
- `POST /api/admin/harbor-store/submissions/:id/approve`
- `POST /api/admin/harbor-store/submissions/:id/reject`
- `GET /api/harbor-store/games`
- `GET /api/harbor-store/games/:gameId/versions/:versionId`
- `GET /api/harbor-store/games/:gameId/versions/:versionId/package`
- `GET /api/health`

Mutation endpoints require authenticated sessions, CSRF protection where cookie sessions are used, bounded request bodies, stable error envelopes, and audit records. Package responses set immutable digest-based cache metadata and safe content headers.

## Operational requirements

- Run Neo Grounds as a dedicated unprivileged service account or constrained user service.
- Grant write access only to its allocated `/mnt/nas/neo-grounds` directories.
- Keep metadata database, pending uploads, approved packages, and backups in separate paths.
- Use a production service entrypoint, not `scripts/dev.mjs`.
- Bind the origin service to loopback and expose it only through the configured tunnel/proxy.
- Pin the trusted Harbor store origin in release builds. Development overrides must be explicit and must display a development warning.
- Provide health, structured logs, graceful shutdown, backup/restore, and deployment rollback instructions.
- Never commit tunnel credentials, signing material, session secrets, or identity proofs.

## Explicit non-goals

- Harbor Game Studio or another editor.
- Arbitrary browser, JavaScript, Unity, or Godot plugins.
- Running remote games inside the embedded store page.
- Automatic execution immediately after download.
- P2P package distribution between contacts.
- Multiplayer networking in the first delivery.
- Payments, advertisements, or automated moderation.
- General-purpose Harbor plugins.

## Rejected alternatives

- **Direct private-key access by Neo Grounds:** rejected because Harbor identity keys must remain inside Harbor.
- **Signing packages as the importing user:** rejected because it destroys creator provenance.
- **Server-only package signatures:** rejected because store approval is not creator authorship.
- **Executing from the watched folder:** rejected because mutable source files break installation integrity.
- **Giving the iframe Tauri access:** rejected because remote store compromise would become local-code authority.
- **Playing inside the remote iframe:** rejected because it prevents reliable offline play and future Harbor networking integration.
- **A second Harbor-specific WASM ABI:** rejected because Neo Grounds already defines the builder/runtime contract.

## Delivery acceptance

The feature is complete only when all scheduled work is integrated and the end-to-end gate proves:

1. A Neo Grounds user links an unlocked Harbor identity through a replay-safe challenge.
2. Neo Grounds builds a real WASM package and Harbor signs its exact canonical digest after confirmation.
3. Submission remains invisible until operator approval.
4. Approval makes the game visible in Harbor's embedded store.
5. Harbor installs the approved package from an origin-checked intent and rejects tampered package, metadata, signature, and attestation variants.
6. Manual import and games-folder discovery install the same valid package without executing in place.
7. The real WASM module runs locally in the Worker runtime with input and Canvas rendering, then launches offline after restart.
8. Two Harbor profiles cannot read or overwrite each other's saves, permissions, or play history.
9. The deployed service survives restart with catalog, auth links, approvals, and package bytes intact.
10. Automated frontend, Rust, Neo Grounds, protocol fixture, security, and production build checks pass, followed by a packaged Harbor smoke against `https://games.social-harbor.com`.
