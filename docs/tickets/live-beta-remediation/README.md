---
ldgr_doc: 1
kind: ticket_index_readme
id: ticket_readme.live-beta-remediation.v1
schema: ldgr.readme.v1
status: ready
tags:
  - harbor
  - live-beta
  - remediation
---

# Harbor Live Beta Remediation

This queue records failures and usability gaps observed during Harbor's first live test between a
Windows user and a macOS user. A ticket is not complete merely because a component test passes. Any
identity, contact, synchronization, notification, or calling behavior must also pass the applicable
two-profile packaged-app scenario.

## Release policy

- `P0` blocks the next broadly shared beta because it prevents entry, contact trust, content
  delivery, or calling, or because it hides a failure behind unusable diagnostics.
- `P1` is required for an understandable public-beta experience.
- `P2` is product discovery. It produces an approved interaction/protocol specification before an
  implementation ticket is opened.
- Privacy authorization remains authoritative during polling, prefetch, refresh, link preview, and
  notification work. A cache or notification must never reveal content the current identity cannot
  read.
- Raw peer IDs and public keys may remain available in an explicitly labelled diagnostic view, but
  they must not be the normal identity presented to users.

## Scheduled tickets

| Order | Priority | LDGR slug | Outcome and acceptance boundary | Depends on |
| ---: | :---: | --- | --- | --- |
| 1 | P0 | `live-0700-reproduction-matrix` | Capture sanitized Windows/macOS versions, relay paths, identity/contact states, and repeatable packaged-profile repros for the live failures. Add a matrix that names which tests require two profiles, media hardware, or deployed redirect infrastructure. | None |
| 2 | P0 | `live-0701-returning-identity-entry` | A returning user unlocks and enters the app using the persisted verified relay-name claim. Harbor never asks them to claim the same name again, never hangs on Claim, and never requires the Beta escape path. Interrupted claim recovery exposes bounded progress, retry, and a useful error. | 0700 |
| 3 | P1 | `live-0702-password-language` | Replace user-facing “passphrase” with “password” across onboarding, unlock, lock, backup, recovery, settings, validation, and docs without changing the encrypted-storage format or command compatibility. | None |
| 4 | P1 | `live-0703-password-confirmation` | Account creation shows neutral, mismatch, and matching password states while typing; mismatch is accessible and blocking, match is visibly green plus non-color status, and paste/password-manager flows work. | 0702 |
| 5 | P1 | `live-0704-lock-warning` | Before locking, explain that the password is required to unlock locally and that Harbor cannot recover a forgotten password; cancel is safe and no secret is shown or logged. | 0702 |
| 6 | P0 | `live-0705-name-propagation` | Replace raw peer IDs/public keys in all normal app surfaces, events, errors, notifications, calls, contacts, feeds, boards, and links with verified relay-qualified names or a safe unresolved-name state. Keep keys only in an explicit diagnostic/details view. | 0701 |
| 7 | P0 | `live-0706-web-invite-redirect` | A copied `https://social-harbor.com/...` contact invite returns a real handoff page instead of 404, preserves the complete invite payload, offers Open Harbor plus install help, and is verified against production hosting and browser fallback behavior. | 0700 |
| 8 | P0 | `live-0707-invite-normalization` | Add Contact accepts both the official HTTPS invite and `harbor://` forms directly, validates and normalizes them internally, rejects malformed/oversized payloads safely, and requires no manual scheme editing. | 0706 |
| 9 | P0 | `live-0708-contact-request-lifecycle` | Persist and render outgoing Pending, incoming Review, Accepted, Declined, Failed, and Revoked states. “Requesting contact” becomes durable row/card feedback and both peers converge after acceptance or rejection. | 0700 |
| 10 | P0 | `live-0709-contact-request-notifications` | Incoming contact requests create an in-app notification/badge with accept, decline, and inspect actions; state is durable across restart and clears consistently when handled. | 0708 |
| 11 | P0 | `live-0710-contact-wall-visibility` | An authorized contact can see eligible public and contacts-only posts while an unauthorized profile cannot. Prove direct, relay/offline, restart, permission-revocation, and stale-cache behavior with three isolated profiles. | 0700, 0708 |
| 12 | P0 | `live-0711-reactive-refresh` | Introduce a single event/reconciliation path that refreshes affected stores when local or remote messages, posts, edits, deletes, contacts, calls, or notifications change. Prevent polling storms, duplicate events, stale views, and cross-profile leakage. | 0708, 0710 |
| 13 | P0 | `live-0712-media-transfer-state` | Posts and messages declare expected media before bytes arrive and render queued, discovering, transferring with measurable progress where available, ready, unavailable, retrying, and failed states instead of appearing empty. | 0710 |
| 14 | P1 | `live-0713-contact-feed-poller` | Add a cancellable background worker that incrementally checks authorized contact feeds using durable cursors, bounded concurrency, jitter/backoff, online/offline handling, and identity-lock/profile-switch isolation. Publish changes through 0711. | 0710, 0711 |
| 15 | P1 | `live-0714-media-prefetch-cache` | From newly discovered authorized posts, prefetch media for a configurable retention horizon under size/concurrency/storage budgets. Verify eviction, retry, metered/offline controls, revocation purge, profile isolation, and no fetch for unauthorized content. | 0712, 0713 |
| 16 | P0 | `live-0715-call-error-contract` | Replace `[object Object]` with a typed call failure contract and human messages for permission denial, missing media API/device, signaling, timeout, ICE/NAT/TURN, busy/rejected, and remote incompatibility. Preserve sanitized diagnostic detail for bug reports. | 0700, 0705 |
| 17 | P0 | `live-0716-macos-media-runtime` | Detect and request macOS microphone/camera permissions correctly, declare required bundle entitlements/usage strings, distinguish denial from API absence, enumerate devices safely, and prove packaged universal/target Mac behavior. | 0715 |
| 18 | P0 | `live-0717-voice-call-live-validation` | Repair as needed and pass packaged Windows-to-macOS 1:1 audio calls in both directions, including ring/accept/reject, audible bidirectional media, hangup, relaunch, relay path, and graceful permission/NAT failure. | 0715, 0716, 0720 |
| 19 | P0 | `live-0718-video-call-live-validation` | Repair as needed and pass packaged Windows-to-macOS 1:1 video calls in both directions, including camera preview, bidirectional media, audio coexistence, device denial, hangup, and relay/NAT behavior. | 0717 |
| 20 | P0 | `live-0719-group-call-live-validation` | Repair as needed and pass the selected small-group topology with at least three packaged profiles, roster convergence, join/leave/rejoin, audio/video state, participant failure isolation, capacity limits, and honest TURN diagnostics. | 0718 |
| 21 | P0 | `live-0720-message-call-notifications` | New messages and incoming/missed calls produce in-app notifications and supported native notifications with sender name, privacy-safe preview policy, navigation/action behavior, deduplication, read state, mute controls, and restart persistence. | 0705, 0711 |
| 22 | P1 | `live-0721-safe-link-metadata` | Every HTTP(S) link can render a consistent metadata card with canonical URL, title, description, image, site, loading/error states, and cache. Fetching must defend against SSRF, local-network access, tracking leakage, oversized content, and unsafe schemes. | 0712 |
| 23 | P1 | `live-0722-provider-embeds` | Add consent-aware responsive players for supported YouTube, SoundCloud, Spotify, and TikTok URLs, with metadata-card fallback, privacy/cookie disclosure, keyboard accessibility, provider allowlisting, and graceful offline/unavailable behavior. | 0721 |
| 24 | P1 | `live-0723-compose-modal` | Remove Wall as a primary section and move post creation to an accessible modal reachable from the main shell. Preserve drafts, attachments, visibility, validation, progress, focus restoration, Escape behavior, and failure recovery. | 0711, 0712 |
| 25 | P1 | `live-0724-feed-filter-model` | Rename Posts to All, assign a canonical media classification to each post, and replace feed tabs with accessible filters for All, Images, Video, and Audio. Persist the preferred filter and verify mixed/multi-media classification. | 0723 |
| 26 | P1 | `live-0725-onboarding-hero` | Show “Social media you control” once during onboarding with Docs and Quick Start links. Make it closable, persist dismissal per identity, and remove the recurring banner that consumes the main app. | None |
| 27 | P1 | `live-0726-interaction-motion` | Define themeable hover, pressed, focus, selected, disabled, and loading feedback for interactive controls/cards, honor reduced-motion, avoid layout shift, and apply it consistently through shared primitives. | None |
| 28 | P1 | `live-0727-keyboard-shortcuts` | Add documented platform-standard shortcuts for navigation, compose, search, send/submit, close/cancel, refresh, settings, and relevant call controls without overriding text editing, screen-reader, or OS conventions. Include a discoverable shortcut reference. | 0723 |
| 29 | P1 | `live-0728-bug-report-tracking-link` | Make the post-submit bug-account reference an actual internal Harbor navigation action to that named account/wall, with browser-safe fallback and no raw key exposure. | 0705, 0707 |
| 30 | P2 | `live-0729-community-identity-spec` | Review the [proposed community forum ADR](../../architecture/adr-0002-community-forum-identity.md), run the [text wireflow](../../architecture/community-forum-wireflow.md) with representative users, resolve its four acceptance questions, and approve or replace it. The [atomic implementation program](community-forum/README.md) remains unscheduled until approval. | 0705, 0724 |
| 31 | P0 | `live-0730-cross-platform-acceptance` | Run a clean packaged Windows/macOS acceptance session with two users and an additional group-call profile. Record sanitized evidence for entry, invites, requests, names, content/privacy, refresh, media, notifications, calls, links, and restart; block broad beta publication on any P0 failure. | All P0 tickets |

