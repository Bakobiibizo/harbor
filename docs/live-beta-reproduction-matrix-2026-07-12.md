# First Live Beta Reproduction Matrix

This matrix converts the first real two-user Harbor session into repeatable release evidence. It
supplements the ticket queue in `docs/tickets/live-beta-remediation/README.md`; it does not treat
component tests as proof that a networked feature works.

## Sanitized session facts

| Actor | Packaged app | Platform | Network role | Evidence |
| --- | --- | --- | --- | --- |
| Primary | `1.4.1-beta.5` | Windows 11 Home x64, build 26200 | Existing identity, production Harbor relay | Installed executable metadata re-read on 2026-07-12. |
| Contact | Public beta available during the session, reported as the macOS build | macOS; exact version and Apple/Intel architecture were not captured | Newly created identity, production Harbor relay | User report from the live session. Capture exact OS/architecture in `live-0730` before final acceptance. |
| Authority | Relay artifact `0.2.0`, namespace `harbor.social` | AWS production relay | Name authority, introductions, relay content/signaling | Existing production smoke evidence. |

The source tree is `1.4.1-beta.6`, but beta.6 publication was deliberately paused. None of these
failures may be dismissed because a source-only or Linux-only test passes.

## Reproduction and ownership

| ID | Live symptom / deterministic reproduction | Expected result | Remediation and required proof |
| --- | --- | --- | --- |
| R01 | Unlock the existing Windows profile, relaunch, and log in again. The UI asks for the already-owned name; Claim can remain busy indefinitely and only the Beta escape enters the shell. | A verified persisted claim resumes directly. Interrupted registration has bounded progress, retry, and an actionable error. | `live-0701`; packaged restart with one persistent profile and relay available/unavailable. |
| R02 | Enter different account passwords, then make them equal. | Mismatch is immediately clear and blocking; match is green plus text/icon status. | `live-0702`-`0704`; keyboard, paste, password-manager, screen-reader, lock-warning tests. |
| R03 | Copy the official single-click contact link into a fresh browser. Production currently returns HTTP 404 for `/add-friend/<payload>`, while `/add-friend/index.html?c=<payload>` returns 200. | A handoff page preserves the payload, opens Harbor, and offers install help. | `live-0706`; production HTTP/browser test with valid, malformed, oversized, and Harbor-absent cases. |
| R04 | Paste that HTTPS link into Add Contact. The UI accepts only `harbor://`, forcing manual scheme editing. | Both official HTTPS and custom-scheme forms normalize and validate internally. | `live-0707`; frontend plus Rust boundary tests. |
| R05 | Send a contact request. A transient “requesting contact” toast appears but the row does not show pending/accepted state; the recipient receives no request notification. | Both peers persist and converge through Pending, Review, Accepted/Declined/Failed/Revoked with notification actions. | `live-0708`, `live-0709`; two packaged profiles, restart, duplicate/replay, rejection. |
| R06 | After reciprocal contact setup, open the other person's wall/feed. Eligible posts are absent. | An authorized contact sees public and contacts-only content; a third unauthorized profile never does. | `live-0710`; direct, relay/offline, restart, revocation, stale-cache three-profile matrix. |
| R07 | Keep feed/chat/contact UI open while the other profile posts, edits, requests contact, or messages. | A single event/reconciliation path refreshes only affected state without manual navigation/reload. | `live-0711`; local/remote event, duplicate, reconnect, profile-switch tests. |
| R08 | Open a post/message whose media is still transferring. No placeholder distinguishes “has media” from “no media.” | Declared media renders queued/discovering/transferring/ready/unavailable/retry/failed state. | `live-0712`; slow/chunked/offline/hash-failure fixtures. |
| R09 | Leave a contact offline, publish posts/media, then reconnect. | A bounded cursor worker discovers authorized feed changes and a budgeted cache preloads media without privacy leaks. | `live-0713`, `live-0714`; offline/backoff/eviction/revocation/profile-isolation tests. |
| R10 | Receive a message or incoming/missed call while viewing another page or while Harbor is backgrounded. | In-app and supported native notifications identify the verified sender safely and route to the event. | `live-0720`; foreground/background/restart/dedupe/mute tests. |
| R11 | Start 1:1 audio, 1:1 video, and group calls. All fail with `Call failed [object, Object]`; the Mac reports no audio API. | Typed human errors distinguish permissions, API/device, signaling, timeout, ICE/TURN, busy, and compatibility; packaged cross-platform media succeeds. | `live-0715`-`live-0719`; Windows/macOS real-media evidence, then three-profile group evidence. |
| R12 | Post arbitrary links and supported media-provider links. | All safe HTTP(S) links receive metadata cards; allowlisted providers can load consent-aware accessible embeds with fallback. | `live-0721`, `live-0722`; SSRF/private-network/size/tracking/provider/offline tests. |
| R13 | Navigate the current Wall, feed modality tabs, recurring hero, and bug-report tracking result. | Compose is a modal; feed uses persistent All/Images/Video/Audio filters; the hero is one-time; bug tracking navigates internally by name. | `live-0723`-`live-0725`, `live-0728`; navigation, focus, persistence, and mixed-media tests. |
| R14 | Traverse buttons/cards with mouse and keyboard. | Shared primitives expose hover/pressed/focus/loading states and standard discoverable shortcuts without breaking editing/assistive technology. | `live-0726`, `live-0727`; reduced-motion and keyboard matrix. |
| R15 | Inspect contacts, calls, feeds, boards, errors, and notifications for peer IDs/public keys. | Verified relay-qualified names are the normal identity everywhere; raw keys exist only in explicit diagnostics. | `live-0705`; UI text scan plus unresolved/stale/revoked identity tests. |
| R16 | Evaluate current community boards with a new user. | An approved forum/community model explains identity, topics, membership, discovery, moderation, and distinct value before implementation. | `live-0729`; ADR/wireflow and follow-on decomposition. |

## Packaged validation topology

Use isolated profile roots and never reuse the operator's real profile:

1. Windows x64 packaged profile `alpha` with a persistent verified relay claim.
2. macOS packaged profile `bravo`, recording OS version, architecture, microphone/camera permission
   state, and whether the package is signed/notarized.
3. Third isolated profile `charlie` for unauthorized-wall assertions and group calls.
4. Production relay path and a controlled local relay path; record direct versus relayed transport.
5. Slow-media fixture, offline/reconnect fixture, strict-NAT/TURN fixture, and browser without Harbor.

For every row record the Harbor commit/version, relay artifact/namespace, profile role, transport
path, outcome, and sanitized logs/screenshots. Never record passwords, private keys, private message
bodies, or the real contact graph.

## Release gate

`live-0730-cross-platform-acceptance` owns the final execution of this matrix. Broad beta
publication remains blocked until every P0 row passes in packaged builds. The exact macOS platform
metadata missing from the first session must be captured there rather than guessed here.