## Community program gate

`live-0729-community-identity-spec` has produced a concrete proposal and decomposition:

- [ADR-0002: Relay-scoped communities as portable signed forums](../../architecture/adr-0002-community-forum-identity.md)
- [Low-fidelity community forum wireflow](../../architecture/community-forum-wireflow.md)
- [Atomic MVP and later ticket program](community-forum/README.md)

ADR-0002 is still **Proposed**. The first allowable follow-up is
`live-0800-community-contract-approval`. Do not schedule `live-0801` through `live-0811` or any
`live-090x` work until that approval ticket records the address syntax, public/open MVP boundary,
governance timing, and legacy-board treatment and changes the ADR status to Accepted. Community work
is P2 product discovery and does not silently become a blocker for the P0 cross-platform beta gate.

## Clarified assumption

The unfinished note “when locking an account it should warn users that they need to have …” is
interpreted as: users must know that they need their password to unlock the local identity and that
Harbor cannot recover a forgotten password. If the intended warning was about backups or another
requirement, amend `live-0704-lock-warning` before implementation.

## Evidence rules

- Use isolated profile/data roots for every actor and prove no cross-profile state contamination.
- Record app version, OS/architecture, relay version/namespace, direct versus relayed path, and
  relevant permission state without recording passwords, private keys, message bodies, or private
  contact graphs.
- Calling completion requires observable media in packaged builds, not signaling-only success.
- Sync completion requires an authorized and unauthorized consumer and a restart/offline path.
- Production website behavior must be verified over the public URL after deployment, including a
  fresh browser with Harbor absent.
